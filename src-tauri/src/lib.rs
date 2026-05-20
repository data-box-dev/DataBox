use db_core::{
    ConnectionConfig, DatabaseDriver, DatabaseInfo, DbError, DriverKind, ExecResult, QueryResult,
    TableSchema,
};
use db_mongo::MongoDriver;
use db_mysql::MysqlDriver;
use db_postgres::PostgresDriver;
use db_redis::RedisDriver;
use db_sqlite::SqliteDriver;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use tauri::{
    AppHandle, LogicalPosition, Manager, State, TitleBarStyle, WebviewUrl, WebviewWindowBuilder,
};
use tokio::sync::RwLock;

#[derive(Default)]
struct AppState {
    saved_connections: RwLock<HashMap<String, ConnectionConfig>>,
    runtime_connections: RwLock<HashMap<String, ManagedConnection>>,
}

struct ManagedConnection {
    config: ConnectionConfig,
    driver: ManagedDriver,
}

enum ManagedDriver {
    Postgres(PostgresDriver),
    Mysql(MysqlDriver),
    Sqlite(SqliteDriver),
    Mongo(MongoDriver),
    Redis(RedisDriver),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedConnectionRecord {
    config: ConnectionConfig,
    is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectionWorkspace {
    connection_id: String,
    driver: DriverKind,
    name: String,
    database: String,
    databases: Vec<DatabaseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SqlExecutionResponse {
    mode: SqlExecutionMode,
    query_result: Option<QueryResult>,
    exec_result: Option<ExecResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SqlExecutionMode {
    Query,
    Execute,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            initialize_saved_connections(app.handle())?;

            let win_builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .decorations(true)
                    .hidden_title(true)
                    .inner_size(1300.0, 900.0)
                    .min_inner_size(980.0, 680.0);

            #[cfg(target_os = "macos")]
            {
                win_builder
                    .title_bar_style(TitleBarStyle::Overlay)
                    .traffic_light_position(LogicalPosition::new(15, 22))
                    .build()
                    .expect("failed to build main window");
            }

            #[cfg(not(target_os = "macos"))]
            {
                win_builder.build().expect("failed to build main window");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_saved_connections,
            save_connection,
            delete_saved_connection,
            test_connection,
            connect_database,
            disconnect_database,
            load_connection_workspace,
            describe_table,
            execute_sql,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn list_saved_connections(
    state: State<'_, AppState>,
) -> Result<Vec<SavedConnectionRecord>, String> {
    Ok(saved_connection_records(&state).await)
}

#[tauri::command]
async fn save_connection(
    app: AppHandle,
    state: State<'_, AppState>,
    config: ConnectionConfig,
) -> Result<Vec<SavedConnectionRecord>, String> {
    {
        let mut saved_connections = state.saved_connections.write().await;
        saved_connections.insert(config.id.clone(), config);
    }

    persist_saved_connections(&app, &state).await?;
    Ok(saved_connection_records(&state).await)
}

#[tauri::command]
async fn delete_saved_connection(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<SavedConnectionRecord>, String> {
    let removed = {
        let mut saved_connections = state.saved_connections.write().await;
        saved_connections.remove(&id)
    };

    if removed.is_none() {
        return Err(DbError::ConnectionNotFound { id }.to_string());
    }

    disconnect_runtime_connection(&state, &id).await?;
    persist_saved_connections(&app, &state).await?;
    Ok(saved_connection_records(&state).await)
}

#[tauri::command]
async fn test_connection(config: ConnectionConfig) -> Result<(), String> {
    let connection = ManagedConnection::connect(config).await?;
    connection.ping().await?;
    connection.close().await?;
    Ok(())
}

#[tauri::command]
async fn connect_database(
    state: State<'_, AppState>,
    config: ConnectionConfig,
) -> Result<ConnectionWorkspace, String> {
    let connection = ManagedConnection::connect(config).await?;
    let connection_id = connection.config.id.clone();
    let workspace = connection.workspace().await?;

    if let Some(previous_connection) = {
        let mut runtime_connections = state.runtime_connections.write().await;
        runtime_connections.insert(connection_id, connection)
    } {
        previous_connection.close().await?;
    }

    Ok(workspace)
}

#[tauri::command]
async fn disconnect_database(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<SavedConnectionRecord>, String> {
    disconnect_runtime_connection(&state, &id).await?;
    Ok(saved_connection_records(&state).await)
}

#[tauri::command]
async fn load_connection_workspace(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<ConnectionWorkspace, String> {
    let runtime_connections = state.runtime_connections.read().await;
    let connection = runtime_connections.get(&connection_id).ok_or_else(|| {
        DbError::ConnectionNotFound {
            id: connection_id.clone(),
        }
        .to_string()
    })?;

    connection.workspace().await
}

#[tauri::command]
async fn describe_table(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<TableSchema, String> {
    let runtime_connections = state.runtime_connections.read().await;
    let connection = runtime_connections.get(&connection_id).ok_or_else(|| {
        DbError::ConnectionNotFound {
            id: connection_id.clone(),
        }
        .to_string()
    })?;

    connection.describe_table(&database, &table).await
}

#[tauri::command]
async fn execute_sql(
    state: State<'_, AppState>,
    connection_id: String,
    sql: String,
) -> Result<SqlExecutionResponse, String> {
    let trimmed_sql = sql.trim();
    if trimmed_sql.is_empty() {
        return Err(DbError::QueryFailed("SQL 不能为空".to_string()).to_string());
    }

    let runtime_connections = state.runtime_connections.read().await;
    let connection = runtime_connections.get(&connection_id).ok_or_else(|| {
        DbError::ConnectionNotFound {
            id: connection_id.clone(),
        }
        .to_string()
    })?;

    let is_query = connection.is_query_statement(trimmed_sql);

    if is_query {
        let query_result = connection.query(trimmed_sql).await?;
        Ok(SqlExecutionResponse {
            mode: SqlExecutionMode::Query,
            query_result: Some(query_result),
            exec_result: None,
        })
    } else {
        let exec_result = connection.execute(trimmed_sql).await?;
        Ok(SqlExecutionResponse {
            mode: SqlExecutionMode::Execute,
            query_result: None,
            exec_result: Some(exec_result),
        })
    }
}

impl ManagedConnection {
    async fn connect(config: ConnectionConfig) -> Result<Self, String> {
        let driver = match config.driver {
            DriverKind::Postgres => ManagedDriver::Postgres(
                PostgresDriver::connect(&config)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
            DriverKind::Mysql => ManagedDriver::Mysql(
                MysqlDriver::connect(&config)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
            DriverKind::Sqlite => ManagedDriver::Sqlite(
                SqliteDriver::connect(&config)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
            DriverKind::Mongo => ManagedDriver::Mongo(
                MongoDriver::connect(&config)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
            DriverKind::Redis => ManagedDriver::Redis(
                RedisDriver::connect(&config)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
        };

        Ok(Self { config, driver })
    }

    async fn ping(&self) -> Result<(), String> {
        match &self.driver {
            ManagedDriver::Postgres(driver) => driver.ping().await,
            ManagedDriver::Mysql(driver) => driver.ping().await,
            ManagedDriver::Sqlite(driver) => driver.ping().await,
            ManagedDriver::Mongo(driver) => driver.ping().await,
            ManagedDriver::Redis(driver) => driver.ping().await,
        }
        .map_err(|error| error.to_string())
    }

    async fn close(self) -> Result<(), String> {
        match self.driver {
            ManagedDriver::Postgres(driver) => driver.close().await,
            ManagedDriver::Mysql(driver) => driver.close().await,
            ManagedDriver::Sqlite(driver) => driver.close().await,
            ManagedDriver::Mongo(driver) => driver.close().await,
            ManagedDriver::Redis(driver) => driver.close().await,
        }
        .map_err(|error| error.to_string())
    }

    async fn workspace(&self) -> Result<ConnectionWorkspace, String> {
        let databases = match &self.driver {
            ManagedDriver::Postgres(driver) => driver.list_databases().await,
            ManagedDriver::Mysql(driver) => driver.list_databases().await,
            ManagedDriver::Sqlite(driver) => driver.list_databases().await,
            ManagedDriver::Mongo(driver) => driver.list_databases().await,
            ManagedDriver::Redis(driver) => driver.list_databases().await,
        }
        .map_err(|error| error.to_string())?;

        Ok(ConnectionWorkspace {
            connection_id: self.config.id.clone(),
            driver: self.config.driver.clone(),
            name: self.config.name.clone(),
            database: self.config.database.clone(),
            databases,
        })
    }

    async fn describe_table(&self, database: &str, table: &str) -> Result<TableSchema, String> {
        match &self.driver {
            ManagedDriver::Postgres(driver) => driver.describe_table(database, table).await,
            ManagedDriver::Mysql(driver) => driver.describe_table(database, table).await,
            ManagedDriver::Sqlite(driver) => driver.describe_table(database, table).await,
            ManagedDriver::Mongo(driver) => driver.describe_table(database, table).await,
            ManagedDriver::Redis(driver) => driver.describe_table(database, table).await,
        }
        .map_err(|error| error.to_string())
    }

    async fn query(&self, sql: &str) -> Result<QueryResult, String> {
        match &self.driver {
            ManagedDriver::Postgres(driver) => driver.query(sql, vec![]).await,
            ManagedDriver::Mysql(driver) => driver.query(sql, vec![]).await,
            ManagedDriver::Sqlite(driver) => driver.query(sql, vec![]).await,
            ManagedDriver::Mongo(driver) => driver.query(sql, vec![]).await,
            ManagedDriver::Redis(driver) => driver.query(sql, vec![]).await,
        }
        .map_err(|error| error.to_string())
    }

    async fn execute(&self, sql: &str) -> Result<ExecResult, String> {
        match &self.driver {
            ManagedDriver::Postgres(driver) => driver.execute(sql, vec![]).await,
            ManagedDriver::Mysql(driver) => driver.execute(sql, vec![]).await,
            ManagedDriver::Sqlite(driver) => driver.execute(sql, vec![]).await,
            ManagedDriver::Mongo(driver) => driver.execute(sql, vec![]).await,
            ManagedDriver::Redis(driver) => driver.execute(sql, vec![]).await,
        }
        .map_err(|error| error.to_string())
    }

    fn is_query_statement(&self, statement: &str) -> bool {
        match self.config.driver {
            DriverKind::Postgres | DriverKind::Mysql | DriverKind::Sqlite => {
                is_relational_query(statement)
            }
            DriverKind::Mongo => is_mongo_query(statement),
            DriverKind::Redis => is_redis_query(statement),
        }
    }
}

fn is_relational_query(statement: &str) -> bool {
    let lower = statement.trim().to_ascii_lowercase();
    lower.starts_with("select")
        || lower.starts_with("with")
        || lower.starts_with("show")
        || lower.starts_with("pragma")
        || lower.starts_with("describe")
        || lower.starts_with("desc")
        || lower.starts_with("explain")
}

fn is_mongo_query(statement: &str) -> bool {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(statement) {
        if let Some(action) = value.get("action").and_then(|action| action.as_str()) {
            return matches!(
                action.to_ascii_lowercase().as_str(),
                "find" | "aggregate" | "count" | "list_collections" | "list_databases"
            );
        }
    }

    let command = statement
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        command.as_str(),
        "find" | "aggregate" | "count" | "show" | "list"
    )
}

fn is_redis_query(statement: &str) -> bool {
    let command = statement
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        command.as_str(),
        "ping"
            | "get"
            | "ttl"
            | "type"
            | "exists"
            | "keys"
            | "scan"
            | "hgetall"
            | "lrange"
            | "smembers"
            | "zrange"
            | "info"
    )
}

async fn disconnect_runtime_connection(state: &AppState, id: &str) -> Result<(), String> {
    let connection = {
        let mut runtime_connections = state.runtime_connections.write().await;
        runtime_connections.remove(id)
    };

    if let Some(connection) = connection {
        connection.close().await
    } else {
        Ok(())
    }
}

async fn saved_connection_records(state: &AppState) -> Vec<SavedConnectionRecord> {
    let saved_connections = state.saved_connections.read().await;
    let runtime_connections = state.runtime_connections.read().await;

    let mut records = saved_connections
        .values()
        .cloned()
        .map(|config| SavedConnectionRecord {
            is_connected: runtime_connections.contains_key(&config.id),
            config,
        })
        .collect::<Vec<_>>();

    records.sort_by(|left, right| left.config.name.cmp(&right.config.name));
    records
}

fn initialize_saved_connections(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let connections = load_saved_connections_from_disk(app)?;
    let state = app.state::<AppState>();

    let mut saved_connections = state.saved_connections.blocking_write();
    saved_connections.clear();
    for config in connections {
        saved_connections.insert(config.id.clone(), config);
    }

    Ok(())
}

async fn persist_saved_connections(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let saved_connections = state.saved_connections.read().await;
    let mut values = saved_connections.values().cloned().collect::<Vec<_>>();
    values.sort_by(|left, right| left.name.cmp(&right.name));

    let path = saved_connections_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let payload = serde_json::to_vec_pretty(&values).map_err(|error| error.to_string())?;
    fs::write(path, payload).map_err(|error| error.to_string())
}

fn load_saved_connections_from_disk(app: &AppHandle) -> Result<Vec<ConnectionConfig>, String> {
    let path = saved_connections_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&content).map_err(|error| error.to_string())
}

fn saved_connections_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    Ok(config_dir.join("connections.json"))
}
