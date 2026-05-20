use async_trait::async_trait;
use db_core::{
    traits::DatabaseDriver,
    types::*,
    DbError,
    DbResult,
};
use sqlx::sqlite::SqlitePoolOptions;

/// SQLite 驱动实现
///
/// 与 PostgreSQL/MySQL 不同，SQLite 使用文件路径作为连接标识。
/// 连接字符串格式: `sqlite://{file_path}` 或 `sqlite::memory:`
pub struct SqliteDriver {
    pool: sqlx::SqlitePool,
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    /// 从连接配置建立 SQLite 连接
    ///
    /// SQLite 的 `ConnectionConfig` 字段映射：
    /// - `database` 字段：文件路径（如 `/path/to/db.sqlite`）或 `:memory:`
    /// - `host`/`port`：不使用
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
        // SQLite 连接字符串：file-path 或 :memory:
        let db_path = if config.database == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite://{}", config.database)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_path)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        Ok(Self { pool })
    }

    /// 检查连接是否存活
    async fn ping(&self) -> DbResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        Ok(())
    }

    /// 关闭连接池
    async fn close(&self) -> DbResult<()> {
        self.pool.close().await;
        Ok(())
    }

    /// 执行 SELECT 查询，返回完整结果集
    async fn query(&self, sql: &str, _params: Vec<serde_json::Value>) -> DbResult<QueryResult> {
        let start = std::time::Instant::now();

        let rows = sqlx::query(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let elapsed = start.elapsed().as_millis() as u64;

        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                row_count: 0,
                elapsed_ms: elapsed,
            });
        }

        // 从第一行提取列元数据
        let first_row = &rows[0];
        let column_names = first_row.column_names();

        let columns: Vec<ColumnMeta> = column_names
            .iter()
            .map(|name| ColumnMeta {
                name: name.to_string(),
                data_type: "unknown".to_string(), // SQLite 类型系统较宽松
                nullable: true,
            })
            .collect();

        // 将 sqlx Row 映射为 DbValue Row
        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut map = std::collections::HashMap::new();
            for (idx, col_name) in column_names.iter().enumerate() {
                let value = sqlite_row_to_db_value(row, idx)?;
                map.insert(col_name.to_string(), value);
            }
            result_rows.push(map);
        }

        Ok(QueryResult {
            columns,
            rows: result_rows,
            row_count: rows.len(),
            elapsed_ms: elapsed,
        })
    }

    /// 执行 INSERT / UPDATE / DELETE / DDL
    async fn execute(&self, sql: &str, _params: Vec<serde_json::Value>) -> DbResult<ExecResult> {
        let start = std::time::Instant::now();
        let result = sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(ExecResult {
            rows_affected: result.rows_affected(),
            last_insert_id: result.last_insert_rowid().map(|id| id as i64),
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 事务内批量执行（全部成功才提交）
    async fn execute_batch(&self, statements: Vec<String>) -> DbResult<Vec<ExecResult>> {
        let mut results = Vec::with_capacity(statements.len());
        let mut tx = self.pool.begin().await.map_err(|e| DbError::QueryFailed(e.to_string()))?;

        for stmt in statements {
            let start = std::time::Instant::now();
            let result = sqlx::query(&stmt).execute(&mut *tx).await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
            results.push(ExecResult {
                rows_affected: result.rows_affected(),
                last_insert_id: result.last_insert_rowid().map(|id| id as i64),
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        }

        tx.commit().await.map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(results)
    }

    /// EXPLAIN 查询执行计划
    async fn explain(&self, sql: &str) -> DbResult<String> {
        let rows = sqlx::query(&format!("EXPLAIN {}", sql))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let plan = rows
            .into_iter()
            .map(|row: sqlx::sqlite::SqliteRow| {
                let val: String = row.get(0);
                val
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(plan)
    }

    /// 列出数据库（SQLite 单文件只有一个数据库）
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        // 返回一个代表当前数据库的虚拟 DatabaseInfo
        let db_name = std::path::Path::new(&self.pool.connect_options().get_filename())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("main")
            .to_string();

        Ok(vec![DatabaseInfo {
            name: db_name,
            tables: vec![],
        }])
    }

    /// 列出所有表（SQLite: sqlite_master）
    async fn list_tables(&self, _database: &str) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    /// 获取表完整 Schema（列 + 索引 + 主键）
    async fn describe_table(
        &self,
        _database: &str,
        table: &str,
    ) -> DbResult<TableSchema> {
        // 查询列信息（PRAGMA table_info）
        let rows = sqlx::query(&format!("PRAGMA table_info({})", table))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut columns = Vec::new();
        let mut primary_keys = Vec::new();

        for row in rows {
            let cid: i64 = row.get("cid");
            let name: String = row.get("name");
            let data_type: String = row.get("type");
            let not_null: i64 = row.get("notnull");
            let default: Option<String> = row.get("dflt_value");
            let pk: i64 = row.get("pk");

            if pk > 0 {
                primary_keys.push(name.clone());
            }

            columns.push(ColumnSchema {
                name: name.clone(),
                data_type,
                nullable: not_null == 0,
                default_value: default,
                is_primary_key: pk > 0,
                is_unique: false, // TODO: PRAGMA index_list
                comment: None,
                char_max_length: None,
            });
        }

        // 查询索引信息
        let index_rows = sqlx::query(&format!("PRAGMA index_list({})", table))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut indexes = Vec::new();
        for row in index_rows {
            let idx_name: String = row.get("name");
            let unique: i64 = row.get("unique");

            // 获取索引列
            let col_rows = sqlx::query(&format!("PRAGMA index_info({})", idx_name))
                .fetch_all(&self.pool)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

            let idx_columns: Vec<String> = col_rows
                .into_iter()
                .map(|r: sqlx::sqlite::SqliteRow| r.get("name"))
                .collect();

            indexes.push(IndexSchema {
                name: idx_name,
                columns: idx_columns,
                is_unique: unique != 0,
                is_primary: false,
            });
        }

        Ok(TableSchema {
            schema: None,
            name: table.to_string(),
            columns,
            indexes,
            primary_keys,
        })
    }

    /// 快速获取列元数据
    async fn get_columns(
        &self,
        _database: &str,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let rows = sqlx::query(&format!("PRAGMA table_info({})", table))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row: sqlx::sqlite::SqliteRow| ColumnMeta {
                name: row.get("name"),
                data_type: row.get("type"),
                nullable: row.get::<i64, _>("notnull") == 0,
            })
            .collect())
    }
}

/// 将 SQLite 行中的值转换为 DbValue
fn sqlite_row_to_db_value(row: &sqlx::sqlite::SqliteRow, idx: usize) -> DbResult<DbValue> {
    use sqlx::sqlite::types::SqliteValueRef;

    let value_ref = row
        .try_get_raw(idx)
        .map_err(|e| DbError::TypeConversion {
            column: format!("column_{}", idx),
            target_type: "unknown".to_string(),
        })?;

    // 检查 NULL
    if value_ref.is_null() {
        return Ok(DbValue::Null);
    }

    // SQLite 类型解码
    #[derive(Debug)]
    enum Decoded {
        Bool(bool),
        Int(i64),
        Float(f64),
        Text(String),
        Bytes(Vec<u8>),
    }

    let decoded = if let Ok(v) = value_ref.try_decode::<bool>() {
        Decoded::Bool(v)
    } else if let Ok(v) = value_ref.try_decode::<i32>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<i64>() {
        Decoded::Int(v)
    } else if let Ok(v) = value_ref.try_decode::<f64>() {
        Decoded::Float(v)
    } else if let Ok(v) = value_ref.try_decode::<String>() {
        Decoded::Text(v)
    } else if let Ok(v) = value_ref.try_decode::<&str>() {
        Decoded::Text(v.to_string())
    } else if let Ok(v) = value_ref.try_decode::<Vec<u8>>() {
        Decoded::Bytes(v)
    } else {
        // 回退到文本表示
        let text: String = value_ref
            .text_decode::<&str>()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "<unable to decode>".to_string());
        return Ok(DbValue::Text(text));
    };

    Ok(match decoded {
        Decoded::Bool(v) => DbValue::Bool(v),
        Decoded::Int(v) => DbValue::Int(v),
        Decoded::Float(v) => DbValue::Float(v),
        Decoded::Text(v) => DbValue::Text(v),
        Decoded::Bytes(v) => DbValue::Bytes(v),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_connect_memory() {
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Sqlite,
            host: String::new(),
            port: 0,
            database: ":memory:".to_string(),
            username: String::new(),
            password: String::new(),
            ssl: false,
            options: std::collections::HashMap::new(),
        };

        let driver = SqliteDriver::connect(&config).await.unwrap();
        assert!(driver.ping().await.is_ok());
    }

    #[tokio::test]
    async fn test_sqlite_create_and_query() {
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Sqlite,
            host: String::new(),
            port: 0,
            database: ":memory:".to_string(),
            username: String::new(),
            password: String::new(),
            ssl: false,
            options: std::collections::HashMap::new(),
        };

        let driver = SqliteDriver::connect(&config).await.unwrap();

        // 创建表
        driver
            .execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)",
                vec![],
            )
            .await
            .unwrap();

        // 插入数据
        driver
            .execute(
                "INSERT INTO users (name, age) VALUES ('Alice', 30), ('Bob', 25)",
                vec![],
            )
            .await
            .unwrap();

        // 查询数据
        let result = driver
            .query("SELECT * FROM users WHERE age > ?", vec!["25".into()])
            .await
            .unwrap();

        assert_eq!(result.row_count, 2);
        assert_eq!(result.columns.len(), 3);

        // 验证数据
        let first_row = &result.rows[0];
        assert!(first_row.contains_key("name"));
        assert!(first_row.contains_key("age"));
    }

    #[tokio::test]
    async fn test_sqlite_list_tables() {
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Sqlite,
            host: String::new(),
            port: 0,
            database: ":memory:".to_string(),
            username: String::new(),
            password: String::new(),
            ssl: false,
            options: std::collections::HashMap::new(),
        };

        let driver = SqliteDriver::connect(&config).await.unwrap();
        driver
            .execute("CREATE TABLE test_table (id INTEGER)", vec![])
            .await
            .unwrap();

        let tables = driver.list_tables("").await.unwrap();
        assert!(tables.contains(&"test_table".to_string()));
    }

    #[tokio::test]
    async fn test_sqlite_describe_table() {
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Sqlite,
            host: String::new(),
            port: 0,
            database: ":memory:".to_string(),
            username: String::new(),
            password: String::new(),
            ssl: false,
            options: std::collections::HashMap::new(),
        };

        let driver = SqliteDriver::connect(&config).await.unwrap();
        driver
            .execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER)",
                vec![],
            )
            .await
            .unwrap();

        let schema = driver.describe_table("", "users").await.unwrap();
        assert_eq!(schema.name, "users");
        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.primary_keys, vec!["id"]);
    }

    #[tokio::test]
    async fn test_sqlite_execute_batch() {
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Sqlite,
            host: String::new(),
            port: 0,
            database: ":memory:".to_string(),
            username: String::new(),
            password: String::new(),
            ssl: false,
            options: std::collections::HashMap::new(),
        };

        let driver = SqliteDriver::connect(&config).await.unwrap();

        let results = driver
            .execute_batch(vec![
                "CREATE TABLE t1 (id INTEGER)".to_string(),
                "INSERT INTO t1 VALUES (1)".to_string(),
                "INSERT INTO t1 VALUES (2)".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].rows_affected, 0); // CREATE
        assert_eq!(results[1].rows_affected, 1); // INSERT
        assert_eq!(results[2].rows_affected, 1); // INSERT
    }
}
