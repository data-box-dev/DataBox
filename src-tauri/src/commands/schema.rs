use crate::types::ConnectionConfig;
use crate::connection_registry::ConnectionRegistry;
use db_core::traits::DatabaseDriver;
use db_postgres::PostgresDriver;
use tauri::State;

/// 驱动工厂：根据驱动类型创建驱动实例
pub async fn create_driver(
    config: &ConnectionConfig,
) -> Result<Box<dyn DatabaseDriver>, String> {
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

/// 列出指定连接的所有数据库
#[tauri::command]
pub async fn list_databases(
    conn_id: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::DatabaseInfo>, String> {
    let entry = registry.get(conn_id).await.ok_or("Connection not found")?;

    // TODO: 反序列化驱动实例并调用 list_databases()
    // 当前为 stub，等待驱动实例序列化方案确定
    Err("Driver instance not yet serializable".to_string())
}

/// 列出数据库内所有 Schema
#[tauri::command]
pub async fn list_schemas(
    conn_id: &str,
    _database: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<String>, String> {
    let _entry = registry.get(conn_id).await.ok_or("Connection not found")?;
    Err("Not yet implemented".to_string())
}

/// 列出 Schema 内所有表
#[tauri::command]
pub async fn list_tables(
    conn_id: &str,
    _database: &str,
    _schema: Option<&str>,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::TableInfo>, String> {
    let _entry = registry.get(conn_id).await.ok_or("Connection not found")?;
    Err("Not yet implemented".to_string())
}

/// 列出表的所有列
#[tauri::command]
pub async fn list_columns(
    conn_id: &str,
    _database: &str,
    _schema: Option<&str>,
    _table: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::ColumnMeta>, String> {
    let _entry = registry.get(conn_id).await.ok_or("Connection not found")?;
    Err("Not yet implemented".to_string())
}

/// 获取完整表 Schema
#[tauri::command]
pub async fn describe_table(
    conn_id: &str,
    _database: &str,
    _table: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<crate::types::TableSchema, String> {
    let _entry = registry.get(conn_id).await.ok_or("Connection not found")?;
    Err("Not yet implemented".to_string())
}
