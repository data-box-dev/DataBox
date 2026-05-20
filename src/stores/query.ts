import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { QueryResult, QueryHistoryItem, ExecResult } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

export const useQueryStore = defineStore('query', () => {
  const editorContent = ref('')
  const queryHistory = ref<QueryHistoryItem[]>([])
  const currentResult = ref<QueryResult | null>(null)
  const multiResults = ref<Map<string, QueryResult>>(new Map())
  const isExecuting = ref(false)
  const error = ref<string | null>(null)

  function setEditorContent(sql: string): void {
    editorContent.value = sql
  }

  async function execute(connId: string, sql: string): Promise<QueryResult> {
    isExecuting.value = true
    error.value = null
    try {
      const result = await tauriCommands.executeSql(connId, sql)
      currentResult.value = result
      addToHistory(sql, result.elapsedMs, connId)
      return result
    } catch (e) {
      error.value = `Query failed: ${e}`
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
    multiResults.value = new Map()
  }

  return {
    editorContent,
    queryHistory,
    currentResult,
    multiResults,
    isExecuting,
    error,
    setEditorContent,
    execute,
    executeBatch,
    addToHistory,
    clearResult,
  }
})
