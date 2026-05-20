# DataBox Development Guide

本指南描述如何为 DataBox 开发新功能、添加数据库驱动和进行调试。

## 开发工作流

### 前置要求

- **Rust**：1.77+ (`rustup`)
- **Node.js**：18+ (用于前端构建)
- **Tauri CLI**：2.x (`npm install -g @tauri-apps/cli`)

### 环境检查

```bash
# 检查 Rust 工具链
rustc --version
cargo --version

# 检查 Node.js 和 npm
node --version
npm --version

# 检查 Tauri CLI
cargo tauri --version
```

### 启动开发服务器

```bash
# 1. 安装依赖
npm install

# 2. 启动前端开发服务器
npm run dev
# 或使用 Tauri 开发命令
cargo tauri dev

# 3. 新开终端，检查类型
npm run type-check
```

### 构建生产版本

```bash
# 前端构建
npm run build

# Tauri 应用构建
cargo tauri build
```

## 项目结构

```
src/
├── types/               # TypeScript 类型定义
├── composables/         # Vue Composables（业务逻辑复用）
├── stores/              # Pinia Stores（状态管理）
├── components/          # Vue 组件
│   ├── layout/          # 布局组件
│   ├── toolbar/         # 工具栏组件
│   ├── sidebar/         # 侧边栏组件
│   ├── editor/          # 编辑器组件
│   └── result/          # 结果展示组件
└── router/              # Vue Router 配置

src-tauri/src/
├── state.rs             # Tauri State 管理
├── connection_registry.rs  # 驱动实例缓存
├── commands/            # Tauri Commands（IPC）
│   ├── connection.rs    # 连接管理
│   ├── config.rs        # 配置持久化
│   ├── database.rs      # 数据库生命周期
│   ├── schema.rs        # Schema 元数据
│   └── query.rs         # SQL 执行
└── crates/              # 数据库驱动
    ├── db-core/         # 核心抽象层
    ├── db-postgres/     # PostgreSQL 驱动（完整实现）
    ├── db-sqlite/       # SQLite 驱动（完整实现 + 测试）
    ├── db-mysql/        # MySQL 驱动（完整实现）
    ├── db-mongo/        # MongoDB 驱动（完整实现）
    └── db-redis/        # Redis 驱动（完整实现）
```

## 添加新数据库驱动

### 1. 创建驱动 crate

```bash
# 创建驱动目录
mkdir -p src-tauri/crates/db-{driver-name}/src

# 创建 Cargo.toml
cat > src-tauri/crates/db-{driver-name}/Cargo.toml << 'EOF'
[package]
name = "db-{driver-name}"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
db-core.workspace = true
async-trait.workspace = true
# 数据库客户端库
thiserror.workspace = true
EOF
```

### 2. 实现 DatabaseDriver trait

参考 `src-tauri/crates/db-postgres/src/lib.rs` 或 `src-tauri/crates/db-sqlite/src/lib.rs`

```rust
use async_trait::async_trait;
use db_core::{
    traits::DatabaseDriver,
    types::*,
    DbResult,
};

pub struct {DriverName}Driver {
    // 驱动实例字段
}

#[async_trait]
impl DatabaseDriver for {DriverName}Driver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self> { ... }
    async fn ping(&self) -> DbResult<()> { ... }
    async fn close(&self) -> DbResult<()> { ... }
    async fn query(&self, sql: &str, params: Vec<Value>) -> DbResult<QueryResult> { ... }
    async fn execute(&self, sql: &str, params: Vec<Value>) -> DbResult<ExecResult> { ... }
    async fn execute_batch(&self, stmts: Vec<String>) -> DbResult<Vec<ExecResult>> { ... }
    async fn explain(&self, sql: &str) -> DbResult<String> { ... }
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>> { ... }
    async fn list_tables(&self, db: &str) -> DbResult<Vec<String>> { ... }
    async fn describe_table(&self, db: &str, table: &str) -> DbResult<TableSchema> { ... }
    async fn get_columns(&self, db: &str, table: &str) -> DbResult<Vec<ColumnMeta>> { ... }
}
```

### 3. 添加驱动工厂

编辑 `src-tauri/src/commands/database.rs`：

```rust
pub async fn create_driver(config: &ConnectionConfig) -> Result<Box<dyn DatabaseDriver>, String> {
    match config.driver {
        db_core::types::DriverKind::Postgres => {
            let driver = PostgresDriver::connect(config).await.map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        db_core::types::DriverKind::Sqlite => {
            let driver = SqliteDriver::connect(config).await.map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        db_core::types::DriverKind::{YourDriver} => {
            let driver = {YourDriver}::connect(config).await.map_err(|e| e.to_string())?;
            Ok(Box::new(driver))
        }
        _ => Err(format!("Driver {:?} not yet implemented", config.driver)),
    }
}
```

### 4. 注册到 Tauri 命令

编辑 `src-tauri/src/commands/database.rs`：

```rust
// 在命令开头添加
use db_{driver_name}::{YourDriver}Driver;
```

### 5. 添加到工作空间

编辑 `src-tauri/Cargo.toml`：

```toml
[workspace.dependencies]
# ... 现有依赖

[workspace.members]
# ... 现有成员
"crates/db-{driver-name}",

[dependencies]
# ... 现有依赖
db-{driver-name} = { path = "crates/db-{driver-name}" }
```

### 6. 更新前端驱动选择

编辑 `src/components/sidebar/ConnectionDialog.vue`：

```typescript
const driverOptions = [
  { label: 'PostgreSQL', value: 'Postgres' },
  { label: 'MySQL', value: 'Mysql' },
  { label: 'SQLite', value: 'Sqlite' },
  { label: 'Your DB', value: 'YourDriver' },  // 新增
]
```

## 添加新的 Tauri 命令

### 1. 创建命令文件

```rust
// src-tauri/src/commands/mycommand.rs
use tauri::State;

#[tauri::command]
pub async fn my_new_command(
    param: &str,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 实现逻辑
    Ok("result".to_string())
}
```

### 2. 声明模块

```rust
// src-tauri/src/commands/mod.rs
pub mod mycommand;
```

### 3. 注册命令

```rust
// src-tauri/src/lib.rs
.invoke_handler(tauri::generate_handler![
    // ... 现有命令
    mycommand::my_new_command,
])
```

## 调试技巧

### 前端调试

```bash
# 启动开发服务器
npm run dev

# 打开浏览器开发者工具（F12）
# Vue Devtools 已集成
```

### Rust 调试

```bash
# 调试模式构建
cargo build

# 带调试符号运行
RUST_LOG=debug cargo tauri dev

# 使用 GDB/LLDB
cargo tauri dev --debugger
```

### Tauri 日志

```rust
// 在 Rust 代码中使用 tracing
tracing::info!("Connection created: {}", config.id);
tracing::error!("Failed to connect: {}", e);
tracing::debug!("Query: {}", sql);
```

### 数据库查询调试

```bash
# 启用 SQL 日志
RUST_LOG=sqlx=debug,cargo=trace

# 查看完整 SQL 执行情况
RUST_LOG=debug
```

## 测试

### 前端测试

```bash
# 类型检查
npm run type-check

# 构建验证
npm run build

# 代码检查
npm run lint
```

### Rust 测试

```bash
# 运行所有测试
cargo test

# 运行特定 crate 测试
cargo test -p db-postgres

# 运行特定测试
cargo test test_sqlite_connect

# 带日志输出
RUST_LOG=debug cargo test
```

### 集成测试

```bash
# 启动应用
cargo tauri dev

# 手动测试：
# 1. 创建新连接
# 2. 执行查询
# 3. 查看结果
# 4. 验证错误处理
```

## 代码规范

### Rust

- **格式化**：`cargo fmt`
- **Linter**：`cargo clippy`
- **测试**：`cargo test`
- **提交信息**：Conventional Commits

### TypeScript/Vue

- **格式化**：`npm run format` (Prettier)
- **类型检查**：`npm run type-check`
- **代码规范**：`npm run lint`
- **组件风格**：组合式 API + `<script setup>`

### Git 工作流

```bash
# 1. 创建功能分支
git checkout -b feat/my-feature

# 2. 开发 + 测试
npm run type-check
cargo test

# 3. 提交
git commit -m "feat: add my feature"

# 4. 推送 PR
git push origin feat/my-feature
```

## 性能优化

### 前端

- **按需加载**：Monaco Editor 懒加载
- **虚拟滚动**：n-data-table >1000 行时启用
- **Pinia 选择性订阅**：组件只订阅需要的 state

### Rust

- **连接池**：PgPoolOptions.max_connections(5)
- **驱动实例缓存**：ConnectionRegistry
- **批量查询**：execute_batch 使用事务
- **零拷贝序列化**：DbValue → JSON 最小化复制

## 常见问题

### Q: 如何添加新的连接配置字段？

1. 编辑 `src/types/database.ts`：更新 `ConnectionConfig`
2. 编辑 `src-tauri/crates/db-core/src/types.rs`：更新 `ConnectionConfig`
3. 编辑 `src/components/sidebar/ConnectionDialog.vue`：添加表单字段
4. 更新 `StoredConnection`（`src-tauri/src/commands/config.rs`）

### Q: 如何处理大结果集？

- 前端：`n-data-table` 自动启用虚拟滚动（>1000 行）
- Rust：QueryResult 已包含 `row_count` 字段

### Q: 如何添加查询历史 UI？

1. 在 `QueryToolbar.vue` 添加"历史"按钮
2. 创建 `src/components/sidebar/QueryHistory.vue`
3. 从 `queryStore.queryHistory` 获取数据
4. 点击历史条目：填充到编辑器

### Q: 如何实现驱动自动发现？

1. 在 `db-core` 添加 `DriverFactory` trait
2. 各驱动实现 `DriverFactory`
3. 主应用通过 entrypoints 动态加载

## 资源

- **Tauri 文档**：https://tauri.app/
- **Vue 3 文档**：https://vuejs.org/
- **naive-ui 文档**：https://www.naiveui.com/
- **sqlx 文档**：https://github.com/launchbadge/sqlx
- **Rust 异步编程**：https://rust-lang.github.io/async-book/

---

**最后更新**：2026-05-20  
**维护者**：DataBox 开发团队
