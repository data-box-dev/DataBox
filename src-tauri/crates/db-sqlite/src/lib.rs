use async_trait::async_trait;
use db_core::{
    traits::DatabaseDriver,
    types::*,
    DbError,
    DbResult,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, Column, ColumnIndex};

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
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
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

        let first_row = &rows[0];
        let column_names = first_row.columns().iter().map(|c| c.name()).collect::<Vec<_>>();

        let columns: Vec<ColumnMeta> = column_names
            .iter()
            .map(|name| ColumnMeta {
                name: name.to_string(),
                data_type: "unknown".to_string(),
                nullable: true,
            })
            .collect();

        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut map = std::collections::HashMap::new();
            for (idx, _col_name) in column_names.iter().enumerate() {
                let value = sqlite_row_to_db_value(row, idx)?;
                map.insert(column_names[idx].to_string(), value);
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
            last_insert_id: Some(result.last_insert_rowid()),
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
                last_insert_id: Some(result.last_insert_rowid()),
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
                is_unique: false,
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
        for idx_row in index_rows {
            let idx_name: String = idx_row.get("name");
            let unique: i64 = idx_row.get("unique");

            // 查询索引列
            let col_rows = sqlx::query(&format!("PRAGMA index_info({})", idx_name))
                .fetch_all(&self.pool)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

            let cols: Vec<String> = col_rows
                .into_iter()
                .map(|r: sqlx::sqlite::SqliteRow| r.get("name"))
                .collect();

            indexes.push(IndexSchema {
                name: idx_name,
                columns: cols,
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
    async fn get_columns(&self, _database: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
        let rows = sqlx::query(&format!("PRAGMA table_info({})", table))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row: sqlx::sqlite::SqliteRow| {
                let name: String = row.get("name");
                let data_type: String = row.get("type");
                ColumnMeta {
                    name,
                    data_type,
                    nullable: true,
                }
            })
            .collect())
    }
}

/// 将 SQLite 行中的值转换为 DbValue
fn sqlite_row_to_db_value(row: &sqlx::sqlite::SqliteRow, idx: usize) -> DbResult<DbValue> {
    // Use try_get<Option<T>> — avoids ValueRef entirely
    if let Ok(v) = row.try_get::<Option<bool>, usize>(idx) { return Ok(v.map(DbValue::Bool).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<i64>, usize>(idx) { return Ok(v.map(DbValue::Int).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<f64>, usize>(idx) { return Ok(v.map(DbValue::Float).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<String>, usize>(idx) { return Ok(v.map(DbValue::Text).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, usize>(idx) { return Ok(v.map(DbValue::Bytes).unwrap_or(DbValue::Null)); }
    Ok(DbValue::Text("<unable to decode>".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
