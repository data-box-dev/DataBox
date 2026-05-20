<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  NDataTable,
  NText,
  NButton,
  NSpace,
  NTooltip,
  NEmpty,
  NCard,
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
  return props.result.columns.map(col => ({
    title: col.name,
    key: col.name,
    sorter: 'default',
    resizable: true,
    minWidth: 80,
    ellipsis: {
      tooltip: {
        width: 'trigger',
        maxWidth: 600,
      },
    },
    render(row: Record<string, unknown>) {
      const value = row[col.name]
      if (value === null || value === undefined) return 'NULL'
      if (typeof value === 'object') {
        try {
          return JSON.stringify(value)
        } catch {
          return String(value)
        }
      }
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
    if (typeof aVal === 'number' && typeof bVal === 'number') {
      return sortOrder.value === 'ascend' ? aVal - bVal : bVal - aVal
    }
    if (sortOrder.value === 'ascend') {
      return String(aVal) < String(bVal) ? -1 : 1
    }
    return String(aVal) < String(bVal) ? 1 : -1
  })
  return sorted
})

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
  const header = cols.map(c => escapeCsv(c.name)).join(',')
  const lines = rows.map(row =>
    cols.map(c => escapeCsv(cellValue(row, c))).join(','),
  )
  const csv = [header, ...lines].join('\n')
  const blob = new Blob(['﻿' + csv], {
    type: 'text/csv;charset=utf-8;',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'query_result.csv'
  a.click()
  URL.revokeObjectURL(url)
}

function downloadJson(): void {
  if (!props.result) return
  const payload = {
    columns: props.result.columns.map(c => c.name),
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
  a.download = 'query_result.json'
  a.click()
  URL.revokeObjectURL(url)
}

function cellValue(row: Record<string, unknown>, col: ColumnMeta): string {
  const value = row[col.name]
  return value === null || value === undefined ? 'NULL' : String(value)
}
</script>

<template>
  <NCard
    v-if="result"
    class="query-result-card"
    :bordered="true"
    size="small"
  >
    <template #header>
      <div class="result-header">
        <NSpace :size="12" align="center">
          <NTag size="tiny" :bordered="false" type="info" round>
            {{ result.rowCount }} 行
          </NTag>
          <NText depth="3" class="result-time">{{ result.elapsedMs }}ms</NText>
        </NSpace>
        <NSpace :size="4">
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
    </template>
    <NDataTable
      :columns="columns"
      :data="sortedData"
      :row-key="(row: Record<string, unknown>) =>
        JSON.stringify(row) as DataTableRowKey
      "
      :virtual-scroll="result.rowCount > 1000"
      :max-height="500"
      :striped="true"
      :bordered="false"
      :single-line="false"
      :size="'small'"
    />
  </NCard>
  <NEmpty v-else description="执行查询以查看结果" size="small" class="result-empty" />
</template>

<style scoped>
.query-result-card {
  --n-card-padding: 0;
}
.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}
.result-time {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.result-empty {
  padding: 40px 0;
}
.cell-null {
  color: var(--n-text-color-3);
  font-style: italic;
}
.cell-json {
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
}
</style>
