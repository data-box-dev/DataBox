import { invoke } from '@tauri-apps/api/core'
import type {
  ConnectionConfig,
  DatabaseInfo,
  TableInfo,
  ColumnMeta,
  QueryResult,
  ExecResult,
  StoredConnection,
} from '@/types/database'

export const tauriCommands = {
  // Connection
  async listConnections(): Promise<string[]> {
    return invoke<string[]>('list_connections')
  },

  async saveConnection(config: ConnectionConfig): Promise<void> {
    return invoke('save_connection', { config })
  },

  async loadPassword(id: string): Promise<string> {
    return invoke('load_password', { id })
  },

  async testConnection(config: ConnectionConfig): Promise<void> {
    return invoke('test_connection', { config })
  },

  // Config (persistence)
  async loadConnections(): Promise<StoredConnection[]> {
    return invoke<StoredConnection[]>('load_connections')
  },

  async saveConnections(connections: StoredConnection[]): Promise<void> {
    return invoke('save_connections', { connections })
  },

  // Database lifecycle
  async connect(config: ConnectionConfig): Promise<string> {
    return invoke<string>('connect', { config })
  },

  async ping(connId: string): Promise<void> {
    return invoke('ping', { connId })
  },

  async disconnect(connId: string): Promise<void> {
    return invoke('disconnect', { connId })
  },

  // Schema
  async listDatabases(connId: string): Promise<DatabaseInfo[]> {
    return invoke<DatabaseInfo[]>('list_databases', { connId })
  },

  async listSchemas(connId: string, database: string): Promise<string[]> {
    return invoke<string[]>('list_schemas', { connId, database })
  },

  async listTables(
    connId: string,
    database: string,
    schema?: string,
  ): Promise<TableInfo[]> {
    return invoke<TableInfo[]>('list_tables', { connId, database, schema })
  },

  async listColumns(
    connId: string,
    database: string,
    schema: string,
    table: string,
  ): Promise<ColumnMeta[]> {
    return invoke<ColumnMeta[]>('list_columns', { connId, database, schema, table })
  },

  // Query
  async executeSql(connId: string, sql: string): Promise<QueryResult> {
    return invoke<QueryResult>('execute_sql', { connId, sql })
  },

  async executeBatch(
    connId: string,
    statements: string[],
  ): Promise<ExecResult[]> {
    return invoke<ExecResult[]>('execute_batch', { connId, statements })
  },
}
