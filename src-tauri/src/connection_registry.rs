use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use db_core::types::ConnectionConfig;

/// 连接注册表：存储连接配置，驱动按需重建
#[derive(Clone)]
pub struct ConnectionRegistry {
    instances: Arc<Mutex<HashMap<String, Arc<Mutex<ConnectionConfig>>>>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            instances: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册连接配置
    pub async fn register(&self, id: String, config: ConnectionConfig) {
        self.instances
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(config)));
    }

    /// 根据 ID 获取连接配置
    pub async fn get_config(&self, id: &str) -> Option<Arc<Mutex<ConnectionConfig>>> {
        self.instances.lock().unwrap().get(id).cloned()
    }

    /// 移除连接
    pub async fn remove(&self, id: &str) -> Option<Arc<Mutex<ConnectionConfig>>> {
        self.instances.lock().unwrap().remove(id)
    }

    /// 列出所有已注册连接 ID
    pub async fn list(&self) -> Vec<String> {
        self.instances.lock().unwrap().keys().cloned().collect()
    }
}
