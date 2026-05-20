use crate::state::AppState;
use crate::types::{ColumnMeta, DatabaseInfo, TableInfo, TableSchema};
use tauri::State;

/// 列出指定连接的所有数据库
#[tauri::command]
pub async fn list_databases(
    conn_id: &str,
    state: State<'_, AppState>,
) -> Result<Vec<DatabaseInfo>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    // TODO: Phase 2 驱动实现后接入真实驱动
    Ok(vec![])
}

/// 列出数据库内所有 Schema
#[tauri::command]
pub async fn list_schemas(
    conn_id: &str,
    database: &str,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Ok(vec![])
}

/// 列出 Schema 内所有表
#[tauri::command]
pub async fn list_tables(
    conn_id: &str,
    database: &str,
    schema: Option<&str>,
    state: State<'_, AppState>,
) -> Result<Vec<TableInfo>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Ok(vec![])
}

/// 列出表的所有列
#[tauri::command]
pub async fn list_columns(
    conn_id: &str,
    database: &str,
    schema: Option<&str>,
    table: &str,
    state: State<'_, AppState>,
) -> Result<Vec<ColumnMeta>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Ok(vec![])
}

/// 获取完整表 Schema（列 + 索引 + 主键）
#[tauri::command]
pub async fn describe_table(
    conn_id: &str,
    database: &str,
    table: &str,
    state: State<'_, AppState>,
) -> Result<TableSchema, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Err("Not implemented: driver not ready".to_string())
}
