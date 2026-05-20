<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  NDataTable,
  NText,
} from 'naive-ui'
import type {
  DataTableColumns,
  DataTableRowKey,
} from 'naive-ui'
import type { ColumnMeta, QueryResult } from '@/types/database'

const props = defineProps<{
  result: QueryResult | null
}>()

const sortColumn = ref<string | null>(null)
const sortOrder = ref<'ascend' | 'descend' | null>(null)

const columns = computed<DataTableColumns<Record<string, unknown>>>(() => {
  if (!props.result) return []
  return props.result.columns.map((col) => ({
    title: col.name,
    key: col.name,
    sorter: 'default',
    resizable: true,
    render: (row: Record<string, unknown>) => {
      const value = row[col.name]
      if (value === null || value === undefined) return 'NULL'
      return String(value)
    },
  }))
})

const data = computed(() => props.result?.rows || [])

const sortedData = computed(() => {
  if (!sortColumn.value || !sortOrder.value || !props.result) {
    return data.value
  }
  const sorted = [...data.value].sort((a, b) => {
    const aVal = a[sortColumn.value!]
    const bVal = b[sortColumn.value!]
    if (aVal === bVal) return 0
    if (aVal === null || aVal === undefined) return 1
    if (bVal === null || bVal === undefined) return -1
    if (sortOrder.value === 'ascend') {
      return aVal < bVal ? -1 : 1
    }
    return aVal < bVal ? 1 : -1
  })
  return sorted
})

function onCopyCell(row: Record<string, unknown>, col: ColumnMeta): void {
  const value = row[col.name]
  const text = value === null || value === undefined ? 'NULL' : String(value)
  navigator.clipboard.writeText(text)
}

function onCopyRow(row: Record<string, unknown>): void {
  if (!props.result) return
  const text = props.result.columns
    .map((col) => {
      const val = row[col.name]
      return val === null || val === undefined ? 'NULL' : String(val)
    })
    .join('\t')
  navigator.clipboard.writeText(text)
}
</script>

<template>
  <div class="query-result">
    <div v-if="result" class="result-header">
      <NText depth="3">
        {{ result.rowCount }} 行 · 耗时 {{ result.elapsedMs }}ms
      </NText>
    </div>
    <NDataTable
      v-if="result"
      :columns="columns"
      :data="sortedData"
      :row-key="(row: Record<string, unknown>) =>
        JSON.stringify(row) as DataTableRowKey
      "
      :virtual-scroll="result.rowCount > 1000"
      :max-height="500"
      :striped="true"
      :bordered="true"
      @cell-click="(row: Record<string, unknown>, col: ColumnMeta) =>
        onCopyCell(row, col)
      "
    />
    <div v-else class="placeholder">
      执行查询以查看结果
    </div>
  </div>
</template>

<style scoped>
.query-result {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.result-header {
  padding: 4px 12px;
  border-bottom: 1px solid var(--n-border-color);
  font-size: 12px;
  flex-shrink: 0;
}
.placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--n-text-color-3);
  font-size: 14px;
}
</style>
