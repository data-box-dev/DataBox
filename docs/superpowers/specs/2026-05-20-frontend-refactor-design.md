# DataGrip 风格前端重构设计

## 项目目标

重构 DataBox 前端，实现类似 DataGrip 的专业数据库管理界面。

- 完整树形连接浏览器（连接 → 数据库 → Schema → 表 → 列）
- 垂直拆分 SQL 编辑器 + 查询结果
- 支持列排序、单元格复制
- Tauri secure store 存储连接密码

## 整体布局

```
┌──────────────────────────────────────────────────────────┐
│  工具栏 [▶运行] [⏸停止] [💾保存] [历史] [连接+] [设置]   │  固定 40px
├──────────┬───────────────────────────────────────────────┤
│          │  ┌──────────────────────────────────────────┐ │
│ 连接树    │  │  SQL 编辑器（Monaco Editor）              │ │
│          │  │                                           │ │
│ 📁 连接1 │  │  SELECT * FROM users WHERE active = $1   │ │
│   📁 DB1 │  │  -- 多查询，分号分隔，可分别执行           │ │
│     📋表1│  └──────────────────────────────────────────┘ │
│       📊列│  ┌──────────────────────────────────────────┐ │
│   📁 DB2 │  │  查询结果（可排序表格）                   │ │
│     📋... │  │  ┌──┬──┬──┬──┬──┐  ← 拖拽调整高度       │ │
│ 📁 连接2 │  │  │id│nm│em│..│..│  ↑ 固定表头             │ │
│          │  │  ├──┼──┼──┼──┼──┤                        │ │
│          │  │  │..│..│..│..│..│                        │ │
│          │  │  └──┴──┴──┴──┴──┘                        │ │
│          │  └──────────────────────────────────────────┘ │
├──────────┴───────────────────────────────────────────────┤
│  状态栏：已连接 | 127.0.0.1:5432 | 耗时 23ms             │  固定 28px
└──────────────────────────────────────────────────────────┘
```

## 布局组件

**AppLayout.vue** — 三区块固定框架

- `n-layout-header` (40px)：工具栏
- `n-layout` with `n-split` (min="80px"|"300px")：连接树 | 主工作区
- `n-layout-footer` (28px)：状态栏

连接树与主工作区可通过拖拽调整宽度；编辑器与结果区通过 `n-split`（方向：horizontal）调整高度。

## 连接树组件

**ConnectionTree.vue** — 树形连接浏览器

```
📁 my-postgres  (展开状态)
  ├── postgres   (系统库, 折叠)
  ├── my_app     (展开)
  │     ├── public
  │     │     ├── users        ← 点击选中，右侧加载表结构
  │     │     └── posts
  │     └── audit
  └── templates  (折叠)
📁 my-mysql  (折叠)
  └── ...
```

数据结构：
```
type TreeNode =
  | { kind: "connection"; id: string; name: string; icon: string }
  | { kind: "database"; id: string; name: string; parentId: string }
  | { kind: "schema";  id: string; name: string; parentId: string }
  | { kind: "table";   id: string; name: string; parentId: string; rowCount?: number }
  | { kind: "column";  id: string; name: string; dataType: string; parentId: string }
```

点击列节点时，侧边栏展示列详情（类型、是否可空、默认值、注释）。

状态机：每个节点的 `expanded` 布尔值存储于 `connections store` 的 `expandedNodes: Map<string, boolean>`。

**ConnectionDialog.vue** — 新建/编辑连接模态框

- 驱动类型选择（PostgreSQL / MySQL / SQLite / MongoDB / Redis）
- 驱动特有字段动态渲染（host/port 对 SQL 型；file-path 对 SQLite；cluster-url 对 MongoDB）
- 密码框使用 Tauri `tauri-plugin-secure-store` 写入系统钥匙串，不落明文内存
- "测试连接" 按钮：执行 `ping()`，成功才允许保存

## SQL 编辑器

**SqlEditor.vue** — Monaco Editor 封装

- `@monaco-editor/loader` 懒加载，首屏不阻塞
- 配置：`language: "sql"`, `theme: "vs" | "vs-dark"`, `minimap: { enabled: false }`, `scrollBeyondLastLine: false`
- 行号、自动括号补全、SQL 关键字高亮
- 快捷键：`Ctrl+Enter` = 执行当前查询；`Ctrl+Shift+Enter` = 执行全部查询
- 查询以 `;` 分隔，执行时拆分为多条，分别显示结果标签
- 工具栏：驱动选择、执行按钮、停止按钮、格式化按钮

## 查询结果组件

**QueryResult.vue** — 可排序结果表格

- 固定表头（滚动时保持可见）
- 点击列头排序（升序/降序/默认，三态切换）
- 点击单元格 → `Ctrl+C` / `Cmd+C` 复制单元格文本
- 选中多行 → 复制为 TSV 格式（粘贴到 Excel/Google Sheets 可用）
- 右键菜单：复制值 / 复制行 / 复制列名
- 大结果集（>1000 行）：虚拟滚动（naive-ui `n-data-table` `virtual-scroll`）
- 列宽可拖拽调整

## 状态管理

### connections store

```ts
interface ConnectionState {
  connections: ConnectionConfig[]
  activeConnectionId: string | null
  expandedNodes: Map<string, boolean>
  // 连接缓存：已成功连接的驱动实例，避免重复握手
  driverCache: Map<string, DatabaseDriver | DocumentDriver | CacheDriver>

  // Actions
  addConnection(config: ConnectionConfig): Promise<void>
  removeConnection(id: string): Promise<void>
  updateConnection(id: string, updates: Partial<ConnectionConfig>): Promise<void>
  setActiveConnection(id: string): void
  toggleExpand(nodeId: string): void
  connect(id: string): Promise<void>
  disconnect(id: string): Promise<void>
}
```

### query store

```ts
interface QueryState {
  editorContent: string
  queryHistory: QueryHistoryItem[]      // 最多保留 100 条
  currentResult: QueryResult | null
  isExecuting: boolean
  // 多查询结果：查询内容 → 结果
  multiResults: Map<string, QueryResult>

  // Actions
  setEditorContent(sql: string): void
  execute(sql: string): Promise<QueryResult>
  executeBatch(sqls: string[]): Promise<QueryResult[]>
  stop(): Promise<void>
  addToHistory(sql: string, elapsedMs: number): void
}
```

## Tauri IPC 命令层

### commands/connection.rs

```rust
#[tauri::command]
async fn list_connections(state: State<'_, AppState>) -> Result<Vec<ConnectionConfig>, String>
#[tauri::command]
async fn save_connection(config: ConnectionConfig, secure_store: State<'_, SecureStore>) -> Result<(), String>
#[tauri::command]
async fn delete_connection(id: &str) -> Result<(), String>
#[tauri::command]
async fn get_password(service: &str, account: &str) -> Result<String, String>
#[tauri::command]
async fn set_password(service: &str, account: &str, password: &str) -> Result<(), String>
```

### commands/database.rs

```rust
#[tauri::command]
async fn connect(config: ConnectionConfig) -> Result<ConnectionHandle, String>
#[tauri::command]
async fn ping(handle: ConnectionHandle) -> Result<(), String>
#[tauri::command]
async fn disconnect(handle: ConnectionHandle) -> Result<(), String>
```

### commands/schema.rs

```rust
#[tauri::command]
async fn list_databases(handle: ConnectionHandle) -> Result<Vec<DatabaseInfo>, String>
#[tauri::command]
async fn list_schemas(handle: ConnectionHandle, database: &str) -> Result<Vec<String>, String>
#[tauri::command]
async fn list_tables(handle: ConnectionHandle, database: &str, schema: Option<&str>) -> Result<Vec<TableInfo>, String>
#[tauri::command]
async fn list_columns(handle: ConnectionHandle, database: &str, schema: Option<&str>, table: &str) -> Result<Vec<ColumnMeta>, String>
```

### commands/query.rs

```rust
#[tauri::command]
async fn execute(handle: ConnectionHandle, sql: &str, params: Vec<JsonValue>) -> Result<QueryResult, String>
#[tauri::command]
async fn execute_batch(handle: ConnectionHandle, statements: Vec<String>) -> Result<Vec<ExecResult>, String>
```

## Rust 后端改造

### 密码存储（Tauri Secure Store）

使用 `tauri-plugin-secure-store`（macOS Keychain / Windows Credential Manager / Linux Secret Service）。

```rust
// commands/connection.rs
use tauri_plugin_secure_store::SecureStore;

#[tauri::command]
async fn save_connection(
    config: ConnectionConfig,
    secure_store: State<'_, SecureStore>,
) -> Result<(), String> {
    // 密码写入钥匙串，不随 ConnectionConfig 一起序列化
    secure_store.set(&format!("db:password:{}", config.id), &config.password)
        .await
        .map_err(|e| e.to_string())?;

    // 保存不含密码的配置到本地存储
    let mut clean_config = config.clone();
    clean_config.password = String::new();
    save_config_locally(&clean_config)?;
    Ok(())
}
```

### Rust 驱动实例缓存（AppState）

连接句柄在 Rust 侧统一管理，避免每查询一次都建立新连接：

```rust
struct AppState {
    connections: Mutex<HashMap<String, Arc<Mutex<Box<dyn DatabaseDriver>>>>,
    secure_store: SecureStore,
}
```

---

## 技术选型

| 类别 | 选型 | 理由 |
|---|---|---|
| 代码编辑器 | `@monaco-editor/loader` | 功能最全，支持 SQL 语法高亮，DataGrip 同款体验 |
| 结果表格 | `n-data-table`（naive-ui） | 原生虚拟滚动 + 排序 + 单元格选中，零额外依赖 |
| 树形组件 | `n-tree`（naive-ui） | 图标、折叠、拖拽原生支持 |
| 对话框 | `n-modal` + `n-form`（naive-ui） | 内联表单验证 |
| 状态管理 | Pinia | 与现有依赖一致 |
| 密码存储 | `tauri-plugin-secure-store` | Tauri 官方插件，系统钥匙串 |
| Toast | `n-message`（naive-ui） | 同套组件 |

---

## 数据流

```
用户操作
   │
   ▼
Vue Component
   │  dispatch store action / 调用 useTauriCommands
   │
   ▼
Pinia Store / composable
   │  tauri.invoke("command_name", payload)
   │
   ▼
Tauri IPC ──▶ Rust Command
                   │
                   ▼
              Rust 驱动层
              (DatabaseDriver / DocumentDriver / CacheDriver)
                   │
                   ▼
              数据库
                   │
                   ▼
              Rust → 序列化 DbValue/QueryResult → JSON
                   │
                   ▼
              Tauri IPC ←── 返回 JSON
                   │
                   ▼
           Frontend Store ←── query/store 更新
                   │
                   ▼
              Component 重新渲染
```

## 文件变更清单

### 新增文件

```
src/
├── types/
│   └── database.ts              # 前端类型定义
├── composables/
│   └── useTauriCommands.ts      # Tauri IPC 封装
├── stores/
│   ├── connections.ts           # 连接管理 store
│   └── query.ts                 # 查询管理 store
├── components/
│   ├── toolbar/
│   │   └── QueryToolbar.vue
│   ├── sidebar/
│   │   ├── ConnectionTree.vue
│   │   └── ConnectionDialog.vue
│   ├── editor/
│   │   └── SqlEditor.vue
│   └── result/
│       └── QueryResult.vue

src-tauri/src/
├── commands/
│   ├── mod.rs
│   ├── connection.rs
│   ├── database.rs
│   ├── query.rs
│   └── schema.rs
└── state.rs                    # AppState 驱动缓存
```

### 保留并改造

```
src/components/layout/AppLayout.vue    ← 重写为三层固定框架
src-tauri/src/lib.rs                  ← 注册命令 + 初始化 AppState
```

### 删除（模板代码）

```
src/components/HelloWorld.vue
src/components/TheWelcome.vue
src/components/WelcomeItem.vue
src/components/icons/               ← 不再需要
src/views/AboutView.vue
src/views/HomeView.vue              ← 功能移至 AppLayout
src/stores/counter.ts               ← 无用
```

---
