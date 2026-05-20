<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import {
  NLayout,
  NLayoutHeader,
  NLayoutSider,
  NLayoutContent,
  NLayoutFooter,
  NSplit,
  NFlex,
  NTag,
} from 'naive-ui'
import { useConnectionsStore } from '@/stores/connections'
import { useQueryStore } from '@/stores/query'
import { useSavedQueriesStore } from '@/stores/savedQueries'
import QueryToolbar from '@/components/toolbar/QueryToolbar.vue'
import ConnectionTree from '@/components/sidebar/ConnectionTree.vue'
import ConnectionDialog from '@/components/sidebar/ConnectionDialog.vue'
import QueryHistory from '@/components/sidebar/QueryHistory.vue'
import TableDetail from '@/components/sidebar/TableDetail.vue'
import SavedQueriesPanel from '@/components/sidebar/SavedQueriesPanel.vue'
import SqlEditor from '@/components/editor/SqlEditor.vue'
import QueryResult from '@/components/result/QueryResult.vue'
import MultiResult from '@/components/result/MultiResult.vue'
import type { ConnectionConfig } from '@/types/database'

const connectionsStore = useConnectionsStore()
const queryStore = useQueryStore()
const savedStore = useSavedQueriesStore()

const siderWidth = ref('260px')
const showDialog = ref(false)
const showHistory = ref(false)
const showTableDetail = ref(false)
const selectedTableNodeId = ref<string | null>(null)
const showSaveQuery = ref(false)
const editingConfig = ref<ConnectionConfig | null>(null)

onMounted(() => {
  connectionsStore.loadConnectionsList()
})

function handleRun() {
  if (connectionsStore.activeConnectionId && queryStore.editorContent) {
    queryStore.execute(connectionsStore.activeConnectionId, queryStore.editorContent)
  }
}

function handleRunAll() {
  if (connectionsStore.activeConnectionId && queryStore.editorContent) {
    queryStore.executeMulti(connectionsStore.activeConnectionId, queryStore.editorContent)
  }
}

function handleStop() {
  // TODO: query cancellation
}

function handleHistory() {
  showHistory.value = !showHistory.value
}

function handleHistorySelect(sql: string): void {
  queryStore.setEditorContent(sql)
  showHistory.value = false
}

function handleNewConnection() {
  editingConfig.value = null
  showDialog.value = true
}

async function handleConnectionSaved(config: ConnectionConfig) {
  try {
    if (editingConfig.value) {
      await connectionsStore.updateConnection(config.id, config)
    } else {
      await connectionsStore.addConnection(config)
    }
    showDialog.value = false
  } catch (e) {
    console.error('Failed to save connection:', e)
  }
}

function handleConnectionDeleted(id: string) {
  connectionsStore.removeConnection(id)
}

function handleExpand(keys: string[]) {
  const prev = new Set([...connectionsStore.expandedNodes])
  const newlyExpanded = keys.filter(k => !prev.has(k) && k.includes('-tbl-'))
  connectionsStore.expandedNodes = new Set(keys)
  for (const tableNodeId of newlyExpanded) {
    connectionsStore.loadColumns(tableNodeId)
  }
}

function handleTreeSelect(key: string) {
  connectionsStore.setActive(key)
  if (key.includes('-tbl-')) {
    selectedTableNodeId.value = key
    showTableDetail.value = true
  } else {
    showTableDetail.value = false
  }
}
</script>

<template>
  <NLayout class="app-layout" has-sider>
    <NLayoutHeader bordered class="toolbar-header">
      <QueryToolbar
        :connection-id="connectionsStore.activeConnectionId"
        :is-executing="queryStore.isExecuting"
        @run="handleRun"
        @run-all="handleRunAll"
        @stop="handleStop"
        @new-connection="handleNewConnection"
        @history="handleHistory"
        @save="showSaveQuery = !showSaveQuery"
      />
    </NLayoutHeader>

    <NLayout has-sider class="main-layout">
      <NLayoutSider
        bordered
        :width="siderWidth"
        :native-scrollbar="false"
        collapse-mode="transform"
        :collapsed-width="0"
        show-trigger="none"
      >
        <ConnectionTree
          :nodes="connectionsStore.treeNodes"
          :expanded-keys="Array.from(connectionsStore.expandedNodes)"
          :selected-key="connectionsStore.activeConnectionId"
          :loading="connectionsStore.loading"
          @expand="handleExpand"
          @select="handleTreeSelect"
          @new-connection="handleNewConnection"
        />
      </NLayoutSider>

      <NLayoutContent :native-scrollbar="false" class="main-content">
        <NSplit
          direction="vertical"
          :default-size="0.5"
          :min-size="0.15"
          :max-size="0.85"
        >
          <template #1>
            <div class="editor-pane">
              <SqlEditor
                v-model="queryStore.editorContent"
                :connection-id="connectionsStore.activeConnectionId"
                @execute="(sql) => queryStore.execute(connectionsStore.activeConnectionId!, sql)"
                @execute-multi="(sql) => queryStore.executeMulti(connectionsStore.activeConnectionId!, sql)"
              />
            </div>
          </template>
          <template #2>
            <div class="result-pane">
              <MultiResult
                v-if="queryStore.multiResults.length > 0"
                :results="queryStore.multiResults"
              />
              <QueryResult
                v-else-if="queryStore.currentResult"
                :result="queryStore.currentResult"
              />
              <div v-else class="result-placeholder">
                <span class="placeholder-icon">&#9679;</span>
                <span>执行查询以查看结果</span>
              </div>
            </div>
          </template>
        </NSplit>
      </NLayoutContent>
    </NLayout>

    <NLayoutFooter bordered class="status-bar">
      <NFlex justify="space-between" align="center" size="small">
        <span v-if="connectionsStore.activeConnection" class="status-conn">
          <NTag size="tiny" :bordered="false" round type="info">
            {{ connectionsStore.activeConnection.driver }}
          </NTag>
          <span class="conn-name">{{ connectionsStore.activeConnection.name }}</span>
          <span class="conn-host">{{ connectionsStore.activeConnection.host }}:{{ connectionsStore.activeConnection.port }}</span>
          <span class="conn-db">/ {{ connectionsStore.activeConnection.database }}</span>
        </span>
        <span v-else class="status-disconnected">未连接</span>
        <span v-if="queryStore.multiResults.length > 0" class="status-query">
          {{ queryStore.multiResults.length }} 个结果集 &middot; {{ queryStore.totalRows }} 行
        </span>
        <span v-else-if="queryStore.currentResult" class="status-query">
          {{ queryStore.currentResult.rowCount }} 行 &middot; {{ queryStore.currentResult.elapsedMs }}ms
        </span>
        <span v-else class="status-time">{{ new Date().toLocaleTimeString('zh-CN') }}</span>
      </NFlex>
    </NLayoutFooter>

    <ConnectionDialog
      :visible="showDialog"
      :editing-config="editingConfig"
      @update:visible="showDialog = $event"
      @saved="handleConnectionSaved"
    />

    <QueryHistory
      :visible="showHistory"
      @update:visible="showHistory = $event"
      @select="handleHistorySelect"
    />

    <TableDetail
      :visible="showTableDetail"
      :table-node-id="selectedTableNodeId"
      @update:visible="showTableDetail = $event"
    />

    <NPopover
      :show="showSaveQuery"
      placement="bottom-start"
      trigger="manual"
      :keep-alive-on-hide="true"
      @update:show="showSaveQuery = $event"
    >
      <SavedQueriesPanel
        :visible="showSaveQuery"
        @update:visible="showSaveQuery = $event"
      />
    </NPopover>
  </NLayout>
</template>

<style scoped>
.app-layout {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
.toolbar-header {
  height: 36px;
  flex-shrink: 0;
}
.main-layout {
  flex: 1;
  min-height: 0;
}
.main-content {
  height: 100%;
  overflow: hidden;
}
.editor-pane,
.result-pane {
  height: 100%;
  overflow: hidden;
}
.result-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--n-text-color-3);
  font-size: 13px;
  gap: 6px;
}
.placeholder-icon {
  opacity: 0.3;
}
.status-bar {
  height: 24px;
  padding: 0 10px;
  font-size: 11px;
  flex-shrink: 0;
  background: var(--db-bg-status);
  color: #fff;
}
.status-conn {
  display: flex;
  align-items: center;
  gap: 6px;
}
.conn-name {
  font-weight: 500;
}
.conn-host {
  color: rgba(255, 255, 255, 0.65);
}
.conn-db {
  color: rgba(255, 255, 255, 0.45);
}
.status-disconnected {
  color: rgba(255, 255, 255, 0.5);
  font-style: italic;
}
.status-query {
  color: rgba(255, 255, 255, 0.85);
}
.status-time {
  color: rgba(255, 255, 255, 0.45);
  font-variant-numeric: tabular-nums;
}
</style>
