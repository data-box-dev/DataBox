# DataGrip 风格前端重构 - 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 DataBox 从 Vite 空白模板重构为 DataGrip 风格数据库管理工具

**Architecture:** Rust Tauri 后端暴露 IPC 命令（连接管理 / 元数据查询 / SQL 执行），Vue3 前端通过 Monaco Editor + n-data-table 构建专业 UI

**Tech Stack:** Tauri 2 / Rust 2024 / Vue 3 / Pinia / naive-ui / Monaco Editor / tauri-plugin-secure-store

---

## 前置准备

- [ ] 阅读 spec：`docs/superpowers/specs/2026-05-20-frontend-refactor-design.md`
- [ ] 阅读 db-core trait 定义：`src-tauri/crates/db-core/src/traits.rs`、`src-tauri/crates/db-core/src/types.rs`
- [ ] 确认 package.json 中已安装 naive-ui、@monaco-editor/loader、tauri-plugin-secure-store

---

## Phase 1：Rust 后端 — 基础设施（AppState + 连接命令）

### Task 1: 定义 ConnectionHandle 和 AppState

**Files:**
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 state.rs**

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

// 连接句柄：驱动实例的线程安全引用
pub type ConnectionHandle = String;

#[derive(Clone)]
pub struct AppState {
    pub connections: Arc<Mutex<HashMap<String, ConnectionHandle>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, handle: ConnectionHandle) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.connections.lock().unwrap().insert(id.clone(), handle);
        id
    }

    pub fn get(&self, id: &str) -> Option<ConnectionHandle> {
        self.connections.lock().unwrap().get(id).cloned()
    }

    pub fn remove(&self, id: &str) -> Option<ConnectionHandle> {
        self.connections.lock().unwrap().remove(id)
    }

    pub fn list(&self) -> Vec<String> {
        self.connections.lock().unwrap().keys().cloned().collect()
    }
}
```

- [ ] **Step 2: 在 lib.rs 中注册 AppState**

```rust
mod state;
use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(AppState::new())
        // ... setup 和 run 保持现有逻辑
}
```

- [ ] **Step 3: 检查 Cargo.toml 添加 uuid 依赖**

在 `[dependencies]` 添加：
```toml
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 4: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(tauri): add AppState with ConnectionHandle registry"
```

---

### Task 2: 连接管理命令（connection.rs）

**Files:**
- Create: `src-tauri/src/commands/connection.rs`
- Create: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 commands/mod.rs**

```rust
pub mod connection;
pub mod database;
pub mod schema;
pub mod query;
```

- [ ] **Step 2: 创建 connection.rs**

```rust
use crate::state::{AppState, ConnectionHandle};
use crate::types::ConnectionConfig;
use tauri::{State, command};

#[command]
pub async fn save_connection(
    config: ConnectionConfig,
) -> Result<(), String> {
    // TODO Phase 3 集成 secure store，此处先保存到文件系统
    // password 先不处理，Phase 3 加密
    Ok(())
}

#[command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state.list())
}

#[command]
pub async fn register_connection(
    handle: ConnectionHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    Ok(state.register(handle))
}

#[command]
pub async fn get_connection(
    id: &str,
    state: State<'_, AppState>,
) -> Result<Option<ConnectionHandle>, String> {
    Ok(state.get(id))
}

#[command]
pub async fn unregister_connection(
    id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.remove(id);
    Ok(())
}
```

- [ ] **Step 3: 在 lib.rs 中注册命令**

在 `tauri::Builder` 的 `.invoke_handler(tauri::generate_handler![...])` 中：

```rust
.use(move || {
    use tauri_plugin_secure_store::SecureStore;
    SecureStore::new()
})()
.invoke_handler(tauri::generate_handler![
    // connection
    connection::list_connections,
    connection::save_connection,
    connection::register_connection,
    connection::get_connection,
    connection::unregister_connection,
])
```

- [ ] **Step 4: 添加 secure store 插件依赖**

`src-tauri/Cargo.toml`：
```toml
tauri-plugin-secure-store = "2"
```

- [ ] **Step 5: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/ src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(tauri): add connection management commands"
```

---

### Task 3: Schema 元数据命令（schema.rs）

**Files:**
- Create: `src-tauri/src/commands/schema.rs`

- [ ] **Step 1: 创建 schema.rs**

```rust
use crate::state::{AppState, ConnectionHandle};
use crate::types::{ColumnMeta, DatabaseInfo, TableInfo, TableSchema};
use db_core::DatabaseDriver;
use tauri::State;
use tauri::command;

/// 列出指定连接的所有数据库
#[command]
pub async fn list_databases(
    conn_id: &str,
    state: State<'_, AppState>,
) -> Result<Vec<DatabaseInfo>, String> {
    let handle = state.get(conn_id).ok_or("Connection not found")?;
    // TODO: 反序列化驱动实例，调用 trait 方法
    // Phase 2 驱动实现后接入
    Ok(vec![])
}

/// 列出数据库内所有 Schema
#[command]
pub async fn list_schemas(
    conn_id: &str,
    database: &str,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Ok(vec![])
}

/// 列出 Schema 内所有表
#[command]
pub async fn list_tables(
    conn_id: &str,
    database: &str,
    schema: Option<&str>,
    state: State<'_, AppState>,
) -> Result<Vec<TableInfo>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Ok(vec![])
}

/// 列出表的列
#[command]
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

/// 获取完整表 Schema
#[command]
pub async fn describe_table(
    conn_id: &str,
    database: &str,
    table: &str,
    state: State<'_, AppState>,
) -> Result<TableSchema, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Err("Not implemented: driver not ready".to_string())
}
```

- [ ] **Step 2: 在 commands/mod.rs 添加**

```rust
pub mod schema;
```

- [ ] **Step 3: 在 lib.rs 注册 handler**

```rust
schema::list_databases,
schema::list_schemas,
schema::list_tables,
schema::list_columns,
schema::describe_table,
```

- [ ] **Step 4: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/schema.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): add schema metadata commands (stubs)"
```

---

### Task 4: SQL 查询命令（query.rs）

**Files:**
- Create: `src-tauri/src/commands/query.rs`

- [ ] **Step 1: 创建 query.rs**

```rust
use crate::state::{AppState, ConnectionHandle};
use crate::types::QueryResult;
use tauri::State;
use tauri::command;

/// 执行单条 SQL
#[command]
pub async fn execute_sql(
    conn_id: &str,
    sql: &str,
    state: State<'_, AppState>,
) -> Result<QueryResult, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    // TODO: 驱动实现后接入
    Err("Not implemented: driver not ready".to_string())
}

/// 批量执行 SQL（事务内）
#[command]
pub async fn execute_batch(
    conn_id: &str,
    statements: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::types::ExecResult>, String> {
    let _handle = state.get(conn_id).ok_or("Connection not found")?;
    Err("Not implemented: driver not ready".to_string())
}
```

- [ ] **Step 2: 注册并检查**

```rust
// commands/mod.rs
pub mod query;

// lib.rs handler
query::execute_sql,
query::execute_batch,
```

```bash
cd src-tauri && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/query.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): add query execution commands (stubs)"
```

---

## Phase 2：Rust 后端 — 驱动实现（以 db-postgres 为第一个）

### Task 5: 实现 db-postgres DatabaseDriver

**Files:**
- Create: `src-tauri/crates/db-postgres/src/lib.rs`
- Modify: `src-tauri/crates/db-postgres/Cargo.toml`

- [ ] **Step 1: 更新 db-postgres/Cargo.toml**

确保 `db-postgres/Cargo.toml` 包含：
```toml
[package]
name = "db-postgres"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
db-core.workspace = true
async-trait.workspace = true
sqlx = { workspace = true, features = ["postgres", "json"] }
thiserror.workspace = true
```

- [ ] **Step 2: 实现 DatabaseDriver trait**

```rust
use async_trait::async_trait;
use db_core::{types::*, traits::DatabaseDriver, DbResult};
use sqlx::postgres::PgPoolOptions;

pub struct PostgresDriver {
    pool: sqlx::PgPool,
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized,
    {
        let url = config.to_url();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        Ok(Self { pool })
    }

    async fn ping(&self) -> DbResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        Ok(())
    }

    async fn close(&self) -> DbResult<()> {
        self.pool.close().await;
        Ok(())
    }

    async fn query(&self, sql: &str, _params: Vec<serde_json::Value>) -> DbResult<QueryResult> {
        let start = std::time::Instant::now();
        let rows = sqlx::query(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let columns = if let Some(first) = rows.first() {
            first
                .column_names()
                .iter()
                .map(|name| ColumnMeta {
                    name: name.to_string(),
                    data_type: "unknown".to_string(),
                    nullable: true,
                })
                .collect()
        } else {
            vec![]
        };

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(QueryResult {
            columns,
            rows: vec![],
            row_count: rows.len(),
            elapsed_ms: elapsed,
        })
    }

    async fn execute(&self, sql: &str, _params: Vec<serde_json::Value>) -> DbResult<ExecResult> {
        let start = std::time::Instant::now();
        let result = sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(ExecResult {
            rows_affected: result.rows_affected(),
            last_insert_id: None,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn execute_batch(&self, _statements: Vec<String>) -> DbResult<Vec<ExecResult>> {
        Err(DbError::Unsupported("execute_batch not yet implemented".to_string()))
    }

    async fn explain(&self, sql: &str) -> DbResult<String> {
        let row: (String,) = sqlx::query_as(&format!("EXPLAIN {}", sql))
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(row.0)
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT datname FROM pg_database WHERE datistemplate = false")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(name,)| DatabaseInfo {
                name,
                tables: vec![],
            })
            .collect())
    }

    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>> {
        let conn_str = format!("postgres://postgres:postgres@localhost:5432/{}", database);
        let db_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&conn_str)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT tablename FROM pg_tables WHERE schemaname = 'public'")
                .fetch_all(&db_pool)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn describe_table(
        &self,
        _database: &str,
        _table: &str,
    ) -> DbResult<TableSchema> {
        Err(DbError::Unsupported("describe_table not yet implemented".to_string()))
    }

    async fn get_columns(
        &self,
        _database: &str,
        _table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        Err(DbError::Unsupported("get_columns not yet implemented".to_string()))
    }
}
```

- [ ] **Step 3: 运行 `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db-postgres/src/lib.rs src-tauri/crates/db-postgres/Cargo.toml
git commit -m "feat(db-postgres): implement DatabaseDriver trait for PostgreSQL"
```

---

### Task 6: Rust 命令层集成 db-postgres

**Files:**
- Modify: `src-tauri/src/commands/connection.rs`
- Modify: `src-tauri/src/commands/schema.rs`
- Modify: `src-tauri/src/commands/query.rs`

- [ ] **Step 1: 修改 connection.rs — 实现 ping/save/connect**

```rust
use db_postgres::PostgresDriver;
use db_core::types::ConnectionConfig;
use crate::types::ConnectionConfig;

#[command]
pub async fn test_connection(config: ConnectionConfig) -> Result<(), String> {
    PostgresDriver::connect(&config)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
pub async fn ping_connection(
    conn_id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // TODO: Phase 3 实现驱动实例恢复
    Ok(())
}
```

- [ ] **Step 2: 修改 schema.rs — 接入真实驱动**

在 `list_databases` 中：
```rust
let handle = state.get(conn_id).ok_or("Connection not found")?;
// 此处需要反序列化驱动实例，Phase 3 处理
Ok(vec![])
```

- [ ] **Step 3: 修改 query.rs — 接入真实驱动**

在 `execute_sql` 中：
```rust
let driver = PostgresDriver::connect(&config).await.map_err(|e| e.to_string())?;
// 执行查询
```

- [ ] **Step 4: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/
git commit -m "feat(tauri): wire db-postgres into command layer"
```

---

## Phase 3：Rust 后端 — Secure Store + 驱动实例管理

### Task 7: 集成 Tauri Secure Store

**Files:**
- Modify: `src-tauri/src/commands/connection.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 添加 secure store 依赖**

```toml
tauri-plugin-secure-store = "2"
```

- [ ] **Step 2: 实现 save_connection（密码加密）**

```rust
use tauri_plugin_secure_store::SecureStore;

#[command]
pub async fn save_connection(
    config: ConnectionConfig,
    secure_store: State<'_, SecureStore>,
) -> Result<(), String> {
    // 写入密码到系统钥匙串
    let key = format!("databox:password:{}", config.id);
    secure_store
        .set(&key, &config.password)
        .await
        .map_err(|e| format!("Failed to save password: {}", e))?;

    // 保存不含密码的配置（TODO: 文件系统存储）
    tracing::info!("Connection saved: {}", config.id);
    Ok(())
}

#[command]
pub async fn load_password(
    id: &str,
    secure_store: State<'_, SecureStore>,
) -> Result<String, String> {
    let key = format!("databox:password:{}", id);
    secure_store
        .get(&key)
        .await
        .map_err(|e| format!("Failed to load password: {}", e))
}
```

- [ ] **Step 3: 在 lib.rs 注册 secure store**

```rust
.use(move || {
    tauri_plugin_secure_store::SecureStore::new()
})()
```

- [ ] **Step 4: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/connection.rs src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(tauri): integrate tauri-plugin-secure-store for password encryption"
```

---

### Task 8: 驱动实例缓存（ConnectionRegistry）

**Files:**
- Create: `src-tauri/src/connection_registry.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 connection_registry.rs**

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;
use db_core::types::ConnectionConfig;

pub struct ConnectionRegistry {
    instances: Mutex<HashMap<String, Arc<Mutex<RegistryEntry>>>>,
}

pub struct RegistryEntry {
    pub config: ConnectionConfig,
    // 使用 Box<dyn Any> 持有不同驱动实例
    pub driver: Mutex<Option<Arc<dyn std::any::Any + Send + Sync>>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(HashMap::new()),
        }
    }

    pub async fn register(
        &self,
        id: String,
        config: ConnectionConfig,
        driver: Arc<dyn std::any::Any + Send + Sync>,
    ) {
        let entry = RegistryEntry {
            config,
            driver: Mutex::new(Some(driver)),
        };
        self.instances.lock().unwrap().insert(id, Arc::new(Mutex::new(entry)));
    }

    pub async fn get(&self, id: &str) -> Option<Arc<Mutex<RegistryEntry>>> {
        self.instances.lock().unwrap().get(id).cloned()
    }

    pub async fn remove(&self, id: &str) -> Option<RegistryEntry> {
        let mut guard = self.instances.lock().unwrap();
        guard.remove(id).map(|arc| {
            let arc_inner = Arc::try_unwrap(arc).unwrap_or_else(|_| panic!("Still in use"));
            Arc::try_unwrap(arc_inner).unwrap_or_else(|_| panic!("Still locked"))
        })
    }
}
```

- [ ] **Step 2: 在 lib.rs 注册 ConnectionRegistry**

```rust
.manage(ConnectionRegistry::new())
```

- [ ] **Step 3: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/connection_registry.rs src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): add ConnectionRegistry for driver instance caching"
```

---

## Phase 4：前端 — 基础设施

### Task 9: 前端类型定义和 Tauri IPC 封装

**Files:**
- Create: `src/types/database.ts`
- Create: `src/composables/useTauriCommands.ts`

- [ ] **Step 1: 创建 types/database.ts**

```typescript
export interface ConnectionConfig {
  id: string
  name: string
  driver: DriverKind
  host: string
  port: number
  database: string
  username: string
  password: string // 内存中临时持有，不持久化
  ssl: boolean
  options: Record<string, string>
}

export type DriverKind = 'Postgres' | 'Mysql' | 'Sqlite' | 'Mongo' | 'Redis'

export interface TreeNode {
  id: string
  kind: 'connection' | 'database' | 'schema' | 'table' | 'column'
  name: string
  parentId: string | null
  children?: TreeNode[]
  // 节点特有
  icon?: string
  rowCount?: number
  dataType?: string
  nullable?: boolean
}

export interface ColumnMeta {
  name: string
  dataType: string
  nullable: boolean
}

export interface QueryResult {
  columns: ColumnMeta[]
  rows: Record<string, unknown>[]
  rowCount: number
  elapsedMs: number
}

export interface QueryHistoryItem {
  id: string
  sql: string
  elapsedMs: number
  timestamp: number
  connectionId: string
}

export interface ExecResult {
  rowsAffected: number
  lastInsertId?: number
  elapsedMs: number
}
```

- [ ] **Step 2: 创建 composables/useTauriCommands.ts**

```typescript
import { invoke } from '@tauri-apps/api/core'

export const tauriCommands = {
  // Connection
  async listConnections(): Promise<string[]> {
    return invoke<string[]>('list_connections')
  },

  async saveConnection(config: ConnectionConfig): Promise<void> {
    return invoke('save_connection', { config })
  },

  async testConnection(config: ConnectionConfig): Promise<void> {
    return invoke('test_connection', { config })
  },

  // Schema
  async listDatabases(connId: string): Promise<DatabaseInfo[]> {
    return invoke('list_databases', { connId })
  },

  async listTables(connId: string, database: string, schema?: string): Promise<TableInfo[]> {
    return invoke('list_tables', { connId, database, schema })
  },

  async listColumns(connId: string, database: string, schema: string, table: string): Promise<ColumnMeta[]> {
    return invoke('list_columns', { connId, database, schema, table })
  },

  // Query
  async executeSql(connId: string, sql: string): Promise<QueryResult> {
    return invoke('execute_sql', { connId, sql })
  },

  async executeBatch(connId: string, statements: string[]): Promise<ExecResult[]> {
    return invoke('execute_batch', { connId, statements })
  },
}
```

- [ ] **Step 3: Run type check**

```bash
pnpm type-check
```

- [ ] **Step 4: Commit**

```bash
git add src/types/database.ts src/composables/useTauriCommands.ts
git commit -m "feat(frontend): add type definitions and Tauri IPC composable"
```

---

### Task 10: Pinia stores — connections + query

**Files:**
- Create: `src/stores/connections.ts`
- Create: `src/stores/query.ts`

- [ ] **Step 1: 创建 stores/connections.ts**

```typescript
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ConnectionConfig, TreeNode, DriverKind } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

export const useConnectionsStore = defineStore('connections', () => {
  const connections = ref<ConnectionConfig[]>([])
  const activeConnectionId = ref<string | null>(null)
  const treeNodes = ref<TreeNode[]>([])
  const expandedNodes = ref<Set<string>>(new Set())
  const loading = ref(false)
  const error = ref<string | null>(null)

  const activeConnection = computed(() =>
    connections.value.find(c => c.id === activeConnectionId.value) || null
  )

  async function addConnection(config: ConnectionConfig): Promise<void> {
    try {
      await tauriCommands.saveConnection(config)
      connections.value.push(config)
    } catch (e) {
      error.value = `Failed to save connection: ${e}`
      throw e
    }
  }

  async function removeConnection(id: string): Promise<void> {
    // TODO: invoke unregister_connection
    connections.value = connections.value.filter(c => c.id !== id)
    if (activeConnectionId.value === id) {
      activeConnectionId.value = null
    }
  }

  async function connect(id: string): Promise<void> {
    try {
      const config = connections.value.find(c => c.id === id)
      if (!config) throw new Error('Connection not found')
      await tauriCommands.testConnection(config)
      activeConnectionId.value = id
      await loadTree(id)
    } catch (e) {
      error.value = `Connection failed: ${e}`
      throw e
    }
  }

  function disconnect(): void {
    activeConnectionId.value = null
    treeNodes.value = []
  }

  function setActive(id: string): void {
    activeConnectionId.value = id
  }

  function toggleExpand(nodeId: string): void {
    if (expandedNodes.value.has(nodeId)) {
      expandedNodes.value.delete(nodeId)
    } else {
      expandedNodes.value.add(nodeId)
    }
    expandedNodes.value = new Set(expandedNodes.value)
  }

  async function loadTree(connId: string): Promise<void> {
    loading.value = true
    try {
      const [databases] = await Promise.all([
        tauriCommands.listDatabases(connId),
      ])
      treeNodes.value = databases.map(db => ({
        id: `${connId}-${db.name}`,
        kind: 'database' as const,
        name: db.name,
        parentId: connId,
      }))
    } catch (e) {
      error.value = `Failed to load tree: ${e}`
    } finally {
      loading.value = false
    }
  }

  return {
    connections,
    activeConnectionId,
    activeConnection,
    treeNodes,
    expandedNodes,
    loading,
    error,
    addConnection,
    removeConnection,
    connect,
    disconnect,
    setActive,
    toggleExpand,
    loadTree,
  }
})
```

- [ ] **Step 2: 创建 stores/query.ts**

```typescript
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { QueryResult, QueryHistoryItem, ExecResult } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

export const useQueryStore = defineStore('query', () => {
  const editorContent = ref('')
  const queryHistory = ref<QueryHistoryItem[]>([])
  const currentResult = ref<QueryResult | null>(null)
  const multiResults = ref<Map<string, QueryResult>>(new Map())
  const isExecuting = ref(false)
  const error = ref<string | null>(null)

  function setEditorContent(sql: string): void {
    editorContent.value = sql
  }

  async function execute(connId: string, sql: string): Promise<QueryResult> {
    isExecuting.value = true
    error.value = null
    try {
      const result = await tauriCommands.executeSql(connId, sql)
      currentResult.value = result
      addToHistory(sql, result.elapsedMs)
      return result
    } catch (e) {
      error.value = `Query failed: ${e}`
      throw e
    } finally {
      isExecuting.value = false
    }
  }

  function addToHistory(sql: string, elapsedMs: number): void {
    const item: QueryHistoryItem = {
      id: crypto.randomUUID(),
      sql,
      elapsedMs,
      timestamp: Date.now(),
      connectionId: '',
    }
    queryHistory.value = [item, ...queryHistory.value].slice(0, 100)
  }

  return {
    editorContent,
    queryHistory,
    currentResult,
    multiResults,
    isExecuting,
    error,
    setEditorContent,
    execute,
    addToHistory,
  }
})
```

- [ ] **Step 3: Run type check**

```bash
pnpm type-check
```

- [ ] **Step 4: Commit**

```bash
git add src/stores/connections.ts src/stores/query.ts
git commit -m "feat(frontend): add Pinia stores for connections and query"
```

---

### Task 11: 安装 Monaco Editor 依赖

**Files:**
- Modify: `package.json`

- [ ] **Step 1: 安装依赖**

```bash
pnpm add @monaco-editor/loader
```

- [ ] **Step 2: 确认安装成功**

```bash
pnpm list @monaco-editor/loader
```

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore: add @monaco-editor/loader dependency"
```

---

## Phase 5：前端 — 连接树和对话框

### Task 12: ConnectionDialog 组件

**Files:**
- Create: `src/components/sidebar/ConnectionDialog.vue`

- [ ] **Step 1: 创建 ConnectionDialog.vue**

```vue
<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import {
  NModal,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NInputNumber,
  NSwitch,
  NButton,
  NSpin,
} from 'naive-ui'

const props = defineProps<{
  visible: boolean
  editingConfig?: ConnectionConfig | null
}>()

const emit = defineEmits<{
  updateVisible: [value: boolean]
  saved: [config: ConnectionConfig]
}>()

const formRef = ref()
const loading = ref(false)
const testing = ref(false)

const formModel = ref<Partial<ConnectionConfig>>({
  driver: 'Postgres',
  name: '',
  host: '',
  port: 5432,
  database: '',
  username: '',
  password: '',
  ssl: false,
})

const driverOptions = [
  { label: 'PostgreSQL', value: 'Postgres' },
  { label: 'MySQL', value: 'Mysql' },
  { label: 'SQLite', value: 'Sqlite' },
  { label: 'MongoDB', value: 'Mongo' },
  { label: 'Redis', value: 'Redis' },
]

const portByDriver: Record<DriverKind, number> = {
  Postgres: 5432,
  Mysql: 3306,
  Sqlite: 0,
  Mongo: 27017,
  Redis: 6379,
}

watch(() => props.visible, (visible) => {
  if (visible && props.editingConfig) {
    formModel.value = { ...props.editingConfig }
  } else if (visible) {
    formModel.value = { driver: 'Postgres', name: '', host: '', port: 5432, database: '', username: '', password: '', ssl: false }
  }
})

watch(() => formModel.value.driver, (driver) => {
  if (driver) {
    formModel.value.port = portByDriver[driver as DriverKind]
  }
})

async function testConnection(): Promise<void> {
  testing.value = true
  try {
    await invoke('test_connection', { config: formModel.value })
    // TODO: show success toast
  } catch (e) {
    // TODO: show error toast
  } finally {
    testing.value = false
  }
}

async function save(): Promise<void> {
  try {
    await formRef.value?.validate()
    loading.value = true
    const config: ConnectionConfig = {
      ...formModel.value,
      id: props.editingConfig?.id || crypto.randomUUID(),
    } as ConnectionConfig
    await emit('saved', config)
    emit('updateVisible', false)
  } catch (e) {
    // validation error
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <NModal
    :show="visible"
    @update:show="emit('updateVisible', $event)"
    preset="card"
    title="新建连接"
    style="width: 500px"
  >
    <NForm ref="formRef" :model="formModel" label-placement="left" label-width="80">
      <NFormItem label="名称" path="name" required>
        <NInput v-model:value="formModel.name" placeholder="My PostgreSQL" />
      </NFormItem>
      <NFormItem label="驱动" path="driver" required>
        <NSelect v-model:value="formModel.driver" :options="driverOptions" />
      </NFormItem>
      <NFormItem label="主机" path="host" required>
        <NInput v-model:value="formModel.host" placeholder="localhost" />
      </NFormItem>
      <NFormItem label="端口" path="port">
        <NInputNumber v-model:value="formModel.port" :min="1" :max="65535" />
      </NFormItem>
      <NFormItem label="数据库" path="database">
        <NInput v-model:value="formModel.database" placeholder="mydb" />
      </NFormItem>
      <NFormItem label="用户名" path="username">
        <NInput v-model:value="formModel.username" placeholder="postgres" />
      </NFormItem>
      <NFormItem label="密码" path="password">
        <NInput
          v-model:value="formModel.password"
          type="password"
          show-password-on="click"
          placeholder="••••••••"
        />
      </NFormItem>
      <NFormItem label="SSL">
        <NSwitch v-model:value="formModel.ssl" />
      </NFormItem>
    </NForm>
    <template #footer>
      <NButton @click="testConnection" :loading="testing">测试连接</NButton>
      <NButton type="primary" @click="save" :loading="loading">保存</NButton>
    </template>
  </NModal>
</template>
```

- [ ] **Step 2: 运行类型检查**

```bash
pnpm type-check
```

- [ ] **Step 3: Commit**

```bash
git add src/components/sidebar/ConnectionDialog.vue
git commit -m "feat(frontend): add ConnectionDialog component"
```

---

### Task 13: ConnectionTree 组件

**Files:**
- Create: `src/components/sidebar/ConnectionTree.vue`

- [ ] **Step 1: 创建 ConnectionTree.vue**

```vue
<script setup lang="ts">
import { computed } from 'vue'
import { NTree, NIcon, NButton, NSpin } from 'naive-ui'
import { DatabaseOutline, TableOutline, CubeOutline } from '@vicons/tabler'
import type { TreeOption } from 'naive-ui'
import { useConnectionsStore } from '@/stores/connections'

const store = useConnectionsStore()

const treeData = computed<TreeOption[]>(() =>
  store.treeNodes.map(node => ({
    key: node.id,
    label: node.name,
    isLeaf: node.kind === 'column',
    icon: () => {
      if (node.kind === 'connection') return h(DatabaseOutline)
      if (node.kind === 'database' || node.kind === 'schema') return h(CubeOutline)
      return h(TableOutline)
    },
  }))
)

function onExpand(keys: string[]) {
  store.expandedNodes = new Set(keys)
}

async function onSelect(keys: string[]) {
  if (keys.length === 0) return
  const nodeId = keys[0]
  // TODO: 点击列节点时展示列详情
}
</script>

<template>
  <div class="connection-tree">
    <div class="tree-header">
      <span>连接浏览器</span>
      <NButton text size="small" @click="/* TODO: emit open-dialog */">
        + 新建
      </NButton>
    </div>
    <NSpin :show="store.loading">
      <NTree
        :data="treeData"
        :expanded-keys="Array.from(store.expandedNodes)"
        :selected-keys="store.activeConnectionId ? [store.activeConnectionId] : []"
        selectable
        checkable={false}
        @update:expanded-keys="onExpand"
        @update:selected-keys="onSelect"
      />
    </NSpin>
  </div>
</template>

<style scoped>
.connection-tree {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.tree-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--n-border-color);
}
</style>
```

- [ ] **Step 2: 运行类型检查**

```bash
pnpm type-check
```

- [ ] **Step 3: Commit**

```bash
git add src/components/sidebar/ConnectionTree.vue
git commit -m "feat(frontend): add ConnectionTree component"
```

---

## Phase 6：前端 — SQL 编辑器和查询结果

### Task 14: SqlEditor 组件（Monaco Editor）

**Files:**
- Create: `src/components/editor/SqlEditor.vue`

- [ ] **Step 1: 创建 SqlEditor.vue**

```vue
<script setup lang="ts">
import { ref, shallowRef, onMounted, onBeforeUnmount, watch } from 'vue'
import { NButton, NTooltip, NIcon } from 'naive-ui'
import { PlayOutline, StopOutline, SaveOutline } from '@vicons/tabler'
import * as monaco from 'monaco-editor'

const props = defineProps<{
  modelValue: string
  connectionId?: string | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  execute: [sql: string]
  stop: []
}>()

const containerRef = ref<HTMLElement>()
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor>()
const isExecuting = ref(false)

onMounted(async () => {
  if (!containerRef.value) return

  editor.value = monaco.editor.create(containerRef.value, {
    value: props.modelValue,
    language: 'sql',
    theme: 'vs',
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    lineNumbers: 'on',
    automaticLayout: true,
    fontSize: 14,
  })

  editor.value.onDidChangeModelContent(() => {
    emit('update:modelValue', editor.value!.getValue())
  })

  editor.value.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
    const sql = editor.value!.getValue()
    if (sql.trim()) {
      emit('execute', sql)
    }
  })

  editor.value.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.Enter, () => {
    const sql = editor.value!.getValue()
    if (sql.trim()) {
      emit('execute', sql)
    }
  })
})

onBeforeUnmount(() => {
  editor.value?.dispose()
})

watch(() => props.modelValue, (newVal) => {
  if (editor.value && newVal !== editor.value.getValue()) {
    editor.value.setValue(newVal)
  }
})

function runQuery(): void {
  const sql = editor.value?.getValue() || ''
  if (sql.trim()) {
    emit('execute', sql)
  }
}

function stopQuery(): void {
  emit('stop')
}
</script>

<template>
  <div class="sql-editor">
    <div class="editor-toolbar">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" @click="runQuery" :loading="isExecuting" quaternary circle>
            <template #icon>
              <NIcon :component="PlayOutline" />
            </template>
          </NButton>
        </template>
        <span>运行 (Ctrl+Enter)</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary circle @click="stopQuery">
            <template #icon>
              <NIcon :component="StopOutline" />
            </template>
          </NButton>
        </template>
        <span>停止</span>
      </NTooltip>
    </div>
    <div ref="containerRef" class="editor-container" />
  </div>
</template>

<style scoped>
.sql-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  border-bottom: 1px solid var(--n-border-color);
}
.editor-toolbar {
  display: flex;
  gap: 4px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--n-border-color);
}
.editor-container {
  flex: 1;
  min-height: 0;
}
</style>
```

- [ ] **Step 2: 运行类型检查**

```bash
pnpm type-check
```

- [ ] **Step 3: 启动 dev server 确认编辑器渲染**

```bash
pnpm dev
```

- [ ] **Step 4: Commit**

```bash
git add src/components/editor/SqlEditor.vue
git commit -m "feat(frontend): add SqlEditor with Monaco Editor"
```

---

### Task 15: QueryResult 组件

**Files:**
- Create: `src/components/result/QueryResult.vue`

- [ ] **Step 1: 创建 QueryResult.vue**

```vue
<script setup lang="ts">
import { computed, ref, h } from 'vue'
import { NDataTable, NCard, NButton, NText } from 'naive-ui'
import type { DataTableColumns, DataTableRowKey } from 'naive-ui'
import type { ColumnMeta, QueryResult } from '@/types/database'

const props = defineProps<{
  result: QueryResult | null
}>()

const sortColumn = ref<string | null>(null)
const sortOrder = ref<'ascend' | 'descend' | null>(null)

const columns = computed<DataTableColumns<Record<string, unknown>>>(() => {
  if (!props.result) return []
  return props.result.columns.map(col => ({
    title: col.name,
    key: col.name,
    sorter: 'default',
    resizable: true,
    render: (row: Record<string, unknown>) => {
      const value = row[col.name]
      if (value === null || value === undefined) return 'NULL'
      return String(value)
    },
  }))
})

const data = computed(() => props.result?.rows || [])

function handleSort(a: Record<string, unknown>, b: Record<string, unknown>) {
  if (!sortColumn.value || !sortOrder.value) return 0
  const aVal = a[sortColumn.value]
  const bVal = b[sortColumn.value]
  if (aVal === bVal) return 0
  if (aVal === null || aVal === undefined) return 1
  if (bVal === null || bVal === undefined) return -1
  return aVal < bVal ? -1 : 1
}

function getSortedData() {
  if (!sortColumn.value || !sortOrder.value || !props.result) {
    return data.value
  }
  const sorted = [...data.value].sort((a, b) => {
    const aVal = a[sortColumn.value!]
    const bVal = b[sortColumn.value!]
    if (aVal === bVal) return 0
    if (sortOrder.value === 'ascend') {
      return aVal < bVal ? -1 : 1
    }
    return aVal < bVal ? 1 : -1
  })
  return sorted
}

function onCopyCell(row: Record<string, unknown>, col: ColumnMeta): void {
  const value = row[col.name]
  const text = value === null || value === undefined ? 'NULL' : String(value)
  navigator.clipboard.writeText(text)
}

function onCopyRow(row: Record<string, unknown>): void {
  const text = props.result?.columns.map(col => {
    const val = row[col.name]
    return val === null || val === undefined ? 'NULL' : String(val)
  }).join('\t') || ''
  navigator.clipboard.writeText(text)
}
</script>

<template>
  <div class="query-result">
    <div v-if="result" class="result-header">
      <NText depth="3">
        {{ result.rowCount }} 行 · 耗时 {{ result.elapsedMs }}ms
      </NText>
    </div>
    <NDataTable
      v-if="result"
      :columns="columns"
      :data="getSortedData()"
      :row-key="(row: Record<string, unknown>) => Math.random() as DataTableRowKey"
      :virtual-scroll="result.rowCount > 1000"
      :max-height="500"
      @cell-click="(row: Record<string, unknown>, col: ColumnMeta) => onCopyCell(row, col)"
    />
    <div v-else class="placeholder">
      执行查询以查看结果
    </div>
  </div>
</template>

<style scoped>
.query-result {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.result-header {
  padding: 4px 12px;
  border-bottom: 1px solid var(--n-border-color);
}
.placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--n-text-color-3);
}
</style>
```

- [ ] **Step 2: 运行类型检查**

```bash
pnpm type-check
```

- [ ] **Step 3: Commit**

```bash
git add src/components/result/QueryResult.vue
git commit -m "feat(frontend): add QueryResult table with sorting and copy"
```

---

## Phase 7：前端 — AppLayout 工具栏和组合

### Task 16: QueryToolbar 组件

**Files:**
- Create: `src/components/toolbar/QueryToolbar.vue`

- [ ] **Step 1: 创建 QueryToolbar.vue**

```vue
<script setup lang="ts">
import { computed } from 'vue'
import {
  NButton,
  NIcon,
  NSelect,
  NTooltip,
} from 'naive-ui'
import {
  PlayOutline,
  StopOutline,
  SaveOutline,
  HistoryOutline,
  SettingsOutline,
  AddCircleOutline,
} from '@vicons/tabler'
import { useConnectionsStore } from '@/stores/connections'

const props = defineProps<{
  connectionId?: string | null
  isExecuting?: boolean
}>()

const emit = defineEmits<{
  run: []
  stop: []
  save: []
  history: []
  settings: []
  newConnection: []
}>()

const store = useConnectionsStore()

const driverOptions = computed(() =>
  store.connections.map(c => ({
    label: c.name,
    value: c.id,
  }))
)
</script>

<template>
  <div class="query-toolbar">
    <div class="toolbar-group">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" @click="emit('run')" :loading="isExecuting" quaternary circle>
            <template #icon>
              <NIcon :component="PlayOutline" />
            </template>
          </NButton>
        </template>
        <span>运行</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary circle @click="emit('stop')">
            <template #icon>
              <NIcon :component="StopOutline" />
            </template>
          </NButton>
        </template>
        <span>停止</span>
      </NTooltip>
    </div>
    <div class="toolbar-group">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary circle @click="emit('save')">
            <template #icon>
              <NIcon :component="SaveOutline" />
            </template>
          </NButton>
        </template>
        <span>保存</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary circle @click="emit('history')">
            <template #icon>
              <NIcon :component="HistoryOutline" />
            </template>
          </NButton>
        </template>
        <span>执行历史</span>
      </NTooltip>
    </div>
    <div class="toolbar-group">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary circle @click="emit('newConnection')">
            <template #icon>
              <NIcon :component="AddCircleOutline" />
            </template>
          </NButton>
        </template>
        <span>新建连接</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary circle @click="emit('settings')">
            <template #icon>
              <NIcon :component="SettingsOutline" />
            </template>
          </NButton>
        </template>
        <span>设置</span>
      </NTooltip>
    </div>
  </div>
</template>

<style scoped>
.query-toolbar {
  display: flex;
  gap: 8px;
  padding: 4px 8px;
  align-items: center;
  height: 100%;
}
.toolbar-group {
  display: flex;
  gap: 2px;
}
</style>
```

- [ ] **Step 2: 运行类型检查**

```bash
pnpm type-check
```

- [ ] **Step 3: Commit**

```bash
git add src/components/toolbar/QueryToolbar.vue
git commit -m "feat(frontend): add QueryToolbar component"
```

---

### Task 17: 重写 AppLayout

**Files:**
- Modify: `src/components/layout/AppLayout.vue`

- [ ] **Step 1: 重写 AppLayout.vue**

```vue
<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import {
  NLayout,
  NLayoutHeader,
  NLayoutSider,
  NLayoutContent,
  NLayoutFooter,
  NSplit,
  NFlex,
  NButton,
  NIcon,
} from 'naive-ui'
import { AddCircleOutline } from '@vicons/tabler'
import { useConnectionsStore } from '@/stores/connections'
import { useQueryStore } from '@/stores/query'
import QueryToolbar from '@/components/toolbar/QueryToolbar.vue'
import ConnectionTree from '@/components/sidebar/ConnectionTree.vue'
import ConnectionDialog from '@/components/sidebar/ConnectionDialog.vue'
import SqlEditor from '@/components/editor/SqlEditor.vue'
import QueryResult from '@/components/result/QueryResult.vue'

const connectionsStore = useConnectionsStore()
const queryStore = useQueryStore()

const siderWidth = ref('300px')
const editorRatio = ref(0.5) // 编辑器高度占比
const showDialog = ref(false)
const editingConfig = ref<import('@/types/database').ConnectionConfig | null>(null)

onMounted(() => {
  // 初始化连接列表
  loadConnections()
})

async function loadConnections() {
  try {
    const ids = await tauriCommands.listConnections()
    // TODO: 通过 invoke 获取每个连接的详细信息
  } catch (e) {
    console.error('Failed to load connections:', e)
  }
}

function handleRun() {
  if (connectionsStore.activeConnectionId && queryStore.editorContent) {
    queryStore.execute(connectionsStore.activeConnectionId, queryStore.editorContent)
  }
}

function handleStop() {
  // TODO: implement stop
}

function handleNewConnection() {
  editingConfig.value = null
  showDialog.value = true
}

async function handleConnectionSaved(config: import('@/types/database').ConnectionConfig) {
  try {
    if (editingConfig.value) {
      await connectionsStore.updateConnection(config.id, config)
    } else {
      await connectionsStore.addConnection(config)
    }
    showDialog.value = false
  } catch (e) {
    console.error('Failed to save connection:', e)
  }
}
</script>

<template>
  <NLayout class="app-layout">
    <NLayoutHeader bordered class="toolbar-header">
      <NFlex justify="end" align="center" size="small" class="h-100%">
        <QueryToolbar
          :connection-id="connectionsStore.activeConnectionId"
          :is-executing="queryStore.isExecuting"
          @run="handleRun"
          @stop="handleStop"
          @new-connection="handleNewConnection"
        />
      </NFlex>
    </NLayoutHeader>

    <NLayout has-sider class="main-layout">
      <NSplit
        direction="horizontal"
        :default-size="siderWidth"
        :min-size="200"
        :max-size="500"
        @update:size="siderWidth = $event"
      >
        <template #1>
          <NLayoutSider bordered :width="siderWidth" :native-scrollbar="false">
            <ConnectionTree />
          </NLayoutSider>
        </template>
        <template #2>
          <NLayoutContent :native-scrollbar="false" class="main-content">
            <NSplit
              direction="vertical"
              :default-size="editorRatio"
              :min-size="0.2"
              :max-size="0.8"
            >
              <template #1>
                <div class="editor-section">
                  <SqlEditor
                    v-model="queryStore.editorContent"
                    :connection-id="connectionsStore.activeConnectionId"
                    @execute="(sql) => queryStore.execute(connectionsStore.activeConnectionId!, sql)"
                  />
                </div>
              </template>
              <template #2>
                <div class="result-section">
                  <QueryResult :result="queryStore.currentResult" />
                </div>
              </template>
            </NSplit>
          </NLayoutContent>
        </template>
      </NSplit>
    </NLayout>

    <NLayoutFooter bordered class="status-bar">
      <NFlex justify="space-between" align="center" size="small">
        <span v-if="connectionsStore.activeConnection">
          {{ connectionsStore.activeConnection.name }}
          | {{ connectionsStore.activeConnection.host }}:{{ connectionsStore.activeConnection.port }}
          | {{ connectionsStore.activeConnection.database }}
        </span>
        <span v-else>未连接</span>
        <span v-if="queryStore.currentResult">
          {{ queryStore.currentResult.rowCount }} 行 · {{ queryStore.currentResult.elapsedMs }}ms
        </span>
      </NFlex>
    </NLayoutFooter>
  </NLayout>

  <ConnectionDialog
    :visible="showDialog"
    :editing-config="editingConfig"
    @update:visible="showDialog = $event"
    @saved="handleConnectionSaved"
  />
</template>

<style scoped>
.app-layout {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
.toolbar-header {
  height: 40px;
}
.main-layout {
  flex: 1;
  min-height: 0;
}
.main-content {
  height: 100%;
  overflow: hidden;
}
.editor-section,
.result-section {
  height: 100%;
  overflow: hidden;
}
.status-bar {
  height: 28px;
  padding: 0 12px;
  font-size: 12px;
}
.h-100% {
  height: 100%;
}
</style>
```

- [ ] **Step 2: 运行类型检查和启动**

```bash
pnpm type-check
pnpm dev
```

- [ ] **Step 3: 浏览器确认布局渲染**

检查连接树、编辑器、结果区三栏布局是否正确显示

- [ ] **Step 4: Commit**

```bash
git add src/components/layout/AppLayout.vue
git commit -m "feat(frontend): rewrite AppLayout with DataGrip-style layout"
```

---

### Task 18: 删除模板代码

**Files:**
- Delete: `src/components/HelloWorld.vue`
- Delete: `src/components/TheWelcome.vue`
- Delete: `src/components/WelcomeItem.vue`
- Delete: `src/components/icons/IconCommunity.vue`
- Delete: `src/components/icons/IconDocumentation.vue`
- Delete: `src/components/icons/IconEcosystem.vue`
- Delete: `src/components/icons/IconSupport.vue`
- Delete: `src/components/icons/IconTooling.vue`
- Delete: `src/views/AboutView.vue`
- Delete: `src/views/HomeView.vue`
- Delete: `src/stores/counter.ts`

- [ ] **Step 1: 删除所有模板文件**

```bash
rm src/components/HelloWorld.vue src/components/TheWelcome.vue src/components/WelcomeItem.vue
rm src/components/icons/*.vue
rm src/views/AboutView.vue src/views/HomeView.vue
rm src/stores/counter.ts
```

- [ ] **Step 2: 清理 router（保留基本结构）**

```bash
# src/router/index.ts 保持存在，稍后可选简化
```

- [ ] **Step 3: 运行构建确认无引用**

```bash
pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove template boilerplate files"
```

---

## Phase 8：样式和打磨

### Task 19: UnoCSS 配置和主题

**Files:**
- Modify: `uno.config.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: 扩展 uno.config.ts**

```typescript
import { defineConfig, presetAttributify, presetIcons, presetUno } from 'unocss'

export default defineConfig({
  presets: [
    presetAttributify(),
    presetIcons({
      scale: 1.2,
      warn: true,
    }),
    presetUno(),
  ],
  shortcuts: {
    'h-100%': 'height: 100%',
    'min-h-0': 'min-height: 0',
    'flex-1': 'flex: 1',
  },
})
```

- [ ] **Step 2: Commit**

```bash
git add uno.config.ts
git commit -m "chore: extend UnoCSS with icon preset and shortcuts"
```

---

## Phase 9：Rust 驱动实现（优先 db-postgres）

### Task 20: 完成 db-postgres 核心方法

**Files:**
- Modify: `src-tauri/crates/db-postgres/src/lib.rs`

- [ ] **Step 1: 实现完整的 query 返回值映射**

填充 `query()` 方法中 DbValue → Row 的映射逻辑

- [ ] **Step 2: 实现 list_schemas**

```rust
async fn list_schemas(&self, _database: &str) -> DbResult<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('pg_catalog', 'information_schema')"
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| DbError::QueryFailed(e.to_string()))?;
    Ok(rows.into_iter().map(|(s,)| s).collect())
}
```

- [ ] **Step 3: 实现 get_columns**

```rust
async fn get_columns(&self, _database: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
    let rows = sqlx::query(
        "SELECT column_name, data_type, is_nullable FROM information_schema.columns WHERE table_name = $1"
    )
    .bind(table)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| DbError::QueryFailed(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row: sqlx::postgres::PgRow| ColumnMeta {
            name: row.get("column_name"),
            data_type: row.get("data_type"),
            nullable: row.get::<String, _>("is_nullable") == "YES",
        })
        .collect())
}
```

- [ ] **Step 4: Run `cargo check`**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db-postgres/src/lib.rs
git commit -m "feat(db-postgres): implement query value mapping and schema discovery"
```

---

## 验收标准

完成上述所有 Task 后：

1. ✅ `pnpm dev` 启动成功，浏览器打开应用
2. ✅ 左侧连接树可展开/折叠
3. ✅ 点击"新建连接"弹出表单，能填写 PostgreSQL 连接信息
4. ✅ Monaco Editor 渲染 SQL 编辑器
5. ✅ Ctrl+Enter 触发查询执行（即使结果为空，流程贯通）
6. ✅ 查询结果表格显示列名和数据
7. ✅ 连接密码不保存到文件（通过 secure store）
8. ✅ `cargo check` 无错误
9. ✅ `pnpm type-check` 无错误
10. ✅ `pnpm build` 成功

---

## 并行执行建议

以下 Task 可并行进行：
- **Task 9 + Task 11**：类型定义和依赖安装互相独立
- **Task 12 + Task 13**：ConnectionDialog 和 ConnectionTree 独立
- **Task 14 + Task 15**：SqlEditor 和 QueryResult 独立
- **Task 16 + Task 17**：QueryToolbar 和 AppLayout 在最终集成阶段合并
- **Phase 1 和 Phase 4** 可部分并行（Rust 基础 + 前端类型同时开发）
