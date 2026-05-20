use crate::commands::database::create_driver;
use crate::connection_registry::ConnectionRegistry;
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

/// 列出指定连接的所有数据库
#[tauri::command]
pub async fn list_databases(
    conn_id: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::DatabaseInfo>, String> {
    let driver = get_driver(&registry, conn_id).await?;
    let dbs = driver
        .list_databases()
        .await
        .map_err(|e| e.to_string())?;
    Ok(dbs)
}

/// 列出数据库内所有 Schema
#[tauri::command]
pub async fn list_schemas(
    conn_id: &str,
    database: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<String>, String> {
    let _driver = get_driver(&registry, conn_id).await?;
    // 关系型数据库以 database 作为 schema 视图
    let tables = _driver
        .list_tables(database)
        .await
        .map_err(|e| e.to_string())?;
    // 如果没有 table-level schema 分离，返回空列表
    // PostgreSQL 用户可扩展为从 information_schema.schemata 查询
    Ok(vec![])
}

/// 列出 Schema 内所有表
#[tauri::command]
pub async fn list_tables(
    conn_id: &str,
    database: &str,
    _schema: Option<&str>,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::TableInfo>, String> {
    let driver = get_driver(&registry, conn_id).await?;

    let table_names = driver
        .list_tables(database)
        .await
        .map_err(|e| e.to_string())?;

    let tables: Vec<crate::types::TableInfo> = table_names
        .into_iter()
        .map(|name| crate::types::TableInfo {
            name,
            table_type: crate::types::TableType::Table,
            row_estimate: None,
            size_bytes: None,
        })
        .collect();

    Ok(tables)
}

/// 列出表的所有列
#[tauri::command]
pub async fn list_columns(
    conn_id: &str,
    database: &str,
    schema: Option<&str>,
    table: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<crate::types::ColumnMeta>, String> {
    let driver = get_driver(&registry, conn_id).await?;
    let schema_desc = driver
        .describe_table(database, table)
        .await
        .map_err(|e| e.to_string())?;

    // describe_table 返回 ColumnSchema，转成 ColumnMeta
    let columns: Vec<crate::types::ColumnMeta> = schema_desc
        .columns
        .into_iter()
        .map(|cs| crate::types::ColumnMeta {
            name: cs.name,
            data_type: cs.data_type,
            nullable: cs.nullable,
        })
        .collect();

    Ok(columns)
}

/// 获取完整表 Schema
#[tauri::command]
pub async fn describe_table(
    conn_id: &str,
    database: &str,
    table: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<crate::types::TableSchema, String> {
    let driver = get_driver(&registry, conn_id).await?;
    driver
        .describe_table(database, table)
        .await
        .map_err(|e| e.to_string())
}
