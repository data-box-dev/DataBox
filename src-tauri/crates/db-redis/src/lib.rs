use async_trait::async_trait;
use redis::AsyncConnection;
use std::collections::HashMap;

use db_core::{
    traits::CacheDriver,
    types::*,
    DbError,
    DbResult,
};

/// Redis 驱动实现
///
/// 连接字符串格式: `redis://{user}:{password}@{host}:{port}/{database}`
pub struct RedisDriver {
    /// 复用客户端创建新连接
    client: redis::Client,
    /// 选中的数据库编号
    database: u8,
}

#[async_trait]
impl CacheDriver for RedisDriver {
    /// 从连接配置建立 Redis 连接
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
        let url = config.to_url();
        let client = redis::Client::open(url.as_str())
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // 测试连接
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // 解析数据库编号（从配置中提取，默认 0）
        let database = config
            .options
            .get("database")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        // 执行 SELECT 切换数据库
        if database > 0 {
            redis::cmd("SELECT")
                .arg(database)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        }

        Ok(Self { client, database })
    }

    /// 检查连接是否存活
    async fn ping(&self) -> DbResult<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        Ok(())
    }

    /// 关闭连接（Redis 连接池由客户端管理）
    async fn close(&self) -> DbResult<()> {
        Ok(())
    }

    // ── 基本 KV ───────────────────────────────────────────────

    /// GET key
    async fn get(&self, key: &str) -> DbResult<Option<String>> {
        let mut conn = self.get_conn().await?;
        let result: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// SET key value [EX seconds]
    async fn set(&self, key: &str, value: &str, ttl_secs: Option<u64>) -> DbResult<()> {
        let mut conn = self.get_conn().await?;

        if let Some(ttl) = ttl_secs {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .arg("EX")
                .arg(ttl as i64)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// DEL key [key ...]
    async fn del(&self, keys: &[String]) -> DbResult<u64> {
        let mut conn = self.get_conn().await?;
        let mut cmd = redis::cmd("DEL");
        for key in keys {
            cmd.arg(key);
        }
        let deleted: u64 = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(deleted)
    }

    /// EXISTS key
    async fn exists(&self, key: &str) -> DbResult<bool> {
        let mut conn = self.get_conn().await?;
        let result: bool = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// TTL key（-1=无过期, -2=不存在）
    async fn ttl(&self, key: &str) -> DbResult<i64> {
        let mut conn = self.get_conn().await?;
        let result: i64 = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// EXPIRE key seconds
    async fn expire(&self, key: &str, secs: u64) -> DbResult<bool> {
        let mut conn = self.get_conn().await?;
        let result: bool = redis::cmd("EXPIRE")
            .arg(key)
            .arg(secs as i64)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    // ── 浏览 ──────────────────────────────────────────────────

    /// SCAN cursor MATCH pattern COUNT count（返回一批 key）
    async fn scan(&self, pattern: &str, count: u32) -> DbResult<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let keys: Vec<String> = redis::cmd("SCAN")
            .arg(0_u64) // cursor start at 0
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(keys)
    }

    /// TYPE key
    async fn key_type(&self, key: &str) -> DbResult<RedisKeyType> {
        let mut conn = self.get_conn().await?;
        let type_str: String = redis::cmd("TYPE")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        match type_str.as_str() {
            "string" => Ok(RedisKeyType::String),
            "hash" => Ok(RedisKeyType::Hash),
            "list" => Ok(RedisKeyType::List),
            "set" => Ok(RedisKeyType::Set),
            "zset" => Ok(RedisKeyType::ZSet),
            "stream" => Ok(RedisKeyType::Stream),
            "none" | "" => Ok(RedisKeyType::Unknown),
            _ => Ok(RedisKeyType::Unknown),
        }
    }

    // ── Hash ──────────────────────────────────────────────────

    /// HGET key field
    async fn hget(&self, key: &str, field: &str) -> DbResult<Option<String>> {
        let mut conn = self.get_conn().await?;
        let result: Option<String> = redis::cmd("HGET")
            .arg(key)
            .arg(field)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// HSET key field value
    async fn hset(&self, key: &str, field: &str, value: &str) -> DbResult<()> {
        let mut conn = self.get_conn().await?;
        redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(value)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    /// HGETALL key
    async fn hgetall(&self, key: &str) -> DbResult<HashMap<String, String>> {
        let mut conn = self.get_conn().await?;
        let result: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// HDEL key field [field ...]
    async fn hdel(&self, key: &str, fields: &[String]) -> DbResult<u64> {
        let mut conn = self.get_conn().await?;
        let mut cmd = redis::cmd("HDEL");
        cmd.arg(key);
        for field in fields {
            cmd.arg(field);
        }
        let deleted: u64 = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(deleted)
    }

    // ── List ──────────────────────────────────────────────────

    /// LRANGE key start stop
    async fn lrange(&self, key: &str, start: i64, stop: i64) -> DbResult<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let result: Vec<String> = redis::cmd("LRANGE")
            .arg(key)
            .arg(start)
            .arg(stop)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// LPUSH key value [value ...]
    async fn lpush(&self, key: &str, values: &[String]) -> DbResult<u64> {
        let mut conn = self.get_conn().await?;
        let mut cmd = redis::cmd("LPUSH");
        cmd.arg(key);
        for v in values {
            cmd.arg(v);
        }
        let len: u64 = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(len)
    }

    /// RPUSH key value [value ...]
    async fn rpush(&self, key: &str, values: &[String]) -> DbResult<u64> {
        let mut conn = self.get_conn().await?;
        let mut cmd = redis::cmd("RPUSH");
        cmd.arg(key);
        for v in values {
            cmd.arg(v);
        }
        let len: u64 = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(len)
    }

    // ── Set ───────────────────────────────────────────────────

    /// SMEMBERS key
    async fn smembers(&self, key: &str) -> DbResult<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let result: Vec<String> = redis::cmd("SMEMBERS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result)
    }

    /// SADD key member [member ...]
    async fn sadd(&self, key: &str, members: &[String]) -> DbResult<u64> {
        let mut conn = self.get_conn().await?;
        let mut cmd = redis::cmd("SADD");
        cmd.arg(key);
        for m in members {
            cmd.arg(m);
        }
        let added: u64 = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(added)
    }

    // ── ZSet ──────────────────────────────────────────────────

    /// ZRANGE key start stop [WITHSCORES]
    async fn zrange(&self, key: &str, start: i64, stop: i64) -> DbResult<Vec<(String, f64)>> {
        let mut conn = self.get_conn().await?;

        // 使用 WITHSCORES 获取分数
        let raw: Vec<redis::Value> = redis::cmd("ZRANGE")
            .arg(key)
            .arg(start)
            .arg(stop)
            .arg("WITHSCORES")
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        // redis::Value 配对转成 (String, f64)
        let mut result = Vec::new();
        for chunk in raw.chunks(2) {
            if let (Some(redis::Value::Data(member)), Some(redis::Value::Data(score_bytes))) =
                (chunk.get(0), chunk.get(1))
            {
                let member_str = String::from_utf8_lossy(member).to_string();
                let score_str = String::from_utf8_lossy(score_bytes);
                if let Ok(score) = score_str.parse::<f64>() {
                    result.push((member_str, score));
                }
            }
        }
        Ok(result)
    }

    // ── 服务器信息 ────────────────────────────────────────────

    /// INFO 命令原始输出
    async fn server_info(&self) -> DbResult<String> {
        let mut conn = self.get_conn().await?;
        let info: String = redis::cmd("INFO")
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(info)
    }

    /// 执行任意原始命令（高级调试用）
    async fn raw_command(&self, args: Vec<String>) -> DbResult<serde_json::Value> {
        let mut conn = self.get_conn().await?;
        let mut cmd = redis::cmd(&args[0]);
        for arg in args.iter().skip(1) {
            cmd.arg(arg);
        }
        let raw: redis::Value = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(redis_value_to_json(&raw))
    }
}

impl RedisDriver {
    /// 获取一个新连接（每次操作创建新连接，桌面工具操作频率低可以接受）
    async fn get_conn(&self) -> DbResult<redis::AsyncConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))
    }
}

/// 将 redis-rs 的 Value 转换为 serde_json::Value
fn redis_value_to_json(val: &redis::Value) -> serde_json::Value {
    match val {
        redis::Value::Data(bytes) => {
            match std::str::from_utf8(bytes) {
                Ok(s) => serde_json::Value::String(s.to_string()),
                Err(_) => serde_json::json!({ "_raw": format!("{:?}", bytes) }),
            }
        }
        redis::Value::Bulk(vec) => {
            let arr: Vec<serde_json::Value> =
                vec.iter().map(redis_value_to_json).collect();
            serde_json::Value::Array(arr)
        }
        redis::Value::Status(s) => serde_json::Value::String(s.clone()),
        redis::Value::Okay => serde_json::Value::String("OK".to_string()),
        redis::Value::Int(i) => serde_json::json!(*i),
        redis::Value::Double(f) => serde_json::json!(*f),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_value_to_json_data() {
        let val = redis_value_to_json(&redis::Value::Data(b"hello".to_vec()));
        assert_eq!(val, serde_json::json!("hello"));
    }

    #[test]
    fn test_redis_value_to_json_int() {
        let val = redis_value_to_json(&redis::Value::Int(42));
        assert_eq!(val, serde_json::json!(42));
    }

    #[test]
    fn test_redis_value_to_json_okay() {
        let val = redis_value_to_json(&redis::Value::Okay);
        assert_eq!(val, serde_json::json!("OK"));
    }

    #[test]
    fn test_redis_value_to_json_bulk() {
        let bulk = redis::Value::Bulk(vec![
            redis::Value::Data(b"key1".to_vec()),
            redis::Value::Data(b"key2".to_vec()),
        ]);
        let val = redis_value_to_json(&bulk);
        assert_eq!(val, serde_json::json!(["key1", "key2"]));
    }

    #[test]
    fn test_redis_value_to_json_double() {
        let val = redis_value_to_json(&redis::Value::Double(3.14));
        assert_eq!(val, serde_json::json!(3.14));
    }

    #[test]
    fn test_key_type_serialization() {
        use serde_json;
        assert_eq!(
            serde_json::to_string(&RedisKeyType::String).unwrap(),
            "\"string\""
        );
        assert_eq!(
            serde_json::to_string(&RedisKeyType::Hash).unwrap(),
            "\"hash\""
        );
        assert_eq!(
            serde_json::to_string(&RedisKeyType::List).unwrap(),
            "\"list\""
        );
        assert_eq!(
            serde_json::to_string(&RedisKeyType::Set).unwrap(),
            "\"set\""
        );
        assert_eq!(
            serde_json::to_string(&RedisKeyType::ZSet).unwrap(),
            "\"z_set\""
        );
        assert_eq!(
            serde_json::to_string(&RedisKeyType::Stream).unwrap(),
            "\"stream\""
        );
        assert_eq!(
            serde_json::to_string(&RedisKeyType::Unknown).unwrap(),
            "\"unknown\""
        );
    }
}
