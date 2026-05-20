use async_trait::async_trait;
use db_core::{
    traits::DatabaseDriver,
    types::*,
    DbError,
    DbResult,
};
use sqlx::mysql::MySqlPoolOptions;

/// MySQL 驱动实现
///
/// 连接字符串格式: `mysql://{user}:{password}@{host}:{port}/{database}`
pub struct MySqlDriver {
    pool: sqlx::MySqlPool,
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    /// 从连接配置建立 MySQL 连接
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
        let url = config.to_url();
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
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
                data_type: "unknown".to_string(),
                nullable: true,
            })
            .collect();

        // 将 sqlx Row 映射为 DbValue Row
        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut map = std::collections::HashMap::new();
            for (idx, col_name) in column_names.iter().enumerate() {
                let value = mysql_row_to_db_value(row, idx)?;
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

        // MySQL 使用 LAST_INSERT_ID() 获取自增 ID
        let last_insert_id: Option<i64> = sqlx::query_scalar("SELECT LAST_INSERT_ID()")
            .fetch_one(&self.pool)
            .await
            .ok()
            .filter(|id: &i64| *id != 0);

        Ok(ExecResult {
            rows_affected: result.rows_affected(),
            last_insert_id,
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
            let last_insert_id: Option<i64> = sqlx::query_scalar("SELECT LAST_INSERT_ID()")
                .fetch_one(&mut *tx)
                .await
                .ok()
                .filter(|id: &i64| *id != 0);
            results.push(ExecResult {
                rows_affected: result.rows_affected(),
                last_insert_id,
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
            .map(|row: sqlx::mysql::MySqlRow| {
                let val: String = row.get(0);
                val
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(plan)
    }

    /// 列出所有数据库
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys') ORDER BY SCHEMA_NAME"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(name,)| DatabaseInfo {
                name,
                tables: vec![],
            })
            .collect())
    }

    /// 列出数据库内所有表
    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT TABLE_NAME, TABLE_TYPE FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_TYPE IN ('BASE TABLE', 'VIEW') ORDER BY TABLE_NAME"
        )
        .bind(database)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows.into_iter().map(|(name, _)| name).collect())
    }

    /// 获取表完整 Schema（列 + 索引 + 主键）
    async fn describe_table(
        &self,
        database: &str,
        table: &str,
    ) -> DbResult<TableSchema> {
        // 查询列信息
        let column_rows = sqlx::query(
            r#"
            SELECT
                COLUMN_NAME,
                DATA_TYPE,
                IS_NULLABLE,
                COLUMN_DEFAULT,
                CHARACTER_MAXIMUM_LENGTH,
                COLUMN_KEY
            FROM information_schema.COLUMNS
            WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
            ORDER BY ORDINAL_POSITION
            "#
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut columns = Vec::new();
        let mut primary_keys = Vec::new();

        for row in column_rows {
            let name: String = row.get("COLUMN_NAME");
            let data_type: String = row.get("DATA_TYPE");
            let is_nullable: String = row.get("IS_NULLABLE");
            let default: Option<String> = row.get("COLUMN_DEFAULT");
            let char_max_length: Option<u32> = row.get("CHARACTER_MAXIMUM_LENGTH");
            let column_key: String = row.get("COLUMN_KEY");

            let is_primary_key = column_key == "PRI";
            if is_primary_key {
                primary_keys.push(name.clone());
            }

            columns.push(ColumnSchema {
                name: name.clone(),
                data_type,
                nullable: is_nullable == "YES",
                default_value: default,
                is_primary_key,
                is_unique: column_key == "UNI",
                comment: None,
                char_max_length,
            });
        }

        // 查询索引信息
        let index_rows = sqlx::query(
            r#"
            SELECT
                INDEX_NAME,
                COLUMN_NAME,
                NON_UNIQUE
            FROM information_schema.STATISTICS
            WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
            ORDER BY INDEX_NAME, SEQ_IN_INDEX
            "#
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut indexes = Vec::new();
        let mut current_index: Option<(String, bool, Vec<String>)> = None;

        for row in index_rows {
            let index_name: String = row.get("INDEX_NAME");
            let column_name: String = row.get("COLUMN_NAME");
            let non_unique: i64 = row.get("NON_UNIQUE");

            match current_index {
                Some((ref name, ref unique, ref cols)) if name == &index_name => {
                    let mut cols = cols.clone();
                    cols.push(column_name);
                    current_index = Some((index_name, *unique, cols));
                }
                _ => {
                    if let Some((_, _, cols)) = current_index {
                        indexes.push(IndexSchema {
                            name: cols[0].clone(),
                            columns: cols,
                            is_unique: !unique,
                            is_primary: false,
                        });
                    }
                    current_index = Some((index_name, non_unique == 0, vec![column_name]));
                }
            }
        }

        if let Some((_, _, cols)) = current_index {
            indexes.push(IndexSchema {
                name: cols[0].clone(),
                columns: cols,
                is_unique: true,
                is_primary: false,
            });
        }

        Ok(TableSchema {
            schema: Some(database.to_string()),
            name: table.to_string(),
            columns,
            indexes,
            primary_keys,
        })
    }

    /// 快速获取列元数据
    async fn get_columns(
        &self,
        database: &str,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let rows = sqlx::query(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION"
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row: sqlx::mysql::MySqlRow| ColumnMeta {
                name: row.get("COLUMN_NAME"),
                data_type: row.get("DATA_TYPE"),
                nullable: row.get::<String, _>("IS_NULLABLE") == "YES",
            })
            .collect())
    }
}

/// 将 MySQL 行中的值转换为 DbValue
fn mysql_row_to_db_value(row: &sqlx::mysql::MySqlRow, idx: usize) -> DbResult<DbValue> {
    use sqlx::mysql::types::MySqlValueRef;

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

    // MySQL 类型解码
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
    } else if let Ok(v) = value_ref.try_decode::<i8>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<i16>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<i32>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<i64>() {
        Decoded::Int(v)
    } else if let Ok(v) = value_ref.try_decode::<u8>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<u16>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<u32>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<u64>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<f32>() {
        Decoded::Float(v as f64)
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
    async fn test_mysql_driver_creation() {
        // 需要一个真实的 MySQL 服务器
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Mysql,
            host: "localhost".to_string(),
            port: 3306,
            database: "test".to_string(),
            username: "root".to_string(),
            password: String::new(),
            ssl: false,
            options: std::collections::HashMap::new(),
        };

        // 在 CI/CD 环境中可以跳过
        match MySqlDriver::connect(&config).await {
            Ok(_) => {
                // MySQL 服务器可用
            }
            Err(_) => {
                // 没有 MySQL 服务器，跳过
            }
        }
    }
}
