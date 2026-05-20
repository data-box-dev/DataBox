export interface ConnectionConfig {
  id: string
  name: string
  driver: DriverKind
  host: string
  port: number
  database: string
  username: string
  password: string // 内存中临时持有，不持久化到 localStorage
  ssl: boolean
  options: Record<string, string>
}

export type DriverKind = 'Postgres' | 'Mysql' | 'Sqlite' | 'Mongo' | 'Redis'

export interface DatabaseInfo {
  name: string
  tables: TableInfo[]
}

export interface TableInfo {
  name: string
  tableType: TableType
  rowEstimate?: number
  sizeBytes?: number
}

export type TableType = 'table' | 'view' | 'materialized_view'

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

/// 持久化存储的连接配置（不含密码）
export interface StoredConnection {
  id: string
  name: string
  driver: DriverKind
  host: string
  port: number
  database: string
  username: string
  ssl: boolean
  options: Record<string, string>
}

export interface TreeNode {
  id: string
  kind: 'connection' | 'database' | 'schema' | 'table' | 'column'
  name: string
  parentId: string | null
  children?: TreeNode[]
  icon?: string
  rowCount?: number
  dataType?: string
  nullable?: boolean
}
