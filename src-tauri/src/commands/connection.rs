use crate::state::{AppState, ConnectionHandle};
use tauri::{State, command};
use tauri_plugin_secure_store::SecureStore;
use db_core::types::ConnectionConfig;

/// 列出所有已注册的连接 ID
#[command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state.list())
}

/// 注册一个新连接句柄，返回分配的连接 ID
#[command]
pub async fn register_connection(
    handle: ConnectionHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    Ok(state.register(handle))
}

/// 根据 ID 获取连接句柄
#[command]
pub async fn get_connection(
    id: &str,
    state: State<'_, AppState>,
) -> Result<Option<ConnectionHandle>, String> {
    Ok(state.get(id))
}

/// 注销并移除连接句柄
#[command]
pub async fn unregister_connection(
    id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.remove(id);
    Ok(())
}

/// 测试连接连通性（不注册，仅验证）
#[command]
pub async fn test_connection(
    config: ConnectionConfig,
) -> Result<(), String> {
    use crate::commands::database::create_driver;

    let driver = create_driver(&config).await?;
    driver.ping().await?;
    // 驱动会在函数返回时被 drop
    Ok(())
}

/// 保存连接配置（密码加密存储到系统钥匙串）
#[command]
pub async fn save_connection(
    config: ConnectionConfig,
    secure_store: State<'_, SecureStore>,
) -> Result<(), String> {
    // 写入密码到系统钥匙串
    let key = format!("databox:password:{}", config.id);
    secure_store
        .set(&key, &config.password)
        .await
        .map_err(|e| format!("Failed to save password: {}", e))?;

    // TODO: 保存不含密码的配置到本地存储（文件系统或 SQLite）
    tracing::info!("Connection saved: {}", config.id);
    Ok(())
}

/// 从系统钥匙串加载连接密码
#[command]
pub async fn load_password(
    id: &str,
    secure_store: State<'_, SecureStore>,
) -> Result<String, String> {
    let key = format!("databox:password:{}", id);
    secure_store
        .get(&key)
        .await
        .map_err(|e| format!("Failed to load password: {}", e))
}
