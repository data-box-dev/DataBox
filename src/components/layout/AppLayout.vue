<script setup lang="ts">
import { ref, computed, onMounted, h } from 'vue'
import type { VNode } from 'vue'
import {
  NFlex,
  NSplit,
  NSpin,
  NButton,
  NTag,
  NText,
  NEmpty,
  NModal,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NInputNumber,
  NSwitch,
  NPopover,
  NList,
  NListItem,
  NThing,
  NSpace,
  NTooltip,
  NTree,
} from 'naive-ui'
import type { FormInst, FormItemRule } from 'naive-ui'
import type { ConnectionConfig, DriverKind } from '@/types/database'
import { useConnectionsStore } from '@/stores/connections'
import { useQueryStore } from '@/stores/query'
import { tauriCommands } from '@/composables/useTauriCommands'
import SqlEditor from '@/components/editor/SqlEditor.vue'
import QueryResult from '@/components/result/QueryResult.vue'
import TableDetail from '@/components/sidebar/TableDetail.vue'

const connectionsStore = useConnectionsStore()
const queryStore = useQueryStore()

// ── UI State ──
const showDialog = ref(false)
const showHistory = ref(false)
const showTableDetail = ref(false)
const selectedTableNodeId = ref<string | null>(null)
const showSaveQuery = ref(false)
const editingConfig = ref<ConnectionConfig | null>(null)

onMounted(() => {
  connectionsStore.loadConnectionsList()
})

// ── Toolbar actions ──
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

function handleStop() {}

function handleHistory() {
  showHistory.value = !showHistory.value
}

function handleHistorySelect(sql: string) {
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

// ── Tree ──
function handleExpand(keys: string[]) {
  const prev = new Set([...connectionsStore.expandedNodes])
  const newlyExpanded = keys.filter(k => !prev.has(k) && k.includes('-tbl-'))
  connectionsStore.expandedNodes = new Set(keys)
  for (const tid of newlyExpanded) {
    connectionsStore.loadColumns(tid)
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

// ── Connection dialog ──
const dialogFormRef = ref<FormInst>()
const dialogTesting = ref(false)
const dialogLoading = ref(false)

const formModel = ref({
  driver: 'Postgres' as DriverKind,
  name: '',
  host: 'localhost',
  port: 5432,
  database: '',
  username: '',
  password: '',
  ssl: false,
})

const driverOptions = [
  { label: 'PostgreSQL', value: 'Postgres' },
  { label: 'MySQL', value: 'Mysql' },
  { label: 'SQLite', value: 'Sqlite' },
  { label: 'MongoDB', value: 'Mongo' },
  { label: 'Redis', value: 'Redis' },
]

const portByDriver: Record<DriverKind, number> = {
  Postgres: 5432, Mysql: 3306, Sqlite: 0, Mongo: 27017, Redis: 6379,
}

const rules: Record<string, FormItemRule[]> = {
  name: [{ required: true, message: '请输入连接名称', trigger: 'blur' }],
  driver: [{ required: true, message: '请选择数据库驱动', trigger: 'change' }],
  host: [{ required: true, message: '请输入主机地址', trigger: 'blur' }],
}

watch(() => showDialog.value, (open) => {
  if (!open) return
  if (editingConfig.value) {
    formModel.value = {
      driver: editingConfig.value.driver,
      name: editingConfig.value.name,
      host: editingConfig.value.host,
      port: editingConfig.value.port,
      database: editingConfig.value.database,
      username: editingConfig.value.username,
      password: editingConfig.value.password,
      ssl: editingConfig.value.ssl,
    }
  } else {
    formModel.value = { driver: 'Postgres', name: '', host: 'localhost', port: 5432, database: '', username: '', password: '', ssl: false }
  }
})

watch(() => formModel.value.driver, (driver) => {
  if (driver) formModel.value.port = portByDriver[driver as DriverKind]
})

async function testConnection() {
  dialogTesting.value = true
  try {
    await dialogFormRef.value?.validate()
    await tauriCommands.testConnection(formModel.value as ConnectionConfig)
  } catch (e) {
    // error shown by form validation
  } finally {
    dialogTesting.value = false
  }
}

async function saveConnection() {
  try {
    await dialogFormRef.value?.validate()
    dialogLoading.value = true
    const config: ConnectionConfig = {
      ...formModel.value,
      id: editingConfig.value?.id || crypto.randomUUID(),
      password: '',
      options: {},
    } as ConnectionConfig
    handleConnectionSaved(config)
    showDialog.value = false
  } catch {
    // validation error
  } finally {
    dialogLoading.value = false
  }
}

// ── Tree icon helper ──
function treeIcon(kind: string): VNode {
  const icons: Record<string, string> = {
    connection: '🗄️', database: '📁', table: '📋', column: '│', schema: '📂',
  }
  return h('span', { class: 'tree-icon' }, icons[kind] || '')
}

function buildTreeData() {
  return connectionsStore.treeNodes.map(node => ({
    key: node.id,
    label: node.name,
    isLeaf: node.kind === 'column',
    prefix: () => treeIcon(node.kind),
    children: node.children?.map(child => ({
      key: child.id,
      label: child.name,
      isLeaf: child.kind === 'column',
      prefix: () => treeIcon(child.kind),
    })),
  }))
}
</script>

<template>
  <div class="app-root">
    <!-- ══ Toolbar ══ -->
    <header class="toolbar-header">
      <NFlex align="center" size="small" class="toolbar-row">
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" @click="handleRun" :loading="queryStore.isExecuting" quaternary class="btn-run">运行</NButton>
          </template>
          <span>运行当前语句 (Ctrl+Enter)</span>
        </NTooltip>
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" @click="handleRunAll" :loading="queryStore.isExecuting" quaternary>全部执行</NButton>
          </template>
          <span>执行所有语句 (Ctrl+Shift+Enter)</span>
        </NTooltip>
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" quaternary @click="handleStop">停止</NButton>
          </template>
          <span>停止查询</span>
        </NTooltip>
        <span class="sep" />
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" quaternary @click="showSaveQuery = !showSaveQuery">保存</NButton>
          </template>
          <span>保存查询</span>
        </NTooltip>
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" quaternary @click="handleHistory">历史</NButton>
          </template>
          <span>查询历史</span>
        </NTooltip>
        <span v-if="connectionsStore.activeConnectionId" class="sep" />
        <template v-if="connectionsStore.activeConnectionId">
          <NSelect
            :value="connectionsStore.currentDatabase"
            :options="connectionsStore.availableDatabases.map(db => ({ label: db, value: db }))"
            :loading="connectionsStore.loadingDatabases"
            placeholder="数据库"
            size="small"
            style="width: 150px"
            @update:value="(v) => v && connectionsStore.switchDatabase(v)"
          />
        </template>
        <div class="flex-1" />
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" quaternary @click="handleNewConnection">+ 连接</NButton>
          </template>
          <span>新建连接</span>
        </NTooltip>
      </NFlex>
    </header>

    <!-- ══ Main ══ -->
    <div class="main-area">
      <!-- Sidebar -->
      <aside class="sidebar">
        <div class="sidebar-hdr">
          <span class="sidebar-title">连接浏览器</span>
          <NButton text size="small" @click="handleNewConnection">+ 新建</NButton>
        </div>
        <NSpin :show="connectionsStore.loading">
          <NTree
            :data="buildTreeData()"
            :expanded-keys="Array.from(connectionsStore.expandedNodes)"
            :selected-keys="connectionsStore.activeConnectionId ? [connectionsStore.activeConnectionId] : []"
            selectable
            :block-line="true"
            @update:expanded-keys="handleExpand"
            @update:selected-keys="(keys) => keys.length > 0 && handleTreeSelect(keys[0])"
          />
        </NSpin>
      </aside>

      <!-- Content -->
      <main class="content">
        <NSplit direction="vertical" :default-size="0.5" :min-size="0.15" :max-size="0.85">
          <template #1>
            <div class="editor-pane">
              <div class="pane-hdr">
                <NText depth="3" class="pane-title">编辑器</NText>
              </div>
              <SqlEditor
                v-model="queryStore.editorContent"
                :connection-id="connectionsStore.activeConnectionId"
                @execute="(sql) => { if (connectionsStore.activeConnectionId) queryStore.execute(connectionsStore.activeConnectionId, sql) }"
                @execute-multi="(sql) => { if (connectionsStore.activeConnectionId) queryStore.executeMulti(connectionsStore.activeConnectionId, sql) }"
              />
            </div>
          </template>
          <template #2>
            <div class="result-pane">
              <div class="pane-hdr">
                <template v-if="queryStore.multiResults.length > 0">
                  <NTag size="tiny" :bordered="false" round type="info">{{ queryStore.multiResults.length }} 个结果集</NTag>
                  <NText depth="3">{{ queryStore.totalRows }} 行</NText>
                </template>
                <template v-else-if="queryStore.currentResult">
                  <NTag size="tiny" :bordered="false" round type="info">{{ queryStore.currentResult.rowCount }} 行</NTag>
                  <NText depth="3">{{ queryStore.currentResult.elapsedMs }}ms</NText>
                </template>
                <template v-else>
                  <NText depth="3" class="pane-title">结果</NText>
                </template>
                <div class="flex-1" />
                <NTooltip v-if="queryStore.currentResult" placement="top">
                  <template #trigger>
                    <NButton size="tiny" quaternary @click="() => {
                      if (!queryStore.currentResult) return
                      const cols = queryStore.currentResult.columns
                      const rows = queryStore.currentResult.rows
                      const header = cols.map((c: any) => c.name).join(',')
                      const lines = rows.map((r: any) => cols.map((c: any) => { const v = r[c.name]; return (v == null) ? 'NULL' : String(v) }).join(',')).join('\n')
                      const blob = new Blob(['﻿' + [header, ...lines].join('\n')], { type: 'text/csv;charset=utf-8;' })
                      const url = URL.createObjectURL(blob); const a = document.createElement('a'); a.href = url; a.download = 'query_result.csv'; a.click(); URL.revokeObjectURL(url)
                    }">导出 CSV</NButton>
                  </template>
                  <span>下载为 CSV</span>
                </NTooltip>
              </div>
              <div class="result-body">
                <template v-if="queryStore.multiResults.length > 0">
                  <div v-for="(result, idx) in queryStore.multiResults" :key="idx" class="multi-block">
                    <div class="multi-label">
                      结果 {{ idx + 1 }}
                      <span v-if="result.rowCount > 0">{{ result.rowCount }} 行 · {{ result.elapsedMs }}ms</span>
                      <span v-else class="dim">无返回</span>
                    </div>
                    <QueryResult v-if="result.rowCount > 0 || result.columns.length > 0" :result="result" />
                  </div>
                </template>
                <template v-else>
                  <QueryResult v-if="queryStore.currentResult" :result="queryStore.currentResult" />
                  <NEmpty v-else description="执行查询以查看结果" size="small" />
                </template>
              </div>
            </div>
          </template>
        </NSplit>
      </main>
    </div>

    <!-- ══ Status Bar ══ -->
    <footer class="status-bar">
      <NFlex justify="space-between" align="center" size="small" style="width:100%">
        <template v-if="connectionsStore.activeConnection">
          <NTag size="tiny" :bordered="false" round type="info">{{ connectionsStore.activeConnection.driver }}</NTag>
          <span class="s-name">{{ connectionsStore.activeConnection.name }}</span>
          <span class="s-host">{{ connectionsStore.activeConnection.host }}:{{ connectionsStore.activeConnection.port }}</span>
          <span class="s-db">/ {{ connectionsStore.activeConnection.database }}</span>
        </template>
        <span v-else class="s-dim">未连接</span>
        <template v-if="queryStore.multiResults.length > 0">
          <span>{{ queryStore.multiResults.length }} 个结果集 · {{ queryStore.totalRows }} 行</span>
        </template>
        <template v-else-if="queryStore.currentResult">
          <span>{{ queryStore.currentResult.rowCount }} 行 · {{ queryStore.currentResult.elapsedMs }}ms</span>
        </template>
        <template v-else>
          <span>{{ new Date().toLocaleTimeString('zh-CN') }}</span>
        </template>
      </NFlex>
    </footer>

    <!-- ══ Connection Dialog ══ -->
    <NModal :show="showDialog" @update:show="(v: boolean) => showDialog = v" preset="card" :title="editingConfig ? '编辑连接' : '新建连接'" style="width: 520px">
      <NForm ref="dialogFormRef" :model="formModel" :rules="rules" label-placement="left" label-width="80">
        <NFormItem label="名称" path="name" required>
          <NInput v-model:value="formModel.name" placeholder="My PostgreSQL" />
        </NFormItem>
        <NFormItem label="驱动" path="driver" required>
          <NSelect v-model:value="formModel.driver" :options="driverOptions" placeholder="选择数据库类型" />
        </NFormItem>
        <NFormItem label="主机" path="host" required>
          <NInput v-model:value="formModel.host" placeholder="localhost" />
        </NFormItem>
        <NFormItem label="端口" path="port">
          <NInputNumber v-model:value="formModel.port" :min="1" :max="65535" style="width: 100%" />
        </NFormItem>
        <NFormItem label="数据库" path="database">
          <NInput v-model:value="formModel.database" placeholder="mydb" />
        </NFormItem>
        <NFormItem label="用户名" path="username">
          <NInput v-model:value="formModel.username" placeholder="postgres" />
        </NFormItem>
        <NFormItem label="密码" path="password">
          <NInput v-model:value="formModel.password" type="password" show-password-on="click" placeholder="••••••••" />
        </NFormItem>
        <NFormItem label="SSL">
          <NSwitch v-model:value="formModel.ssl" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NButton @click="testConnection" :loading="dialogTesting">测试连接</NButton>
        <NButton type="primary" @click="saveConnection" :loading="dialogLoading">保存</NButton>
      </template>
    </NModal>

    <!-- ══ Query History ══ -->
    <NPopover :show="showHistory" placement="bottom-start" :arrow-point-to-center="false" style="width: 520px" @update:show="showHistory = $event">
      <template #trigger><span /></template>
      <div class="history-panel">
        <div class="hist-hdr">
          <NText strong>查询历史</NText>
          <NText depth="3">{{ queryStore.queryHistory.length }} 条</NText>
        </div>
        <NList v-if="queryStore.queryHistory.length > 0" bordered hoverable clickable class="hist-list">
          <NListItem v-for="item in queryStore.queryHistory" :key="item.id" @click="() => { handleHistorySelect(item.sql) }">
            <NThing :title="item.sql" :description="new Date(item.timestamp).toLocaleTimeString('zh-CN')" #header-extra>
              <NSpace :size="4" align="center">
                <NTag size="tiny" type="info" round>{{ item.elapsedMs }}ms</NTag>
                <NTag v-if="item.connectionId" size="tiny" round>{{ (() => { const c = connectionsStore.connections.find((x: any) => x.id === item.connectionId); return c?.name || item.connectionId.slice(0, 8) })() }}</NTag>
              </NSpace>
            </NThing>
          </NListItem>
        </NList>
        <NEmpty v-else description="暂无查询历史" size="small" />
        <div v-if="queryStore.queryHistory.length > 0" class="hist-footer">
          <NButton text size="tiny" @click="() => { queryStore.queryHistory = []; showHistory = false }">清空历史</NButton>
        </div>
      </div>
    </NPopover>

    <!-- ══ Table Detail ══ -->
    <TableDetail
      :visible="showTableDetail"
      :table-node-id="selectedTableNodeId"
      @update:visible="showTableDetail = $event"
    />
  </div>
</template>

<style scoped>
.app-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--db-bg);
  color: var(--db-text);
  overflow: hidden;
}
.toolbar-header {
  height: 36px;
  flex-shrink: 0;
  background: var(--db-bg-toolbar);
  border-bottom: 1px solid var(--db-border);
  padding: 0 6px;
}
.toolbar-row {
  height: 100%;
}
.btn-run { font-weight: 500; }
.sep {
  width: 1px; height: 18px;
  background: var(--db-border);
  margin: 0 6px;
  flex-shrink: 0;
}
.flex-1 { flex: 1; }

.main-area {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

/* sidebar */
.sidebar {
  width: 260px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--db-border);
  background: var(--db-bg-panel);
  overflow: hidden;
}
.sidebar-hdr {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 10px;
  border-bottom: 1px solid var(--db-border);
  flex-shrink: 0;
}
.sidebar-title {
  font-weight: 600;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--n-text-color-3);
}
.tree-icon {
  font-size: 13px;
  width: 16px;
  display: inline-block;
  text-align: center;
  margin-right: 4px;
}

/* content */
.content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* panes */
.editor-pane, .result-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--db-bg-editor);
}
.pane-hdr {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 8px;
  border-bottom: 1px solid var(--db-border);
  background: var(--db-bg-toolbar);
  flex-shrink: 0;
  height: 26px;
}
.pane-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.result-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.multi-block {
  border-bottom: 1px solid var(--db-border);
}
.multi-block:last-child { border-bottom: none; }
.multi-label {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 10px;
  font-size: 11px;
  font-weight: 500;
  background: var(--db-bg-toolbar);
  border-bottom: 1px solid var(--db-border);
}
.dim { color: var(--n-text-color-3); font-style: italic; font-weight: normal; }

/* status bar */
.status-bar {
  height: 24px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 0 10px;
  background: var(--db-bg-status);
  color: #fff;
  font-size: 11px;
}
.s-name { font-weight: 500; }
.s-host { color: rgba(255,255,255,0.65); }
.s-db { color: rgba(255,255,255,0.45); }
.s-dim { color: rgba(255,255,255,0.5); font-style: italic; }

/* history */
.history-panel {
  display: flex;
  flex-direction: column;
  max-height: 400px;
}
.hist-hdr {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--db-border);
}
.hist-list { overflow-y: auto; max-height: 320px; flex: 1; }
.hist-footer { display: flex; justify-content: flex-end; padding: 4px 8px; border-top: 1px solid var(--db-border); }
.hist-list :deep(.n-list-item) { padding: 0; }
.hist-list :deep(.n-list-item__content) { padding: 4px 0; }
.hist-list :deep(.n-thing) { --n-title-font-size: 13px; }
.hist-list :deep(.n-thing__title) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 420px; }
</style>
