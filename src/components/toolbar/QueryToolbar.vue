<script setup lang="ts">
import { computed, watch } from 'vue'
import {
  NButton,
  NTooltip,
  NSelect,
} from 'naive-ui'
import type { ConnectionConfig } from '@/types/database'
import { useConnectionsStore } from '@/stores/connections'

const props = defineProps<{
  connectionId?: string | null
  isExecuting?: boolean
}>()

const emit = defineEmits<{
  run: []
  stop: []
  save: []
  history: []
  settings: []
  newConnection: []
  runAll: []
}>()

const store = useConnectionsStore()

const databaseOptions = computed(() =>
  store.availableDatabases.map((db) => ({
    label: db,
    value: db,
  })),
)

watch(
  () => props.connectionId,
  (connId) => {
    if (connId) {
      store.loadAvailableDatabases(connId)
    } else {
      store.availableDatabases = []
    }
  },
  { immediate: true },
)

function onDatabaseChange(database: string | null) {
  if (database) {
    store.switchDatabase(database)
  }
}
</script>

<template>
  <div class="query-toolbar">
    <div class="toolbar-group">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton
            size="small"
            @click="emit('run')"
            :loading="isExecuting"
            quaternary
          >
            运行
          </NButton>
        </template>
        <span>运行 (Ctrl+Enter)</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton
            size="small"
            @click="emit('runAll')"
            :loading="isExecuting"
            quaternary
          >
            全部执行
          </NButton>
        </template>
        <span>执行所有语句 (Ctrl+Shift+Enter)</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton
            size="small"
            quaternary
            @click="emit('stop')"
          >
            停止
          </NButton>
        </template>
        <span>停止</span>
      </NTooltip>
    </div>
    <div class="toolbar-group">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary @click="emit('save')">
            保存
          </NButton>
        </template>
        <span>保存查询</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary @click="emit('history')">
            历史
          </NButton>
        </template>
        <span>执行历史</span>
      </NTooltip>
    </div>
    <div v-if="connectionId" class="toolbar-group toolbar-db-switcher">
      <NSelect
        :value="store.currentDatabase"
        :options="databaseOptions"
        :loading="store.loadingDatabases"
        placeholder="数据库"
        size="small"
        style="width: 160px"
        @update:value="onDatabaseChange"
      />
    </div>
    <div class="toolbar-group">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary @click="emit('newConnection')">
            + 连接
          </NButton>
        </template>
        <span>新建连接</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary @click="emit('settings')">
            设置
          </NButton>
        </template>
        <span>设置</span>
      </NTooltip>
    </div>
  </div>
</template>

<style scoped>
.query-toolbar {
  display: flex;
  gap: 8px;
  padding: 4px 8px;
  align-items: center;
  height: 100%;
}
.toolbar-group {
  display: flex;
  gap: 2px;
}
</style>
