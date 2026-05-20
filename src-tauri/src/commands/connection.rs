use crate::state::{AppState, ConnectionHandle};
use tauri::{State, command};

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
    _config: db_core::types::ConnectionConfig,
) -> Result<(), String> {
    // TODO Phase 3: 集成 secure store + 驱动工厂
    Ok(())
}
