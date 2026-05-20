use async_trait::async_trait;
use db_core::{
    traits::DatabaseDriver,
    types::*,
    DbError,
    DbResult,
};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{Row, Column, ColumnIndex};

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
            let col_key: String = row.get("COLUMN_KEY");

            if col_key == "PRI" {
                primary_keys.push(name.clone());
            }

            columns.push(ColumnSchema {
                name,
                data_type,
                nullable: is_nullable == "YES",
                default_value: default,
                is_primary_key: col_key == "PRI",
                is_unique: col_key == "UNI",
                comment: None,
                char_max_length,
            });
        }

        let mut columns = columns;

        // 查询索引
        let index_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT INDEX_NAME, NON_UNIQUE FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?"
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let indexes: Vec<IndexSchema> = index_rows
            .into_iter()
            .map(|(name, non_unique)| {
                let is_primary = primary_keys.contains(&name);
                IndexSchema {
                    name,
                    columns: vec![],
                    is_unique: non_unique == "0",
                    is_primary,
                }
            })
            .collect();

        Ok(TableSchema {
            schema: Some(database.to_string()),
            name: table.to_string(),
            columns,
            indexes,
            primary_keys,
        })
    }

    /// 快速获取列元数据
    async fn get_columns(&self, database: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
        let rows = sqlx::query(
            r#"SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION"#
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row: sqlx::mysql::MySqlRow| {
                let name: String = row.get("COLUMN_NAME");
                let data_type: String = row.get("DATA_TYPE");
                let is_nullable: String = row.get("IS_NULLABLE");
                ColumnMeta {
                    name,
                    data_type,
                    nullable: is_nullable == "YES",
                }
            })
            .collect())
    }
}

/// 将 MySQL 行中的值转换为 DbValue
fn mysql_row_to_db_value(row: &sqlx::mysql::MySqlRow, idx: usize) -> DbResult<DbValue> {
    // Use try_get<Option<T>> — avoids ValueRef entirely
    if let Ok(v) = row.try_get::<Option<bool>, usize>(idx) { return Ok(v.map(DbValue::Bool).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<i16>, usize>(idx) { return Ok(v.map(|i| DbValue::Int(i as i64)).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<i32>, usize>(idx) { return Ok(v.map(|i| DbValue::Int(i as i64)).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<i64>, usize>(idx) { return Ok(v.map(DbValue::Int).unwrap_or(DbValue::Null)); }
    if let Ok(v) = row.try_get::<Option<f32>, usize>(idx) { return Ok(v.map(|f| DbValue::Float(f as f64)).unwrap_or(DbValue::Null)); }
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
