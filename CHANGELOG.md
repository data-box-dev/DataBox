# Changelog

All notable changes to DataBox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-05-20

### Added

#### 前端（Vue 3 + naive-ui）
- **DataGrip 风格布局**：三区块固定框架（工具栏 40px + 主工作区 + 状态栏 28px）
- **连接树形浏览器**：五级树形结构（连接 → 数据库 → Schema → 表 → 列）
- **连接配置对话框**：驱动选择、主机/端口/数据库/用户名/密码/SSL 配置
- **连接配置持久化**：StoredConnection 保存到本地 JSON 文件
- **密码加密存储**：通过 Tauri Secure Store 写入系统钥匙串
- **SQL 编辑器**：textarea + Ctrl+Enter 执行（Monaco Editor 待集成）
- **查询结果表格**：列排序、单元格复制、TSV 多行复制、虚拟滚动（>1000 行）
- **查询历史**：保留最近 100 条查询记录
- **Pinia 状态管理**：connections store + query store
- **Tauri IPC 封装**：类型安全的命令调用

#### Rust 后端（Tauri 2 + 多驱动架构）
- **AppState**：连接句柄注册表（UUID 管理）
- **ConnectionRegistry**：驱动实例缓存（避免重复握手）
- **连接管理命令**：list/save/register/unregister/test_connection
- **配置持久化命令**：save_connections/load_connections
- **Schema 元数据命令**：list_databases/list_schemas/list_tables/list_columns/describe_table
- **SQL 执行命令**：execute_sql/execute_batch
- **Tauri Secure Store 集成**：macOS Keychain / Windows Credential Manager / Linux Secret Service
- **db-core trait 定义**：DatabaseDriver / DocumentDriver / CacheDriver

#### 数据库驱动

**PostgreSQL (db-postgres) — 完整实现**
- connect/ping/close
- query: PgRow → DbValue 类型映射
- execute: DML 支持（rows_affected）
- execute_batch: 事务内批量执行
- list_databases: pg_database 查询
- list_tables: pg_tables 查询
- describe_table: information_schema 查询（列 + 主键）
- get_columns: 轻量级列元数据
- explain: EXPLAIN 包装

**SQLite (db-sqlite) — 完整实现（5 个单元测试）**
- 内存数据库和文件数据库支持（`:memory:` 或文件路径）
- PRAGMA table_info 元数据查询
- PRAGMA index_list 索引查询
- 事务内批量执行
- last_insert_rowid 支持
- 5 个单元测试覆盖核心场景

**MySQL (db-mysql) — 完整实现**
- MySqlPool 连接池管理
- query: MySqlRow → DbValue 类型映射（支持所有 MySQL 数值类型）
- execute: DML with LAST_INSERT_ID() 支持
- execute_batch: 事务内批量执行
- list_databases: information_schema.SCHEMATA 查询
- list_tables: information_schema.TABLES 查询（BASE TABLE + VIEW）
- describe_table: information_schema.COLUMNS + STATISTICS
  - COLUMN_KEY 检测（PRI/UNI）
  - CHARACTER_MAXIMUM_LENGTH 支持
  - 多列索引分组
- get_columns: 轻量级列元数据
- explain: EXPLAIN 查询计划

**Redis (db-redis) — 完整实现**
- 连接：redis:// DSN + multiplexed async connection + SELECT db
- ping: PING 命令
- 基本 KV：get / set(EX ttl) / del / exists / ttl / expire
- Hash：hget / hset / hgetall / hdel
- List：lrange / lpush / rpush
- Set：smembers / sadd
- ZSet：zrange WITHSCORES → (String, f64)
- 浏览：scan(MATCH/COUNT) / key_type → RedisKeyType
- 服务器：server_info(INFO) / raw_command(任意命令)
- redis_value_to_json: redis::Value → serde_json::Value 转换
- 6 个单元测试覆盖 redis_value_to_json + RedisKeyType 序列化

#### 技术文档
- **README.md**：项目介绍、架构、快速开始、待办事项
- **ARCHITECTURE.md**：完整架构文档（数据流、状态管理、安全模型）
- **DEVELOPMENT.md**：开发指南（添加驱动、调试、测试、常见问题）
- **CHANGELOG.md**：v0.0.0 → v0.1.0 完整变更日志
- **docs/superpowers/specs/2026-05-20-frontend-refactor-design.md**
- **docs/superpowers/plans/2026-05-20-frontend-refactor.md**

### Changed
- 删除 Vite 模板代码（HelloWorld.vue, TheWelcome.vue, WelcomeItem.vue 等）
- 重写 AppLayout：从占位文字布局改为 DataGrip 风格专业界面
- 更新 router：直接指向 AppLayout，移除 HomeView/AboutView
- 扩展 UnoCSS 配置：添加 h-100%, min-h-0, flex-1 等快捷键

### In Progress
- **Monaco Editor 集成**：npm 包源网络限制，暂时使用 textarea

### Known Issues
- npm 包源遇到网络限制（403 Forbidden），Monaco Editor 无法安装
- Rust cargo 检查时 crates.io 索引下载失败
- db-postgres/db-sqlite query() 中的列数据类型目前硬编码为 "unknown"

### Roadmap
- [ ] **v0.2.0**：Monaco Editor + 查询历史 UI + 连接测试
- [ ] **v0.3.0**：查询结果导出（CSV/JSON）+ 错误处理优化
- [ ] **v0.4.0**：连接分组 + 深色主题 + 性能优化
- [ ] **v0.5.0**：快捷键自定义 + 查询格式化
- [ ] **v1.0.0**：端到端测试 + 性能基准 + 发布

## [0.0.0] - 2026-05-18

### Added
- 初始项目结构（Tauri + Vue 3 Vite 模板）
- db-core 核心抽象层（trait + 类型 + 错误）
- 数据库驱动 crate 骨架（db-postgres, db-mysql, db-sqlite, db-mongo, db-redis）
