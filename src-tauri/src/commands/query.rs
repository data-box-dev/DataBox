use crate::commands::database::create_driver;
use crate::connection_registry::ConnectionRegistry;
use db_core::traits::DatabaseDriver;
use tauri::State;
use std::collections::HashMap;

/// 前端友好的查询结果（DbValue → serde_json::Value）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultJson {
    pub columns: Vec<db_core::types::ColumnMeta>,
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub row_count: usize,
    pub elapsed_ms: u64,
}

impl From<db_core::types::QueryResult> for QueryResultJson {
    fn from(result: db_core::types::QueryResult) -> Self {
        Self {
            columns: result.columns,
            rows: result.to_json_rows(),
            row_count: result.row_count,
            elapsed_ms: result.elapsed_ms,
        }
    }
}

/// 从 registry 取配置并重建驱动
async fn get_driver(
    registry: &ConnectionRegistry,
    conn_id: &str,
) -> Result<Box<dyn DatabaseDriver>, String> {
    let config_arc = registry
        .get_config(conn_id)
        .await
        .ok_or("Connection not found")?;
    let config = config_arc.lock().unwrap().clone();
    create_driver(&config).await
}

/// 执行单条 SQL
#[tauri::command]
pub async fn execute_sql(
    conn_id: &str,
    sql: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<QueryResultJson, String> {
    let driver = get_driver(&registry, conn_id).await?;
    let result = driver
        .query(sql, vec![])
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.into())
}

/// 批量执行 SQL（事务内）
#[tauri::command]
pub async fn execute_batch(
    conn_id: &str,
    statements: Vec<String>,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<db_core::types::ExecResult>, String> {
    let driver = get_driver(&registry, conn_id).await?;
    driver
        .execute_batch(statements)
        .await
        .map_err(|e| e.to_string())
}
