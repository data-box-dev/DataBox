# DataBox — DataGrip 风格数据库管理工具

DataBox 是一个基于 **Tauri 2 + Vue 3** 构建的跨平台数据库管理工具，提供类似 DataGrip 的专业数据库管理体验。

## ✨ 核心特性

### 🔐 安全连接管理
- 支持 PostgreSQL / MySQL / SQLite / MongoDB / Redis
- 连接配置持久化到本地（connections.json）
- 密码通过 **Tauri Secure Store** 加密存储到系统钥匙串
- 连接树形浏览器：连接 → 数据库 → Schema → 表 → 列

### 🛠️ 查询编辑器
- SQL 编辑器（当前 textarea，Monaco Editor 待集成）
- Ctrl+Enter 快速执行查询
- 多查询支持（分号分隔）
- 查询历史记录（保留最近 100 条）

### 📊 结果展示
- 可排序结果表格（点击列头升序/降序/默认）
- 单元格复制（Ctrl+C）
- 多行复制为 TSV（可直接粘贴到 Excel/Google Sheets）
- 大结果集虚拟滚动（>1000 行）

### 🎨 DataGrip 风格布局
```
┌─────────────────────────────────────────┐
│ 工具栏 [运行] [停止] [保存] [历史] [+]  │  40px
├──────┬──────────────────────────────────┤
│      │  SQL 编辑器                      │
│ 连接  │                                  │
│ 树    │  ┌────────────────────────────┐ │
│      │  │ 查询结果表格                │ │
├──────┴──────────────────────────────────┤
│ 状态栏: 127.0.0.1:5432 | 23ms          │  28px
└─────────────────────────────────────────┘
```

### 🔐 安全连接
- **密码加密存储**：Tauri Secure Store → 系统钥匙串
- **配置持久化**：connections.json（不含密码）
- **支持数据库**：PostgreSQL ✅ | MySQL ✅ | SQLite ✅ | MongoDB ⏳ | Redis ⏳

## 🏗️ 架构

### 前端（Vue 3 + Pinia + naive-ui）

```
src/
├── types/database.ts              # 统一类型定义
├── composables/useTauriCommands.ts # Tauri IPC 封装
├── stores/
│   ├── connections.ts             # 连接生命周期管理
│   └── query.ts                   # 查询执行和历史
└── components/
    ├── layout/AppLayout.vue       # DataGrip 风格主布局
    ├── toolbar/QueryToolbar.vue   # 顶部工具栏
    ├── sidebar/
    │   ├── ConnectionTree.vue     # 连接树浏览器
    │   └── ConnectionDialog.vue   # 连接配置对话框
    ├── editor/SqlEditor.vue       # SQL 编辑器
    └── result/QueryResult.vue     # 查询结果表格
```

### Rust 后端（Tauri + 多驱动架构）

```
src-tauri/src/
├── state.rs                      # AppState + ConnectionHandle
├── connection_registry.rs        # 驱动实例缓存
├── commands/
│   ├── connection.rs             # 连接管理（Secure Store）
│   ├── config.rs                 # 配置持久化
│   ├── database.rs               # 数据库生命周期
│   ├── schema.rs                 # Schema 元数据查询
│   └── query.rs                  # SQL 执行
└── crates/                       # 数据库驱动
    ├── db-core/                  # 核心抽象层（trait + 类型）
    ├── db-postgres/              # PostgreSQL 驱动（已实现）
    ├── db-mysql/                 # MySQL 驱动（待实现）
    ├── db-sqlite/                # SQLite 驱动（待实现）
    ├── db-mongo/                 # MongoDB 驱动（待实现）
    └── db-redis/                 # Redis 驱动（待实现）
```

### 核心 Trait（db-core）

```rust
// 关系型数据库（PostgreSQL / MySQL / SQLite）
pub trait DatabaseDriver {
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    async fn query(&self, sql: &str, params: Vec<Value>) -> DbResult<QueryResult>
    async fn execute(&self, sql: &str, params: Vec<Value>) -> DbResult<ExecResult>
    async fn list_databases(&self) -> DbResult<Vec<DatabaseInfo>>
    async fn list_tables(&self, database: &str) -> DbResult<Vec<String>>
    async fn describe_table(&self, database: &str, table: &str) -> DbResult<TableSchema>
}

// 文档型数据库（MongoDB）
pub trait DocumentDriver { /* find/insert/update/delete/aggregate */ }

// 缓存数据库（Redis）
pub trait CacheDriver { /* get/set/hget/hset/lpush/sadd/zrange */ }
```

### 数据流

```
用户操作
   ↓
Vue Component
   ↓
Pinia Store / composable
   ↓  tauri.invoke()
   ↓
Tauri IPC
   ↓
Rust Command
   ↓
DatabaseDriver (db-postgres / db-mysql / ...)
   ↓
数据库
   ↓
DbValue / QueryResult → JSON
   ↓
Tauri IPC ← 返回 JSON
   ↓
Frontend Store → 重新渲染
```

## 🚀 快速开始

### 开发环境

```bash
# 安装依赖
npm install

# 前端开发
npm run dev

# 类型检查
npm run type-check

# 构建生产版本
npm run build
```

### Rust 后端

```bash
cd src-tauri

# 检查编译
cargo check

# 运行测试
cargo test

# 构建 Tauri 应用
cargo tauri build
```

### 数据库驱动

当前已实现：**PostgreSQL**（`db-postgres`）

待实现：MySQL、SQLite、MongoDB、Redis

驱动架构参考：`src-tauri/crates/db-postgres/src/lib.rs`

## 🔐 密码管理

DataBox 采用双层存储策略：

1. **连接配置**（不含密码）→ 本地 JSON 文件
   ```
   ~/Library/Application Support/com.tauri.dev/DataBox/connections.json
   ```

2. **连接密码** → 系统钥匙串
   - macOS：Keychain
   - Windows：Credential Manager
   - Linux：Secret Service

密码生命周期：
```
用户输入密码 → 前端内存
     ↓
tauri.invoke('save_connection')
     ↓
SecureStore.set("databox:password:{id}", password)
     ↓
保存不含密码的配置到文件
     ↓
密码从内存清除
```

## 📝 待办事项

### 高优先级
- [ ] **Monaco Editor** 集成：替换 SqlEditor 中的 textarea
- [ ] **MySQL / SQLite 驱动**：参考 db-postgres 实现
- [ ] **驱动实例序列化**：解决 ConnectionRegistry 中驱动恢复问题
- [ ] **连接测试 UI**：ConnectionDialog 中的测试连接功能

### 中优先级
- [ ] **查询历史 UI**：工具栏"历史"按钮的交互
- [ ] **MongoDB / Redis 驱动**：DocumentDriver + CacheDriver 实现
- [ ] **错误处理优化**：统一错误提示和恢复机制
- [ ] **连接分组**：连接树的文件夹功能

### 低优先级
- [ ] **深色主题优化**：当前使用 naive-ui 默认主题
- [ ] **快捷键自定义**：编辑器快捷键配置
- [ ] **查询格式化**：SQL 格式化按钮
- [ ] **导出 CSV/JSON**：查询结果导出功能

## 🧪 测试

```bash
# 前端类型检查
npm run type-check

# Rust 单元测试
cd src-tauri && cargo test

# Rust 特定驱动测试
cd src-tauri && cargo test -p db-postgres
```

## 📚 技术栈

| 类别 | 技术选型 | 理由 |
|---|---|---|
| **前端框架** | Vue 3 + TypeScript | 类型安全、组合式 API |
| **UI 库** | naive-ui | 全功能组件库，Tree/Table/Modal 原生支持 |
| **状态管理** | Pinia | 类型安全、Vue 官方推荐 |
| **代码编辑器** | Monaco Editor（待集成） | DataGrip 同款、SQL 语法高亮 |
| **桌面框架** | Tauri 2 | Rust 后端、小体积、安全性 |
| **数据库驱动** | sqlx / mongodb / redis | Rust 异步生态、类型安全 |
| **密码存储** | tauri-plugin-secure-store | 系统钥匙串、跨平台 |

## 🤝 贡献

### 代码风格

- **Rust**：遵循 rustfmt + clippy
- **TypeScript**：严格模式、2 空格缩进
- **Vue**：组合式 API + `<script setup>` + TypeScript
- **提交信息**：Conventional Commits（feat: / fix: / chore: 等）

### 开发流程

```bash
# 1. 创建功能分支
git checkout -b feat/my-feature

# 2. 开发 + 测试
npm run dev && npm run type-check

# 3. 提交
git commit -m "feat: add my feature"

# 4. 推送 PR
git push origin feat/my-feature
```

## 📄 许可证

[Add your license here]

## 🔗 参考

- **Tauri 文档**：https://tauri.app/
- **Vue 3 文档**：https://vuejs.org/
- **naive-ui 文档**：https://www.naiveui.com/
- **sqlx 文档**：https://github.com/launchbadge/sqlx
- **DataGrip**：https://www.jetbrains.com/datagrip/

---

**版本**: 0.1.0  
**状态**: 前端框架就绪，PostgreSQL 驱动实现，Monaco Editor 待集成
