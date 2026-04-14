use async_trait::async_trait;
use crate::{
    error::DbResult,
    types::{
        ColumnMeta, ConnectionConfig, DatabaseInfo,
        ExecResult, QueryResult, TableSchema,
    },
};

// ══════════════════════════════════════════════════════════════
// 关系型数据库 trait
// ══════════════════════════════════════════════════════════════

/// 所有关系型数据库驱动必须实现这个 trait。
/// PostgreSQL / MySQL / SQLite 的实现 crate 都依赖这个接口。
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    // ── 连接 ──────────────────────────────────────────────────

    /// 从配置创建驱动实例并建立连接
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized;

    /// 检查连接是否存活（用于心跳 / 连接池健康检查）
    async fn ping(&self) -> DbResult<()>;

    /// 释放连接（通常 Drop 会处理，但提供显式关闭接口）
    async fn close(&self) -> DbResult<()>;

    // ── 查询 ──────────────────────────────────────────────────

    /// 执行 SELECT，返回完整结果集
    /// params 是位置参数，如 $1 $2（pg）或 ? ?（mysql/sqlite）
    async fn query(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> DbResult<QueryResult>;

    /// 执行 INSERT / UPDATE / DELETE / DDL，返回影响行数
    async fn execute(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> DbResult<ExecResult>;

    /// 在一个事务里批量执行多条语句（全部成功才提交）
    async fn execute_batch(&self, statements: Vec<String>) -> DbResult<Vec<ExecResult>>;

    /// EXPLAIN / EXPLAIN ANALYZE，返回原始文本（各数据库格式不同）
    async fn explain(&self, sql: &str) -> DbResult<String>;

    // ── Schema 元数据 ─────────────────────────────────────────

    /// 列出所有可访问的数据库
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>>;

    /// 列出某个数据库内的所有表（含视图）
    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>>;

    /// 获取表的完整 Schema（列、索引、主键）
    async fn describe_table(
        &self,
        database: &str,
        table: &str,
    ) -> DbResult<TableSchema>;

    /// 快速获取列元数据（比 describe_table 轻量，用于结果集表头）
    async fn get_columns(
        &self,
        database: &str,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>>;
}

// ══════════════════════════════════════════════════════════════
// 文档型数据库 trait（MongoDB）
// ══════════════════════════════════════════════════════════════

#[async_trait]
pub trait DocumentDriver: Send + Sync {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized;

    async fn ping(&self) -> DbResult<()>;
    async fn close(&self) -> DbResult<()>;

    // ── 集合操作 ──────────────────────────────────────────────

    async fn list_databases(&self) -> DbResult<Vec<String>>;
    async fn list_collections(&self, db: &str) -> DbResult<Vec<String>>;

    /// find：filter 是 JSON 对象，options 支持 limit / skip / sort
    async fn find(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
        options: FindOptions,
    ) -> DbResult<Vec<serde_json::Value>>;

    /// 插入单条文档，返回插入后的 _id
    async fn insert_one(
        &self,
        db: &str,
        collection: &str,
        doc: serde_json::Value,
    ) -> DbResult<String>;

    /// 插入多条文档，返回插入数量
    async fn insert_many(
        &self,
        db: &str,
        collection: &str,
        docs: Vec<serde_json::Value>,
    ) -> DbResult<u64>;

    async fn update_one(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
        update: serde_json::Value,
    ) -> DbResult<u64>;

    async fn delete_many(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
    ) -> DbResult<u64>;

    /// 聚合管道，pipeline 是 JSON 数组
    async fn aggregate(
        &self,
        db: &str,
        collection: &str,
        pipeline: Vec<serde_json::Value>,
    ) -> DbResult<Vec<serde_json::Value>>;

    async fn count(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
    ) -> DbResult<u64>;
}

/// find() 的可选参数
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FindOptions {
    pub limit: Option<u32>,
    pub skip: Option<u32>,
    /// sort: { "field": 1 } 或 { "field": -1 }
    pub sort: Option<serde_json::Value>,
    /// projection: { "field": 1 }
    pub projection: Option<serde_json::Value>,
}

// ══════════════════════════════════════════════════════════════
// 缓存型数据库 trait（Redis）
// ══════════════════════════════════════════════════════════════

#[async_trait]
pub trait CacheDriver: Send + Sync {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized;

    async fn ping(&self) -> DbResult<()>;
    async fn close(&self) -> DbResult<()>;

    // ── 基本 KV ───────────────────────────────────────────────

    async fn get(&self, key: &str) -> DbResult<Option<String>>;
    async fn set(&self, key: &str, value: &str, ttl_secs: Option<u64>) -> DbResult<()>;
    async fn del(&self, keys: &[String]) -> DbResult<u64>;
    async fn exists(&self, key: &str) -> DbResult<bool>;
    async fn ttl(&self, key: &str) -> DbResult<i64>;  // -1=无过期 -2=不存在
    async fn expire(&self, key: &str, secs: u64) -> DbResult<bool>;

    // ── 浏览 ──────────────────────────────────────────────────

    /// SCAN 遍历（不阻塞），pattern 如 "user:*"
    async fn scan(&self, pattern: &str, count: u32) -> DbResult<Vec<String>>;
    async fn key_type(&self, key: &str) -> DbResult<RedisKeyType>;

    // ── Hash ──────────────────────────────────────────────────

    async fn hget(&self, key: &str, field: &str) -> DbResult<Option<String>>;
    async fn hset(&self, key: &str, field: &str, value: &str) -> DbResult<()>;
    async fn hgetall(&self, key: &str) -> DbResult<std::collections::HashMap<String, String>>;
    async fn hdel(&self, key: &str, fields: &[String]) -> DbResult<u64>;

    // ── List ──────────────────────────────────────────────────

    async fn lrange(&self, key: &str, start: i64, stop: i64) -> DbResult<Vec<String>>;
    async fn lpush(&self, key: &str, values: &[String]) -> DbResult<u64>;
    async fn rpush(&self, key: &str, values: &[String]) -> DbResult<u64>;

    // ── Set ───────────────────────────────────────────────────

    async fn smembers(&self, key: &str) -> DbResult<Vec<String>>;
    async fn sadd(&self, key: &str, members: &[String]) -> DbResult<u64>;

    // ── ZSet ──────────────────────────────────────────────────

    async fn zrange(&self, key: &str, start: i64, stop: i64) -> DbResult<Vec<(String, f64)>>;

    // ── 服务器信息 ────────────────────────────────────────────

    /// INFO 命令原始输出
    async fn server_info(&self) -> DbResult<String>;

    /// 执行任意原始命令（高级调试用）
    async fn raw_command(&self, args: Vec<String>) -> DbResult<serde_json::Value>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RedisKeyType {
    String,
    Hash,
    List,
    Set,
    ZSet,
    Stream,
    Unknown,
}

use serde::{Deserialize, Serialize};