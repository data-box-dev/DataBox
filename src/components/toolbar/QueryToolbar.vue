<script setup lang="ts">
import { computed, watch } from 'vue'
import {
  NButton,
  NTooltip,
  NSelect,
  NDivider,
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
  newConnection: []
  runAll: []
}>()

const store = useConnectionsStore()

const databaseOptions = computed(() =>
  store.availableDatabases.map(db => ({ label: db, value: db })),
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
  <div class="toolbar">
    <div class="toolbar-group toolbar-main">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton
            size="small"
            @click="emit('run')"
            :loading="isExecuting"
            quaternary
            class="btn-run"
          >
            运行
          </NButton>
        </template>
        <span>运行当前语句 (Ctrl+Enter)</span>
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
        <span>停止查询</span>
      </NTooltip>
    </div>

    <NDivider vertical class="toolbar-divider" />

    <div class="toolbar-group toolbar-secondary">
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
        <span>查询历史</span>
      </NTooltip>
    </div>

    <NDivider v-if="connectionId" vertical class="toolbar-divider" />

    <div v-if="connectionId" class="toolbar-group toolbar-db-switcher">
      <NSelect
        :value="store.currentDatabase"
        :options="databaseOptions"
        :loading="store.loadingDatabases"
        placeholder="数据库"
        size="small"
        style="width: 150px"
        @update:value="onDatabaseChange"
      />
    </div>

    <div class="toolbar-spacer" />

    <NTooltip placement="bottom">
      <template #trigger>
        <NButton size="small" quaternary @click="emit('newConnection')">
          + 连接
        </NButton>
      </template>
      <span>新建连接</span>
    </NTooltip>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  height: 100%;
  padding: 0 6px;
  gap: 2px;
}
.toolbar-group {
  display: flex;
  align-items: center;
  gap: 1px;
}
.toolbar-main .btn-run {
  font-weight: 500;
}
.toolbar-divider {
  height: 18px;
  margin: 0 6px;
}
.toolbar-secondary {
  opacity: 0.85;
}
.toolbar-db-switcher {
  opacity: 0.9;
}
.toolbar-spacer {
  flex: 1;
}
</style>
