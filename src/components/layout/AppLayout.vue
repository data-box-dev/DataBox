<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import {
  NButton,
  NTag,
  NFlex,
  NSplit,
  NSpin,
  NModal,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NInputNumber,
  NSwitch,
  NMessageProvider,
  NPopover,
  NScrollbar,
  NText,
  NEmpty,
  NDataTable,
  NDrawer,
  NDrawerContent,
  NList,
  NListItem,
  NThing,
  NSpace,
  NTooltip,
  NPopconfirm,
} from 'naive-ui'
import type { FormInst, FormItemRule } from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import type { ConnectionConfig, DriverKind, ColumnSchema, TableSchema } from '@/types/database'
import { useConnectionsStore } from '@/stores/connections'
import { useQueryStore } from '@/stores/query'
import { useSavedQueriesStore } from '@/stores/savedQueries'
import { tauriCommands } from '@/composables/useTauriCommands'

const connectionsStore = useConnectionsStore()
const queryStore = useQueryStore()
const savedStore = useSavedQueriesStore()

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

function handleStop() {}

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

// ── ConnectionDialog ──
const dialogFormRef = ref<FormInst>()
const dialogTesting = ref(false)
const dialogLoading = ref(false)
const dialogMessage = ref<ReturnType<typeof useMessage> | null>(null)

const formModel = ref<Partial<ConnectionConfig>>({
  driver: 'Postgres',
  name: '',
  host: 'localhost',
  port: 5432,
  database: '',
  username: '',
  password: '',
  ssl: false,
  options: {},
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

watch(() => props.visible, (visible) => {
  if (visible && props.editingConfig) {
    formModel.value = { ...props.editingConfig }
  } else if (visible) {
    formModel.value = { driver: 'Postgres', name: '', host: 'localhost', port: 5432, database: '', username: '', password: '', ssl: false, options: {} }
  }
})

watch(() => formModel.value.driver, (driver) => {
  if (driver) formModel.value.port = portByDriver[driver as DriverKind]
})

async function testConnection(): Promise<void> {
  dialogTesting.value = true
  try {
    await dialogFormRef.value?.validate()
    await tauriCommands.testConnection(formModel.value as ConnectionConfig)
    dialogMessage.value?.success('连接成功')
  } catch (e) {
    dialogMessage.value?.error(`连接失败: ${e}`)
  } finally {
    dialogTesting.value = false
  }
}

async function saveConnection(): Promise<void> {
  try {
    await dialogFormRef.value?.validate()
    dialogLoading.value = true
    const config: ConnectionConfig = {
      ...formModel.value,
      id: props.editingConfig?.id || crypto.randomUUID(),
    } as ConnectionConfig
    await emit('saved', config)
    emit('update:visible', false)
    dialogMessage.value?.success('连接已保存')
  } catch {
    // validation error handled by naive-ui
  } finally {
    dialogLoading.value = false
  }
}
</script>

<template>
  <div class="app-root">
    <!-- ══ Toolbar ══ -->
    <header class="toolbar-header">
      <NFlex align="center" size="small" class="toolbar-inner">
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" @click="handleRun" :loading="queryStore.isExecuting" quaternary class="btn-run">
              运行
            </NButton>
          </template>
          <span>运行当前语句 (Ctrl+Enter)</span>
        </NTooltip>
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" @click="handleRunAll" :loading="queryStore.isExecuting" quaternary>
              全部执行
            </NButton>
          </template>
          <span>执行所有语句 (Ctrl+Shift+Enter)</span>
        </NTooltip>
        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" quaternary @click="handleStop">停止</NButton>
          </template>
          <span>停止查询</span>
        </NTooltip>

        <span class="toolbar-divider" />

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

        <span v-if="connectionsStore.activeConnectionId" class="toolbar-divider" />

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

        <div class="toolbar-spacer" />

        <NTooltip placement="bottom">
          <template #trigger>
            <NButton size="small" quaternary @click="handleNewConnection">+ 连接</NButton>
          </template>
          <span>新建连接</span>
        </NTooltip>
      </NFlex>
    </header>

    <!-- ══ Main Area ══ -->
    <div class="main-area">
      <!-- Sidebar -->
      <aside class="sidebar">
        <div class="sidebar-header">
          <span class="sidebar-title">连接浏览器</span>
          <NButton text size="small" @click="handleNewConnection">+ 新建</NButton>
        </div>
        <NSpin :show="connectionsStore.loading">
          <NTree
            :data="connectionsStore.treeNodes.map(node => ({
              key: node.id,
              label: node.name,
              isLeaf: node.kind === 'column',
              prefix: () => h('span', { class: 'tree-icon' }, { default: () => {
                switch (node.kind) {
                  case 'connection': return '🗄️'
                  case 'database': return '📁'
                  case 'table': return '📋'
                  case 'column': return '│'
                  default: return ''
                }
              }}),
              children: node.children?.map(child => ({
                key: child.id,
                label: child.name,
                isLeaf: child.kind === 'column',
                prefix: () => h('span', { class: 'tree-icon' }, { default: () => {
                  switch (child.kind) {
                    case 'database': return '📁'
                    case 'table': return '📋'
                    case 'column': return '│'
                    default: return ''
                  }
                }}),
              })),
            }))"
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
      <main class="content-area">
        <NSplit direction="vertical" :default-size="0.5" :min-size="0.15" :max-size="0.85">
          <template #1>
            <div class="editor-pane">
              <div class="pane-toolbar">
                <NText depth="3" class="pane-title">编辑器</NText>
              </div>
              <div class="editor-body">
                <div class="line-numbers" aria-hidden="true">
                  <pre>1</pre>
                </div>
                <textarea
                  :value="queryStore.editorContent"
                  @input="queryStore.setEditorContent(($event.target as HTMLTextAreaElement).value)"
                  @keydown="(e: KeyboardEvent) => {
                    const ta = e.target as HTMLTextAreaElement
                    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                      e.preventDefault()
                      if (queryStore.editorContent.trim() && connectionsStore.activeConnectionId)
                        queryStore.execute(connectionsStore.activeConnectionId, queryStore.editorContent)
                      return
                    }
                    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'Enter') {
                      e.preventDefault()
                      if (queryStore.editorContent.trim() && connectionsStore.activeConnectionId)
                        queryStore.executeMulti(connectionsStore.activeConnectionId, queryStore.editorContent)
                      return
                    }
                    if (e.key === 'Tab') {
                      e.preventDefault()
                      const s = ta.selectionStart, en = ta.selectionEnd, v = ta.value
                      const nv = v.substring(0, s) + '  ' + v.substring(en)
                      queryStore.setEditorContent(nv)
                      requestAnimationFrame(() => { ta.selectionStart = ta.selectionEnd = s + 2 })
                    }
                  }"
                  class="sql-textarea"
                  spellcheck="false"
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="off"
                  placeholder="输入 SQL 查询...&#10;Ctrl+Enter 运行当前语句&#10;Ctrl+Shift+Enter 执行所有语句&#10;Tab 插入缩进"
                />
              </div>
            </div>
          </template>
          <template #2>
            <div class="result-pane">
              <div class="pane-toolbar">
                <NTag v-if="queryStore.currentResult" size="tiny" :bordered="false" round type="info">
                  {{ queryStore.currentResult.rowCount }} 行
                </NTag>
                <NText v-else-if="queryStore.multiResults.length > 0" depth="3" class="pane-title">
                  {{ queryStore.multiResults.length }} 个结果集 · {{ queryStore.totalRows }} 行
                </NText>
                <NText v-else depth="3" class="pane-title">结果</NText>
                <div class="pane-spacer" />
                <NTooltip v-if="queryStore.currentResult" placement="top">
                  <template #trigger>
                    <NButton size="tiny" quaternary @click="() => {
                      if (!queryStore.currentResult) return
                      const cols = queryStore.currentResult.columns
                      const rows = queryStore.currentResult.rows
                      const header = cols.map(c => c.name).join(',')
                      const lines = rows.map(r => cols.map(c => {
                        const v = r[c.name]
                        return (v === null || v === undefined) ? 'NULL' : String(v)
                      }).join(',')).join('\n')
                      const blob = new Blob(['﻿' + [header, ...lines].join('\n')], { type: 'text/csv;charset=utf-8;' })
                      const url = URL.createObjectURL(blob)
                      const a = document.createElement('a'); a.href = url; a.download = 'query_result.csv'; a.click(); URL.revokeObjectURL(url)
                    }">导出 CSV</NButton>
                  </template>
                  <span>下载为 CSV</span>
                </NTooltip>
              </div>
              <div class="result-body">
                <template v-if="queryStore.multiResults.length > 0">
                  <div v-for="(result, idx) in queryStore.multiResults" :key="idx" class="multi-result-block">
                    <div class="multi-label">结果 {{ idx + 1 }} <span v-if="result.rowCount > 0">{{ result.rowCount }} 行 · {{ result.elapsedMs }}ms</span><span v-else class="empty-label">无返回</span></div>
                    <NDataTable
                      v-if="result.rowCount > 0 || result.columns.length > 0"
                      :columns="result.columns.map(col => ({
                        title: col.name,
                        key: col.name,
                        sorter: 'default',
                        resizable: true,
                        minWidth: 80,
                        ellipsis: { tooltip: { width: 'trigger', maxWidth: 600 } },
                        render(row: Record<string, unknown>) {
                          const v = row[col.name]
                          if (v === null || v === undefined) return 'NULL'
                          if (typeof v === 'object') { try { return JSON.stringify(v) } catch { return String(v) } }
                          return String(v)
                        },
                      }))"
                      :data="result.rows"
                      :row-key="(row: Record<string, unknown>) => JSON.stringify(row)"
                      :virtual-scroll="result.rowCount > 1000"
                      :max-height="400"
                      :striped="true"
                      :bordered="false"
                      :single-line="false"
                      size="small"
                    />
                  </div>
                </template>
                <template v-else-if="queryStore.currentResult">
                  <NDataTable
                    v-if="queryStore.currentResult.rowCount > 0"
                    :columns="queryStore.currentResult.columns.map(col => ({
                      title: col.name,
                      key: col.name,
                      sorter: 'default',
                      resizable: true,
                      minWidth: 80,
                      ellipsis: { tooltip: { width: 'trigger', maxWidth: 600 } },
                      render(row: Record<string, unknown>) {
                        const v = row[col.name]
                        if (v === null || v === undefined) return 'NULL'
                        if (typeof v === 'object') { try { return JSON.stringify(v) } catch { return String(v) } }
                        return String(v)
                      },
                    }))"
                    :data="queryStore.currentResult.rows"
                    :row-key="(row: Record<string, unknown>) => JSON.stringify(row)"
                    :virtual-scroll="queryStore.currentResult.rowCount > 1000"
                    :max-height="500"
                    :striped="true"
                    :bordered="false"
                    :single-line="false"
                    size="small"
                  />
                  <NEmpty v-else description="查询成功，无返回行" size="small" />
                </template>
                <NEmpty v-else description="执行查询以查看结果" size="small" />
              </div>
            </div>
          </template>
        </NSplit>
      </main>
    </div>

    <!-- ══ Status Bar ══ -->
    <footer class="status-bar">
      <NFlex justify="space-between" align="center" size="small" style="width: 100%">
        <span v-if="connectionsStore.activeConnection" class="status-text">
          <NTag size="tiny" :bordered="false" round type="info">{{ connectionsStore.activeConnection.driver }}</NTag>
          <span class="conn-name">{{ connectionsStore.activeConnection.name }}</span>
          <span class="conn-host">{{ connectionsStore.activeConnection.host }}:{{ connectionsStore.activeConnection.port }}</span>
          <span class="conn-db">/ {{ connectionsStore.activeConnection.database }}</span>
        </span>
        <span v-else class="status-disconnected">未连接</span>
        <span v-if="queryStore.multiResults.length > 0" class="status-text">{{ queryStore.multiResults.length }} 个结果集 · {{ queryStore.totalRows }} 行</span>
        <span v-else-if="queryStore.currentResult" class="status-text">{{ queryStore.currentResult.rowCount }} 行 · {{ queryStore.currentResult.elapsedMs }}ms</span>
        <span v-else class="status-text">{{ new Date().toLocaleTimeString('zh-CN') }}</span>
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
      <div class="query-history">
        <div class="history-header">
          <NText strong>查询历史</NText>
          <NText depth="3" class="history-count">{{ queryStore.queryHistory.length }} 条</NText>
        </div>
        <NList v-if="queryStore.queryHistory.length > 0" bordered hoverable clickable class="history-list">
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
        <div v-if="queryStore.queryHistory.length > 0" class="history-footer">
          <NButton text size="tiny" @click="() => { queryStore.queryHistory = []; showHistory = false }">清空历史</NButton>
        </div>
      </div>
    </NPopover>

    <!-- ══ Table Detail ══ -->
    <NDrawer :show="showTableDetail" :width="420" placement="right" :native-scrollbar="false" @update:show="(v: boolean) => showTableDetail = v">
      <NDrawerContent :title="(() => { if (!selectedTableNodeId) return '表详情'; const m = selectedTableNodeId.match(/-tbl-(.+)$/); return m ? m[1] : '表详情' })()" :native-scrollbar="false" closable>
        <NSpin :show="false">
          <template v-if="selectedTableNodeId">
            <TableDetail :visible="showTableDetail" :table-node-id="selectedTableNodeId" @update:visible="showTableDetail = $event" />
          </template>
          <NEmpty v-else description="请选择一个表" />
        </NSpin>
      </NDrawerContent>
    </NDrawer>

    <!-- ══ Saved Queries ══ -->
    <NPopover :show="showSaveQuery" placement="bottom-start" trigger="manual" :keep-alive-on-hide="true" @update:show="showSaveQuery = $event">
      <template #trigger><span /></template>
      <SavedQueriesPanel :visible="showSaveQuery" @update:visible="showSaveQuery = $event" />
    </NPopover>
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

/* ── Toolbar ── */
.toolbar-header {
  height: 36px;
  flex-shrink: 0;
  background: var(--db-bg-toolbar);
  border-bottom: 1px solid var(--db-border);
  padding: 0 6px;
}
.toolbar-inner {
  height: 100%;
}
.btn-run {
  font-weight: 500;
}
.toolbar-divider {
  width: 1px;
  height: 18px;
  background: var(--db-border);
  margin: 0 6px;
  flex-shrink: 0;
}
.toolbar-spacer {
  flex: 1;
}

/* ── Main Area ── */
.main-area {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

/* ── Sidebar ── */
.sidebar {
  width: 260px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--db-border);
  background: var(--db-bg-panel);
  overflow: hidden;
}
.sidebar-header {
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

/* ── Content Area ── */
.content-area {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* ── Editor ── */
.editor-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--db-bg-editor);
}
.pane-toolbar {
  display: flex;
  align-items: center;
  padding: 2px 8px;
  border-bottom: 1px solid var(--db-border);
  background: var(--db-bg-toolbar);
  flex-shrink: 0;
  gap: 8px;
  height: 26px;
}
.pane-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.pane-spacer {
  flex: 1;
}
.editor-body {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
  font-family: 'JetBrains Mono', 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 13px;
  line-height: 1.55;
}
.line-numbers {
  flex-shrink: 0;
  width: 42px;
  padding: 10px 4px 10px 0;
  text-align: right;
  color: var(--n-text-color-3);
  user-select: none;
  border-right: 1px solid var(--db-border);
  background: var(--db-bg-panel);
  overflow: hidden;
}
.line-numbers pre {
  margin: 0;
  line-height: inherit;
}
.sql-textarea {
  flex: 1;
  min-width: 0;
  padding: 10px 12px;
  border: none;
  resize: none;
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
  background: var(--db-bg-editor);
  color: var(--db-text);
  outline: none;
  tab-size: 2;
  overflow: auto;
  white-space: pre;
  overflow-wrap: normal;
}
.sql-textarea::placeholder {
  color: var(--n-text-color-3);
  opacity: 0.5;
}

/* ── Result ── */
.result-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.result-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.multi-result-block {
  border-bottom: 1px solid var(--db-border);
}
.multi-result-block:last-child {
  border-bottom: none;
}
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
.empty-label {
  font-style: italic;
  color: var(--n-text-color-3);
  font-weight: normal;
}

/* ── Status Bar ── */
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
.status-text {
  display: flex;
  align-items: center;
  gap: 6px;
}
.conn-name { font-weight: 500; }
.conn-host { color: rgba(255,255,255,0.65); }
.conn-db { color: rgba(255,255,255,0.45); }
.status-disconnected {
  color: rgba(255,255,255,0.5);
  font-style: italic;
}

/* ── Query History ── */
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
  border-bottom: 1px solid var(--db-border);
}
.history-count { font-size: 12px; }
.history-list { overflow-y: auto; max-height: 320px; flex: 1; }
.history-footer {
  display: flex;
  justify-content: flex-end;
  padding: 4px 8px;
  border-top: 1px solid var(--db-border);
}
.history-list :deep(.n-list-item) { padding: 0; }
.history-list :deep(.n-list-item__content) { padding: 4px 0; }
.history-list :deep(.n-thing) { --n-title-font-size: 13px; }
.history-list :deep(.n-thing__title) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 420px; }
</style>
