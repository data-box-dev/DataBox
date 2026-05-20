<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  NPopover,
  NList,
  NListItem,
  NThing,
  NTag,
  NText,
  NButton,
  NSpace,
  NEmpty,
} from 'naive-ui'
import type { QueryHistoryItem } from '@/types/database'
import { useQueryStore } from '@/stores/query'
import { useConnectionsStore } from '@/stores/connections'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  'select': [sql: string]
}>()

const queryStore = useQueryStore()
const connectionsStore = useConnectionsStore()

function formatTime(timestamp: number): string {
  const d = new Date(timestamp)
  return d.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

function getConnectionName(id: string): string {
  const conn = connectionsStore.connections.find((c) => c.id === id)
  return conn?.name || id.slice(0, 8)
}

function onSelect(item: QueryHistoryItem): void {
  emit('select', item.sql)
  emit('update:visible', false)
}

function clearHistory(): void {
  queryStore.queryHistory = []
  emit('update:visible', false)
}
</script>

<template>
  <NPopover
    :show="visible"
    placement="bottom-start"
    :arrow-point-to-center="false"
    style="width: 520px"
    @update:show="emit('update:visible', $event)"
  >
    <template #trigger>
      <!-- Trigger is managed by parent toolbar button -->
      <span />
    </template>
    <div class="query-history">
      <div class="history-header">
        <NText strong>查询历史</NText>
        <NText depth="3" class="history-count">
          {{ queryStore.queryHistory.length }} 条
        </NText>
      </div>
      <NList
        v-if="queryStore.queryHistory.length > 0"
        bordered
        hoverable
        clickable
        class="history-list"
      >
        <NListItem
          v-for="item in queryStore.queryHistory"
          :key="item.id"
          @click="onSelect(item)"
        >
          <NThing
            :title="item.sql"
            :description="formatTime(item.timestamp)"
            #header-extra
          >
            <NSpace :size="4" align="center">
              <NTag size="tiny" type="info" round>
                {{ item.elapsedMs }}ms
              </NTag>
              <NTag v-if="item.connectionId" size="tiny" round>
                {{ getConnectionName(item.connectionId) }}
              </NTag>
            </NSpace>
          </NThing>
        </NListItem>
      </NList>
      <NEmpty v-else description="暂无查询历史" size="small" />
      <div v-if="queryStore.queryHistory.length > 0" class="history-footer">
        <NButton text size="tiny" @click="clearHistory">
          清空历史
        </NButton>
      </div>
    </div>
  </NPopover>
</template>

<style scoped>
.query-history {
  display: flex;
  flex-direction: column;
  max-height: 400px;
}
.history-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--n-border-color);
}
.history-count {
  font-size: 12px;
}
.history-list {
  overflow-y: auto;
  max-height: 320px;
  flex: 1;
}
.history-footer {
  display: flex;
  justify-content: flex-end;
  padding: 4px 8px;
  border-top: 1px solid var(--n-border-color);
}
.history-list :deep(.n-list-item) {
  padding: 0;
}
.history-list :deep(.n-list-item__content) {
  padding: 4px 0;
}
.history-list :deep(.n-thing) {
  --n-title-font-size: 13px;
}
.history-list :deep(.n-thing__title) {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 420px;
}
</style>
