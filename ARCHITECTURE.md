# DataBox 架构文档

本文档描述 DataBox 的完整架构设计，包括数据流、状态管理、驱动系统和安全模型。

## 1. 整体架构

### 1.1 分层结构

```
┌────────────────────────────────────────────────────┐
│                   前端（Vue 3）                     │
│  Components → Stores → Composables → Tauri IPC     │
├────────────────────────────────────────────────────┤
│              Tauri IPC Bridge                       │
│   type-safe JSON serialization / deserialization   │
├────────────────────────────────────────────────────┤
│                 Rust 后端                           │
│  Commands → State → Driver Layer → Database        │
└────────────────────────────────────────────────────┘
```

### 1.2 模块依赖关系

```
AppLayout
  ├─ QueryToolbar
  │    └─ emits: run, stop, new-connection
  ├─ ConnectionTree
  │    ├─ emits: expand, select, new-connection
  │    └─ uses: useConnectionsStore
  ├─ SqlEditor
  │    ├─ emits: execute, stop
  │    └─ v-model: editorContent
  ├─ QueryResult
  │    └─ uses: queryStore.currentResult
  ├─ ConnectionDialog
  │    ├─ emits: saved
  │    └─ uses: tauriCommands (testConnection, saveConnection)
  └─ uses:
       ├─ useConnectionsStore (tree, activeConnection)
       └─ useQueryStore (editorContent, execute)

useConnectionsStore
  ├─ State: connections, activeConnectionId, treeNodes, expandedNodes
  ├─ Actions: addConnection, removeConnection, connect, disconnect
  └─ uses: tauriCommands

tauriCommands (composable)
  ├─ Connection: listConnections, saveConnection, testConnection
  ├─ Config: loadConnections, saveConnections
  ├─ Schema: listDatabases, listTables, listColumns
  └─ Query: executeSql, executeBatch
```

## 2. 数据流详解

### 2.1 连接创建流程

```
用户在 ConnectionDialog 填写表单
  ↓
emit('saved', config)
  ↓
handleConnectionSaved()
  ↓
connectionsStore.addConnection(config)
  ↓
tauriCommands.saveConnection(config)
  ↓
invoke('save_connection', { config })
  ↓
Rust: connection::save_connection()
  ├─ secure_store.set("databox:password:{id}", password)
  ├─ save_connections([StoredConnection]) → connections.json
  ↓
前端: connections.push(config)
  ├─ 密码在内存中持有
  ├─ 配置保存到文件
  └─ 密码加密存储到系统钥匙串
```

### 2.2 查询执行流程

```
用户在 SqlEditor 输入 SQL
  ↓
Ctrl+Enter → emit('execute', sql)
  ↓
handleRun()
  ↓
queryStore.execute(connId, sql)
  ↓
tauriCommands.executeSql(connId, sql)
  ↓
invoke('execute_sql', { connId, sql })
  ↓
Rust: query::execute_sql()
  ├─ 从 ConnectionRegistry 获取驱动
  ├─ driver.query(sql, params)
  ├─ 映射 PgRow → DbValue → JSON
  ↓
返回 QueryResult
  ├─ columns: ColumnMeta[]
  ├─ rows: Record<string, unknown>[]
  ├─ row_count: number
  └─ elapsed_ms: number
  ↓
前端: queryStore.currentResult = result
  ↓
QueryResult 组件重新渲染表格
```

### 2.3 连接树加载流程

```
用户点击连接
  ↓
connectionsStore.connect(id)
  ├─ 从 Secure Store 加载密码
  ├─ 测试连接
  └─ 设置 activeConnectionId
  ↓
loadTree(connId)
  ↓
tauriCommands.listDatabases(connId)
  ↓
invoke('list_databases', { connId })
  ↓
Rust: schema::list_databases()
  ├─ 从 ConnectionRegistry 获取驱动
  ├─ driver.list_databases()
  └─ 返回 Vec<DatabaseInfo>
  ↓
前端: treeNodes = databases.map(...)
  ├─ database 节点
  ├─ table 子节点（按需加载）
  └─ column 子节点（按需加载）
  ↓
ConnectionTree 渲染树形
```

## 3. 状态管理

### 3.1 connections store

```typescript
interface ConnectionsState {
  // 数据
  connections: ConnectionConfig[]         // 所有连接配置
  activeConnectionId: string | null       // 当前活动连接
  treeNodes: TreeNode[]                   // 连接树节点
  expandedNodes: Set<string>              // 展开的节点
  loading: boolean                        // 加载状态
  error: string | null                    // 错误信息

  // 计算属性
  activeConnection: ConnectionConfig | null

  // Actions
  addConnection(config)                    // 添加连接
  removeConnection(id)                     // 移除连接
  updateConnection(id, updates)            // 更新连接
  connect(id)                              // 连接并加载树
  disconnect()                             // 断开连接
  setActive(id)                            // 设置活动连接
  toggleExpand(nodeId)                     // 切换节点展开
  loadTree(connId)                         // 加载连接树
  loadConnectionsList()                    // 从文件加载列表
  persistConnections()                     // 保存到文件
}
```

### 3.2 query store

```typescript
interface QueryState {
  // 数据
  editorContent: string                    // 编辑器内容
  queryHistory: QueryHistoryItem[]         // 查询历史
  currentResult: QueryResult | null        // 当前结果
  multiResults: Map<string, QueryResult>   // 多查询结果
  isExecuting: boolean                     // 执行状态
  error: string | null                     // 错误信息

  // Actions
  setEditorContent(sql)                    // 设置编辑器内容
  execute(connId, sql)                     // 执行查询
  executeBatch(connId, statements)         // 批量执行
  addToHistory(sql, elapsedMs)             // 添加到历史
  clearResult()                            // 清除结果
}
```

## 4. Rust 命令层

### 4.1 命令注册

```rust
.invoke_handler(tauri::generate_handler![
    // 连接管理
    connection::list_connections,
    connection::save_connection,
    connection::load_password,
    connection::test_connection,

    // 配置持久化
    config::save_connections,
    config::load_connections,

    // 数据库生命周期
    database::connect,
    database::ping,
    database::disconnect,

    // Schema 元数据
    schema::list_databases,
    schema::list_schemas,
    schema::list_tables,
    schema::list_columns,
    schema::describe_table,

    // 查询执行
    query::execute_sql,
    query::execute_batch,
])
```

### 4.2 状态管理

```rust
struct AppState {
    connections: Arc<Mutex<HashMap<String, ConnectionHandle>>>,
}

struct ConnectionRegistry {
    instances: Arc<Mutex<HashMap<String, Arc<Mutex<RegistryEntry>>>>>,
}

// Secure Store（插件）
struct SecureStore { ... }
```

## 5. 数据库驱动层

### 5.1 db-core trait

```rust
pub trait DatabaseDriver: Send + Sync {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    async fn ping(&self) -> DbResult<()>
    async fn close(&self) -> DbResult<()>
    async fn query(&self, sql: &str, params: Vec<Value>) -> DbResult<QueryResult>
    async fn execute(&self, sql: &str, params: Vec<Value>) -> DbResult<ExecResult>
    async fn execute_batch(&self, statements: Vec<String>) -> DbResult<Vec<ExecResult>>
    async fn explain(&self, sql: &str) -> DbResult<String>
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>>
    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>>
    async fn describe_table(&self, database: &str, table: &str) -> DbResult<TableSchema>
    async fn get_columns(&self, database: &str, table: &str) -> DbResult<Vec<ColumnMeta>>
}
```

### 5.2 类型定义

```rust
pub enum DbValue {
    Null, Bool(bool), Int(i64), Float(f64),
    Text(String), Bytes(Vec<u8>), Timestamp(DateTime<Utc>),
    Json(Value), Array(Vec<DbValue>),
}

pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Row>,              // Row = HashMap<String, DbValue>
    pub row_count: usize,
    pub elapsed_ms: u64,
}

pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub driver: DriverKind,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,            // 内存中，不持久化
    pub ssl: bool,
    pub options: HashMap<String, String>,
}
```

### 5.3 PostgreSQL 驱动实现

```rust
pub struct PostgresDriver {
    pool: sqlx::PgPool,
}

// 查询结果映射：PgRow → DbValue
fn pg_row_to_db_value(row: &PgRow, idx: usize) -> DbResult<DbValue> {
    let value_ref = row.try_get_raw(idx)?;
    if value_ref.is_null() { return Ok(DbValue::Null); }

    // 类型推断
    match (
        value_ref.try_decode::<bool>(),
        value_ref.try_decode::<i64>(),
        value_ref.try_decode::<f64>(),
        value_ref.try_decode::<String>(),
    ) {
        (Ok(v), ..) => Ok(DbValue::Bool(v)),
        (_, Ok(v), ..) => Ok(DbValue::Int(v)),
        (_, _, Ok(v), ..) => Ok(DbValue::Float(v)),
        (_, _, _, Ok(v)) => Ok(DbValue::Text(v)),
        _ => Err(DbError::TypeConversion { ... }),
    }
}
```

## 6. 安全模型

### 6.1 密码存储

```
密码生命周期：

1. 用户在 ConnectionDialog 输入
   ↓
2. 前端内存：ConnectionConfig.password
   ↓
3. invoke('save_connection', { config })
   ↓
4. Rust: SecureStore.set("databox:password:{id}", password)
   ↓
5. Rust: StoredConnection 保存到 connections.json（无密码）
   ↓
6. 前端：config.password 仍保留在内存（不持久化）
```

### 6.2 连接配置

```
connections.json 格式：
[
  {
    "id": "uuid-1",
    "name": "My PostgreSQL",
    "driver": "Postgres",
    "host": "localhost",
    "port": 5432,
    "database": "mydb",
    "username": "postgres",
    "ssl": false,
    "options": {}
  }
]
```

### 6.3 运行时密码加载

```typescript
// 连接时从 Secure Store 恢复密码
async function connect(id: string) {
  const config = connections.find(c => c.id === id)
  config.password = await tauriCommands.loadPassword(id)
  await tauriCommands.testConnection(config)
}
```

## 7. 组件生命周期

### 7.1 AppLayout 初始化

```
App.vue → AppLayout
  ↓
onMounted()
  ├─ connectionsStore.loadConnectionsList()
  │    └─ invoke('load_connections')
  │         └─ 读取 connections.json
  │              └─ 填充 connections[]
  │
  └─ 渲染
       ├─ QueryToolbar
       ├─ ConnectionTree（空）
       ├─ SqlEditor（空）
       └─ QueryResult（空）
```

### 7.2 连接创建

```
ConnectionDialog.saved
  ↓
handleConnectionSaved()
  ├─ connectionsStore.addConnection(config)
  │    ├─ tauriCommands.saveConnection(config)
  │    │    ├─ secure_store.set("databox:password:{id}", password)
  │    │    └─ save_connections([StoredConnection])
  │    │         └─ 写入 connections.json
  │    └─ connections.push(config)
  └─ showDialog = false
```

### 7.3 连接激活

```
ConnectionTree.select
  ↓
connectionsStore.setActive(id)
  ↓
connectionsStore.connect(id)
  ├─ loadPassword(id) → 恢复密码
  ├─ testConnection(config) → ping 验证
  ├─ activeConnectionId = id
  └─ loadTree(id)
       └─ invoke('list_databases', { connId })
            └─ 填充 treeNodes[]
```

### 7.4 查询执行

```
SqlEditor.execute
  ↓
queryStore.execute(connId, sql)
  ├─ tauriCommands.executeSql(connId, sql)
  │    ├─ invoke('execute_sql', { connId, sql })
  │    │    └─ Rust: driver.query(sql) → QueryResult
  │    └─ currentResult = result
  └─ addToHistory(sql, elapsedMs)
```

## 8. 错误处理

### 8.1 错误流

```
Rust 错误
  ↓
DbError → String（通过 #[tauri::command] 序列化）
  ↓
Tauri IPC
  ↓
前端: Promise rejection
  ↓
store.error = error message
  ↓
Component: error.value → UI 显示
```

### 8.2 错误类型（Rust）

```rust
pub enum DbError {
    ConnectionFailed(String),
    ConnectionNotFound { id: String },
    ConnectionTimeout,
    QueryFailed(String),
    SyntaxError(String),
    QueryTimeout { timeout_secs: u64 },
    TypeConversion { column: String, target_type: String },
    ColumnNotFound(String),
    PermissionDenied(String),
    AuthFailed,
    Serialization(String),
    Unsupported(String),
    Internal(String),
}
```

## 9. 性能考虑

### 9.1 前端

- **Pinia store 选择性订阅**：组件只订阅需要的 state
- **n-data-table 虚拟滚动**：>1000 行时自动启用
- **Monaco Editor 懒加载**：首屏不阻塞

### 9.2 Rust

- **连接池**：PgPoolOptions.max_connections(5)
- **驱动实例缓存**：ConnectionRegistry 避免重复握手
- **批量查询**：execute_batch 使用事务

## 10. 扩展指南

### 10.1 添加新数据库驱动

1. 创建 `src-tauri/crates/db-{name}/src/lib.rs`
2. 实现 `DatabaseDriver` trait
3. 在 `commands/database.rs` 的 `create_driver()` 中添加匹配分支
4. 在 `Cargo.toml` [workspace.dependencies] 中添加成员
5. 在 `src-tauri/Cargo.toml` [dependencies] 中添加

参考：`src-tauri/crates/db-postgres/src/lib.rs`

### 10.2 添加新命令

1. 在 `src-tauri/src/commands/` 创建命令模块
2. 在 `commands/mod.rs` 添加 `pub mod`
3. 在 `lib.rs` 的 `invoke_handler!` 中注册

---

**最后更新**: 2026-05-20  
**版本**: 0.1.0  
**状态**: 前端框架就绪，PostgreSQL 驱动实现
