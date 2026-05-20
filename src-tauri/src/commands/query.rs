use crate::state::AppState;
use crate::types::QueryResult;
use tauri::State;

/// 执行单条 SQL 查询
#[tauri::command]
pub async fn execute_sql(
    conn_id: &str,
    sql: &str,
    state: State<'_, AppState>,
) -> Result<QueryResult, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    // TODO: Phase 2 驱动实现后接入真实驱动
    Err("Not implemented: driver not ready".to_string())
}

/// 批量执行 SQL 语句（事务内）
#[tauri::command]
pub async fn execute_batch(
    conn_id: &str,
    statements: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::types::ExecResult>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Err("Not implemented: driver not ready".to_string())
}
