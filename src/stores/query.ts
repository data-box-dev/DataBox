import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { QueryResult, QueryHistoryItem, ExecResult } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

export const useQueryStore = defineStore('query', () => {
  const editorContent = ref('')
  const queryHistory = ref<QueryHistoryItem[]>([])
  const currentResult = ref<QueryResult | null>(null)
  const multiResults = ref<QueryResult[]>([])
  const isExecuting = ref(false)
  const error = ref<string | null>(null)
  const activeTab = ref(0) // 0 = single, 1..N = multi result tabs

  const hasResults = computed(
    () => currentResult.value !== null || multiResults.value.length > 0,
  )
  const totalRows = computed(() => {
    if (multiResults.value.length > 0) {
      return multiResults.value.reduce(
        (sum, r) => sum + r.rowCount,
        0,
      )
    }
    return currentResult.value?.rowCount ?? 0
  })

  function setEditorContent(sql: string): void {
    editorContent.value = sql
  }

  async function execute(connId: string, sql: string): Promise<QueryResult> {
    isExecuting.value = true
    error.value = null
    try {
      const result = await tauriCommands.executeSql(connId, sql)
      currentResult.value = result
      multiResults.value = []
      activeTab.value = 0
      addToHistory(sql, result.elapsedMs, connId)
      return result
    } catch (e) {
      error.value = `Query failed: ${e}`
      throw e
    } finally {
      isExecuting.value = false
    }
  }

  async function executeMulti(connId: string, sql: string): Promise<void> {
    isExecuting.value = true
    error.value = null
    try {
      const results = await tauriCommands.executeMulti(connId, sql)
      multiResults.value = results.filter((r) => r.rowCount > 0 || r.columns.length > 0)
      currentResult.value = null
      activeTab.value = multiResults.value.length > 0 ? 0 : -1
    } catch (e) {
      error.value = `Multi-statement failed: ${e}`
      throw e
    } finally {
      isExecuting.value = false
    }
  }

  async function executeBatch(
    connId: string,
    statements: string[],
  ): Promise<ExecResult[]> {
    isExecuting.value = true
    error.value = null
    try {
      const results = await tauriCommands.executeBatch(connId, statements)
      return results
    } catch (e) {
      error.value = `Batch execution failed: ${e}`
      throw e
    } finally {
      isExecuting.value = false
    }
  }

  function addToHistory(
    sql: string,
    elapsedMs: number,
    connectionId: string,
  ): void {
    const item: QueryHistoryItem = {
      id: crypto.randomUUID(),
      sql,
      elapsedMs,
      timestamp: Date.now(),
      connectionId,
    }
    queryHistory.value = [item, ...queryHistory.value].slice(0, 100)
  }

  function clearResult(): void {
    currentResult.value = null
    multiResults.value = []
    activeTab.value = -1
  }

  function setActiveTab(index: number): void {
    activeTab.value = index
  }

  return {
    editorContent,
    queryHistory,
    currentResult,
    multiResults,
    isExecuting,
    error,
    activeTab,
    hasResults,
    totalRows,
    setEditorContent,
    execute,
    executeMulti,
    executeBatch,
    addToHistory,
    clearResult,
    setActiveTab,
  }
})
