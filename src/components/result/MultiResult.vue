<script setup lang="ts">
import { computed } from 'vue'
import {
  NText,
  NButton,
  NSpace,
  NTooltip,
  NCollapse,
  NCollapseItem,
} from 'naive-ui'
import QueryResult from './QueryResult.vue'
import type { QueryResult as QR } from '@/types/database'

const props = defineProps<{
  results: QR[]
}>()

const totalRows = computed(
  () => props.results.reduce((sum, r) => sum + r.rowCount, 0),
)
const totalCols = computed(
  () => props.results[0]?.columns.length ?? 0,
)

function escapeCsv(val: string): string {
  if (val.includes(',') || val.includes('"') || val.includes('\n')) {
    return '"' + val.replace(/"/g, '""') + '"'
  }
  return val
}

function cellValue(row: Record<string, unknown>, col: { name: string }): string {
  const value = row[col.name]
  return value === null || value === undefined ? 'NULL' : String(value)
}

function downloadMultiCsv(): void {
  if (props.results.length === 0) return
  const parts: string[] = []
  for (let i = 0; i < props.results.length; i++) {
    const r = props.results[i]
    parts.push(`-- Result ${i + 1} (${r.rowCount} rows)`)
    const header = r.columns.map(c => escapeCsv(c.name)).join(',')
    parts.push(header)
    const lines = r.rows
      .map(row => r.columns.map(c => escapeCsv(cellValue(row, c))).join(','))
      .join('\n')
    parts.push(lines)
    parts.push('')
  }
  const csv = parts.join('\n')
  const blob = new Blob(['﻿' + csv], {
    type: 'text/csv;charset=utf-8;',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `multi_query.csv`
  a.click()
  URL.revokeObjectURL(url)
}

function downloadMultiJson(): void {
  if (props.results.length === 0) return
  const payload = props.results.map(r => ({
    columns: r.columns.map(c => c.name),
    rows: r.rows,
    rowCount: r.rowCount,
    elapsedMs: r.elapsedMs,
  }))
  const json = JSON.stringify(payload, null, 2)
  const blob = new Blob([json], {
    type: 'application/json;charset=utf-8;',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'multi_query.json'
  a.click()
  URL.revokeObjectURL(url)
}
</script>

<template>
  <div class="multi-result">
    <div class="multi-header">
      <NSpace :size="8" align="center">
        <span class="multi-title">多语句结果</span>
        <NTag size="tiny" :bordered="false" round type="default">
          {{ results.length }} 个结果集
        </NTag>
        <NText depth="3" class="multi-stats">
          {{ totalRows }} 行 &middot; {{ totalCols }} 列
        </NText>
      </NSpace>
      <NSpace :size="4">
        <NTooltip placement="top">
          <template #trigger>
            <NButton size="tiny" quaternary @click="downloadMultiCsv">
              导出 CSV
            </NButton>
          </template>
          <span>下载全部结果为 CSV</span>
        </NTooltip>
        <NTooltip placement="top">
          <template #trigger>
            <NButton size="tiny" quaternary @click="downloadMultiJson">
              导出 JSON
            </NButton>
          </template>
          <span>下载全部结果为 JSON</span>
        </NTooltip>
      </NSpace>
    </div>
    <div class="multi-results">
      <div
        v-for="(result, index) in results"
        :key="index"
        class="result-block"
      >
        <div class="result-label">
          <span>结果 {{ index + 1 }}</span>
          <span v-if="result.rowCount > 0" class="result-meta">
            {{ result.rowCount }} 行 &middot; {{ result.elapsedMs }}ms
          </span>
          <span v-else class="result-meta result-empty">
            无返回
          </span>
        </div>
        <QueryResult :result="result" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.multi-result {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.multi-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 10px;
  border-bottom: 1px solid var(--db-border);
  background: var(--db-bg-panel);
  flex-shrink: 0;
}
.multi-title {
  font-weight: 600;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.multi-stats {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.multi-results {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
.result-block {
  border-bottom: 1px solid var(--db-border);
}
.result-block:last-child {
  border-bottom: none;
}
.result-label {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 10px;
  font-size: 11px;
  font-weight: 500;
  background: var(--db-bg-toolbar);
  border-bottom: 1px solid var(--db-border);
}
.result-meta {
  color: var(--n-text-color-3);
  font-weight: normal;
}
.result-empty {
  font-style: italic;
}
</style>
