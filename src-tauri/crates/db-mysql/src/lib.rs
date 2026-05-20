use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use db_core::{
    ColumnMeta, ColumnSchema, ConnectionConfig, DatabaseDriver, DatabaseInfo, DbError, DbResult,
    DbValue, ExecResult, IndexSchema, QueryResult, Row as DbRow, TableInfo, TableSchema, TableType,
};
use sqlx::{
    Column, MySql, Pool, Row, TypeInfo, ValueRef,
    mysql::{MySqlPoolOptions, MySqlRow},
};
use std::{collections::HashMap, time::Instant};

pub struct MysqlDriver {
    pool: Pool<MySql>,
    database: String,
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&config.to_url())
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
            result_rows.push(mysql_row_to_result_row(row)?);
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
            last_insert_id: i64::try_from(result.last_insert_id()).ok(),
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
        let result = self.query(&format!("EXPLAIN {sql}"), vec![]).await?;
        serde_json::to_string_pretty(&result.rows).map_err(DbError::from)
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let rows = sqlx::query("SHOW DATABASES")
            .fetch_all(&self.pool)
            .await
            .map_err(map_query_error)?;

        let mut databases = Vec::with_capacity(rows.len());
        for row in rows {
            let name = row.try_get::<String, _>(0).map_err(map_query_error)?;
            let tables = if name == self.database {
                self.list_table_infos(&name).await?
            } else {
                Vec::new()
            };

            databases.push(DatabaseInfo { name, tables });
        }

        Ok(databases)
    }

    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>> {
        Ok(self
            .list_table_infos(database)
            .await?
            .into_iter()
            .map(|table| table.name)
            .collect())
    }

    async fn describe_table(&self, database: &str, table: &str) -> DbResult<TableSchema> {
        let rows = sqlx::query(
            r#"
            SELECT
                COLUMN_NAME,
                COLUMN_TYPE,
                IS_NULLABLE,
                COLUMN_DEFAULT,
                COLUMN_KEY,
                COLUMN_COMMENT,
                CHARACTER_MAXIMUM_LENGTH
            FROM information_schema.columns
            WHERE table_schema = ?
              AND table_name = ?
            ORDER BY ORDINAL_POSITION
            "#,
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        if rows.is_empty() {
            return Err(DbError::QueryFailed(format!(
                "表不存在: {database}.{table}"
            )));
        }

        let indexes = self.load_indexes(database, table).await?;
        let mut columns = Vec::with_capacity(rows.len());
        let mut primary_keys = Vec::new();

        for row in rows {
            let name = mysql_string(&row, "COLUMN_NAME")?;
            let key_type = mysql_optional_string(&row, "COLUMN_KEY");
            let is_primary_key = key_type.as_deref() == Some("PRI");
            if is_primary_key {
                primary_keys.push(name.clone());
            }

            let is_unique = indexes.iter().any(|index| {
                index.is_unique && index.columns.len() == 1 && index.columns[0] == name
            });

            columns.push(ColumnSchema {
                name,
                data_type: mysql_string(&row, "COLUMN_TYPE")?,
                nullable: row
                    .try_get::<String, _>("IS_NULLABLE")
                    .map(|value| value.eq_ignore_ascii_case("YES"))
                    .unwrap_or(true),
                default_value: mysql_optional_string(&row, "COLUMN_DEFAULT"),
                is_primary_key,
                is_unique,
                comment: mysql_optional_string(&row, "COLUMN_COMMENT"),
                char_max_length: row
                    .try_get::<Option<i64>, _>("CHARACTER_MAXIMUM_LENGTH")
                    .unwrap_or(None)
                    .and_then(|value| u32::try_from(value).ok()),
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

    async fn get_columns(&self, database: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
        let schema = self.describe_table(database, table).await?;
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

impl MysqlDriver {
    async fn list_table_infos(&self, database: &str) -> DbResult<Vec<TableInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT TABLE_NAME, TABLE_TYPE, TABLE_ROWS, DATA_LENGTH + INDEX_LENGTH AS SIZE_BYTES
            FROM information_schema.tables
            WHERE table_schema = ?
            ORDER BY TABLE_NAME
            "#,
        )
        .bind(database)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        let mut tables = Vec::with_capacity(rows.len());
        for row in rows {
            let table_type =
                mysql_string(&row, "TABLE_TYPE").unwrap_or_else(|_| "BASE TABLE".to_string());
            tables.push(TableInfo {
                name: mysql_string(&row, "TABLE_NAME")?,
                table_type: match table_type.as_str() {
                    "VIEW" => TableType::View,
                    _ => TableType::Table,
                },
                row_estimate: row
                    .try_get::<Option<i64>, _>("TABLE_ROWS")
                    .unwrap_or(None)
                    .and_then(|value| u64::try_from(value).ok()),
                size_bytes: row
                    .try_get::<Option<i64>, _>("SIZE_BYTES")
                    .unwrap_or(None)
                    .and_then(|value| u64::try_from(value).ok()),
            });
        }

        Ok(tables)
    }

    async fn load_indexes(&self, database: &str, table: &str) -> DbResult<Vec<IndexSchema>> {
        let rows = sqlx::query(
            r#"
            SELECT
                INDEX_NAME,
                NON_UNIQUE,
                GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) AS COLUMN_NAMES
            FROM information_schema.statistics
            WHERE table_schema = ?
              AND table_name = ?
            GROUP BY INDEX_NAME, NON_UNIQUE
            ORDER BY INDEX_NAME
            "#,
        )
        .bind(database)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        let mut indexes = Vec::with_capacity(rows.len());
        for row in rows {
            let name = mysql_string(&row, "INDEX_NAME")?;
            let columns = row
                .try_get::<Option<String>, _>("COLUMN_NAMES")
                .or_else(|_| {
                    row.try_get::<Option<Vec<u8>>, _>("COLUMN_NAMES").map(|value| {
                        value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                    })
                })
                .unwrap_or(None)
                .unwrap_or_default()
                .split(',')
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string())
                .collect::<Vec<_>>();

            let non_unique = row.try_get::<i64, _>("NON_UNIQUE").unwrap_or(1);
            indexes.push(IndexSchema {
                name: name.clone(),
                columns,
                is_unique: non_unique == 0,
                is_primary: name == "PRIMARY",
            });
        }

        Ok(indexes)
    }
}

fn mysql_row_to_result_row(row: &MySqlRow) -> DbResult<DbRow> {
    let mut result = HashMap::with_capacity(row.columns().len());
    for (index, column) in row.columns().iter().enumerate() {
        let value = mysql_value_to_db_value(row, index)?;
        result.insert(column.name().to_string(), value);
    }
    Ok(result)
}

fn mysql_string(row: &MySqlRow, column: &str) -> DbResult<String> {
    row.try_get::<String, _>(column)
        .or_else(|_| {
            row.try_get::<Vec<u8>, _>(column)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        })
        .map_err(map_query_error)
}

fn mysql_optional_string(row: &MySqlRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column)
        .or_else(|_| {
            row.try_get::<Option<Vec<u8>>, _>(column)
                .map(|value| value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
        })
        .unwrap_or(None)
}

fn mysql_string_at(row: &MySqlRow, index: usize) -> DbResult<String> {
    row.try_get::<String, _>(index)
        .or_else(|_| {
            row.try_get::<Vec<u8>, _>(index)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        })
        .map_err(map_query_error)
}

fn mysql_value_to_db_value(row: &MySqlRow, index: usize) -> DbResult<DbValue> {
    let raw_value = row.try_get_raw(index).map_err(map_query_error)?;
    if raw_value.is_null() {
        return Ok(DbValue::Null);
    }

    let type_name = raw_value.type_info().name().to_ascii_lowercase();

    if type_name == "bool" || type_name == "boolean" {
        return row
            .try_get::<bool, _>(index)
            .map(DbValue::Bool)
            .map_err(map_query_error);
    }

    if type_name.contains("int") || type_name == "year" {
        return row
            .try_get::<i64, _>(index)
            .map(DbValue::Int)
            .or_else(|_| {
                row.try_get::<u64, _>(index).map(|value| {
                    i64::try_from(value)
                        .map(DbValue::Int)
                        .unwrap_or_else(|_| DbValue::Text(value.to_string()))
                })
            })
            .or_else(|_| row.try_get::<bool, _>(index).map(DbValue::Bool))
            .or_else(|_| mysql_string_at(row, index).map(DbValue::Text));
    }

    if type_name.contains("decimal") || type_name.contains("numeric") {
        return mysql_string_at(row, index).map(DbValue::Text);
    }

    if type_name.contains("float") || type_name.contains("double") {
        return row
            .try_get::<f64, _>(index)
            .map(DbValue::Float)
            .or_else(|_| mysql_string_at(row, index).map(DbValue::Text));
    }

    if type_name.contains("json") {
        return row
            .try_get::<serde_json::Value, _>(index)
            .map(DbValue::Json)
            .or_else(|_| mysql_string_at(row, index).map(DbValue::Text));
    }

    if type_name.contains("blob") || type_name.contains("binary") {
        return row
            .try_get::<Vec<u8>, _>(index)
            .map(DbValue::Bytes)
            .map_err(map_query_error);
    }

    if type_name == "timestamp" {
        return row
            .try_get::<DateTime<Utc>, _>(index)
            .map(DbValue::Timestamp)
            .or_else(|_| {
                row.try_get::<NaiveDateTime, _>(index).map(|value| {
                    DbValue::Timestamp(DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
                })
            })
            .map_err(map_query_error);
    }

    if type_name == "datetime" {
        return row
            .try_get::<NaiveDateTime, _>(index)
            .map(|value| DbValue::Timestamp(DateTime::<Utc>::from_naive_utc_and_offset(value, Utc)))
            .map_err(map_query_error);
    }

    if type_name == "date" {
        return row
            .try_get::<NaiveDate, _>(index)
            .map(|value| DbValue::Text(value.to_string()))
            .map_err(map_query_error);
    }

    if type_name == "time" {
        return row
            .try_get::<NaiveTime, _>(index)
            .map(|value| DbValue::Text(value.to_string()))
            .map_err(map_query_error);
    }

    mysql_string_at(row, index)
        .map(DbValue::Text)
        .or_else(|_| row.try_get::<i64, _>(index).map(DbValue::Int))
        .or_else(|_| {
            row.try_get::<u64, _>(index).map(|value| {
                i64::try_from(value)
                    .map(DbValue::Int)
                    .unwrap_or_else(|_| DbValue::Text(value.to_string()))
            })
        })
        .or_else(|_| row.try_get::<f64, _>(index).map(DbValue::Float))
        .or_else(|_| row.try_get::<bool, _>(index).map(DbValue::Bool))
        .or_else(|_| row.try_get::<Vec<u8>, _>(index).map(DbValue::Bytes))
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
