use async_trait::async_trait;
use db_core::{
    ColumnMeta, ColumnSchema, ConnectionConfig, DatabaseDriver, DatabaseInfo, DbError, DbResult,
    DbValue, ExecResult, IndexSchema, QueryResult, Row as DbRow, TableInfo, TableSchema, TableType,
};
use sqlx::{
    Column, Pool, Row, Sqlite, TypeInfo, ValueRef,
    sqlite::{SqlitePoolOptions, SqliteRow},
};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

pub struct SqliteDriver {
    pool: Pool<Sqlite>,
    database: String,
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self> {
        let url = sqlite_url(&config.database);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(map_connect_error)?;

        Ok(Self {
            pool,
            database: config.database.clone(),
        })
    }

    async fn ping(&self) -> DbResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(map_query_error)?;
        Ok(())
    }

    async fn close(&self) -> DbResult<()> {
        self.pool.close().await;
        Ok(())
    }

    async fn query(&self, sql: &str, params: Vec<serde_json::Value>) -> DbResult<QueryResult> {
        ensure_no_params(&params)?;

        let started_at = Instant::now();
        let rows = sqlx::query(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(map_query_error)?;

        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|column| ColumnMeta {
                        name: column.name().to_string(),
                        data_type: column.type_info().name().to_string(),
                        nullable: true,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            result_rows.push(sqlite_row_to_result_row(row)?);
        }

        Ok(QueryResult::new(
            columns,
            result_rows,
            started_at.elapsed().as_millis() as u64,
        ))
    }

    async fn execute(&self, sql: &str, params: Vec<serde_json::Value>) -> DbResult<ExecResult> {
        ensure_no_params(&params)?;

        let started_at = Instant::now();
        let result = sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(map_query_error)?;

        Ok(ExecResult {
            rows_affected: result.rows_affected(),
            last_insert_id: Some(result.last_insert_rowid()),
            elapsed_ms: started_at.elapsed().as_millis() as u64,
        })
    }

    async fn execute_batch(&self, statements: Vec<String>) -> DbResult<Vec<ExecResult>> {
        let mut results = Vec::with_capacity(statements.len());
        for statement in statements {
            results.push(self.execute(&statement, vec![]).await?);
        }
        Ok(results)
    }

    async fn explain(&self, sql: &str) -> DbResult<String> {
        let plan_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let rows = sqlx::query(&plan_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(map_query_error)?;

        let lines = rows
            .iter()
            .filter_map(|row| row.try_get::<Option<String>, _>("detail").ok().flatten())
            .collect::<Vec<_>>();

        if lines.is_empty() {
            Ok("查询计划为空".to_string())
        } else {
            Ok(lines.join("\n"))
        }
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        Ok(vec![DatabaseInfo {
            name: self.database.clone(),
            tables: self.list_table_infos().await?,
        }])
    }

    async fn list_tables(&self, _database: &str) -> DbResult<Vec<String>> {
        Ok(self
            .list_table_infos()
            .await?
            .into_iter()
            .map(|table| table.name)
            .collect())
    }

    async fn describe_table(&self, _database: &str, table: &str) -> DbResult<TableSchema> {
        let escaped_table = escape_identifier(table);
        let pragma_sql = format!("PRAGMA main.table_info(\"{escaped_table}\")");
        let rows = sqlx::query(&pragma_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(map_query_error)?;

        if rows.is_empty() {
            return Err(DbError::QueryFailed(format!("表不存在: {table}")));
        }

        let unique_columns = self.load_unique_columns(table).await?;
        let mut columns = Vec::with_capacity(rows.len());
        let mut primary_keys_with_order = Vec::new();

        for row in rows {
            let name = row.try_get::<String, _>("name").map_err(map_query_error)?;
            let pk_order = row.try_get::<i64, _>("pk").map_err(map_query_error)?;
            let is_primary_key = pk_order > 0;
            if is_primary_key {
                primary_keys_with_order.push((pk_order, name.clone()));
            }

            columns.push(ColumnSchema {
                name: name.clone(),
                data_type: row
                    .try_get::<String, _>("type")
                    .unwrap_or_else(|_| "TEXT".to_string()),
                nullable: row
                    .try_get::<i64, _>("notnull")
                    .map(|value| value == 0)
                    .unwrap_or(true),
                default_value: row
                    .try_get::<Option<String>, _>("dflt_value")
                    .unwrap_or(None),
                is_primary_key,
                is_unique: unique_columns.contains(&name),
                comment: None,
                char_max_length: None,
            });
        }

        primary_keys_with_order.sort_by_key(|(order, _)| *order);
        let primary_keys = primary_keys_with_order
            .into_iter()
            .map(|(_, column)| column)
            .collect();

        Ok(TableSchema {
            schema: Some("main".to_string()),
            name: table.to_string(),
            columns,
            indexes: self.load_indexes(table).await?,
            primary_keys,
        })
    }

    async fn get_columns(&self, _database: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
        let schema = self.describe_table("main", table).await?;
        Ok(schema
            .columns
            .into_iter()
            .map(|column| ColumnMeta {
                name: column.name,
                data_type: column.data_type,
                nullable: column.nullable,
            })
            .collect())
    }
}

impl SqliteDriver {
    async fn list_table_infos(&self) -> DbResult<Vec<TableInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT name, type
            FROM sqlite_master
            WHERE type IN ('table', 'view')
              AND name NOT LIKE 'sqlite_%'
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        let mut tables = Vec::with_capacity(rows.len());
        for row in rows {
            let kind = row
                .try_get::<String, _>("type")
                .unwrap_or_else(|_| "table".to_string());
            tables.push(TableInfo {
                name: row.try_get::<String, _>("name").map_err(map_query_error)?,
                table_type: match kind.as_str() {
                    "view" => TableType::View,
                    _ => TableType::Table,
                },
                row_estimate: None,
                size_bytes: None,
            });
        }

        Ok(tables)
    }

    async fn load_unique_columns(&self, table: &str) -> DbResult<HashSet<String>> {
        let indexes = self.load_indexes(table).await?;
        let mut unique_columns = HashSet::new();
        for index in indexes {
            if index.is_unique {
                for column in index.columns {
                    unique_columns.insert(column);
                }
            }
        }
        Ok(unique_columns)
    }

    async fn load_indexes(&self, table: &str) -> DbResult<Vec<IndexSchema>> {
        let escaped_table = escape_identifier(table);
        let list_sql = format!("PRAGMA main.index_list(\"{escaped_table}\")");
        let index_rows = sqlx::query(&list_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(map_query_error)?;

        let mut indexes = Vec::with_capacity(index_rows.len());
        for row in index_rows {
            let index_name = row.try_get::<String, _>("name").map_err(map_query_error)?;
            let escaped_index = escape_identifier(&index_name);
            let info_sql = format!("PRAGMA main.index_info(\"{escaped_index}\")");
            let column_rows = sqlx::query(&info_sql)
                .fetch_all(&self.pool)
                .await
                .map_err(map_query_error)?;

            let columns = column_rows
                .into_iter()
                .filter_map(|column_row| {
                    column_row
                        .try_get::<Option<String>, _>("name")
                        .ok()
                        .flatten()
                })
                .collect::<Vec<_>>();

            indexes.push(IndexSchema {
                name: index_name,
                columns,
                is_unique: row
                    .try_get::<i64, _>("unique")
                    .map(|value| value == 1)
                    .unwrap_or(false),
                is_primary: row
                    .try_get::<String, _>("origin")
                    .map(|value| value == "pk")
                    .unwrap_or(false),
            });
        }

        Ok(indexes)
    }
}

fn sqlite_row_to_result_row(row: &SqliteRow) -> DbResult<DbRow> {
    let mut result = HashMap::with_capacity(row.columns().len());
    for (index, column) in row.columns().iter().enumerate() {
        let value = sqlite_value_to_db_value(row, index)?;
        result.insert(column.name().to_string(), value);
    }
    Ok(result)
}

fn sqlite_value_to_db_value(row: &SqliteRow, index: usize) -> DbResult<DbValue> {
    let raw_value = row.try_get_raw(index).map_err(map_query_error)?;
    if raw_value.is_null() {
        return Ok(DbValue::Null);
    }

    let type_name = raw_value.type_info().name().to_ascii_lowercase();
    if type_name.contains("int") {
        return row
            .try_get::<i64, _>(index)
            .map(DbValue::Int)
            .map_err(map_query_error);
    }

    if type_name.contains("real")
        || type_name.contains("float")
        || type_name.contains("double")
        || type_name.contains("numeric")
        || type_name.contains("decimal")
    {
        return row
            .try_get::<f64, _>(index)
            .map(DbValue::Float)
            .or_else(|_| row.try_get::<String, _>(index).map(DbValue::Text))
            .map_err(map_query_error);
    }

    if type_name.contains("blob") || type_name.contains("binary") {
        return row
            .try_get::<Vec<u8>, _>(index)
            .map(DbValue::Bytes)
            .map_err(map_query_error);
    }

    if type_name.contains("bool") {
        return row
            .try_get::<bool, _>(index)
            .map(DbValue::Bool)
            .map_err(map_query_error);
    }

    if type_name.contains("json") {
        return row
            .try_get::<String, _>(index)
            .ok()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
            .map(DbValue::Json)
            .ok_or_else(|| DbError::TypeConversion {
                column: index.to_string(),
                target_type: "json".to_string(),
            })
            .or_else(|_| {
                row.try_get::<String, _>(index)
                    .map(DbValue::Text)
                    .map_err(map_query_error)
            });
    }

    row.try_get::<String, _>(index)
        .map(DbValue::Text)
        .or_else(|_| row.try_get::<i64, _>(index).map(DbValue::Int))
        .or_else(|_| row.try_get::<f64, _>(index).map(DbValue::Float))
        .or_else(|_| row.try_get::<bool, _>(index).map(DbValue::Bool))
        .map_err(map_query_error)
}

fn ensure_no_params(params: &[serde_json::Value]) -> DbResult<()> {
    if params.is_empty() {
        Ok(())
    } else {
        Err(DbError::Unsupported(
            "当前版本暂不支持带参数的关系型查询".to_string(),
        ))
    }
}

fn sqlite_url(database: &str) -> String {
    if database.starts_with("sqlite:") {
        database.to_string()
    } else if database == ":memory:" {
        "sqlite::memory:".to_string()
    } else if database.starts_with('/') {
        format!("sqlite://{database}")
    } else {
        format!("sqlite:{database}")
    }
}

fn escape_identifier(identifier: &str) -> String {
    identifier.replace('"', "\"\"")
}

fn map_connect_error(error: sqlx::Error) -> DbError {
    match error {
        sqlx::Error::PoolTimedOut => DbError::ConnectionTimeout,
        other => DbError::ConnectionFailed(other.to_string()),
    }
}

fn map_query_error(error: sqlx::Error) -> DbError {
    match error {
        sqlx::Error::Database(database_error) => {
            let message = database_error.message().to_string();
            if message.to_ascii_lowercase().contains("syntax") {
                DbError::SyntaxError(message)
            } else {
                DbError::QueryFailed(message)
            }
        }
        other => DbError::QueryFailed(other.to_string()),
    }
}
