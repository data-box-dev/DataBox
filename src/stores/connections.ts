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
    } catch (e) {
      error.value = `Failed to save connection: ${e}`
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
  }

  async function connect(id: string): Promise<void> {
    try {
      const config = connections.value.find((c) => c.id === id)
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

  async function loadConnectionsList(): Promise<void> {
    try {
      const ids = await tauriCommands.listConnections()
      // TODO: 通过 Tauri 命令获取每个连接的详细信息
      // 目前只有 ID 列表，详情需在 save_connection 时持久化
      console.log('Registered connection IDs:', ids)
    } catch (e) {
      error.value = `Failed to load connections: ${e}`
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
    loadConnectionsList,
  }
})
