<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  NDataTable,
  NText,
  NButton,
  NSpace,
  NTooltip,
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
  navigator.clipboard.writeText(cellValue(row, col))
}

function onCopyRow(row: Record<string, unknown>): void {
  if (!props.result) return
  const text = props.result.columns
    .map((col) => cellValue(row, col))
    .join('\t')
  navigator.clipboard.writeText(text)
}

function cellValue(
  row: Record<string, unknown>,
  col: ColumnMeta,
): string {
  const value = row[col.name]
  return value === null || value === undefined ? 'NULL' : String(value)
}

function escapeCsv(val: string): string {
  if (val.includes(',') || val.includes('"') || val.includes('\n')) {
    return '"' + val.replace(/"/g, '""') + '"'
  }
  return val
}

function downloadCsv(): void {
  if (!props.result) return
  const cols = props.result.columns
  const rows = sortedData.value
  const header = cols.map((c) => escapeCsv(c.name)).join(',')
  const lines = rows.map((row) =>
    cols.map((c) => escapeCsv(cellValue(row, c))).join(','),
  )
  const csv = [header, ...lines].join('\n')
  const blob = new Blob(['﻿' + csv], {
    type: 'text/csv;charset=utf-8;',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'query_result_' + Date.now() + '.csv'
  a.click()
  URL.revokeObjectURL(url)
}

function downloadJson(): void {
  if (!props.result) return
  const payload = {
    columns: props.result.columns.map((c) => c.name),
    rows: sortedData.value,
    rowCount: props.result.rowCount,
    elapsedMs: props.result.elapsedMs,
  }
  const json = JSON.stringify(payload, null, 2)
  const blob = new Blob([json], {
    type: 'application/json;charset=utf-8;',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'query_result_' + Date.now() + '.json'
  a.click()
  URL.revokeObjectURL(url)
}
</script>

<template>
  <div class="query-result">
    <div v-if="result" class="result-header">
      <NText depth="3">
        {{ result.rowCount }} 行 · 耗时 {{ result.elapsedMs }}ms
      </NText>
      <NSpace :size="4" class="result-actions">
        <NTooltip placement="top">
          <template #trigger>
            <NButton size="tiny" quaternary @click="downloadCsv">
              导出 CSV
            </NButton>
          </template>
          <span>下载为 CSV 文件</span>
        </NTooltip>
        <NTooltip placement="top">
          <template #trigger>
            <NButton size="tiny" quaternary @click="downloadJson">
              导出 JSON
            </NButton>
          </template>
          <span>下载为 JSON 文件</span>
        </NTooltip>
      </NSpace>
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
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 12px;
  border-bottom: 1px solid var(--n-border-color);
  font-size: 12px;
  flex-shrink: 0;
}
.result-actions {
  margin-left: auto;
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
