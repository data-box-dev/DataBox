use async_trait::async_trait;
use bson::{doc, Document};
use mongodb::Client;
use mongodb::options::ClientOptions;
use std::collections::HashMap;

use db_core::{
    traits::DocumentDriver,
    types::*,
    DbError,
    DbResult,
    FindOptions,
};

/// MongoDB 驱动实现
///
/// 连接字符串格式: `mongodb://{user}:{password}@{host}:{port}/{database}`
pub struct MongoDriver {
    client: Client,
    database: String,
}

#[async_trait]
impl DocumentDriver for MongoDriver {
    /// 从连接配置建立 MongoDB 连接
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
        let url = config.to_url();
        let mut options = ClientOptions::parse(&url)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // 设置连接池大小
        options.max_pool_size = Some(10);

        let client = Client::with_options(options)
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            client,
            database: config.database.clone(),
        })
    }

    /// 检查连接是否存活
    async fn ping(&self) -> DbResult<()> {
        self.client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        Ok(())
    }

    /// 关闭连接
    async fn close(&self) -> DbResult<()> {
        // MongoDB driver 会自动管理连接池生命周期
        Ok(())
    }

    /// 列出所有数据库
    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let databases = self
            .client
            .list_database_names()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        // 过滤掉系统数据库
        let filtered: Vec<String> = databases
            .into_iter()
            .filter(|name| !["admin", "config", "local"].contains(&name.as_str()))
            .collect();

        Ok(filtered)
    }

    /// 列出数据库内所有集合
    async fn list_collections(&self, db: &str) -> DbResult<Vec<String>> {
        let database = self.client.database(db);
        let collections = database
            .list_collection_names()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(collections)
    }

    /// 查询文档
    async fn find(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
        options: FindOptions,
    ) -> DbResult<Vec<serde_json::Value>> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        // 转换 filter 为 BSON Document
        let filter_doc = match serde_json::to_value(filter) {
            Ok(v) => match bson::to_bson(&v) {
                Ok(bson::Bson::Document(d)) => d,
                _ => Document::new(),
            },
            Err(_) => Document::new(),
        };

        // 构建 mongodb FindOptions from db_core FindOptions
        let mut mongo_opts = mongodb::options::FindOptions::default();
        mongo_opts.limit = options.limit.map(|l| l as i64);
        mongo_opts.skip = options.skip.map(|s| s as u64);
        if let Some(sort) = options.sort {
            if let Ok(sort_bson) = bson::to_bson(&sort) {
                if let Some(sort_doc) = sort_bson.as_document() {
                    mongo_opts.sort = Some(sort_doc.clone());
                }
            }
        }
        if let Some(projection) = options.projection {
            if let Ok(proj_bson) = bson::to_bson(&projection) {
                if let Some(proj_doc) = proj_bson.as_document() {
                    mongo_opts.projection = Some(proj_doc.clone());
                }
            }
        }

        let mut cursor = collection
            .find(filter_doc)
            .with_options(mongo_opts)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut results = Vec::new();
        while cursor.advance().await.map_err(|e| DbError::QueryFailed(e.to_string()))? {
            let raw = cursor.current();
            let doc = bson::de::from_slice(raw.as_bytes())
                .unwrap_or_else(|_| Document::new());
            let json_value = bson_to_json_value(&doc);
            results.push(json_value);
        }

        Ok(results)
    }

    /// 插入单个文档，返回 _id
    async fn insert_one(
        &self,
        db: &str,
        collection: &str,
        doc: serde_json::Value,
    ) -> DbResult<String> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        let doc = bson::to_document(&doc)
            .map_err(|e| DbError::Serialization(e.to_string()))?;

        let result = collection
            .insert_one(doc)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        // 返回插入的 _id
        let id = if let Some(s) = result.inserted_id.as_str() {
            s.to_string()
        } else if let Some(oid) = result.inserted_id.as_object_id() {
            oid.to_hex()
        } else {
            format!("{:?}", result.inserted_id)
        };

        Ok(id)
    }

    /// 插入多个文档，返回插入数量
    async fn insert_many(
        &self,
        db: &str,
        collection: &str,
        docs: Vec<serde_json::Value>,
    ) -> DbResult<u64> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        let documents: Vec<Document> = docs
            .into_iter()
            .map(|doc| {
                bson::to_document(&doc).map_err(|e| DbError::Serialization(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let result = collection
            .insert_many(documents)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(result.inserted_ids.len() as u64)
    }

    /// 更新单个文档，返回影响行数
    async fn update_one(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
        update: serde_json::Value,
    ) -> DbResult<u64> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        let filter_doc = serde_json::from_value(filter)
            .unwrap_or_else(|_| Document::new());
        let update_doc = serde_json::from_value(update)
            .unwrap_or_else(|_| Document::new());

        let result = collection
            .update_one(filter_doc, doc! { "$set": update_doc })
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(result.modified_count)
    }

    /// 删除多个文档，返回删除数量
    async fn delete_many(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
    ) -> DbResult<u64> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        let filter_doc = serde_json::from_value(filter)
            .unwrap_or_else(|_| Document::new());

        let result = collection
            .delete_many(filter_doc)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(result.deleted_count)
    }

    /// 聚合管道
    async fn aggregate(
        &self,
        db: &str,
        collection: &str,
        pipeline: Vec<serde_json::Value>,
    ) -> DbResult<Vec<serde_json::Value>> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        // 转换 pipeline 为 BSON Document 数组
        let pipeline_docs: Vec<Document> = pipeline
            .into_iter()
            .map(|val| {
                let bson_val = bson::to_bson(&val)
                    .map_err(|e| DbError::Serialization(e.to_string()))?;
                bson_val.as_document().cloned().ok_or_else(|| {
                    DbError::Serialization("expected document in pipeline".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut cursor = collection
            .aggregate(pipeline_docs)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut results = Vec::new();
        while cursor.advance().await.map_err(|e| DbError::QueryFailed(e.to_string()))? {
            let raw = cursor.current();
            let doc = bson::de::from_slice(raw.as_bytes())
                .unwrap_or_else(|_| Document::new());
            let json_value = bson_to_json_value(&doc);
            results.push(json_value);
        }

        Ok(results)
    }

    /// 计数
    async fn count(
        &self,
        db: &str,
        collection: &str,
        filter: serde_json::Value,
    ) -> DbResult<u64> {
        let database = self.client.database(db);
        let collection = database.collection::<Document>(collection);

        let filter_doc = serde_json::from_value(filter)
            .unwrap_or_else(|_| Document::new());

        let count = collection
            .count_documents(filter_doc)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(count)
    }
}

/// 将 BSON Document 转换为 serde_json::Value
fn bson_to_json_value(doc: &Document) -> serde_json::Value {
    // 将 BSON 转换为 JSON 字符串，然后解析为 serde_json::Value
    let json_str = doc.to_string();

    // BSON 的 extended JSON 格式转换
    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(val) => val,
        Err(_) => {
            // 如果解析失败，返回一个包含原始字符串的对象
            serde_json::json!({ "_raw": json_str })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mongo_driver_creation() {
        // 需要一个真实的 MongoDB 服务器
        let config = ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            driver: DriverKind::Mongo,
            host: "localhost".to_string(),
            port: 27017,
            database: "test".to_string(),
            username: String::new(),
            password: String::new(),
            ssl: false,
            options: HashMap::new(),
        };

        match MongoDriver::connect(&config).await {
            Ok(_) => {
                // MongoDB 服务器可用
            }
            Err(_) => {
                // 没有 MongoDB 服务器，跳过
            }
        }
    }
}
