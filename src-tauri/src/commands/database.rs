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

/// 建立数据库连接并注册到 ConnectionRegistry
#[tauri::command]
pub async fn connect(
    config: ConnectionConfig,
    registry: State<'_, ConnectionRegistry>,
) -> Result<String, String> {
    // 验证连接可用（创建驱动 + ping）
    let _driver = create_driver(&config).await?;

    // 生成连接 ID
    let conn_id = if config.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        config.id.clone()
    };

    // 存入 ConnectionRegistry（配置用于后续按需重建驱动）
    registry.register(conn_id.clone(), config).await;

    Ok(conn_id)
}

/// 检查连接是否存活（从 registry 取配置，重建驱动后 ping）
#[tauri::command]
pub async fn ping(
    conn_id: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<(), String> {
    let config_arc = registry
        .get_config(conn_id)
        .await
        .ok_or("Connection not found")?;
    let config = config_arc.lock().unwrap().clone();
    drop(config_arc);

    let driver = create_driver(&config).await?;
    driver.ping().await?;
    Ok(())
}

/// 断开并注销连接
#[tauri::command]
pub async fn disconnect(
    conn_id: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<(), String> {
    registry.remove(conn_id).await;
    Ok(())
}

