<script setup lang="ts">
import { computed } from 'vue'
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
</script>

<template>
  <div class="multi-result">
    <div class="multi-header">
      <span class="multi-title">多语句结果</span>
      <span class="multi-stats">
        {{ results.length }} 个结果集 · {{ totalRows }} 行 · {{ totalCols }} 列
      </span>
    </div>
    <div class="multi-results">
      <div
        v-for="(result, index) in results"
        :key="index"
        class="result-block"
      >
        <div class="result-label">
          结果 {{ index + 1 }}
          <span v-if="result.rowCount > 0" class="result-meta">
            {{ result.rowCount }} 行 · {{ result.elapsedMs }}ms
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
  padding: 4px 12px;
  border-bottom: 1px solid var(--n-border-color);
  font-size: 12px;
  flex-shrink: 0;
}
.multi-title {
  font-weight: 600;
}
.multi-stats {
  color: var(--n-text-color-3);
}
.multi-results {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
.result-block {
  border-bottom: 1px solid var(--n-border-color);
}
.result-block:last-child {
  border-bottom: none;
}
.result-label {
  padding: 4px 12px;
  font-size: 12px;
  font-weight: 500;
  background: var(--n-color);
  border-bottom: 1px solid var(--n-border-color);
  display: flex;
  align-items: center;
  gap: 8px;
}
.result-meta {
  font-weight: normal;
  color: var(--n-text-color-3);
  font-size: 11px;
}
.result-empty {
  font-style: italic;
}
</style>
