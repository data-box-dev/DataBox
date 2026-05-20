use crate::commands::database::create_driver;
use crate::connection_registry::ConnectionRegistry;
use db_core::traits::DatabaseDriver;
use tauri::State;

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
) -> Result<crate::types::QueryResult, String> {
    let driver = get_driver(&registry, conn_id).await?;
    driver
        .query(sql, vec![])
        .await
        .map_err(|e| e.to_string())
}

/// 批量执行 SQL（事务内）
#[tauri::command]
pub async fn execute_batch(
    conn_id: &str,
    statements: Vec<String>,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::ExecResult>, String> {
    let driver = get_driver(&registry, conn_id).await?;
    driver
        .execute_batch(statements)
        .await
        .map_err(|e| e.to_string())
}
