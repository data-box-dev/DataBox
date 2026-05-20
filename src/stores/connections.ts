import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ConnectionConfig, TreeNode } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

export const useConnectionsStore = defineStore('connections', () => {
  const connections = ref<ConnectionConfig[]>([])
  const activeConnectionId = ref<string | null>(null)
  const treeNodes = ref<TreeNode[]>([])
  const expandedNodes = ref<Set<string>>(new Set())
  const loading = ref(false)
  const error = ref<string | null>(null)

  const activeConnection = computed(() =>
    connections.value.find((c) => c.id === activeConnectionId.value) || null
  )

  async function addConnection(config: ConnectionConfig): Promise<void> {
    try {
      await tauriCommands.saveConnection(config)
      connections.value.push(config)
      await persistConnections()
    } catch (e) {
      error.value = `Failed to save connection: ${e}`
      throw e
    }
  }

  async function updateConnection(
    id: string,
    updates: Partial<ConnectionConfig>,
  ): Promise<void> {
    const index = connections.value.findIndex((c) => c.id === id)
    if (index === -1) {
      throw new Error('Connection not found')
    }
    const updated = { ...connections.value[index], ...updates }
    try {
      await tauriCommands.saveConnection(updated)
      connections.value[index] = updated
      await persistConnections()
    } catch (e) {
      error.value = `Failed to update connection: ${e}`
      throw e
    }
  }

  async function removeConnection(id: string): Promise<void> {
    try {
      await tauriCommands.disconnect(id)
    } catch {
      // ignore disconnect errors
    }
    connections.value = connections.value.filter((c) => c.id !== id)
    if (activeConnectionId.value === id) {
      activeConnectionId.value = null
      treeNodes.value = []
    }
    await persistConnections()
  }

  async function connect(id: string): Promise<void> {
    try {
      const config = connections.value.find((c) => c.id === id)
      if (!config) throw new Error('Connection not found')

      // 从 Secure Store 加载密码
      const password = await tauriCommands.loadPassword(id)
      config.password = password

      // 验证连接可用
      await tauriCommands.testConnection(config)
      // 注册连接（存入 ConnectionRegistry，供后续命令使用）
      await tauriCommands.connect(config)

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
    expandedNodes.value = new Set()
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
      treeNodes.value = databases.map((db) => ({
        id: `${connId}-db-${db.name}`,
        kind: 'database' as const,
        name: db.name,
        parentId: connId,
        children: db.tables.map((t) => ({
          id: `${connId}-db-${db.name}-tbl-${t.name}`,
          kind: 'table' as const,
          name: t.name,
          parentId: `${connId}-db-${db.name}`,
          rowCount: t.rowEstimate,
        })),
      }))
    } catch (e) {
      error.value = `Failed to load tree: ${e}`
    } finally {
      loading.value = false
    }
  }

  /** 展开表节点时加载列信息 */
  async function loadColumns(tableNodeId: string): Promise<void> {
    // 解析连接 ID：格式为 "${connId}-db-${dbName}-tbl-${tableName}"
    const match = tableNodeId.match(/^(.+)-db-(.+)-tbl-(.+)$/)
    if (!match) return
    const [, connId, , tableName] = match

    try {
      const columns = await tauriCommands.listColumns(
        connId,
        '', // database 从 connId 推导，后端会重建驱动时用到
        '', // schema
        tableName,
      )

      // 找到对应的表节点并注入列 children
      const node = findTreeNode(treeNodes.value, tableNodeId)
      if (node) {
        node.children = columns.map((col) => ({
          id: `${tableNodeId}-col-${col.name}`,
          kind: 'column' as const,
          name: col.name,
          parentId: tableNodeId,
          dataType: col.dataType,
          nullable: col.nullable,
        }))
        // 触发响应式更新
        treeNodes.value = [...treeNodes.value]
      }
    } catch (e) {
      error.value = `Failed to load columns: ${e}`
    }
  }

  /** 在树中递归查找节点 */
  function findTreeNode(
    nodes: TreeNode[],
    id: string,
  ): TreeNode | null {
    for (const node of nodes) {
      if (node.id === id) return node
      if (node.children) {
        const found = findTreeNode(node.children, id)
        if (found) return found
      }
    }
    return null
  }

  async function loadConnectionsList(): Promise<void> {
    try {
      const stored = await tauriCommands.loadConnections()
      connections.value = stored.map((s) => ({
        ...s,
        password: '',
      }))
    } catch (e) {
      error.value = `Failed to load connections: ${e}`
    }
  }

  async function persistConnections(): Promise<void> {
    try {
      const stored = connections.value.map((c) => ({
        id: c.id,
        name: c.name,
        driver: c.driver,
        host: c.host,
        port: c.port,
        database: c.database,
        username: c.username,
        ssl: c.ssl,
        options: c.options,
      }))
      await tauriCommands.saveConnections(stored)
    } catch (e) {
      console.error('Failed to persist connections:', e)
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
    updateConnection,
    removeConnection,
    connect,
    disconnect,
    setActive,
    toggleExpand,
    loadTree,
    loadColumns,
    loadConnectionsList,
  }
})
