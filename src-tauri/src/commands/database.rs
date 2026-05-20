use crate::state::AppState;
use tauri::State;

/// 建立数据库连接并注册到 AppState
#[tauri::command]
pub async fn connect(
    _config: db_core::types::ConnectionConfig,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // TODO: Phase 3 驱动工厂实现后接入
    // 返回一个临时的连接 ID
    Ok(state.register("temp-handle".to_string()))
}

/// 检查连接是否存活
#[tauri::command]
pub async fn ping(
    _conn_id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _handle = state.get(_conn_id).ok_or("Connection not found")?;
    // TODO: 调用驱动的 ping() 方法
    Ok(())
}

/// 断开并注销连接
#[tauri::command]
pub async fn disconnect(
    conn_id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.remove(conn_id);
    Ok(())
}
