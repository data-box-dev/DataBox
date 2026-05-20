use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use db_core::{
    ColumnMeta, ColumnSchema, ConnectionConfig, DatabaseDriver, DatabaseInfo, DbError, DbResult,
    DbValue, ExecResult, IndexSchema, QueryResult, Row as DbRow, TableInfo, TableSchema, TableType,
};
use sqlx::{
    Column, Pool, Postgres, Row, TypeInfo, ValueRef,
    postgres::{PgPoolOptions, PgRow},
};
use std::{collections::HashMap, time::Instant};

pub struct PostgresDriver {
    pool: Pool<Postgres>,
    database: String,
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self> {
        let pool = PgPoolOptions::new()
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
            result_rows.push(postgres_row_to_result_row(row)?);
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
            last_insert_id: None,
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
        let rows = sqlx::query(&format!("EXPLAIN {sql}"))
            .fetch_all(&self.pool)
            .await
            .map_err(map_query_error)?;

        let lines = rows
            .iter()
            .filter_map(|row| row.try_get::<Option<String>, _>(0).ok().flatten())
            .collect::<Vec<_>>();

        if lines.is_empty() {
            Ok("查询计划为空".to_string())
        } else {
            Ok(lines.join("\n"))
        }
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT datname
            FROM pg_database
            WHERE datistemplate = false
            ORDER BY datname
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        let mut databases = Vec::with_capacity(rows.len());
        for row in rows {
            let name = row
                .try_get::<String, _>("datname")
                .map_err(map_query_error)?;
            let tables = if name == self.database {
                self.list_table_infos().await?
            } else {
                Vec::new()
            };

            databases.push(DatabaseInfo { name, tables });
        }

        Ok(databases)
    }

    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>> {
        self.ensure_current_database(database)?;
        Ok(self
            .list_table_infos()
            .await?
            .into_iter()
            .map(|table| table.name)
            .collect())
    }

    async fn describe_table(&self, database: &str, table: &str) -> DbResult<TableSchema> {
        self.ensure_current_database(database)?;

        let (schema_name, table_name) = split_schema_and_table(table);
        let rows = sqlx::query(
            r#"
            SELECT
                column_name,
                udt_name,
                is_nullable,
                column_default,
                character_maximum_length
            FROM information_schema.columns
            WHERE table_catalog = $1
              AND table_schema = $2
              AND table_name = $3
            ORDER BY ordinal_position
            "#,
        )
        .bind(database)
        .bind(&schema_name)
        .bind(&table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        if rows.is_empty() {
            return Err(DbError::QueryFailed(format!(
                "表不存在: {schema_name}.{table_name}"
            )));
        }

        let indexes = self.load_indexes(&schema_name, &table_name).await?;
        let primary_keys = self
            .load_primary_keys(database, &schema_name, &table_name)
            .await?;

        let mut columns = Vec::with_capacity(rows.len());
        for row in rows {
            let name = row
                .try_get::<String, _>("column_name")
                .map_err(map_query_error)?;
            let is_primary_key = primary_keys.iter().any(|pk| pk == &name);
            let is_unique = indexes.iter().any(|index| {
                index.is_unique && index.columns.len() == 1 && index.columns[0] == name
            });

            columns.push(ColumnSchema {
                name,
                data_type: row
                    .try_get::<String, _>("udt_name")
                    .map_err(map_query_error)?,
                nullable: row
                    .try_get::<String, _>("is_nullable")
                    .map(|value| value.eq_ignore_ascii_case("YES"))
                    .unwrap_or(true),
                default_value: row
                    .try_get::<Option<String>, _>("column_default")
                    .unwrap_or(None),
                is_primary_key,
                is_unique,
                comment: None,
                char_max_length: row
                    .try_get::<Option<i32>, _>("character_maximum_length")
                    .unwrap_or(None)
                    .and_then(|value| u32::try_from(value).ok()),
            });
        }

        Ok(TableSchema {
            schema: Some(schema_name),
            name: table_name,
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

impl PostgresDriver {
    fn ensure_current_database(&self, database: &str) -> DbResult<()> {
        if database == self.database {
            Ok(())
        } else {
            Err(DbError::Unsupported(format!(
                "当前连接已绑定数据库 `{}`, 如需浏览 `{}` 请新建对应连接",
                self.database, database
            )))
        }
    }

    async fn list_table_infos(&self) -> DbResult<Vec<TableInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT
                table_schema,
                table_name,
                table_type
            FROM information_schema.tables
            WHERE table_catalog = $1
              AND table_schema NOT IN ('pg_catalog', 'information_schema')
            ORDER BY table_schema, table_name
            "#,
        )
        .bind(&self.database)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        let mut tables = Vec::with_capacity(rows.len());
        for row in rows {
            let schema = row
                .try_get::<String, _>("table_schema")
                .map_err(map_query_error)?;
            let name = row
                .try_get::<String, _>("table_name")
                .map_err(map_query_error)?;
            let table_type = row
                .try_get::<String, _>("table_type")
                .unwrap_or_else(|_| "BASE TABLE".to_string());

            tables.push(TableInfo {
                name: format!("{schema}.{name}"),
                table_type: match table_type.as_str() {
                    "VIEW" => TableType::View,
                    _ => TableType::Table,
                },
                row_estimate: None,
                size_bytes: None,
            });
        }

        Ok(tables)
    }

    async fn load_primary_keys(
        &self,
        database: &str,
        schema_name: &str,
        table_name: &str,
    ) -> DbResult<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT kcu.column_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
              ON tc.constraint_name = kcu.constraint_name
             AND tc.table_schema = kcu.table_schema
             AND tc.table_name = kcu.table_name
             AND tc.table_catalog = kcu.table_catalog
            WHERE tc.table_catalog = $1
              AND tc.table_schema = $2
              AND tc.table_name = $3
              AND tc.constraint_type = 'PRIMARY KEY'
            ORDER BY kcu.ordinal_position
            "#,
        )
        .bind(database)
        .bind(schema_name)
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        rows.into_iter()
            .map(|row| {
                row.try_get::<String, _>("column_name")
                    .map_err(map_query_error)
            })
            .collect()
    }

    async fn load_indexes(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> DbResult<Vec<IndexSchema>> {
        let rows = sqlx::query(
            r#"
            SELECT
                index_class.relname AS index_name,
                pg_index.indisunique AS is_unique,
                pg_index.indisprimary AS is_primary,
                string_agg(attribute.attname, ',' ORDER BY column_order.ordinality) AS column_names
            FROM pg_class table_class
            JOIN pg_namespace namespace ON namespace.oid = table_class.relnamespace
            JOIN pg_index ON pg_index.indrelid = table_class.oid
            JOIN pg_class index_class ON index_class.oid = pg_index.indexrelid
            JOIN unnest(pg_index.indkey) WITH ORDINALITY AS column_order(attnum, ordinality) ON TRUE
            JOIN pg_attribute attribute
              ON attribute.attrelid = table_class.oid
             AND attribute.attnum = column_order.attnum
            WHERE namespace.nspname = $1
              AND table_class.relname = $2
            GROUP BY index_class.relname, pg_index.indisunique, pg_index.indisprimary
            ORDER BY index_class.relname
            "#,
        )
        .bind(schema_name)
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(map_query_error)?;

        let mut indexes = Vec::with_capacity(rows.len());
        for row in rows {
            let columns = row
                .try_get::<Option<String>, _>("column_names")
                .unwrap_or(None)
                .unwrap_or_default()
                .split(',')
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string())
                .collect::<Vec<_>>();

            indexes.push(IndexSchema {
                name: row
                    .try_get::<String, _>("index_name")
                    .map_err(map_query_error)?,
                columns,
                is_unique: row.try_get::<bool, _>("is_unique").unwrap_or(false),
                is_primary: row.try_get::<bool, _>("is_primary").unwrap_or(false),
            });
        }

        Ok(indexes)
    }
}

fn postgres_row_to_result_row(row: &PgRow) -> DbResult<DbRow> {
    let mut result = HashMap::with_capacity(row.columns().len());
    for (index, column) in row.columns().iter().enumerate() {
        let value = postgres_value_to_db_value(row, index)?;
        result.insert(column.name().to_string(), value);
    }
    Ok(result)
}

fn postgres_value_to_db_value(row: &PgRow, index: usize) -> DbResult<DbValue> {
    let raw_value = row.try_get_raw(index).map_err(map_query_error)?;
    if raw_value.is_null() {
        return Ok(DbValue::Null);
    }

    let type_name = raw_value.type_info().name().to_ascii_lowercase();

    if type_name == "bool" {
        return row
            .try_get::<bool, _>(index)
            .map(DbValue::Bool)
            .map_err(map_query_error);
    }

    if matches!(type_name.as_str(), "int2" | "int4" | "int8" | "oid") {
        return row
            .try_get::<i64, _>(index)
            .map(DbValue::Int)
            .map_err(map_query_error);
    }

    if matches!(type_name.as_str(), "float4" | "float8" | "numeric") {
        return row
            .try_get::<f64, _>(index)
            .map(DbValue::Float)
            .or_else(|_| row.try_get::<String, _>(index).map(DbValue::Text))
            .map_err(map_query_error);
    }

    if type_name == "json" || type_name == "jsonb" {
        return row
            .try_get::<serde_json::Value, _>(index)
            .map(DbValue::Json)
            .or_else(|_| row.try_get::<String, _>(index).map(DbValue::Text))
            .map_err(map_query_error);
    }

    if type_name == "bytea" {
        return row
            .try_get::<Vec<u8>, _>(index)
            .map(DbValue::Bytes)
            .map_err(map_query_error);
    }

    if type_name == "timestamptz" {
        return row
            .try_get::<DateTime<Utc>, _>(index)
            .map(DbValue::Timestamp)
            .map_err(map_query_error);
    }

    if type_name == "timestamp" {
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

    if type_name.starts_with("time") {
        return row
            .try_get::<NaiveTime, _>(index)
            .map(|value| DbValue::Text(value.to_string()))
            .map_err(map_query_error);
    }

    row.try_get::<String, _>(index)
        .map(DbValue::Text)
        .or_else(|_| row.try_get::<i64, _>(index).map(DbValue::Int))
        .or_else(|_| row.try_get::<f64, _>(index).map(DbValue::Float))
        .or_else(|_| row.try_get::<bool, _>(index).map(DbValue::Bool))
        .map_err(map_query_error)
}

fn split_schema_and_table(input: &str) -> (String, String) {
    if let Some((schema, table)) = input.split_once('.') {
        (schema.to_string(), table.to_string())
    } else {
        ("public".to_string(), input.to_string())
    }
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
