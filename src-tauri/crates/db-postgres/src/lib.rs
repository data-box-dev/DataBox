use async_trait::async_trait;
use db_core::{
    traits::DatabaseDriver,
    types::*,
    DbError,
    DbResult,
};
use sqlx::postgres::PgPoolOptions;

/// PostgreSQL 驱动实现
pub struct PostgresDriver {
    pool: sqlx::PgPool,
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    /// 从连接配置建立 PostgreSQL 连接
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
        let url = config.to_url();
        let pool = PgPoolOptions::new()
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
                data_type: "unknown".to_string(), // TODO: 从 pg_type 获取真实类型
                nullable: true,
            })
            .collect();

        // 将 sqlx Row 映射为 DbValue Row
        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut map = std::collections::HashMap::new();
            for (idx, col_name) in column_names.iter().enumerate() {
                let value = pg_row_to_db_value(row, idx)?;
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
            last_insert_id: None, // PostgreSQL 需用 RETURNING 获取
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
                last_insert_id: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        }

        tx.commit().await.map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(results)
    }

    /// EXPLAIN 查询执行计划
    async fn explain(&self, sql: &str) -> DbResult<String> {
        let row: (String,) = sqlx::query_as(&format!("EXPLAIN {}", sql))
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(row.0)
    }

    /// 列出所有数据库
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname")
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

    /// 列出数据库内所有表（含视图）
    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>> {
        // 连接到指定数据库
        let db_url = format!("{}/{}", self.pool.connect_options().get_database(), database);
        // 使用当前连接查询 pg_tables
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT schemaname, tablename FROM pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema') ORDER BY schemaname, tablename"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(schema, table)| format!("{}.{}", schema, table))
            .collect())
    }

    /// 获取表完整 Schema（列、索引、主键）
    async fn describe_table(
        &self,
        _database: &str,
        table: &str,
    ) -> DbResult<TableSchema> {
        // 解析 schema.table
        let (schema, table_name) = if table.contains('.') {
            let parts: Vec<&str> = table.split('.').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("public".to_string(), table.to_string())
        };

        // 查询列信息
        let column_rows = sqlx::query(
            r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                column_default
            FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2
            ORDER BY ordinal_position
            "#
        )
        .bind(&schema)
        .bind(&table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let columns: Vec<ColumnSchema> = column_rows
            .into_iter()
            .map(|row: sqlx::postgres::PgRow| {
                let name: String = row.get("column_name");
                let data_type: String = row.get("data_type");
                let is_nullable: String = row.get("is_nullable");
                let default: Option<String> = row.get("column_default");

                ColumnSchema {
                    name,
                    data_type,
                    nullable: is_nullable == "YES",
                    default_value: default,
                    is_primary_key: false, // 后续从主键查询填充
                    is_unique: false,      // 后续从索引查询填充
                    comment: None,
                    char_max_length: None,
                }
            })
            .collect();

        // 查询主键
        let pk_rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT a.attname
            FROM pg_index i
            JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            WHERE i.indrelid = $1::regclass AND i.indisprimary
            "#
        )
        .bind(format!("{}.{}", schema, table_name))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let primary_keys: Vec<String> = pk_rows.into_iter().map(|(name,)| name).collect();

        // 填充主键标记
        let mut columns_with_pk = columns;
        for col in &mut columns_with_pk {
            if primary_keys.contains(&col.name) {
                col.is_primary_key = true;
            }
        }

        Ok(TableSchema {
            schema: Some(schema),
            name: table_name,
            columns: columns_with_pk,
            indexes: vec![],
            primary_keys,
        })
    }

    /// 快速获取列元数据
    async fn get_columns(
        &self,
        _database: &str,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let (schema, table_name) = if table.contains('.') {
            let parts: Vec<&str> = table.split('.').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("public".to_string(), table.to_string())
        };

        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position"
        )
        .bind(&schema)
        . bind(&table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row: sqlx::postgres::PgRow| ColumnMeta {
                name: row.get("column_name"),
                data_type: row.get("data_type"),
                nullable: row.get::<String, _>("is_nullable") == "YES",
            })
            .collect())
    }
}

/// 将 PostgreSQL 行中的值转换为 DbValue
fn pg_row_to_db_value(row: &sqlx::postgres::PgRow, idx: usize) -> DbResult<DbValue> {
    use sqlx::postgres::types::PgValueRef;

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

    // 根据 PostgreSQL 类型解码
    // 注意：这里简化处理，实际需要根据 OID 精确匹配类型
    // 使用 try_decode 进行类型推断
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
    } else if let Ok(v) = value_ref.try_decode::<i16>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<i32>() {
        Decoded::Int(v as i64)
    } else if let Ok(v) = value_ref.try_decode::<i64>() {
        Decoded::Int(v)
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

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
