use crate::types::ConnectionConfig;
use crate::connection_registry::ConnectionRegistry;
use db_core::traits::DatabaseDriver;
use db_postgres::PostgresDriver;
use tauri::State;

/// 驱动工厂：创建驱动实例
async fn get_or_create_driver(
    config: &ConnectionConfig,
    registry: &ConnectionRegistry,
) -> Result<Box<dyn DatabaseDriver>, String> {
    // 根据配置的驱动类型创建实例
    match config.driver {
        db_core::types::DriverKind::Postgres => {
            let driver = PostgresDriver::connect(config)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        _ => Err(format!("Driver {:?} not yet implemented", config.driver)),
    }
}

/// 执行单条 SQL
#[tauri::command]
pub async fn execute_sql(
    conn_id: &str,
    sql: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<crate::types::QueryResult, String> {
    let entry = registry.get(conn_id).await.ok_or("Connection not found")?;

    let entry_guard = entry.lock().await;
    let driver_guard = entry_guard.driver.lock().await;

    if let Some(driver) = driver_guard.as_ref() {
        // 从 registry 恢复驱动实例（需要通过 Any 向下转型）
        // 当前为简化实现：每次都创建新驱动
    }

    drop(driver_guard);
    drop(entry_guard);

    // TODO: 集成连接配置以恢复驱动
    // 当前简化：仅返回错误提示
    Err("Driver restoration not yet implemented. Use query command after connection.".to_string())
}

/// 批量执行 SQL（事务内）
#[tauri::command]
pub async fn execute_batch(
    conn_id: &str,
    _statements: Vec<String>,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::ExecResult>, String> {
    let _entry = registry.get(conn_id).await.ok_or("Connection not found")?;
    Err("Not yet implemented".to_string())
}
