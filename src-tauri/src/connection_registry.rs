use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;
use db_core::types::ConnectionConfig;

/// 连接条目：持有配置和驱动实例
pub struct RegistryEntry {
    pub config: ConnectionConfig,
    pub driver: Mutex<Option<Arc<dyn std::any::Any + Send + Sync>>>,
}

/// 连接注册表：统一管理驱动实例的生命周期
#[derive(Clone)]
pub struct ConnectionRegistry {
    instances: Arc<Mutex<HashMap<String, Arc<Mutex<RegistryEntry>>>>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            instances: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册一个连接及其驱动实例
    pub async fn register(
        &self,
        id: String,
        config: ConnectionConfig,
        driver: Arc<dyn std::any::Any + Send + Sync>,
    ) {
        let entry = RegistryEntry {
            config,
            driver: Mutex::new(Some(driver)),
        };
        self.instances
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(entry)));
    }

    /// 根据 ID 获取连接条目
    pub async fn get(&self, id: &str) -> Option<Arc<Mutex<RegistryEntry>>> {
        self.instances.lock().unwrap().get(id).cloned()
    }

    /// 移除并返回连接条目（调用者负责清理驱动资源）
    pub async fn remove(&self, id: &str) -> Option<Arc<Mutex<RegistryEntry>>> {
        self.instances.lock().unwrap().remove(id)
    }

    /// 列出所有已注册的连接 ID
    pub async fn list(&self) -> Vec<String> {
        self.instances.lock().unwrap().keys().cloned().collect()
    }
}
