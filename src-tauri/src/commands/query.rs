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

/// 将 SQL 文本按分号拆分为独立语句
fn split_sql(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .collect()
}

/// 多语句执行：拆分 SQL 文本，逐条执行，返回全部结果集
#[tauri::command]
pub async fn execute_multi(
    conn_id: &str,
    sql: &str,
    registry: State<'_, ConnectionRegistry>,
) -> Result<Vec<QueryResultJson>, String> {
    let stmts = split_sql(sql);
    if stmts.is_empty() {
        return Ok(vec![]);
    }

    // 一次创建驱动，复用连接
    let driver = get_driver(&registry, conn_id).await?;

    let mut results = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        match driver.query(&stmt, vec![]).await {
            Ok(result) => results.push(result.into()),
            Err(e) => {
                tracing::warn!("Statement failed: {} — {}", stmt, e);
                results.push(QueryResultJson {
                    columns: vec![],
                    rows: vec![],
                    row_count: 0,
                    elapsed_ms: 0,
                });
            }
        }
    }

    Ok(results)
}
