use crate::state::AppState;
use db_core::types::ConnectionConfig;
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

/// 列出所有已注册的连接 ID
#[tauri::command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state.list())
}

/// 注册一个新连接句柄，返回分配的连接 ID
#[tauri::command]
pub async fn register_connection(
    handle: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    Ok(state.register(handle))
}

/// 根据 ID 获取连接句柄
#[tauri::command]
pub async fn get_connection(
    id: &str,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    Ok(state.get(id))
}

/// 注销并移除连接句柄
#[tauri::command]
pub async fn unregister_connection(
    id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.remove(id);
    Ok(())
}

/// 测试连接连通性（不注册，仅验证）
#[tauri::command]
pub async fn test_connection(
    config: ConnectionConfig,
) -> Result<(), String> {
    use crate::commands::database::create_driver;

    let driver = create_driver(&config).await?;
    driver.ping().await?;
    Ok(())
}

/// 保存连接配置（密码存到 store，配置存入 JSON 文件）
#[tauri::command]
pub async fn save_connection(
    config: ConnectionConfig,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use crate::commands::config;

    // 1. 保存密码到 store
    let store = app.store("passwords.json")
        .map_err(|e| format\!("Failed to open password store: {}", e))?;
    store.set(format\!("password:{}", config.id), Value::String(config.password.clone()));
    store.save()
        .map_err(|e| format\!("Failed to save password: {}", e))?;

    // 2. 加载已有连接列表，追加或更新
    let mut connections = config::load_connections(&app)
        .await
        .unwrap_or_default();

    let stored: config::StoredConnection = config.into();
    if let Some(idx) = connections.iter().position(|c| c.id == stored.id) {
        connections[idx] = stored;
    } else {
        connections.push(stored);
    }

    // 3. 持久化到 JSON 文件
    config::save_connections(connections, app)
        .await
        .map_err(|e| format\!("Failed to save config: {}", e))?;

    Ok(())
}

/// 从 store 加载连接密码
#[tauri::command]
pub async fn load_password(
    id: &str,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let store = app.store("passwords.json")
        .map_err(|e| format\!("Failed to open password store: {}", e))?;
    store.get(format\!("password:{}", id))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| format\!("Password not found for connection: {}", id))
}
