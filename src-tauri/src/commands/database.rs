use crate::state::AppState;
use crate::types::ConnectionConfig;
use db_core::traits::DatabaseDriver;
use db_postgres::PostgresDriver;
use db_mysql::MySqlDriver;
use db_sqlite::SqliteDriver;
use db_mongo::MongoDriver;
use db_redis::RedisDriver;
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
        db_core::types::DriverKind::Mysql => {
            let driver = MySqlDriver::connect(config)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        db_core::types::DriverKind::Sqlite => {
            let driver = SqliteDriver::connect(config)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        db_core::types::DriverKind::Mongo => {
            let driver = MongoDriver::connect(config)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        db_core::types::DriverKind::Redis => {
            let driver = RedisDriver::connect(config)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        _ => Err(format!("Driver {:?} not yet implemented", config.driver)),
    }
}

/// 建立数据库连接并注册到 AppState
#[tauri::command]
pub async fn connect(
    config: ConnectionConfig,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let driver = create_driver(&config).await?;
    let handle = state.register("driver-handle".to_string());
    // TODO: 将 driver 实例存入 ConnectionRegistry
    Ok(handle)
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
