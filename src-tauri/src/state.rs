use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;
use uuid::Uuid;

/// 连接句柄：驱动实例的线程安全引用（UUID 字符串）
pub type ConnectionHandle = String;

#[derive(Clone)]
pub struct AppState {
    pub connections: Arc<Mutex<HashMap<String, ConnectionHandle>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册一个连接句柄，返回分配的 ID
    pub fn register(&self, handle: ConnectionHandle) -> String {
        let id = Uuid::new_v4().to_string();
        self.connections.lock().unwrap().insert(id.clone(), handle);
        id
    }

    /// 根据 ID 获取连接句柄
    pub fn get(&self, id: &str) -> Option<ConnectionHandle> {
        self.connections.lock().unwrap().get(id).cloned()
    }

    /// 移除连接句柄
    pub fn remove(&self, id: &str) -> Option<ConnectionHandle> {
        self.connections.lock().unwrap().remove(id)
    }

    /// 列出所有连接 ID
    pub fn list(&self) -> Vec<String> {
        self.connections.lock().unwrap().keys().cloned().collect()
    }
}
