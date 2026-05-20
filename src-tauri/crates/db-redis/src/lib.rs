use async_trait::async_trait;
use db_core::{
    ColumnMeta, ColumnSchema, ConnectionConfig, DatabaseDriver, DatabaseInfo, DbError, DbResult,
    DbValue, ExecResult, QueryResult, Row as DbRow, TableInfo, TableSchema, TableType,
};
use serde_json::Value;
use std::{collections::HashMap, time::Instant};

pub struct RedisDriver {
    client: redis::Client,
    database_index: i64,
}

#[async_trait]
impl DatabaseDriver for RedisDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self> {
        let database_index = parse_database_index(config);
        let client = redis::Client::open(redis_connection_url(config, database_index))
            .map_err(map_connect_error)?;
        let driver = Self {
            client,
            database_index,
        };
        driver.ping().await?;
        Ok(driver)
    }

    async fn ping(&self) -> DbResult<()> {
        let mut connection = self.connection().await?;
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
            .map_err(map_query_error)?;
        Ok(())
    }

    async fn close(&self) -> DbResult<()> {
        Ok(())
    }

    async fn query(&self, sql: &str, params: Vec<Value>) -> DbResult<QueryResult> {
        ensure_no_params(&params)?;
        let args = parse_command_args(sql)?;
        if args.is_empty() {
            return Err(DbError::SyntaxError("Redis 命令不能为空".to_string()));
        }

        let command = args[0].to_ascii_uppercase();
        let started_at = Instant::now();
        let mut connection = self.connection().await?;

        match command.as_str() {
            "PING" => {
                let value = redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                Ok(single_row_query_result(
                    vec![column("response", "string", false)],
                    vec![("response", DbValue::Text(value))],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "GET" => {
                let key = required_arg(&args, 1, "key")?;
                let value = redis::cmd("GET")
                    .arg(key)
                    .query_async::<Option<String>>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                Ok(single_row_query_result(
                    vec![column("key", "string", false), column("value", "string", true)],
                    vec![
                        ("key", DbValue::Text(key.to_string())),
                        (
                            "value",
                            value.map(DbValue::Text).unwrap_or(DbValue::Null),
                        ),
                    ],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "TTL" => {
                let key = required_arg(&args, 1, "key")?;
                let ttl = redis::cmd("TTL")
                    .arg(key)
                    .query_async::<i64>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                Ok(single_row_query_result(
                    vec![column("key", "string", false), column("ttl", "int", false)],
                    vec![
                        ("key", DbValue::Text(key.to_string())),
                        ("ttl", DbValue::Int(ttl)),
                    ],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "TYPE" => {
                let key = required_arg(&args, 1, "key")?;
                let value = redis::cmd("TYPE")
                    .arg(key)
                    .query_async::<String>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                Ok(single_row_query_result(
                    vec![column("key", "string", false), column("type", "string", false)],
                    vec![
                        ("key", DbValue::Text(key.to_string())),
                        ("type", DbValue::Text(value)),
                    ],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "EXISTS" => {
                let key = required_arg(&args, 1, "key")?;
                let exists = redis::cmd("EXISTS")
                    .arg(key)
                    .query_async::<i64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
                    > 0;
                Ok(single_row_query_result(
                    vec![column("key", "string", false), column("exists", "bool", false)],
                    vec![
                        ("key", DbValue::Text(key.to_string())),
                        ("exists", DbValue::Bool(exists)),
                    ],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "KEYS" => {
                let pattern = args.get(1).map(String::as_str).unwrap_or("*");
                let keys = redis::cmd("KEYS")
                    .arg(pattern)
                    .query_async::<Vec<String>>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                Ok(list_query_result(
                    vec![column("key", "string", false)],
                    keys.into_iter()
                        .map(|key| {
                            let mut row = DbRow::new();
                            row.insert("key".to_string(), DbValue::Text(key));
                            row
                        })
                        .collect(),
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "SCAN" => {
                let pattern = args.get(1).map(String::as_str).unwrap_or("*");
                let count = args
                    .get(2)
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(100);
                let keys = self.scan_keys(pattern, count, 2000).await?;
                Ok(list_query_result(
                    vec![column("key", "string", false)],
                    keys.into_iter()
                        .map(|key| {
                            let mut row = DbRow::new();
                            row.insert("key".to_string(), DbValue::Text(key));
                            row
                        })
                        .collect(),
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "HGETALL" => {
                let key = required_arg(&args, 1, "key")?;
                let map = redis::cmd("HGETALL")
                    .arg(key)
                    .query_async::<HashMap<String, String>>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                let mut rows = Vec::with_capacity(map.len());
                for (field, value) in map {
                    let mut row = DbRow::new();
                    row.insert("field".to_string(), DbValue::Text(field));
                    row.insert("value".to_string(), DbValue::Text(value));
                    rows.push(row);
                }
                Ok(list_query_result(
                    vec![column("field", "string", false), column("value", "string", true)],
                    rows,
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "LRANGE" => {
                let key = required_arg(&args, 1, "key")?;
                let start = required_arg(&args, 2, "start")?.parse::<i64>().map_err(|_| {
                    DbError::SyntaxError("LRANGE start 必须是整数".to_string())
                })?;
                let stop = required_arg(&args, 3, "stop")?.parse::<i64>().map_err(|_| {
                    DbError::SyntaxError("LRANGE stop 必须是整数".to_string())
                })?;
                let values = redis::cmd("LRANGE")
                    .arg(key)
                    .arg(start)
                    .arg(stop)
                    .query_async::<Vec<String>>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                let rows = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let mut row = DbRow::new();
                        row.insert(
                            "index".to_string(),
                            DbValue::Int(i64::try_from(index).unwrap_or(i64::MAX)),
                        );
                        row.insert("value".to_string(), DbValue::Text(value));
                        row
                    })
                    .collect();
                Ok(list_query_result(
                    vec![column("index", "int", false), column("value", "string", true)],
                    rows,
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "SMEMBERS" => {
                let key = required_arg(&args, 1, "key")?;
                let values = redis::cmd("SMEMBERS")
                    .arg(key)
                    .query_async::<Vec<String>>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                let rows = values
                    .into_iter()
                    .map(|member| {
                        let mut row = DbRow::new();
                        row.insert("member".to_string(), DbValue::Text(member));
                        row
                    })
                    .collect();
                Ok(list_query_result(
                    vec![column("member", "string", false)],
                    rows,
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            "ZRANGE" => {
                let key = required_arg(&args, 1, "key")?;
                let start = required_arg(&args, 2, "start")?.parse::<i64>().map_err(|_| {
                    DbError::SyntaxError("ZRANGE start 必须是整数".to_string())
                })?;
                let stop = required_arg(&args, 3, "stop")?.parse::<i64>().map_err(|_| {
                    DbError::SyntaxError("ZRANGE stop 必须是整数".to_string())
                })?;
                let with_scores = args
                    .get(4)
                    .map(|value| value.eq_ignore_ascii_case("WITHSCORES"))
                    .unwrap_or(false);

                if with_scores {
                    let values = redis::cmd("ZRANGE")
                        .arg(key)
                        .arg(start)
                        .arg(stop)
                        .arg("WITHSCORES")
                        .query_async::<Vec<String>>(&mut connection)
                        .await
                        .map_err(map_query_error)?;
                    let mut rows = Vec::new();
                    let mut chunk = values.chunks_exact(2);
                    for pair in &mut chunk {
                        let mut row = DbRow::new();
                        row.insert("member".to_string(), DbValue::Text(pair[0].clone()));
                        row.insert(
                            "score".to_string(),
                            pair[1]
                                .parse::<f64>()
                                .map(DbValue::Float)
                                .unwrap_or_else(|_| DbValue::Text(pair[1].clone())),
                        );
                        rows.push(row);
                    }
                    Ok(list_query_result(
                        vec![column("member", "string", false), column("score", "float", false)],
                        rows,
                        started_at.elapsed().as_millis() as u64,
                    ))
                } else {
                    let values = redis::cmd("ZRANGE")
                        .arg(key)
                        .arg(start)
                        .arg(stop)
                        .query_async::<Vec<String>>(&mut connection)
                        .await
                        .map_err(map_query_error)?;
                    Ok(list_query_result(
                        vec![column("member", "string", false)],
                        values
                            .into_iter()
                            .map(|member| {
                                let mut row = DbRow::new();
                                row.insert("member".to_string(), DbValue::Text(member));
                                row
                            })
                            .collect(),
                        started_at.elapsed().as_millis() as u64,
                    ))
                }
            }
            "INFO" => {
                let mut cmd = redis::cmd("INFO");
                if let Some(section) = args.get(1) {
                    cmd.arg(section);
                }
                let info = cmd
                    .query_async::<String>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                Ok(single_row_query_result(
                    vec![column("info", "string", false)],
                    vec![("info", DbValue::Text(info))],
                    started_at.elapsed().as_millis() as u64,
                ))
            }
            _ => Err(DbError::Unsupported(format!(
                "不支持的 Redis 查询命令 `{command}`。已支持: PING/GET/TTL/TYPE/EXISTS/KEYS/SCAN/HGETALL/LRANGE/SMEMBERS/ZRANGE/INFO"
            ))),
        }
    }

    async fn execute(&self, sql: &str, params: Vec<Value>) -> DbResult<ExecResult> {
        ensure_no_params(&params)?;
        let args = parse_command_args(sql)?;
        if args.is_empty() {
            return Err(DbError::SyntaxError("Redis 命令不能为空".to_string()));
        }

        let command = args[0].to_ascii_uppercase();
        let started_at = Instant::now();
        let mut connection = self.connection().await?;

        let rows_affected = match command.as_str() {
            "SET" => {
                let key = required_arg(&args, 1, "key")?;
                let value = required_arg(&args, 2, "value")?;
                let mut cmd = redis::cmd("SET");
                cmd.arg(key).arg(value);
                if let Some(flag) = args.get(3) {
                    if flag.eq_ignore_ascii_case("EX") {
                        let ttl = required_arg(&args, 4, "ttl")?.parse::<u64>().map_err(|_| {
                            DbError::SyntaxError("SET EX 的 ttl 必须是正整数".to_string())
                        })?;
                        cmd.arg("EX").arg(ttl);
                    } else {
                        return Err(DbError::SyntaxError(
                            "SET 仅支持可选参数 EX <seconds>".to_string(),
                        ));
                    }
                }
                cmd.query_async::<String>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                1
            }
            "DEL" => {
                if args.len() < 2 {
                    return Err(DbError::SyntaxError("DEL 至少需要一个 key".to_string()));
                }
                redis::cmd("DEL")
                    .arg(&args[1..])
                    .query_async::<u64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
            }
            "EXPIRE" => {
                let key = required_arg(&args, 1, "key")?;
                let ttl = required_arg(&args, 2, "seconds")?
                    .parse::<u64>()
                    .map_err(|_| DbError::SyntaxError("EXPIRE seconds 必须是正整数".to_string()))?;
                let updated = redis::cmd("EXPIRE")
                    .arg(key)
                    .arg(ttl)
                    .query_async::<bool>(&mut connection)
                    .await
                    .map_err(map_query_error)?;
                if updated { 1 } else { 0 }
            }
            "HSET" => {
                let key = required_arg(&args, 1, "key")?;
                let field = required_arg(&args, 2, "field")?;
                let value = required_arg(&args, 3, "value")?;
                redis::cmd("HSET")
                    .arg(key)
                    .arg(field)
                    .arg(value)
                    .query_async::<u64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
            }
            "HDEL" => {
                let key = required_arg(&args, 1, "key")?;
                if args.len() < 3 {
                    return Err(DbError::SyntaxError(
                        "HDEL 至少需要一个 field".to_string(),
                    ));
                }
                redis::cmd("HDEL")
                    .arg(key)
                    .arg(&args[2..])
                    .query_async::<u64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
            }
            "LPUSH" => {
                let key = required_arg(&args, 1, "key")?;
                if args.len() < 3 {
                    return Err(DbError::SyntaxError(
                        "LPUSH 至少需要一个 value".to_string(),
                    ));
                }
                redis::cmd("LPUSH")
                    .arg(key)
                    .arg(&args[2..])
                    .query_async::<u64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
            }
            "RPUSH" => {
                let key = required_arg(&args, 1, "key")?;
                if args.len() < 3 {
                    return Err(DbError::SyntaxError(
                        "RPUSH 至少需要一个 value".to_string(),
                    ));
                }
                redis::cmd("RPUSH")
                    .arg(key)
                    .arg(&args[2..])
                    .query_async::<u64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
            }
            "SADD" => {
                let key = required_arg(&args, 1, "key")?;
                if args.len() < 3 {
                    return Err(DbError::SyntaxError(
                        "SADD 至少需要一个 member".to_string(),
                    ));
                }
                redis::cmd("SADD")
                    .arg(key)
                    .arg(&args[2..])
                    .query_async::<u64>(&mut connection)
                    .await
                    .map_err(map_query_error)?
            }
            _ => {
                return Err(DbError::Unsupported(format!(
                    "不支持的 Redis 执行命令 `{command}`。已支持: SET/DEL/EXPIRE/HSET/HDEL/LPUSH/RPUSH/SADD"
                )));
            }
        };

        Ok(ExecResult {
            rows_affected,
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

    async fn explain(&self, _sql: &str) -> DbResult<String> {
        Err(DbError::Unsupported(
            "Redis 不支持 EXPLAIN".to_string(),
        ))
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let tables = self
            .scan_keys("*", 100, 200)
            .await?
            .into_iter()
            .map(|name| TableInfo {
                name,
                table_type: TableType::Table,
                row_estimate: None,
                size_bytes: None,
            })
            .collect::<Vec<_>>();

        Ok(vec![DatabaseInfo {
            name: format!("db{}", self.database_index),
            tables,
        }])
    }

    async fn list_tables(&self, _database: &str) -> DbResult<Vec<String>> {
        self.scan_keys("*", 100, 200).await
    }

    async fn describe_table(&self, _database: &str, table: &str) -> DbResult<TableSchema> {
        let mut connection = self.connection().await?;
        let key_type = redis::cmd("TYPE")
            .arg(table)
            .query_async::<String>(&mut connection)
            .await
            .map_err(map_query_error)?;
        let ttl = redis::cmd("TTL")
            .arg(table)
            .query_async::<i64>(&mut connection)
            .await
            .map_err(map_query_error)?;

        let length = match key_type.as_str() {
            "string" => redis::cmd("STRLEN")
                .arg(table)
                .query_async::<i64>(&mut connection)
                .await
                .ok(),
            "list" => redis::cmd("LLEN")
                .arg(table)
                .query_async::<i64>(&mut connection)
                .await
                .ok(),
            "hash" => redis::cmd("HLEN")
                .arg(table)
                .query_async::<i64>(&mut connection)
                .await
                .ok(),
            "set" => redis::cmd("SCARD")
                .arg(table)
                .query_async::<i64>(&mut connection)
                .await
                .ok(),
            "zset" => redis::cmd("ZCARD")
                .arg(table)
                .query_async::<i64>(&mut connection)
                .await
                .ok(),
            "stream" => redis::cmd("XLEN")
                .arg(table)
                .query_async::<i64>(&mut connection)
                .await
                .ok(),
            _ => None,
        };

        Ok(TableSchema {
            schema: Some(format!("db{}", self.database_index)),
            name: table.to_string(),
            columns: vec![
                ColumnSchema {
                    name: "key".to_string(),
                    data_type: "string".to_string(),
                    nullable: false,
                    default_value: None,
                    is_primary_key: true,
                    is_unique: true,
                    comment: None,
                    char_max_length: None,
                },
                ColumnSchema {
                    name: "type".to_string(),
                    data_type: "string".to_string(),
                    nullable: false,
                    default_value: Some(key_type),
                    is_primary_key: false,
                    is_unique: false,
                    comment: None,
                    char_max_length: None,
                },
                ColumnSchema {
                    name: "ttl_seconds".to_string(),
                    data_type: "int".to_string(),
                    nullable: true,
                    default_value: Some(ttl.to_string()),
                    is_primary_key: false,
                    is_unique: false,
                    comment: None,
                    char_max_length: None,
                },
                ColumnSchema {
                    name: "length".to_string(),
                    data_type: "int".to_string(),
                    nullable: true,
                    default_value: length.map(|value| value.to_string()),
                    is_primary_key: false,
                    is_unique: false,
                    comment: None,
                    char_max_length: None,
                },
            ],
            indexes: vec![],
            primary_keys: vec!["key".to_string()],
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

impl RedisDriver {
    async fn connection(&self) -> DbResult<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(map_query_error)
    }

    async fn scan_keys(&self, pattern: &str, count: u32, max_items: usize) -> DbResult<Vec<String>> {
        let mut connection = self.connection().await?;
        let mut cursor: u64 = 0;
        let mut keys = Vec::new();

        loop {
            let mut cmd = redis::cmd("SCAN");
            cmd.arg(cursor).arg("MATCH").arg(pattern).arg("COUNT").arg(count);
            let (next_cursor, chunk): (u64, Vec<String>) =
                cmd.query_async(&mut connection).await.map_err(map_query_error)?;

            keys.extend(chunk);
            cursor = next_cursor;

            if cursor == 0 || keys.len() >= max_items {
                break;
            }
        }

        keys.sort();
        keys.dedup();
        if keys.len() > max_items {
            keys.truncate(max_items);
        }
        Ok(keys)
    }
}

fn parse_command_args(input: &str) -> DbResult<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.trim().chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Some(current_quote) => {
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == current_quote {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if ch.is_whitespace() {
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if quote.is_some() {
        return Err(DbError::SyntaxError("命令存在未闭合的引号".to_string()));
    }

    if !current.is_empty() {
        args.push(current);
    }

    Ok(args)
}

fn required_arg<'a>(args: &'a [String], index: usize, name: &str) -> DbResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| DbError::SyntaxError(format!("缺少参数 `{name}`")))
}

fn single_row_query_result(
    columns: Vec<ColumnMeta>,
    values: Vec<(&str, DbValue)>,
    elapsed_ms: u64,
) -> QueryResult {
    let mut row = DbRow::new();
    for (key, value) in values {
        row.insert(key.to_string(), value);
    }
    QueryResult::new(columns, vec![row], elapsed_ms)
}

fn list_query_result(columns: Vec<ColumnMeta>, rows: Vec<DbRow>, elapsed_ms: u64) -> QueryResult {
    QueryResult::new(columns, rows, elapsed_ms)
}

fn column(name: &str, data_type: &str, nullable: bool) -> ColumnMeta {
    ColumnMeta {
        name: name.to_string(),
        data_type: data_type.to_string(),
        nullable,
    }
}

fn parse_database_index(config: &ConnectionConfig) -> i64 {
    config
        .database
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .unwrap_or(0)
}

fn redis_connection_url(config: &ConnectionConfig, database_index: i64) -> String {
    let host = if config.host.trim().is_empty() {
        "127.0.0.1"
    } else {
        config.host.trim()
    };
    let port = if config.port == 0 { 6379 } else { config.port };

    if config.username.trim().is_empty() {
        if config.password.is_empty() {
            format!("redis://{host}:{port}/{database_index}")
        } else {
            format!("redis://:{}@{host}:{port}/{database_index}", config.password)
        }
    } else if config.password.is_empty() {
        format!(
            "redis://{}@{host}:{port}/{database_index}",
            config.username.trim()
        )
    } else {
        format!(
            "redis://{}:{}@{host}:{port}/{database_index}",
            config.username.trim(),
            config.password
        )
    }
}

fn ensure_no_params(params: &[Value]) -> DbResult<()> {
    if params.is_empty() {
        Ok(())
    } else {
        Err(DbError::Unsupported(
            "当前版本暂不支持带参数的 Redis 命令".to_string(),
        ))
    }
}

fn map_connect_error(error: redis::RedisError) -> DbError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") {
        DbError::ConnectionTimeout
    } else if lower.contains("noauth") || lower.contains("wrongpass") {
        DbError::AuthFailed
    } else {
        DbError::ConnectionFailed(message)
    }
}

fn map_query_error(error: redis::RedisError) -> DbError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("syntax error") {
        DbError::SyntaxError(message)
    } else {
        DbError::QueryFailed(message)
    }
}
