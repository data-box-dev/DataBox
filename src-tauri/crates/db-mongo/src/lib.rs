use async_trait::async_trait;
use db_core::{
    ColumnMeta, ColumnSchema, ConnectionConfig, DatabaseDriver, DatabaseInfo, DbError, DbResult,
    DbValue, ExecResult, FindOptions, QueryResult, Row as DbRow, TableInfo, TableSchema, TableType,
};
use mongodb::{
    Client, Collection,
    bson::{Bson, Document, doc},
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{collections::BTreeMap, time::Instant};

pub struct MongoDriver {
    client: Client,
    database: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum MongoQueryCommand {
    Find {
        database: Option<String>,
        collection: String,
        #[serde(default = "default_object")]
        filter: Value,
        #[serde(default)]
        options: FindOptions,
    },
    Aggregate {
        database: Option<String>,
        collection: String,
        pipeline: Vec<Value>,
    },
    Count {
        database: Option<String>,
        collection: String,
        #[serde(default = "default_object")]
        filter: Value,
    },
    ListCollections {
        database: Option<String>,
    },
    ListDatabases,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum MongoExecCommand {
    InsertOne {
        database: Option<String>,
        collection: String,
        document: Value,
    },
    InsertMany {
        database: Option<String>,
        collection: String,
        documents: Vec<Value>,
    },
    UpdateOne {
        database: Option<String>,
        collection: String,
        #[serde(default = "default_object")]
        filter: Value,
        update: Value,
    },
    UpdateMany {
        database: Option<String>,
        collection: String,
        #[serde(default = "default_object")]
        filter: Value,
        update: Value,
    },
    DeleteOne {
        database: Option<String>,
        collection: String,
        #[serde(default = "default_object")]
        filter: Value,
    },
    DeleteMany {
        database: Option<String>,
        collection: String,
        #[serde(default = "default_object")]
        filter: Value,
    },
}

#[async_trait]
impl DatabaseDriver for MongoDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self> {
        let client = Client::with_uri_str(mongo_connection_url(config))
            .await
            .map_err(map_connect_error)?;
        let database = if config.database.trim().is_empty() {
            "admin".to_string()
        } else {
            config.database.clone()
        };

        Ok(Self { client, database })
    }

    async fn ping(&self) -> DbResult<()> {
        self.client
            .database("admin")
            .run_command(doc! { "ping": 1i32 })
            .await
            .map_err(map_query_error)?;
        Ok(())
    }

    async fn close(&self) -> DbResult<()> {
        Ok(())
    }

    async fn query(&self, sql: &str, params: Vec<Value>) -> DbResult<QueryResult> {
        ensure_no_params(&params)?;
        let command: MongoQueryCommand = serde_json::from_str(sql).map_err(|error| {
            DbError::SyntaxError(format!(
                "Mongo 查询命令需要 JSON 格式，示例: {{\"action\":\"find\",\"collection\":\"users\",\"filter\":{{}}}}; {error}"
            ))
        })?;

        let started_at = Instant::now();
        match command {
            MongoQueryCommand::Find {
                database,
                collection,
                filter,
                options,
            } => {
                let collection_ref = self.collection(database.as_deref(), &collection);
                let mut action = collection_ref.find(value_to_document(filter, "filter")?);

                if let Some(limit) = options.limit {
                    action = action.limit(i64::from(limit));
                }
                if let Some(skip) = options.skip {
                    action = action.skip(u64::from(skip));
                }
                if let Some(sort) = options.sort {
                    action = action.sort(value_to_document(sort, "options.sort")?);
                }
                if let Some(projection) = options.projection {
                    action = action.projection(value_to_document(projection, "options.projection")?);
                }

                let documents = collect_documents(action.await.map_err(map_query_error)?).await?;
                query_result_from_documents(documents, started_at.elapsed().as_millis() as u64)
            }
            MongoQueryCommand::Aggregate {
                database,
                collection,
                pipeline,
            } => {
                let collection_ref = self.collection(database.as_deref(), &collection);
                let pipeline = values_to_documents(pipeline, "pipeline")?;
                let documents = collect_documents(
                    collection_ref
                        .aggregate(pipeline)
                        .await
                        .map_err(map_query_error)?,
                )
                .await?;
                query_result_from_documents(documents, started_at.elapsed().as_millis() as u64)
            }
            MongoQueryCommand::Count {
                database,
                collection,
                filter,
            } => {
                let collection_ref = self.collection(database.as_deref(), &collection);
                let count = collection_ref
                    .count_documents(value_to_document(filter, "filter")?)
                    .await
                    .map_err(map_query_error)?;

                Ok(QueryResult::new(
                    vec![ColumnMeta {
                        name: "count".to_string(),
                        data_type: "int".to_string(),
                        nullable: false,
                    }],
                    vec![{
                        let mut row = DbRow::new();
                        row.insert(
                            "count".to_string(),
                            DbValue::Int(i64::try_from(count).unwrap_or(i64::MAX)),
                        );
                        row
                    }],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            MongoQueryCommand::ListCollections { database } => {
                let database = database.unwrap_or_else(|| self.database.clone());
                let names = self
                    .client
                    .database(&database)
                    .list_collection_names()
                    .await
                    .map_err(map_query_error)?;
                let rows = names
                    .into_iter()
                    .map(|name| {
                        let mut row = DbRow::new();
                        row.insert("name".to_string(), DbValue::Text(name));
                        row
                    })
                    .collect::<Vec<_>>();
                Ok(QueryResult::new(
                    vec![ColumnMeta {
                        name: "name".to_string(),
                        data_type: "string".to_string(),
                        nullable: false,
                    }],
                    rows,
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            MongoQueryCommand::ListDatabases => {
                let names = self
                    .client
                    .list_database_names()
                    .await
                    .map_err(map_query_error)?;
                let rows = names
                    .into_iter()
                    .map(|name| {
                        let mut row = DbRow::new();
                        row.insert("name".to_string(), DbValue::Text(name));
                        row
                    })
                    .collect::<Vec<_>>();
                Ok(QueryResult::new(
                    vec![ColumnMeta {
                        name: "name".to_string(),
                        data_type: "string".to_string(),
                        nullable: false,
                    }],
                    rows,
                    started_at.elapsed().as_millis() as u64,
                ))
            }
        }
    }

    async fn execute(&self, sql: &str, params: Vec<Value>) -> DbResult<ExecResult> {
        ensure_no_params(&params)?;
        let command: MongoExecCommand = serde_json::from_str(sql).map_err(|error| {
            DbError::SyntaxError(format!(
                "Mongo 执行命令需要 JSON 格式，示例: {{\"action\":\"insert_one\",\"collection\":\"users\",\"document\":{{\"name\":\"Alice\"}}}}; {error}"
            ))
        })?;

        let started_at = Instant::now();
        let rows_affected = match command {
            MongoExecCommand::InsertOne {
                database,
                collection,
                document,
            } => {
                self.collection(database.as_deref(), &collection)
                    .insert_one(value_to_document(document, "document")?)
                    .await
                    .map_err(map_query_error)?;
                1
            }
            MongoExecCommand::InsertMany {
                database,
                collection,
                documents,
            } => {
                let documents = values_to_documents(documents, "documents")?;
                let result = self
                    .collection(database.as_deref(), &collection)
                    .insert_many(documents)
                    .await
                    .map_err(map_query_error)?;
                u64::try_from(result.inserted_ids.len()).unwrap_or(u64::MAX)
            }
            MongoExecCommand::UpdateOne {
                database,
                collection,
                filter,
                update,
            } => {
                let result = self
                    .collection(database.as_deref(), &collection)
                    .update_one(
                        value_to_document(filter, "filter")?,
                        value_to_document(update, "update")?,
                    )
                    .await
                    .map_err(map_query_error)?;
                result.modified_count
            }
            MongoExecCommand::UpdateMany {
                database,
                collection,
                filter,
                update,
            } => {
                let result = self
                    .collection(database.as_deref(), &collection)
                    .update_many(
                        value_to_document(filter, "filter")?,
                        value_to_document(update, "update")?,
                    )
                    .await
                    .map_err(map_query_error)?;
                result.modified_count
            }
            MongoExecCommand::DeleteOne {
                database,
                collection,
                filter,
            } => {
                let result = self
                    .collection(database.as_deref(), &collection)
                    .delete_one(value_to_document(filter, "filter")?)
                    .await
                    .map_err(map_query_error)?;
                result.deleted_count
            }
            MongoExecCommand::DeleteMany {
                database,
                collection,
                filter,
            } => {
                let result = self
                    .collection(database.as_deref(), &collection)
                    .delete_many(value_to_document(filter, "filter")?)
                    .await
                    .map_err(map_query_error)?;
                result.deleted_count
            }
        };

        Ok(ExecResult {
            rows_affected,
            last_insert_id: None,
            elapsed_ms: started_at.elapsed().as_millis() as u64,
        })
    }

    async fn execute_batch(&self, _statements: Vec<String>) -> DbResult<Vec<ExecResult>> {
        Err(DbError::Unsupported(
            "MongoDB 暂不支持批量 SQL 语句执行".to_string(),
        ))
    }

    async fn explain(&self, _sql: &str) -> DbResult<String> {
        Err(DbError::Unsupported(
            "MongoDB 暂不支持 EXPLAIN 文本输出，请改用 aggregate + explain".to_string(),
        ))
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let names = self
            .client
            .list_database_names()
            .await
            .map_err(map_query_error)?;
        let mut databases = Vec::with_capacity(names.len());

        for name in names {
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
            .client
            .database(database)
            .list_collection_names()
            .await
            .map_err(map_query_error)?)
    }

    async fn describe_table(&self, database: &str, table: &str) -> DbResult<TableSchema> {
        let collection = self.client.database(database).collection::<Document>(table);
        let sample = collection
            .find_one(doc! {})
            .await
            .map_err(map_query_error)?;

        let mut columns = Vec::new();
        if let Some(document) = sample {
            for (name, value) in document {
                columns.push(ColumnSchema {
                    name: name.clone(),
                    data_type: bson_type_name(&value),
                    nullable: name != "_id",
                    default_value: None,
                    is_primary_key: name == "_id",
                    is_unique: name == "_id",
                    comment: None,
                    char_max_length: None,
                });
            }
        } else {
            columns.push(ColumnSchema {
                name: "_id".to_string(),
                data_type: "object_id".to_string(),
                nullable: false,
                default_value: None,
                is_primary_key: true,
                is_unique: true,
                comment: None,
                char_max_length: None,
            });
        }

        columns.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(TableSchema {
            schema: Some(database.to_string()),
            name: table.to_string(),
            columns,
            indexes: vec![],
            primary_keys: vec!["_id".to_string()],
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

impl MongoDriver {
    fn collection(&self, database: Option<&str>, name: &str) -> Collection<Document> {
        let database = database.unwrap_or(&self.database);
        self.client.database(database).collection(name)
    }

    async fn list_table_infos(&self, database: &str) -> DbResult<Vec<TableInfo>> {
        let names = self
            .client
            .database(database)
            .list_collection_names()
            .await
            .map_err(map_query_error)?;

        Ok(names
            .into_iter()
            .map(|name| TableInfo {
                name,
                table_type: TableType::Table,
                row_estimate: None,
                size_bytes: None,
            })
            .collect())
    }
}

async fn collect_documents(mut cursor: mongodb::Cursor<Document>) -> DbResult<Vec<Document>> {
    let mut documents = Vec::new();
    while cursor.advance().await.map_err(map_query_error)? {
        let document = cursor
            .deserialize_current()
            .map_err(|error| DbError::QueryFailed(error.to_string()))?;
        documents.push(document);
    }
    Ok(documents)
}

fn query_result_from_documents(documents: Vec<Document>, elapsed_ms: u64) -> DbResult<QueryResult> {
    let mut json_rows = Vec::with_capacity(documents.len());
    for document in documents {
        json_rows.push(
            serde_json::to_value(document)
                .map_err(|error| DbError::Serialization(error.to_string()))?,
        );
    }

    query_result_from_json_values(json_rows, elapsed_ms)
}

fn query_result_from_json_values(values: Vec<Value>, elapsed_ms: u64) -> DbResult<QueryResult> {
    let mut columns = BTreeMap::<String, String>::new();
    let mut rows = Vec::<DbRow>::with_capacity(values.len());

    for value in values {
        let object = match value {
            Value::Object(object) => object,
            other => {
                let mut object = Map::new();
                object.insert("value".to_string(), other);
                object
            }
        };

        let mut row = DbRow::with_capacity(object.len());
        for (key, value) in object {
            columns
                .entry(key.clone())
                .or_insert_with(|| json_value_type_name(&value));
            row.insert(key, json_value_to_db_value(value));
        }
        rows.push(row);
    }

    let columns = columns
        .into_iter()
        .map(|(name, data_type)| ColumnMeta {
            name,
            data_type,
            nullable: true,
        })
        .collect::<Vec<_>>();

    Ok(QueryResult::new(columns, rows, elapsed_ms))
}

fn json_value_to_db_value(value: Value) -> DbValue {
    match value {
        Value::Null => DbValue::Null,
        Value::Bool(value) => DbValue::Bool(value),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                DbValue::Int(value)
            } else if let Some(value) = value.as_f64() {
                DbValue::Float(value)
            } else {
                DbValue::Text(value.to_string())
            }
        }
        Value::String(value) => DbValue::Text(value),
        Value::Array(values) => DbValue::Array(values.into_iter().map(json_value_to_db_value).collect()),
        Value::Object(object) => DbValue::Json(Value::Object(object)),
    }
}

fn json_value_type_name(value: &Value) -> String {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .to_string()
}

fn bson_type_name(value: &Bson) -> String {
    match value {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "document",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascript_scope",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binary",
        Bson::ObjectId(_) => "object_id",
        Bson::DateTime(_) => "datetime",
        Bson::Symbol(_) => "symbol",
        Bson::Decimal128(_) => "decimal128",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "max_key",
        Bson::MinKey => "min_key",
        Bson::DbPointer(_) => "db_pointer",
    }
    .to_string()
}

fn value_to_document(value: Value, field_name: &str) -> DbResult<Document> {
    match value {
        Value::Null => Ok(Document::new()),
        Value::Object(_) => mongodb::bson::to_document(&value)
            .map_err(|error| DbError::Serialization(format!("{field_name} 不是合法对象: {error}"))),
        _ => Err(DbError::SyntaxError(format!(
            "{field_name} 必须是 JSON 对象"
        ))),
    }
}

fn values_to_documents(values: Vec<Value>, field_name: &str) -> DbResult<Vec<Document>> {
    let mut documents = Vec::with_capacity(values.len());
    for value in values {
        documents.push(value_to_document(value, field_name)?);
    }
    Ok(documents)
}

fn ensure_no_params(params: &[Value]) -> DbResult<()> {
    if params.is_empty() {
        Ok(())
    } else {
        Err(DbError::Unsupported(
            "当前版本暂不支持带参数的 Mongo 命令".to_string(),
        ))
    }
}

fn default_object() -> Value {
    Value::Object(Map::new())
}

fn mongo_connection_url(config: &ConnectionConfig) -> String {
    let host = if config.host.trim().is_empty() {
        "127.0.0.1"
    } else {
        config.host.trim()
    };
    let port = if config.port == 0 { 27017 } else { config.port };
    let database = if config.database.trim().is_empty() {
        "admin"
    } else {
        config.database.trim()
    };

    if config.username.trim().is_empty() {
        format!("mongodb://{host}:{port}/{database}")
    } else if config.password.is_empty() {
        format!("mongodb://{}@{host}:{port}/{database}", config.username.trim())
    } else {
        format!(
            "mongodb://{}:{}@{host}:{port}/{database}",
            config.username.trim(),
            config.password
        )
    }
}

fn map_connect_error(error: mongodb::error::Error) -> DbError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") {
        DbError::ConnectionTimeout
    } else if lower.contains("authentication") || lower.contains("auth failed") {
        DbError::AuthFailed
    } else {
        DbError::ConnectionFailed(message)
    }
}

fn map_query_error(error: mongodb::error::Error) -> DbError {
    DbError::QueryFailed(error.to_string())
}
