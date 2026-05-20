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
} from 'naive-ui'
import { useConnectionsStore } from '@/stores/connections'
import { useQueryStore } from '@/stores/query'
import QueryToolbar from '@/components/toolbar/QueryToolbar.vue'
import ConnectionTree from '@/components/sidebar/ConnectionTree.vue'
import ConnectionDialog from '@/components/sidebar/ConnectionDialog.vue'
import SqlEditor from '@/components/editor/SqlEditor.vue'
import QueryResult from '@/components/result/QueryResult.vue'
import type { ConnectionConfig } from '@/types/database'

const connectionsStore = useConnectionsStore()
const queryStore = useQueryStore()

const siderWidth = ref('300px')
const showDialog = ref(false)
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

function handleStop() {
  // TODO: implement query cancellation
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
</script>

<template>
  <NLayout class="app-layout">
    <NLayoutHeader bordered class="toolbar-header">
      <NFlex justify="end" align="center" size="small" class="h-100%">
        <QueryToolbar
          :connection-id="connectionsStore.activeConnectionId"
          :is-executing="queryStore.isExecuting"
          @run="handleRun"
          @stop="handleStop"
          @new-connection="handleNewConnection"
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
              @expand="(keys) => connectionsStore.expandedNodes = new Set(keys)"
              @select="connectionsStore.setActive"
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
                  />
                </div>
              </template>
              <template #2>
                <div class="result-section">
                  <QueryResult :result="queryStore.currentResult" />
                </div>
              </template>
            </NSplit>
          </NLayoutContent>
        </template>
      </NSplit>
    </NLayout>

    <NLayoutFooter bordered class="status-bar">
      <NFlex justify="space-between" align="center" size="small">
        <span v-if="connectionsStore.activeConnection">
          {{ connectionsStore.activeConnection.name }}
          | {{ connectionsStore.activeConnection.host }}:{{
            connectionsStore.activeConnection.port
          }}
          | {{ connectionsStore.activeConnection.database }}
        </span>
        <span v-else>未连接</span>
        <span v-if="queryStore.currentResult">
          {{ queryStore.currentResult.rowCount }} 行 ·
          {{ queryStore.currentResult.elapsedMs }}ms
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
.h-100% {
  height: 100%;
}
</style>
