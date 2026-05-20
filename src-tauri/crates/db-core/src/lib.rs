//! db-core — 数据库管理工具的核心抽象层
//!
//! 这个 crate 只定义 trait、类型和错误，不包含任何具体数据库实现。
//! 所有驱动 crate（db-postgres、db-mysql、db-mongo…）都依赖此 crate。
//!
//! # 模块结构
//!
//! - [`error`] — 统一错误类型 [`DbError`] 和 [`DbResult`]
//! - [`types`] — 核心数据类型：[`DbValue`]、[`QueryResult`]、[`TableSchema`] 等
//! - [`traits`] — 三个驱动 trait：[`DatabaseDriver`]、[`DocumentDriver`]、[`CacheDriver`]

pub mod error;
pub mod traits;
pub mod types;

// 把最常用的类型提升到 crate 根，驱动 crate 只需 use db_core::*
pub use error::{DbError, DbResult};
pub use traits::{
    CacheDriver, DatabaseDriver, DocumentDriver, FindOptions, RedisKeyType,
};
pub use types::{
    ColumnMeta, ColumnSchema, ConnectionConfig, DatabaseInfo, DbValue,
    db_value_to_json,
    DriverKind, ExecResult, IndexSchema, QueryResult, Row, TableInfo,
    TableSchema, TableType,
};
