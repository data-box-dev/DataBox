use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

/// 连接配置存储（不含密码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConnection {
    pub id: String,
    pub name: String,
    pub driver: db_core::types::DriverKind,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub ssl: bool,
    pub options: std::collections::HashMap<String, String>,
}

impl From<db_core::types::ConnectionConfig> for StoredConnection {
    fn from(config: db_core::types::ConnectionConfig) -> Self {
        Self {
            id: config.id,
            name: config.name,
            driver: config.driver,
            host: config.host,
            port: config.port,
            database: config.database,
            username: config.username,
            ssl: config.ssl,
            options: config.options,
        }
    }
}

impl From<StoredConnection> for db_core::types::ConnectionConfig {
    fn from(stored: StoredConnection) -> Self {
        Self {
            id: stored.id,
            name: stored.name,
            driver: stored.driver,
            host: stored.host,
            port: stored.port,
            database: stored.database,
            username: stored.username,
            password: String::new(), // 密码从 store 单独加载
            ssl: stored.ssl,
            options: stored.options,
        }
    }
}

/// 获取配置文件路径
fn get_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app.path().app_config_dir()
        .map_err(|e| format\!("Failed to get app config dir: {}", e))?;

    fs::create_dir_all(&app_dir)
        .map_err(|e| format\!("Failed to create config dir: {}", e))?;

    Ok(app_dir.join("connections.json"))
}

/// 保存连接列表（不含密码）
#[tauri::command]
pub async fn save_connections(
    connections: Vec<StoredConnection>,
    app: AppHandle,
) -> Result<(), String> {
    let path = get_config_path(&app)?;
    let json = serde_json::to_string_pretty(&connections)
        .map_err(|e| format\!("Failed to serialize: {}", e))?;
    fs::write(&path, json)
        .map_err(|e| format\!("Failed to write config: {}", e))?;
    Ok(())
}

/// 加载连接列表
#[tauri::command]
pub async fn load_connections(
    app: AppHandle,
) -> Result<Vec<StoredConnection>, String> {
    let path = get_config_path(&app)?;

    if \!path.exists() {
        return Ok(vec\![]);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format\!("Failed to read config: {}", e))?;
    let connections: Vec<StoredConnection> = serde_json::from_str(&content)
        .map_err(|e| format\!("Failed to parse config: {}", e))?;
    Ok(connections)
}
