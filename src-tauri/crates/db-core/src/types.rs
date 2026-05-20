use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
// 核心值类型
// ══════════════════════════════════════════════════════════════

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
        matches!(self, DbValue::Null)
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

// 常见类型的 From 转换，方便驱动层构造 DbValue
impl From<bool> for DbValue {
    fn from(v: bool) -> Self {
        DbValue::Bool(v)
    }
}
impl From<i32> for DbValue {
    fn from(v: i32) -> Self {
        DbValue::Int(v as i64)
    }
}
impl From<i64> for DbValue {
    fn from(v: i64) -> Self {
        DbValue::Int(v)
    }
}
impl From<f64> for DbValue {
    fn from(v: f64) -> Self {
        DbValue::Float(v)
    }
}
impl From<String> for DbValue {
    fn from(v: String) -> Self {
        DbValue::Text(v)
    }
}
impl From<&str> for DbValue {
    fn from(v: &str) -> Self {
        DbValue::Text(v.to_string())
    }
}
impl From<serde_json::Value> for DbValue {
    fn from(v: serde_json::Value) -> Self {
        DbValue::Json(v)
    }
}
impl<T: Into<DbValue>> From<Option<T>> for DbValue {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => DbValue::Null,
        }
    }
}

// ══════════════════════════════════════════════════════════════
// 查询结果
// ══════════════════════════════════════════════════════════════

/// 一行数据：列名 → 值 的有序映射
pub type Row = HashMap<String, DbValue>;

/// SELECT 查询的完整结果集
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Self {
            columns,
            rows,
            row_count,
            elapsed_ms,
        }
    }

    pub fn empty() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            elapsed_ms: 0,
        }
    }
}

/// INSERT / UPDATE / DELETE 的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    /// 影响的行数
    pub rows_affected: u64,
    /// 最后插入的自增 ID（如有）
    pub last_insert_id: Option<i64>,
    /// 查询耗时（毫秒）
    pub elapsed_ms: u64,
}

// ══════════════════════════════════════════════════════════════
// Schema 元数据
// ══════════════════════════════════════════════════════════════

/// 列的元数据（用于结果集和 Schema 浏览）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub name: String,
    /// 数据库原生类型名，如 "varchar", "int4", "timestamptz"
    pub data_type: String,
    pub nullable: bool,
}

/// 表的完整 Schema 描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub schema: Option<String>, // PostgreSQL schema / MySQL database
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub indexes: Vec<IndexSchema>,
    pub primary_keys: Vec<String>,
}

/// 一列的详细定义
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 数据库/Schema 的简要信息（用于左侧树形列表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub tables: Vec<TableInfo>,
}

/// 表的简要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ══════════════════════════════════════════════════════════════
// 连接配置
// ══════════════════════════════════════════════════════════════

/// 通用连接参数，前端填表单后传过来
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,   // UUID，前端生成
    pub name: String, // 用户给这个连接起的名字
    pub driver: DriverKind,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String, // 实际存储时应加密
    pub ssl: bool,
    pub options: HashMap<String, String>, // 驱动特有的额外参数
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
    /// 构建标准 DSN，各驱动可以用也可以自己拼
    pub fn to_url(&self) -> String {
        match self.driver {
            DriverKind::Postgres => format!(
                "postgres://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DriverKind::Mysql => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DriverKind::Sqlite => self.database.clone(),
            DriverKind::Mongo => format!(
                "mongodb://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DriverKind::Redis => format!(
                "redis://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
        }
    }
}
