use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// 核心值类型

/// 数据库中一个单元格的值，涵盖所有驱动的公共类型。
/// 驱动负责把原生类型映射到这个 enum。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum DbValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Timestamp(DateTime<Utc>),
    Json(serde_json::Value),
    Array(Vec<DbValue>),
}

impl DbValue {
    pub fn is_null(&self) -> bool {
        matches\!(self, DbValue::Null)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            DbValue::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DbValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DbValue::Float(f) => Some(*f),
            DbValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

impl From<bool> for DbValue {
    fn from(v: bool) -> Self { DbValue::Bool(v) }
}
impl From<i32> for DbValue {
    fn from(v: i32) -> Self { DbValue::Int(v as i64) }
}
impl From<i64> for DbValue {
    fn from(v: i64) -> Self { DbValue::Int(v) }
}
impl From<f64> for DbValue {
    fn from(v: f64) -> Self { DbValue::Float(v) }
}
impl From<String> for DbValue {
    fn from(v: String) -> Self { DbValue::Text(v) }
}
impl From<&str> for DbValue {
    fn from(v: &str) -> Self { DbValue::Text(v.to_string()) }
}
impl From<serde_json::Value> for DbValue {
    fn from(v: serde_json::Value) -> Self { DbValue::Json(v) }
}
impl<T: Into<DbValue>> From<Option<T>> for DbValue {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => DbValue::Null,
        }
    }
}

/// 将 DbValue 转换为 serde_json::Value，消除 tagged enum 序列化开销
pub fn db_value_to_json(val: &DbValue) -> serde_json::Value {
    match val {
        DbValue::Null => serde_json::Value::Null,
        DbValue::Bool(b) => serde_json::json\!(*b),
        DbValue::Int(i) => serde_json::json\!(*i),
        DbValue::Float(f) => serde_json::json\!(*f),
        DbValue::Text(s) => serde_json::Value::String(s.clone()),
        DbValue::Bytes(b) => {
            use base64::Engine as _;
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        DbValue::Timestamp(ts) => serde_json::Value::String(ts.to_rfc3339()),
        DbValue::Json(v) => v.clone(),
        DbValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(db_value_to_json).collect())
        }
    }
}

// 查询结果

/// 一行数据：列名 -> 值 的有序映射
pub type Row = HashMap<String, DbValue>;

/// SELECT 查询的完整结果集
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    /// 列定义（保留顺序）
    pub columns: Vec<ColumnMeta>,
    /// 数据行
    pub rows: Vec<Row>,
    /// 实际返回行数
    pub row_count: usize,
    /// 查询耗时（毫秒）
    pub elapsed_ms: u64,
}

impl QueryResult {
    pub fn new(columns: Vec<ColumnMeta>, rows: Vec<Row>, elapsed_ms: u64) -> Self {
        let row_count = rows.len();
        Self { columns, rows, row_count, elapsed_ms }
    }

    pub fn empty() -> Self {
        Self { columns: vec\![], rows: vec\![], row_count: 0, elapsed_ms: 0 }
    }

    /// 转换为前端友好格式：DbValue -> serde_json::Value
    pub fn to_json_rows(&self) -> Vec<HashMap<String, serde_json::Value>> {
        self.rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|(k, v)| (k.clone(), db_value_to_json(v)))
                    .collect()
            })
            .collect()
    }
}

/// INSERT / UPDATE / DELETE 的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    /// 影响的行数
    pub rows_affected: u64,
    /// 最后插入的自增 ID（如有）
    pub last_insert_id: Option<i64>,
    /// 查询耗时（毫秒）
    pub elapsed_ms: u64,
}

// Schema 元数据

/// 列的元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

/// 表的完整 Schema 描述
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub schema: Option<String>,
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub indexes: Vec<IndexSchema>,
    pub primary_keys: Vec<String>,
}

/// 一列的详细定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
    pub is_unique: bool,
    pub comment: Option<String>,
    pub char_max_length: Option<u32>,
}

/// 索引信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSchema {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
}

/// 数据库/Schema 的简要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub name: String,
    pub tables: Vec<TableInfo>,
}

/// 表的简要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub table_type: TableType,
    pub row_estimate: Option<u64>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableType {
    Table,
    View,
    MaterializedView,
}

// 连接配置

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub driver: DriverKind,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl: bool,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriverKind {
    Postgres,
    Mysql,
    Sqlite,
    Mongo,
    Redis,
}

impl ConnectionConfig {
    pub fn to_url(&self) -> String {
        match self.driver {
            DriverKind::Postgres => format\!(
                "postgres://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DriverKind::Mysql => format\!(
                "mysql://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DriverKind::Sqlite => self.database.clone(),
            DriverKind::Mongo => format\!(
                "mongodb://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DriverKind::Redis => format\!(
                "redis://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
        }
    }
}
