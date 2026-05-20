<script setup lang="ts">
import { ref, onMounted } from 'vue'
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
import QueryToolbar from '@/components/toolbar/QueryToolbar.vue'
import ConnectionTree from '@/components/sidebar/ConnectionTree.vue'
import ConnectionDialog from '@/components/sidebar/ConnectionDialog.vue'
import QueryHistory from '@/components/sidebar/QueryHistory.vue'
import TableDetail from '@/components/sidebar/TableDetail.vue'
import SqlEditor from '@/components/editor/SqlEditor.vue'
import QueryResult from '@/components/result/QueryResult.vue'
import MultiResult from '@/components/result/MultiResult.vue'
import type { ConnectionConfig } from '@/types/database'

const connectionsStore = useConnectionsStore()
const queryStore = useQueryStore()

const siderWidth = ref('300px')
const showDialog = ref(false)
const showHistory = ref(false)
const showTableDetail = ref(false)
const selectedTableNodeId = ref<string | null>(null)
const editingConfig = ref<ConnectionConfig | null>(null)

onMounted(() => {
  connectionsStore.loadConnectionsList()
})

function handleRun() {
  if (connectionsStore.activeConnectionId && queryStore.editorContent) {
    queryStore.execute(
      connectionsStore.activeConnectionId,
      queryStore.editorContent,
    )
  }
}

function handleRunAll() {
  if (
    connectionsStore.activeConnectionId &&
    queryStore.editorContent
  ) {
    queryStore.executeMulti(
      connectionsStore.activeConnectionId,
      queryStore.editorContent,
    )
  }
}

function handleStop() {
  // TODO: implement query cancellation
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
  const newlyExpanded = keys.filter(
    (k) => !prev.has(k) && k.includes('-tbl-'),
  )

  connectionsStore.expandedNodes = new Set(keys)

  for (const tableNodeId of newlyExpanded) {
    connectionsStore.loadColumns(tableNodeId)
  }
}

function handleTreeSelect(key: string) {
  connectionsStore.setActive(key)
  // If a table node is selected, open the detail drawer
  if (key.includes('-tbl-')) {
    selectedTableNodeId.value = key
    showTableDetail.value = true
  } else {
    showTableDetail.value = false
  }
}
</script>

<template>
  <NLayout class="app-layout">
    <NLayoutHeader bordered class="toolbar-header">
      <NFlex justify="end" align="center" size="small" class="h-100%">
        <QueryToolbar
          :connection-id="connectionsStore.activeConnectionId"
          :is-executing="queryStore.isExecuting"
          @run="handleRun"
          @run-all="handleRunAll"
          @stop="handleStop"
          @new-connection="handleNewConnection"
          @history="handleHistory"
        />
      </NFlex>
    </NLayoutHeader>

    <NLayout has-sider class="main-layout">
      <NSplit
        direction="horizontal"
        :default-size="siderWidth"
        :min-size="200"
        :max-size="500"
      >
        <template #1>
          <NLayoutSider bordered :width="siderWidth" :native-scrollbar="false">
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
        </template>
        <template #2>
          <NLayoutContent :native-scrollbar="false" class="main-content">
            <NSplit
              direction="vertical"
              :default-size="0.5"
              :min-size="0.2"
              :max-size="0.8"
            >
              <template #1>
                <div class="editor-section">
                  <SqlEditor
                    v-model="queryStore.editorContent"
                    :connection-id="connectionsStore.activeConnectionId"
                    @execute="(sql) =>
                      queryStore.execute(
                        connectionsStore.activeConnectionId!,
                        sql,
                      )
                    "
                    @execute-multi="(sql) =>
                      queryStore.executeMulti(
                        connectionsStore.activeConnectionId!,
                        sql,
                      )
                    "
                  />
                </div>
              </template>
              <template #2>
                <div class="result-section">
                  <MultiResult
                    v-if="queryStore.multiResults.length > 0"
                    :results="queryStore.multiResults"
                  />
                  <QueryResult
                    v-else-if="queryStore.currentResult"
                    :result="queryStore.currentResult"
                  />
                  <div v-else class="placeholder">
                    执行查询以查看结果
                  </div>
                </div>
              </template>
            </NSplit>
          </NLayoutContent>
        </template>
      </NSplit>
    </NLayout>

    <NLayoutFooter bordered class="status-bar">
      <NFlex justify="space-between" align="center" size="small">
        <span v-if="connectionsStore.activeConnection" class="status-conn">
          <NTag size="tiny" :bordered="false" round type="info">
            {{ connectionsStore.activeConnection.driver }}
          </NTag>
          {{ connectionsStore.activeConnection.name }}
          <span class="status-host">
            {{ connectionsStore.activeConnection.host }}:{{
              connectionsStore.activeConnection.port
            }}
          </span>
          / {{ connectionsStore.activeConnection.database }}
        </span>
        <span v-else class="status-disconnected">
          未连接
        </span>
        <span v-if="queryStore.multiResults.length > 0" class="status-query">
          {{ queryStore.multiResults.length }} 个结果集 ·
          {{ queryStore.totalRows }} 行
        </span>
        <span v-else-if="queryStore.currentResult" class="status-query">
          {{ queryStore.currentResult.rowCount }} 行 ·
          {{ queryStore.currentResult.elapsedMs }}ms
        </span>
        <span v-else class="status-time">
          {{ new Date().toLocaleTimeString('zh-CN') }}
        </span>
      </NFlex>
    </NLayoutFooter>
  </NLayout>

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
</template>

<style scoped>
.app-layout {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
.toolbar-header {
  height: 40px;
}
.main-layout {
  flex: 1;
  min-height: 0;
}
.main-content {
  height: 100%;
  overflow: hidden;
}
.editor-section,
.result-section {
  height: 100%;
  overflow: hidden;
}
.status-bar {
  height: 28px;
  padding: 0 12px;
  font-size: 12px;
}
.status-conn {
  display: flex;
  align-items: center;
  gap: 6px;
}
.status-host {
  color: var(--n-text-color-3);
}
.status-disconnected {
  color: var(--n-text-color-3);
  font-style: italic;
}
.status-query {
  color: var(--n-text-color-2);
}
.status-time {
  color: var(--n-text-color-3);
  font-variant-numeric: tabular-nums;
}
.h-100% {
  height: 100%;
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
