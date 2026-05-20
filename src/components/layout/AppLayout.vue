<script setup lang="ts">
import { computed, h, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import {
  Crown,
  Crosshair,
  Database,
  DatabaseOff,
  Dashboard,
  DeviceFloppy,
  Download,
  Eraser,
  History,
  LayoutSidebarLeftCollapse,
  LayoutSidebarLeftExpand,
  PlayerPlay,
  PlugConnected,
  Plus,
  Puzzle,
  Refresh,
  Robot,
  Search,
  Send,
  TestPipe,
  Trash,
  UserCircle,
  Bell,
} from '@vicons/tabler'
import { createDiscreteApi, type TreeOption } from 'naive-ui'

import {
  connectDatabase,
  deleteSavedConnection,
  describeTable,
  disconnectDatabase,
  executeSql,
  listSavedConnections,
  loadConnectionWorkspace,
  saveConnection,
  testConnection,
} from '@/lib/tauri'
import type {
  ColumnSchema,
  ConnectionConfig,
  ConnectionWorkspace,
  DbValue,
  DriverKind,
  SavedConnectionRecord,
  SqlExecutionResponse,
  TableSchema,
} from '@/types/databox'

const { message } = createDiscreteApi(['message'])

const driverMeta: Record<
  DriverKind,
  { label: string; port: number; supported: boolean; sampleSql: string }
> = {
  postgres: {
    label: 'PostgreSQL',
    port: 5432,
    supported: true,
    sampleSql: 'SELECT current_database() AS database_name, current_user AS current_user;',
  },
  mysql: {
    label: 'MySQL',
    port: 3306,
    supported: true,
    sampleSql: 'SHOW TABLES;',
  },
  sqlite: {
    label: 'SQLite',
    port: 0,
    supported: true,
    sampleSql: "SELECT name, type FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name;",
  },
  mongo: {
    label: 'MongoDB',
    port: 27017,
    supported: true,
    sampleSql: `{
  "action": "find",
  "collection": "your_collection",
  "filter": {},
  "options": { "limit": 20 }
}`,
  },
  redis: {
    label: 'Redis',
    port: 6379,
    supported: true,
    sampleSql: 'PING',
  },
}

interface ExplorerTreeOption extends TreeOption {
  rawType:
    | 'connection'
    | 'database'
    | 'group'
    | 'table'
    | 'columns_group'
    | 'indexes_group'
    | 'keys_group'
    | 'column'
    | 'index'
    | 'key'
  databaseName?: string
  tableName?: string
  tableType?: string
}

interface SqlEditorTab {
  id: string
  title: string
  sql: string
  execution: SqlExecutionResponse | null
  lastRunAt: string | null
}

interface QueryHistoryItem {
  id: string
  connectionName: string
  tabTitle: string
  sql: string
  mode: SqlExecutionResponse['mode']
  executedAt: string
}

interface SavedConsoleItem {
  id: string
  title: string
  sql: string
  connectionName: string
  savedAt: string
}

interface ExportProgressItem {
  id: string
  title: string
  rows: number
  status: 'success'
  finishedAt: string
}

const loading = reactive({
  list: false,
  save: false,
  test: false,
  connect: false,
  run: false,
  refresh: false,
  schema: false,
})

const drawerVisible = ref(false)
const historyVisible = ref(false)
const savedConsoleVisible = ref(false)
const exportVisible = ref(false)
const sqlEditorVisible = ref(false)
const explorerCollapsed = ref(false)
const workspaceTab = ref<'data' | 'schema'>('data')
const editorSplitSize = ref(0.22)
const explorerSearch = ref('')
const editorTabs = ref<SqlEditorTab[]>([createEditorTab(driverMeta.sqlite.sampleSql)])
const activeEditorTabId = ref(editorTabs.value[0].id)

const savedConnections = ref<SavedConnectionRecord[]>([])
const activeConnectionId = ref<string | null>(null)
const activeWorkspace = ref<ConnectionWorkspace | null>(null)
const selectedDatabase = ref<string | null>(null)
const selectedTable = ref<string | null>(null)
const selectedSchema = ref<TableSchema | null>(null)
const resultView = ref<SqlExecutionResponse | null>(null)
const tableSelectionVersion = ref(0)
const selectedTreeKeys = ref<string[]>([])
const expandedTreeKeys = ref<string[]>([])
const queryHistory = ref<QueryHistoryItem[]>([])
const savedConsoles = ref<SavedConsoleItem[]>([])
const exportProgress = ref<ExportProgressItem[]>([])

const form = reactive<ConnectionConfig>(createConnectionConfig('sqlite'))

const driverOptions = computed(() =>
  (Object.entries(driverMeta) as Array<[DriverKind, (typeof driverMeta)[DriverKind]]>).map(
    ([value, meta]) => ({
      label: meta.supported ? meta.label : `${meta.label}（稍后）`,
      value,
      disabled: !meta.supported,
    }),
  ),
)

const activeEditorTab = computed(
  () => editorTabs.value.find((tab) => tab.id === activeEditorTabId.value) ?? null,
)

const activeSqlDraft = computed({
  get: () => activeEditorTab.value?.sql ?? '',
  set: (value: string) => {
    if (!activeEditorTab.value) {
      return
    }

    const targetTab = editorTabs.value.find((tab) => tab.id === activeEditorTab.value?.id)
    if (targetTab) {
      targetTab.sql = value
    }
  },
})

const queryColumnNames = computed(() =>
  (resultView.value?.query_result?.columns ?? []).map((column) => column.name),
)

const treeData = computed<ExplorerTreeOption[]>(() => {
  if (!activeWorkspace.value) {
    return []
  }

  const tableNodes = activeWorkspace.value.databases.flatMap((database) =>
    database.tables.map((table) => {
      const isSelected =
        selectedDatabase.value === database.name && selectedTable.value === table.name
      return {
        key: `table:${database.name}:${table.name}`,
        label: table.name,
        rawType: 'table' as const,
        databaseName: database.name,
        tableName: table.name,
        tableType: table.table_type,
        children:
          isSelected && selectedSchema.value
            ? buildSelectedTableChildren(selectedSchema.value)
            : undefined,
      }
    }),
  )

  return [
    {
      key: `connection:${activeWorkspace.value.connection_id}`,
      label: activeWorkspace.value.name,
      rawType: 'connection',
      children: [
        {
          key: `database:${activeWorkspace.value.database}`,
          label: activeWorkspace.value.database,
          rawType: 'database',
          databaseName: activeWorkspace.value.database,
          children: [
            {
              key: `group:${activeWorkspace.value.database}:tables`,
              label: 'tables',
              rawType: 'group',
              databaseName: activeWorkspace.value.database,
              children: tableNodes,
            },
          ],
        },
      ],
    },
  ]
})

const browsableDatabases = computed(() =>
  (activeWorkspace.value?.databases ?? []).filter((database) => database.tables.length > 0),
)

const tableCount = computed(
  () =>
    activeWorkspace.value?.databases.reduce(
      (count, database) => count + database.tables.length,
      0,
    ) ?? 0,
)

const resultSummary = computed(() => {
  if (resultView.value?.query_result) {
    return [
      `行数 ${resultView.value.query_result.row_count}`,
      `列 ${resultView.value.query_result.columns.length}`,
      `耗时 ${resultView.value.query_result.elapsed_ms} ms`,
    ]
  }

  if (resultView.value?.exec_result) {
    return [
      `影响行数 ${resultView.value.exec_result.rows_affected}`,
      `耗时 ${resultView.value.exec_result.elapsed_ms} ms`,
      `插入 ID ${resultView.value.exec_result.last_insert_id ?? '-'}`,
    ]
  }

  return []
})

const latestExport = computed(() => exportProgress.value[0] ?? null)

onMounted(async () => {
  await refreshSavedConnections()
  window.addEventListener('keydown', handleGlobalKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
})

watch(activeEditorTabId, () => {
  resultView.value = activeEditorTab.value?.execution ?? null
})

async function refreshSavedConnections() {
  loading.list = true
  try {
    savedConnections.value = await listSavedConnections()
  } catch (error) {
    message.error(readError(error))
  } finally {
    loading.list = false
  }
}

function openCreateDrawer() {
  fillForm(createConnectionConfig('sqlite'))
  drawerVisible.value = true
}

function openEditDrawer(config: ConnectionConfig) {
  fillForm(config)
  drawerVisible.value = true
}

async function handleSaveConnection() {
  const config = normalizedConfig()
  if (!config.name.trim()) {
    message.warning('请填写连接名称')
    return
  }

  if (!config.database.trim()) {
    message.warning(config.driver === 'sqlite' ? '请填写数据库文件路径' : '请填写数据库名')
    return
  }

  loading.save = true
  try {
    savedConnections.value = await saveConnection(config)
    form.id = config.id
    drawerVisible.value = false
    message.success('连接已保存')
  } catch (error) {
    message.error(readError(error))
  } finally {
    loading.save = false
  }
}

async function handleTestConnection() {
  loading.test = true
  try {
    await testConnection(normalizedConfig())
    message.success('连接测试成功')
  } catch (error) {
    message.error(readError(error))
  } finally {
    loading.test = false
  }
}

async function handleConnect(config: ConnectionConfig) {
  loading.connect = true
  try {
    const workspace = await connectDatabase(config)
    tableSelectionVersion.value += 1
    activeConnectionId.value = workspace.connection_id
    activeWorkspace.value = workspace
    selectedDatabase.value = null
    selectedTable.value = null
    selectedSchema.value = null
    resultView.value = null
    loading.schema = false
    workspaceTab.value = 'data'
    resetEditorTabs(driverMeta[workspace.driver].sampleSql)
    selectedTreeKeys.value = []
    expandedTreeKeys.value = defaultExpandedTreeKeys(workspace)
    await refreshSavedConnections()
    message.success(`已连接到 ${workspace.name}`)
  } catch (error) {
    message.error(readError(error))
  } finally {
    loading.connect = false
  }
}

async function handleConnectFromDrawer() {
  const config = normalizedConfig()
  if (!config.name.trim()) {
    message.warning('请填写连接名称')
    return
  }
  await handleConnect(config)
}

async function handleDisconnect(id: string) {
  try {
    savedConnections.value = await disconnectDatabase(id)
    if (activeConnectionId.value === id) {
      clearWorkspace()
    }
    message.success('连接已断开')
  } catch (error) {
    message.error(readError(error))
  }
}

async function handleDelete(id: string) {
  try {
    savedConnections.value = await deleteSavedConnection(id)
    if (activeConnectionId.value === id) {
      clearWorkspace()
    }
    message.success('已删除保存连接')
  } catch (error) {
    message.error(readError(error))
  }
}

async function handleRefreshWorkspace() {
  if (!activeConnectionId.value) {
    return
  }

  loading.refresh = true
  try {
    const workspace = await loadConnectionWorkspace(activeConnectionId.value)
    activeWorkspace.value = workspace
    expandedTreeKeys.value = defaultExpandedTreeKeys(workspace)
    message.success('资源树已刷新')
  } catch (error) {
    message.error(readError(error))
  } finally {
    loading.refresh = false
  }
}

async function selectTable(databaseName: string, tableName: string) {
  if (!activeConnectionId.value) {
    return
  }

  const selectionVersion = ++tableSelectionVersion.value
  loading.schema = true
  try {
    selectedDatabase.value = databaseName
    selectedTable.value = tableName
    selectedSchema.value = null
    resultView.value = null
    const schema = await describeTable(activeConnectionId.value, databaseName, tableName)
    if (selectionVersion !== tableSelectionVersion.value) {
      return
    }
    selectedSchema.value = schema
    const tab = openTableQueryTab(databaseName, tableName, schema)
    workspaceTab.value = 'data'
    editorSplitSize.value = 0.22
    expandedTreeKeys.value = Array.from(
      new Set([
        ...expandedTreeKeys.value,
        `connection:${activeWorkspace.value?.connection_id}`,
        `database:${databaseName}`,
        `group:${databaseName}:tables`,
        `table:${databaseName}:${tableName}`,
      ]),
    )
    if (tab) {
      tab.execution = null
      if (activeEditorTabId.value === tab.id) {
        resultView.value = null
      }
      const result = await runQueryForTab(tab.id, selectionVersion)
      if (selectionVersion === tableSelectionVersion.value && result) {
        resultView.value = result
      }
    }
  } catch (error) {
    if (selectionVersion === tableSelectionVersion.value) {
      message.error(readError(error))
    }
  } finally {
    if (selectionVersion === tableSelectionVersion.value) {
      loading.schema = false
    }
  }
}

async function handleTreeSelect(keys: Array<string | number>, options: Array<TreeOption | null>) {
  selectedTreeKeys.value = keys.map((item) => String(item))
  const option = options[0] as ExplorerTreeOption | null
  if (!option || option.rawType !== 'table' || !option.tableName || !option.databaseName) {
    return
  }

  await selectTable(option.databaseName, option.tableName)
}

async function handleRunSql() {
  if (!activeConnectionId.value) {
    message.warning('请先建立连接')
    return
  }

  if (!activeEditorTab.value) {
    message.warning('请先创建 SQL 标签')
    return
  }

  if (!activeEditorTab.value.sql.trim()) {
    message.warning('请输入 SQL')
    return
  }

  loading.run = true
  try {
    const result = await executeSql(activeConnectionId.value, activeEditorTab.value.sql)
    const targetTab = editorTabs.value.find((tab) => tab.id === activeEditorTab.value?.id)
    const executedAt = new Date().toLocaleString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      month: '2-digit',
      day: '2-digit',
    })
    if (targetTab) {
      targetTab.execution = result
      targetTab.lastRunAt = executedAt
    }
    pushQueryHistory(
      result.mode,
      activeEditorTab.value.title,
      activeEditorTab.value.sql,
      executedAt,
    )
    resultView.value = result
    workspaceTab.value = 'data'
    if (result.mode === 'execute') {
      await handleRefreshWorkspace()
    }
    message.success(result.mode === 'query' ? '查询执行完成' : '语句执行完成')
  } catch (error) {
    message.error(readError(error))
  } finally {
    loading.run = false
  }
}

function clearWorkspace() {
  tableSelectionVersion.value += 1
  activeConnectionId.value = null
  activeWorkspace.value = null
  selectedDatabase.value = null
  selectedTable.value = null
  selectedSchema.value = null
  resultView.value = null
  loading.schema = false
  resetEditorTabs(driverMeta.sqlite.sampleSql)
  selectedTreeKeys.value = []
  expandedTreeKeys.value = []
}

function pushQueryHistory(
  mode: SqlExecutionResponse['mode'],
  tabTitle: string,
  sql: string,
  executedAt: string,
) {
  const connectionName = activeWorkspace.value?.name ?? '未连接'
  queryHistory.value.unshift({
    id: createId(),
    connectionName,
    tabTitle,
    sql,
    mode,
    executedAt,
  })
  queryHistory.value = queryHistory.value.slice(0, 30)
}

async function runQueryForTab(tabId: string, selectionVersion?: number) {
  const tab = editorTabs.value.find((item) => item.id === tabId)
  if (!tab || !activeConnectionId.value || !tab.sql.trim()) {
    return null
  }
  if (
    typeof selectionVersion === 'number' &&
    selectionVersion !== tableSelectionVersion.value
  ) {
    return null
  }

  loading.run = true
  try {
    const result = await executeSql(activeConnectionId.value, tab.sql)
    if (
      typeof selectionVersion === 'number' &&
      selectionVersion !== tableSelectionVersion.value
    ) {
      return null
    }
    const executedAt = new Date().toLocaleString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      month: '2-digit',
      day: '2-digit',
    })

    tab.execution = result
    tab.lastRunAt = executedAt
    pushQueryHistory(result.mode, tab.title, tab.sql, executedAt)
    activeEditorTabId.value = tab.id
    return result
  } finally {
    loading.run = false
  }
}

function addEditorTab(initialSql = '', title?: string) {
  const nextTab = createEditorTab(
    initialSql || driverMeta[activeWorkspace.value?.driver ?? 'sqlite'].sampleSql,
    title,
  )
  editorTabs.value.push(nextTab)
  activeEditorTabId.value = nextTab.id
}

function closeEditorTab(tabId: string) {
  if (editorTabs.value.length === 1) {
    const onlyTab = editorTabs.value[0]
    onlyTab.sql = driverMeta[activeWorkspace.value?.driver ?? 'sqlite'].sampleSql
    onlyTab.execution = null
    onlyTab.lastRunAt = null
    onlyTab.title = 'Query 1'
    activeEditorTabId.value = onlyTab.id
    return
  }

  const currentIndex = editorTabs.value.findIndex((tab) => tab.id === tabId)
  if (currentIndex < 0) {
    return
  }

  editorTabs.value.splice(currentIndex, 1)
  if (activeEditorTabId.value === tabId) {
    const fallback = editorTabs.value[Math.max(0, currentIndex - 1)] ?? editorTabs.value[0]
    activeEditorTabId.value = fallback.id
  }
}

function resetEditorTabs(initialSql: string) {
  const firstTab = createEditorTab(initialSql, 'Query 1')
  editorTabs.value = [firstTab]
  activeEditorTabId.value = firstTab.id
}

function openTableQueryTab(databaseName: string, tableName: string, schema?: TableSchema) {
  const title = tableName.includes('.') ? tableName : `${databaseName}.${tableName}`
  const sql = buildSelectSql(databaseName, tableName, schema)
  const existing = editorTabs.value.find((tab) => tab.title === title && tab.sql === sql)
  if (existing) {
    activeEditorTabId.value = existing.id
    return existing
  }

  addEditorTab(sql, title)
  return editorTabs.value[editorTabs.value.length - 1]
}

function handleEditorTabAdd() {
  addEditorTab('', `Query ${editorTabs.value.length + 1}`)
}

function handleEditorTabClose(name: string | number) {
  closeEditorTab(String(name))
}

function reopenHistoryItem(item: QueryHistoryItem) {
  addEditorTab(item.sql, `${item.tabTitle} Replay`)
  historyVisible.value = false
}

function saveCurrentConsole() {
  if (!activeEditorTab.value) {
    message.warning('当前没有可保存的 SQL 控制台')
    return
  }

  const sql = activeEditorTab.value.sql.trim()
  if (!sql) {
    message.warning('空白 SQL 无需保存')
    return
  }

  const savedAt = new Date().toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })

  savedConsoles.value.unshift({
    id: createId(),
    title: activeEditorTab.value.title,
    sql,
    connectionName: activeWorkspace.value?.name ?? '未连接',
    savedAt,
  })
  savedConsoles.value = savedConsoles.value.slice(0, 30)
  message.success('控制台已保存')
}

function reopenSavedConsole(item: SavedConsoleItem) {
  addEditorTab(item.sql, `${item.title} Saved`)
  savedConsoleVisible.value = false
}

function removeSavedConsole(id: string) {
  savedConsoles.value = savedConsoles.value.filter((item) => item.id !== id)
}

function focusCurrentObject() {
  if (selectedSchema.value) {
    workspaceTab.value = 'schema'
    return
  }

  if (activeEditorTab.value) {
    workspaceTab.value = 'data'
    return
  }

  message.info('当前没有可定位的对象')
}

function notifyNavigation(label: string) {
  message.info(`${label} 入口已预留，后续继续实现`)
}

function exportCurrentResult() {
  const queryResult = resultView.value?.query_result
  if (!queryResult) {
    message.warning('当前没有可导出的查询结果')
    return
  }

  const columns = queryResult.columns.map((column) => column.name)
  const csvLines = [
    columns.join(','),
    ...queryResult.rows.map((row) =>
      columns.map((column) => escapeCsvCell(formatDbValue(row[column]))).join(','),
    ),
  ]

  const blob = new Blob([`\uFEFF${csvLines.join('\n')}`], {
    type: 'text/csv;charset=utf-8;',
  })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  const title = sanitizeFileName(activeEditorTab.value?.title ?? 'query-result')
  link.href = url
  link.download = `${title}.csv`
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)

  const finishedAt = new Date().toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })

  exportProgress.value.unshift({
    id: createId(),
    title,
    rows: queryResult.row_count,
    status: 'success',
    finishedAt,
  })
  exportProgress.value = exportProgress.value.slice(0, 30)
  message.success('CSV 已导出')
}

function fillForm(config: ConnectionConfig) {
  form.id = config.id
  form.name = config.name
  form.driver = config.driver
  form.host = config.host
  form.port = config.port
  form.database = config.database
  form.username = config.username
  form.password = config.password
  form.ssl = config.ssl
  form.options = { ...config.options }
}

function handleDriverChange(driver: DriverKind) {
  const next = createConnectionConfig(driver)
  form.driver = driver
  form.host = next.host
  form.port = next.port
  form.database = next.database
  form.username = next.username
  form.password = next.password
  form.ssl = next.ssl
}

function normalizedConfig(): ConnectionConfig {
  return {
    id: form.id || createId(),
    name: form.name.trim(),
    driver: form.driver,
    host: form.driver === 'sqlite' ? '' : form.host.trim(),
    port: form.driver === 'sqlite' ? 0 : Number(form.port) || driverMeta[form.driver].port,
    database: form.database.trim(),
    username: form.driver === 'sqlite' ? '' : form.username.trim(),
    password: form.driver === 'sqlite' ? '' : form.password,
    ssl: form.driver === 'sqlite' ? false : form.ssl,
    options: { ...form.options },
  }
}

function createConnectionConfig(driver: DriverKind): ConnectionConfig {
  if (driver === 'sqlite') {
    return {
      id: createId(),
      name: 'Local SQLite',
      driver,
      host: '',
      port: 0,
      database: './databox.sqlite',
      username: '',
      password: '',
      ssl: false,
      options: {},
    }
  }

  return {
    id: createId(),
    name: `${driverMeta[driver].label} Connection`,
    driver,
    host: '127.0.0.1',
    port: driverMeta[driver].port,
    database:
      driver === 'postgres'
        ? 'postgres'
        : driver === 'mysql'
          ? 'mysql'
          : driver === 'mongo'
            ? 'admin'
            : '0',
    username: driver === 'redis' ? '' : 'root',
    password: '',
    ssl: false,
    options: {},
  }
}

function createId() {
  return globalThis.crypto?.randomUUID?.() ?? `dbx-${Date.now()}-${Math.random()}`
}

function createEditorTab(initialSql: string, title?: string): SqlEditorTab {
  return {
    id: createId(),
    title: title ?? 'Query',
    sql: initialSql,
    execution: null,
    lastRunAt: null,
  }
}

function buildSelectSql(databaseName: string, tableName: string, schema?: TableSchema) {
  const driver = activeWorkspace.value?.driver ?? 'sqlite'
  const resolvedSchema = schema?.schema ?? null
  const resolvedTable = schema?.name ?? tableName

  if (driver === 'mysql') {
    return `SELECT * FROM ${quoteIdentifier(databaseName, '`')}.${quoteIdentifier(resolvedTable, '`')} LIMIT 100;`
  }

  if (driver === 'postgres') {
    const schemaName = resolvedSchema ?? splitQualifiedTableName(tableName).schema ?? 'public'
    const table = schema?.name ?? splitQualifiedTableName(tableName).table ?? tableName
    return `SELECT * FROM ${quoteIdentifier(schemaName, '"')}.${quoteIdentifier(table, '"')} LIMIT 100;`
  }

  if (driver === 'mongo') {
    const collection = schema?.name ?? splitQualifiedTableName(tableName).table ?? tableName
    return JSON.stringify(
      {
        action: 'find',
        database: databaseName,
        collection,
        filter: {},
        options: { limit: 100 },
      },
      null,
      2,
    )
  }

  if (driver === 'redis') {
    return `GET ${JSON.stringify(resolvedTable)}`
  }

  return `SELECT * FROM ${quoteIdentifier(resolvedTable, '"')} LIMIT 100;`
}

function quoteIdentifier(identifier: string, quoteChar: '"' | '`') {
  if (quoteChar === '"') {
    return `"${identifier.replace(/"/g, '""')}"`
  }

  return `\`${identifier.replace(/`/g, '``')}\``
}

function splitQualifiedTableName(value: string) {
  const [schema, ...tableParts] = value.split('.')
  if (!tableParts.length) {
    return { schema: null, table: schema || null }
  }

  return {
    schema: schema || null,
    table: tableParts.join('.') || null,
  }
}

function readError(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

function formatDbValue(value?: DbValue) {
  if (!value) {
    return ''
  }

  if (value.type === 'Null') {
    return 'NULL'
  }

  if (value.type === 'Json' || value.type === 'Array') {
    return JSON.stringify(value.value)
  }

  if (value.type === 'Bytes') {
    const bytes = Array.isArray(value.value) ? value.value : []
    return `0x${bytes
      .slice(0, 12)
      .map((item) => Number(item).toString(16).padStart(2, '0'))
      .join('')}${bytes.length > 12 ? '…' : ''}`
  }

  return String(value.value ?? '')
}

function escapeCsvCell(value: string) {
  const normalized = value.replace(/"/g, '""')
  return `"${normalized}"`
}

function sanitizeFileName(value: string) {
  return value
    .replace(/[\\/:*?"<>|]/g, '-')
    .replace(/\s+/g, '-')
    .toLowerCase()
}

function buildSelectedTableChildren(schema: TableSchema): ExplorerTreeOption[] {
  const columnNodes = schema.columns.map((column) => ({
    key: `column:${schema.name}:${column.name}`,
    label: column.name,
    rawType: 'column' as const,
    isLeaf: true,
  }))

  const keyNodes = schema.primary_keys.map((columnName) => ({
    key: `key:${schema.name}:${columnName}`,
    label: columnName,
    rawType: 'key' as const,
    isLeaf: true,
  }))

  const indexNodes = schema.indexes.map((index) => ({
    key: `index:${schema.name}:${index.name}`,
    label: index.name,
    rawType: 'index' as const,
    isLeaf: true,
  }))

  return [
    {
      key: `group:${schema.name}:columns`,
      label: 'columns',
      rawType: 'columns_group',
      children: columnNodes,
    },
    {
      key: `group:${schema.name}:keys`,
      label: 'keys',
      rawType: 'keys_group',
      children: keyNodes,
    },
    {
      key: `group:${schema.name}:indexes`,
      label: 'indexes',
      rawType: 'indexes_group',
      children: indexNodes,
    },
  ]
}

function defaultExpandedTreeKeys(workspace: ConnectionWorkspace) {
  return [
    `connection:${workspace.connection_id}`,
    `database:${workspace.database}`,
    `group:${workspace.database}:tables`,
  ]
}

function renderTreeLabel({ option }: { option: TreeOption }) {
  const node = option as ExplorerTreeOption
  if (node.rawType === 'connection') {
    return h('div', { class: 'tree-label tree-label-database' }, [
      h('span', { class: 'tree-main' }, node.label as string),
      h('span', { class: 'tree-meta' }, 'CONNECTION'),
    ])
  }

  if (node.rawType === 'database') {
    return h('div', { class: 'tree-label tree-label-database' }, [
      h('span', { class: 'tree-main' }, (node.databaseName ?? node.label) as string),
      h('span', { class: 'tree-meta' }, 'SCHEMA'),
    ])
  }

  if (node.rawType === 'group' || node.rawType.endsWith('_group')) {
    return h('div', { class: 'tree-label tree-label-group' }, [
      h('span', { class: 'tree-main' }, String(node.label ?? '')),
      h('span', { class: 'tree-meta' }, 'GROUP'),
    ])
  }

  if (node.rawType === 'column') {
    return h('div', { class: 'tree-label' }, [
      h('span', { class: 'tree-main' }, String(node.label ?? '')),
      h('span', { class: 'tree-meta' }, 'COLUMN'),
    ])
  }

  if (node.rawType === 'index' || node.rawType === 'key') {
    return h('div', { class: 'tree-label' }, [
      h('span', { class: 'tree-main' }, String(node.label ?? '')),
      h('span', { class: 'tree-meta' }, node.rawType === 'key' ? 'KEY' : 'INDEX'),
    ])
  }

  return h('div', { class: 'tree-label' }, [
    h('span', { class: 'tree-main' }, node.tableName ?? ''),
    h('span', { class: 'tree-meta' }, node.tableType ?? ''),
  ])
}

function renderTreePrefix({ option }: { option: TreeOption }) {
  const node = option as ExplorerTreeOption
  const labelMap: Record<string, string> = {
    connection: 'CN',
    database: 'DB',
    group: 'GP',
    table: 'TB',
    columns_group: 'CL',
    indexes_group: 'IX',
    keys_group: 'KY',
    column: 'C',
    index: 'I',
    key: 'K',
  }
  return h('span', { class: 'tree-prefix' }, labelMap[node.rawType] ?? 'N')
}

function totalSchemaColumns(columns: ColumnSchema[]) {
  return columns.length
}

function handleExpandedKeys(keys: Array<string | number>) {
  expandedTreeKeys.value = keys.map((item) => String(item))
}

function handleEditorSplit(size: number) {
  editorSplitSize.value = size
}

function handleGlobalKeydown(event: KeyboardEvent) {
  const isModifier = event.metaKey || event.ctrlKey
  if (!isModifier) {
    return
  }

  const key = event.key.toLowerCase()

  if (key === 's' && !event.shiftKey) {
    event.preventDefault()
    saveCurrentConsole()
    return
  }

  if (key === 'l' && event.shiftKey) {
    event.preventDefault()
    handleEditorTabAdd()
    return
  }

  if (key === 'r' && !event.shiftKey) {
    event.preventDefault()
    void handleRunSql()
  }
}
</script>

<template>
  <n-layout class="chat-layout">
    <n-layout-header bordered class="layout-header">
      <div class="toolbar-bar">
        <div class="toolbar-actions">
          <n-button type="primary" secondary @click="openCreateDrawer">
            <template #icon>
              <n-icon :component="Database" />
            </template>
            Create Connection
          </n-button>
          <n-button :disabled="!activeConnectionId" @click="handleRefreshWorkspace">
            <template #icon>
              <n-icon :component="Refresh" />
            </template>
            Refresh
          </n-button>
          <n-button :disabled="!activeConnectionId" @click="focusCurrentObject">
            <template #icon>
              <n-icon :component="Crosshair" />
            </template>
            Locate
          </n-button>
        </div>

        <div class="header-search">
          <n-input v-model:value="explorerSearch" clearable placeholder="搜索连接、数据库或表">
            <template #prefix>
              <n-icon :component="Search" />
            </template>
          </n-input>
        </div>

        <div class="toolbar-actions">
          <n-button
            type="primary"
            :disabled="!activeConnectionId"
            :loading="loading.run"
            @click="handleRunSql"
          >
            <template #icon>
              <n-icon :component="PlayerPlay" />
            </template>
            Run
          </n-button>
        </div>
      </div>
    </n-layout-header>

    <n-layout has-sider class="layout-body">
      <n-split :default-size="0.24" :max="0.4" :min="0.16" resize-trigger-size="2">
        <template #1>
          <div class="explorer-shell">
            <aside class="nav-rail">
              <div class="rail-logo">D</div>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('个人信息')"
                  >
                    <template #icon>
                      <n-icon :component="UserCircle" />
                    </template>
                  </n-button>
                </template>
                Personal Info
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button class="rail-button" type="primary" secondary circle>
                    <template #icon>
                      <n-icon :component="LayoutSidebarLeftExpand" />
                    </template>
                  </n-button>
                </template>
                Workspace
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('Dashboard')"
                  >
                    <template #icon>
                      <n-icon :component="Dashboard" />
                    </template>
                  </n-button>
                </template>
                Dashboard
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('AI Chat')"
                  >
                    <template #icon>
                      <n-icon :component="Robot" />
                    </template>
                  </n-button>
                </template>
                AI Chat
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('Plugin')"
                  >
                    <template #icon>
                      <n-icon :component="Puzzle" />
                    </template>
                  </n-button>
                </template>
                Plugin
              </n-tooltip>

              <div class="rail-spacer" />

              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button class="rail-button" quaternary circle @click="handleRefreshWorkspace">
                    <template #icon>
                      <n-icon :component="Refresh" />
                    </template>
                  </n-button>
                </template>
                Global Refresh
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('Notifications')"
                  >
                    <template #icon>
                      <n-icon :component="Bell" />
                    </template>
                  </n-button>
                </template>
                Notifications
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('Feedback')"
                  >
                    <template #icon>
                      <n-icon :component="Send" />
                    </template>
                  </n-button>
                </template>
                Feedback
              </n-tooltip>
              <n-tooltip trigger="hover" placement="right">
                <template #trigger>
                  <n-button
                    class="rail-button"
                    quaternary
                    circle
                    @click="notifyNavigation('Upgrade')"
                  >
                    <template #icon>
                      <n-icon :component="Crown" />
                    </template>
                  </n-button>
                </template>
                Upgrade
              </n-tooltip>
            </aside>

            <div class="explorer-panel" :class="{ collapsed: explorerCollapsed }">
              <div class="panel-topbar">
                <div>
                  <strong>View</strong>
                  <small>Database Resource Manager · {{ activeWorkspace?.name ?? '未连接' }}</small>
                </div>
                <n-button
                  class="icon-button"
                  quaternary
                  circle
                  @click="explorerCollapsed = !explorerCollapsed"
                >
                  <template #icon>
                    <n-icon
                      :component="
                        explorerCollapsed ? LayoutSidebarLeftExpand : LayoutSidebarLeftCollapse
                      "
                    />
                  </template>
                </n-button>
              </div>

              <template v-if="!explorerCollapsed">
                <div class="connection-actions">
                  <n-button secondary type="primary" @click="openCreateDrawer">
                    <template #icon>
                      <n-icon :component="Plus" />
                    </template>
                    新建连接
                  </n-button>
                  <n-button :loading="loading.list" @click="refreshSavedConnections">
                    <template #icon>
                      <n-icon :component="Refresh" />
                    </template>
                    同步
                  </n-button>
                  <n-button quaternary @click="historyVisible = true">
                    <template #icon>
                      <n-icon :component="History" />
                    </template>
                    历史
                  </n-button>
                </div>

                <div class="section-block connection-strip">
                  <div class="section-title">
                    <span>连接</span>
                    <n-tag size="small" bordered="false">{{ savedConnections.length }}</n-tag>
                  </div>

                  <div v-if="savedConnections.length" class="connection-chip-list">
                    <button
                      v-for="record in savedConnections"
                      :key="record.config.id"
                      class="connection-chip"
                      :class="{ active: record.config.id === activeConnectionId }"
                      @click="handleConnect(record.config)"
                    >
                      <span class="connection-chip-name">{{ record.config.name }}</span>
                      <span class="connection-chip-meta">
                        {{ record.config.driver === 'sqlite' ? 'sqlite' : record.config.database }}
                      </span>
                    </button>
                  </div>
                  <n-empty v-else size="small" description="还没有保存的连接" />
                </div>

                <div class="section-block tree-section">
                  <div class="section-title">
                    <span>表列表</span>
                    <n-tag size="small" bordered="false">{{ tableCount }}</n-tag>
                  </div>

                  <n-empty
                    v-if="!activeWorkspace"
                    size="small"
                    description="连接后显示数据库表列表"
                  />

                  <n-scrollbar v-else class="table-browser-scroll">
                    <div v-if="browsableDatabases.length" class="table-browser">
                      <section
                        v-for="database in browsableDatabases"
                        :key="database.name"
                        class="table-database-group"
                      >
                        <div class="table-database-label">
                          <span>{{ database.name }}</span>
                          <small>{{ database.tables.length }} tables</small>
                        </div>

                        <div class="table-list">
                          <button
                            v-for="table in database.tables"
                            :key="`${database.name}:${table.name}`"
                            class="table-row"
                            :class="{
                              active:
                                selectedDatabase === database.name && selectedTable === table.name,
                            }"
                            @click="selectTable(database.name, table.name)"
                          >
                            <span class="table-row-name">{{ table.name }}</span>
                            <span class="table-row-type">{{ table.table_type }}</span>
                          </button>

                          <div
                            v-if="
                              selectedSchema && selectedDatabase === database.name && selectedTable
                            "
                            class="table-children"
                          >
                            <div
                              v-if="database.tables.some((table) => table.name === selectedTable)"
                              class="table-child-group"
                            >
                              <div class="table-child-label">columns</div>
                              <div
                                v-for="column in selectedSchema.columns"
                                :key="column.name"
                                class="table-child-item"
                              >
                                {{ column.name }}
                              </div>
                            </div>

                            <div
                              v-if="
                                selectedSchema.primary_keys.length &&
                                database.tables.some((table) => table.name === selectedTable)
                              "
                              class="table-child-group"
                            >
                              <div class="table-child-label">keys</div>
                              <div
                                v-for="key in selectedSchema.primary_keys"
                                :key="key"
                                class="table-child-item"
                              >
                                {{ key }}
                              </div>
                            </div>

                            <div
                              v-if="
                                selectedSchema.indexes.length &&
                                database.tables.some((table) => table.name === selectedTable)
                              "
                              class="table-child-group"
                            >
                              <div class="table-child-label">indexes</div>
                              <div
                                v-for="index in selectedSchema.indexes"
                                :key="index.name"
                                class="table-child-item"
                              >
                                {{ index.name }}
                              </div>
                            </div>
                          </div>
                        </div>
                      </section>
                    </div>
                    <n-empty v-else size="small" description="当前数据库没有可浏览的表" />
                  </n-scrollbar>
                </div>
              </template>
            </div>
          </div>
        </template>

        <template #2>
          <div class="workspace-shell">
            <template v-if="activeWorkspace">
              <n-tabs
                v-model:value="workspaceTab"
                type="segment"
                animated
                class="workspace-tabs"
                :pane-wrapper-style="{ height: '100%' }"
                :pane-style="{ height: '100%' }"
              >
                <n-tab-pane name="data" tab="表数据">
                  <n-card class="workspace-card result-card result-card-full" :bordered="false">
                    <template #header>
                      <div class="card-header">
                        <div>
                          <strong>表数据结果</strong>
                          <p>
                            {{
                              selectedDatabase && selectedTable
                                ? `${selectedDatabase}.${selectedTable}`
                                : '选择左侧表后将自动加载前 100 行数据，也可以手动修改 SQL'
                            }}
                          </p>
                        </div>
                        <div class="card-actions">
                          <n-button quaternary @click="sqlEditorVisible = !sqlEditorVisible">
                            <template #icon>
                              <n-icon :component="Search" />
                            </template>
                            {{ sqlEditorVisible ? '收起 SQL' : '展开 SQL' }}
                          </n-button>
                          <n-button quaternary @click="handleEditorTabAdd">
                            <template #icon>
                              <n-icon :component="Plus" />
                            </template>
                            新标签
                          </n-button>
                          <n-button quaternary @click="saveCurrentConsole">
                            <template #icon>
                              <n-icon :component="DeviceFloppy" />
                            </template>
                            保存 Console
                          </n-button>
                          <n-button
                            quaternary
                            @click="
                              activeSqlDraft = driverMeta[activeWorkspace.driver].sampleSql
                            "
                          >
                            <template #icon>
                              <n-icon :component="Eraser" />
                            </template>
                            示例
                          </n-button>
                          <n-button type="primary" :loading="loading.run" @click="handleRunSql">
                            <template #icon>
                              <n-icon :component="PlayerPlay" />
                            </template>
                            运行 SQL
                          </n-button>
                          <n-button
                            v-if="resultView?.query_result"
                            quaternary
                            @click="exportCurrentResult"
                          >
                            <template #icon>
                              <n-icon :component="Download" />
                            </template>
                            导出 CSV
                          </n-button>
                        </div>
                      </div>
                    </template>

                    <div class="result-layout result-layout-full">
                      <div class="result-tags">
                        <n-tag
                          v-for="item in resultSummary"
                          :key="item"
                          size="small"
                          bordered="false"
                        >
                          {{ item }}
                        </n-tag>
                      </div>

                      <div v-if="sqlEditorVisible" class="sql-editor-panel">
                        <n-tabs
                          v-model:value="activeEditorTabId"
                          type="card"
                          addable
                          closable
                          class="editor-tabs"
                          @add="handleEditorTabAdd"
                          @close="handleEditorTabClose"
                        >
                          <n-tab-pane
                            v-for="tab in editorTabs"
                            :key="tab.id"
                            :name="tab.id"
                            :tab="tab.title"
                            :closable="editorTabs.length > 1"
                          />
                        </n-tabs>

                        <div v-if="activeEditorTab" class="editor-status">
                          <n-tag size="small" bordered="false">
                            {{ activeEditorTab.title }}
                          </n-tag>
                          <n-tag size="small" bordered="false" type="info">
                            {{
                              activeEditorTab.lastRunAt
                                ? `上次执行 ${activeEditorTab.lastRunAt}`
                                : '尚未执行'
                            }}
                          </n-tag>
                        </div>

                        <n-input
                          v-model:value="activeSqlDraft"
                          type="textarea"
                          class="editor-input"
                          :autosize="{ minRows: 4, maxRows: 10 }"
                          placeholder="输入 SQL 语句"
                        />
                      </div>

                      <div class="result-main result-main-full">
                        <template v-if="resultView?.query_result">
                          <div
                            v-if="resultView.query_result.row_count > 0"
                            class="result-table-wrap"
                          >
                            <table class="result-table">
                              <thead>
                                <tr>
                                  <th class="cell-index">#</th>
                                  <th
                                    v-for="(columnName, columnIndex) in queryColumnNames"
                                    :key="`head-${columnName}-${columnIndex}`"
                                  >
                                    {{ columnName }}
                                  </th>
                                </tr>
                              </thead>
                              <tbody>
                                <tr
                                  v-for="(row, rowIndex) in resultView.query_result.rows"
                                  :key="`row-${rowIndex}`"
                                >
                                  <td class="cell-index">{{ rowIndex + 1 }}</td>
                                  <td
                                    v-for="(columnName, columnIndex) in queryColumnNames"
                                    :key="`cell-${rowIndex}-${columnIndex}`"
                                    :title="formatDbValue(row[columnName])"
                                  >
                                    {{ formatDbValue(row[columnName]) || ' ' }}
                                  </td>
                                </tr>
                              </tbody>
                            </table>
                          </div>
                          <n-empty v-else description="当前表暂无数据（0 行）" />
                        </template>

                        <template v-else-if="resultView?.exec_result">
                          <div class="exec-summary">
                            <n-statistic
                              label="影响行数"
                              :value="resultView.exec_result.rows_affected"
                            />
                            <n-statistic
                              label="耗时"
                              :value="resultView.exec_result.elapsed_ms"
                              suffix="ms"
                            />
                            <n-statistic
                              label="插入 ID"
                              :value="resultView.exec_result.last_insert_id ?? '-'"
                            />
                          </div>
                        </template>

                        <n-empty v-else description="运行 SQL 后，这里会显示结果集或执行信息" />
                      </div>
                    </div>
                  </n-card>
                </n-tab-pane>

                <n-tab-pane name="schema" tab="表结构">
                  <n-card class="workspace-card schema-card schema-card-clear" :bordered="false">
                    <template #header>
                      <div class="card-header">
                        <div>
                          <strong>表结构字典</strong>
                          <p>
                            {{
                              selectedSchema
                                ? `${selectedSchema.schema ? `${selectedSchema.schema}.` : ''}${selectedSchema.name}`
                                : '从左侧资源树选择一个表'
                            }}
                          </p>
                        </div>
                        <n-tag v-if="selectedSchema" size="small" bordered="false" type="info">
                          {{ totalSchemaColumns(selectedSchema.columns) }} 列
                        </n-tag>
                      </div>
                    </template>

                    <template v-if="selectedSchema">
                      <div class="schema-page">
                        <div class="schema-metrics">
                          <div class="schema-metric-card">
                            <span>字段数</span>
                            <strong>{{ selectedSchema.columns.length }}</strong>
                          </div>
                          <div class="schema-metric-card">
                            <span>主键字段</span>
                            <strong>{{ selectedSchema.primary_keys.length }}</strong>
                          </div>
                          <div class="schema-metric-card">
                            <span>索引数</span>
                            <strong>{{ selectedSchema.indexes.length }}</strong>
                          </div>
                          <div class="schema-metric-card">
                            <span>可空字段</span>
                            <strong>{{
                              selectedSchema.columns.filter((column) => column.nullable).length
                            }}</strong>
                          </div>
                        </div>

                        <div class="schema-content">
                          <section class="schema-panel schema-columns-panel">
                            <header class="schema-panel-head">
                              <strong>字段定义</strong>
                              <span>{{ selectedSchema.columns.length }} columns</span>
                            </header>
                            <div class="schema-columns-wrap">
                              <table class="schema-columns-table">
                                <thead>
                                  <tr>
                                    <th>字段</th>
                                    <th>类型</th>
                                    <th>可空</th>
                                    <th>默认值</th>
                                    <th>约束</th>
                                    <th>备注</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  <tr v-for="column in selectedSchema.columns" :key="column.name">
                                    <td class="schema-col-name">{{ column.name }}</td>
                                    <td>{{ column.data_type }}</td>
                                    <td>{{ column.nullable ? 'YES' : 'NO' }}</td>
                                    <td>{{ column.default_value ?? '-' }}</td>
                                    <td>
                                      <span v-if="column.is_primary_key" class="schema-flag schema-flag-pk"
                                        >PK</span
                                      >
                                      <span v-if="column.is_unique" class="schema-flag schema-flag-unique"
                                        >UNIQUE</span
                                      >
                                      <span v-if="!column.nullable" class="schema-flag schema-flag-not-null"
                                        >NOT NULL</span
                                      >
                                      <span
                                        v-if="!column.is_primary_key && !column.is_unique && column.nullable"
                                        class="schema-flag schema-flag-muted"
                                      >
                                        -
                                      </span>
                                    </td>
                                    <td>{{ column.comment ?? '-' }}</td>
                                  </tr>
                                </tbody>
                              </table>
                            </div>
                          </section>

                          <section class="schema-panel schema-side-panel">
                            <header class="schema-panel-head">
                              <strong>主键与索引</strong>
                              <span>{{ selectedSchema.indexes.length }} indexes</span>
                            </header>

                            <div class="schema-side-scroll">
                              <div class="schema-side-block">
                                <h4>Primary Keys</h4>
                                <ul v-if="selectedSchema.primary_keys.length" class="schema-chip-list">
                                  <li v-for="key in selectedSchema.primary_keys" :key="key">
                                    {{ key }}
                                  </li>
                                </ul>
                                <p v-else class="schema-muted">无主键</p>
                              </div>

                              <div class="schema-side-block">
                                <h4>Indexes</h4>
                                <div v-if="selectedSchema.indexes.length" class="schema-index-list-v2">
                                  <article
                                    v-for="index in selectedSchema.indexes"
                                    :key="index.name"
                                    class="schema-index-item-v2"
                                  >
                                    <div class="schema-index-top">
                                      <strong>{{ index.name }}</strong>
                                      <div class="schema-flag-group">
                                        <span v-if="index.is_primary" class="schema-flag schema-flag-pk"
                                          >PRIMARY</span
                                        >
                                        <span v-if="index.is_unique" class="schema-flag schema-flag-unique"
                                          >UNIQUE</span
                                        >
                                      </div>
                                    </div>
                                    <p>{{ index.columns.join(', ') || '无列信息' }}</p>
                                  </article>
                                </div>
                                <p v-else class="schema-muted">无索引</p>
                              </div>
                            </div>
                          </section>
                        </div>
                      </div>
                    </template>

                    <n-empty v-else description="选择左侧表后，这里展示列定义和索引信息" />
                  </n-card>
                </n-tab-pane>
              </n-tabs>
            </template>

            <n-card v-else class="workspace-card empty-card" :bordered="false">
              <div class="empty-state">
                <n-icon :component="DatabaseOff" class="empty-icon" />
                <h3>没有活动连接</h3>
                <p>从左侧资源管理器新建或打开一个连接，然后选择表查看表数据和表结构。</p>
                <div class="empty-actions">
                  <n-button type="primary" @click="openCreateDrawer">
                    <template #icon>
                      <n-icon :component="Plus" />
                    </template>
                    新建连接
                  </n-button>
                  <n-button quaternary @click="historyVisible = true">
                    <template #icon>
                      <n-icon :component="History" />
                    </template>
                    查看执行日志
                  </n-button>
                </div>
              </div>
            </n-card>

            <div class="status-bar">
              <div class="status-left">
                <span>Workspace</span>
                <span>{{ activeWorkspace?.name ?? '未连接' }}</span>
                <span>{{ selectedTable ?? '未选择对象' }}</span>
              </div>
              <div class="status-right">
                <span>
                  {{
                    latestExport
                      ? `最近导出 ${latestExport.title} · ${latestExport.rows} rows`
                      : '暂无导出任务'
                  }}
                </span>
                <span v-if="activeEditorTab?.lastRunAt"
                  >Last Run {{ activeEditorTab.lastRunAt }}</span
                >
              </div>
            </div>
          </div>
        </template>
      </n-split>
    </n-layout>

    <n-drawer v-model:show="drawerVisible" :width="420" placement="right">
      <n-drawer-content title="连接设置" closable>
        <n-form label-placement="top">
          <n-form-item label="连接名称">
            <n-input v-model:value="form.name" placeholder="例如：开发库 / 本地 SQLite" />
          </n-form-item>

          <n-form-item label="驱动">
            <n-select
              v-model:value="form.driver"
              :options="driverOptions"
              @update:value="handleDriverChange"
            />
          </n-form-item>

          <template v-if="form.driver === 'sqlite'">
            <n-form-item label="数据库文件">
              <n-input
                v-model:value="form.database"
                placeholder="./databox.sqlite 或 /tmp/demo.db"
              />
            </n-form-item>
          </template>

          <template v-else>
            <n-grid :cols="2" :x-gap="12">
              <n-form-item-gi label="主机">
                <n-input v-model:value="form.host" placeholder="127.0.0.1" />
              </n-form-item-gi>
              <n-form-item-gi label="端口">
                <n-input-number v-model:value="form.port" class="w-full" :min="1" :max="65535" />
              </n-form-item-gi>
            </n-grid>

            <n-form-item label="数据库">
              <n-input v-model:value="form.database" placeholder="数据库名" />
            </n-form-item>

            <n-grid :cols="2" :x-gap="12">
              <n-form-item-gi label="用户名">
                <n-input v-model:value="form.username" placeholder="用户名" />
              </n-form-item-gi>
              <n-form-item-gi label="密码">
                <n-input
                  v-model:value="form.password"
                  type="password"
                  show-password-on="click"
                  placeholder="密码"
                />
              </n-form-item-gi>
            </n-grid>

            <n-form-item label="SSL">
              <n-switch v-model:value="form.ssl" />
            </n-form-item>
          </template>
        </n-form>

        <template #footer>
          <div class="drawer-actions">
            <n-button @click="fillForm(createConnectionConfig(form.driver))">
              <template #icon>
                <n-icon :component="Eraser" />
              </template>
              重置
            </n-button>
            <n-button :loading="loading.test" @click="handleTestConnection">
              <template #icon>
                <n-icon :component="TestPipe" />
              </template>
              测试
            </n-button>
            <n-button
              secondary
              type="primary"
              :loading="loading.save"
              @click="handleSaveConnection"
            >
              <template #icon>
                <n-icon :component="DeviceFloppy" />
              </template>
              保存
            </n-button>
            <n-button type="primary" :loading="loading.connect" @click="handleConnectFromDrawer">
              <template #icon>
                <n-icon :component="PlugConnected" />
              </template>
              连接
            </n-button>
          </div>
        </template>
      </n-drawer-content>
    </n-drawer>

    <n-drawer v-model:show="historyVisible" :width="520" placement="right">
      <n-drawer-content title="执行历史" closable>
        <div v-if="queryHistory.length" class="history-list">
          <div v-for="item in queryHistory" :key="item.id" class="history-item">
            <div class="history-head">
              <div>
                <strong>{{ item.tabTitle }}</strong>
                <p>{{ item.connectionName }} · {{ item.executedAt }}</p>
              </div>
              <n-tag
                size="small"
                bordered="false"
                :type="item.mode === 'query' ? 'info' : 'success'"
              >
                {{ item.mode === 'query' ? '查询' : '执行' }}
              </n-tag>
            </div>
            <pre class="history-sql">{{ item.sql }}</pre>
            <div class="history-actions">
              <n-button size="small" type="primary" @click="reopenHistoryItem(item)">
                打开为新标签
              </n-button>
            </div>
          </div>
        </div>
        <n-empty v-else description="还没有执行记录" />
      </n-drawer-content>
    </n-drawer>

    <n-drawer v-model:show="savedConsoleVisible" :width="520" placement="right">
      <n-drawer-content title="已保存的控制台" closable>
        <div v-if="savedConsoles.length" class="history-list">
          <div v-for="item in savedConsoles" :key="item.id" class="history-item">
            <div class="history-head">
              <div>
                <strong>{{ item.title }}</strong>
                <p>{{ item.connectionName }} · {{ item.savedAt }}</p>
              </div>
              <n-space size="small">
                <n-button size="small" type="primary" @click="reopenSavedConsole(item)">
                  打开
                </n-button>
                <n-button size="small" quaternary type="error" @click="removeSavedConsole(item.id)">
                  删除
                </n-button>
              </n-space>
            </div>
            <pre class="history-sql">{{ item.sql }}</pre>
          </div>
        </div>
        <n-empty v-else description="还没有保存的控制台" />
      </n-drawer-content>
    </n-drawer>

    <n-drawer v-model:show="exportVisible" :width="480" placement="right">
      <n-drawer-content title="导出进度" closable>
        <div v-if="exportProgress.length" class="history-list">
          <div v-for="item in exportProgress" :key="item.id" class="history-item">
            <div class="history-head">
              <div>
                <strong>{{ item.title }}</strong>
                <p>{{ item.finishedAt }}</p>
              </div>
              <n-tag size="small" bordered="false" type="success">
                {{ item.status === 'success' ? '完成' : item.status }}
              </n-tag>
            </div>
            <div class="export-meta">
              <span>Rows: {{ item.rows }}</span>
              <span>File: {{ item.title }}.csv</span>
            </div>
          </div>
        </div>
        <n-empty v-else description="还没有导出任务" />
      </n-drawer-content>
    </n-drawer>
  </n-layout>
</template>

<style scoped>
.chat-layout {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #f3f5f7;
}

.layout-header {
  padding: 0 12px;
  background: #ffffff;
  flex-shrink: 0;
}

.toolbar-bar,
.toolbar-actions,
.card-header,
.card-actions,
.connection-row,
.connection-buttons,
.schema-card-head,
.drawer-actions,
.status-bar,
.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.toolbar-bar,
.status-bar {
  justify-content: space-between;
}

.toolbar-bar {
  padding-inline: 4px;
}

.header-search {
  flex: 1;
  max-width: 420px;
  min-width: 220px;
}

.panel-topbar small,
.card-header p,
.connection-row p,
.empty-state p {
  color: #64748b;
}

.layout-body {
  flex: 1;
  min-height: 0;
}

.explorer-shell {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  height: 100%;
  border-right: 1px solid rgba(148, 163, 184, 0.16);
  background: #ffffff;
}

.nav-rail {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 16px 10px;
  border-right: 1px solid rgba(148, 163, 184, 0.16);
  background: #161c27;
}

.rail-logo {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  margin-bottom: 4px;
  border-radius: 10px;
  background: #7c3aed;
  color: #fff;
  font-size: 18px;
  font-weight: 700;
}

.rail-button {
  color: #dbeafe;
}

.rail-spacer {
  flex: 1;
}

.explorer-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
  min-height: 0;
  padding: 14px;
  background: #f8fafc;
}

.explorer-panel.collapsed {
  padding-inline: 10px;
}

.panel-topbar,
.section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.connection-actions {
  display: flex;
  gap: 10px;
}

.section-block,
.workspace-card {
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 12px;
  background: #fff;
  box-shadow: none;
}

.section-block {
  padding: 12px;
}

.tree-section {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.table-browser-scroll {
  flex: 1;
  min-height: 0;
  height: 100%;
  margin-top: 10px;
}

.table-browser {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: min-content;
}

.table-database-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.table-database-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: #475569;
  font-size: 13px;
  font-weight: 600;
}

.table-database-label small {
  color: #94a3b8;
  font-weight: 500;
}

.table-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.table-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 8px;
  background: #fff;
  color: #334155;
  cursor: pointer;
  text-align: left;
}

.table-row:hover {
  background: #f8fbff;
  border-color: rgba(59, 130, 246, 0.25);
}

.table-row.active {
  background: #eef6ff;
  border-color: rgba(59, 130, 246, 0.28);
}

.table-row-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.table-row-type {
  flex-shrink: 0;
  color: #94a3b8;
  font-size: 11px;
  text-transform: uppercase;
}

.table-children {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-left: 16px;
  padding-left: 12px;
  border-left: 1px dashed rgba(148, 163, 184, 0.28);
}

.table-child-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.table-child-label {
  color: #94a3b8;
  font-size: 11px;
  text-transform: lowercase;
}

.table-child-item {
  color: #64748b;
  font-size: 12px;
}

.connection-scroll {
  max-height: 260px;
  margin-top: 10px;
}

.connection-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.connection-strip {
  padding-bottom: 10px;
  flex-shrink: 0;
}

.connection-chip-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 10px;
}

.connection-chip {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 12px;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 10px;
  background: #fff;
  text-align: left;
  cursor: pointer;
}

.connection-chip.active {
  border-color: rgba(59, 130, 246, 0.3);
  background: #eef6ff;
}

.connection-chip-name {
  font-weight: 600;
  color: #334155;
}

.connection-chip-meta {
  font-size: 12px;
  color: #94a3b8;
}

.connection-item {
  padding: 12px;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 10px;
  background: #f8fafc;
}

.connection-item.active {
  border-color: rgba(59, 130, 246, 0.36);
  background: rgba(239, 246, 255, 0.92);
}

.connection-meta {
  margin: 10px 0;
  font-size: 12px;
  line-height: 1.5;
  color: #64748b;
  word-break: break-all;
}

.connection-buttons {
  flex-wrap: wrap;
}

.workspace-shell {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding: 14px;
  background: #eef2f6;
}

.empty-state h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.workspace-tabs {
  flex: 1;
  min-height: 0;
}

.workspace-tabs :deep(.n-tabs-pane-wrapper) {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.workspace-tabs :deep(.n-tab-pane) {
  flex: 1;
  min-height: 0;
}

.workspace-card {
  height: 100%;
}

.editor-card :deep(.n-card__content),
.result-card :deep(.n-card__content),
.schema-card :deep(.n-card__content) {
  height: calc(100% - 8px);
}

.result-card-full :deep(.n-card__content) {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.editor-input :deep(textarea) {
  font-family: 'SFMono-Regular', 'Menlo', monospace;
  line-height: 1.6;
}

.editor-tabs {
  margin-bottom: 12px;
}

.editor-status {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
}

.result-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.result-layout {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-height: 0;
}

.result-layout-full {
  flex: 1;
}

.sql-editor-panel {
  flex-shrink: 0;
  padding: 12px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 10px;
  background: #f8fafc;
}

.result-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}

.result-main-full {
  min-height: 0;
}

.result-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 10px;
  background: #ffffff;
}

.result-table {
  width: max-content;
  min-width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: fixed;
  font-size: 12px;
  color: #334155;
}

.result-table th,
.result-table td {
  max-width: 280px;
  padding: 8px 10px;
  overflow: hidden;
  border-right: 1px solid rgba(148, 163, 184, 0.18);
  border-bottom: 1px solid rgba(148, 163, 184, 0.18);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-table th {
  position: sticky;
  top: 0;
  z-index: 2;
  font-weight: 600;
  text-align: left;
  background: #0f172a;
  color: #e2e8f0;
}

.result-table tbody tr:nth-child(even) td {
  background: #f8fafc;
}

.result-table .cell-index {
  position: sticky;
  left: 0;
  z-index: 1;
  width: 56px;
  min-width: 56px;
  max-width: 56px;
}

.result-table thead .cell-index {
  z-index: 3;
}

.result-table tbody .cell-index {
  background: #ffffff;
  color: #64748b;
}

.result-table tbody tr:nth-child(even) .cell-index {
  background: #f8fafc;
}

.export-side-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-left: 12px;
  border-left: 1px solid rgba(148, 163, 184, 0.14);
}

.export-side-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 28px;
}

.export-side-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.export-side-item {
  padding: 10px;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 10px;
  background: #fafafa;
}

.export-side-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  color: #334155;
}

.export-side-meta {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  margin-top: 6px;
  font-size: 12px;
  color: #94a3b8;
}

.exec-summary {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.schema-card-clear :deep(.n-card__content) {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.schema-page {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-height: 0;
}

.schema-metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.schema-metric-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 10px;
  background: #f8fafc;
}

.schema-metric-card span {
  color: #64748b;
  font-size: 12px;
}

.schema-metric-card strong {
  color: #0f172a;
  font-size: 18px;
}

.schema-content {
  display: grid;
  grid-template-columns: minmax(0, 1.8fr) minmax(280px, 0.85fr);
  gap: 12px;
  flex: 1;
  min-height: 0;
}

.schema-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 12px;
  background: #ffffff;
}

.schema-panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.16);
}

.schema-panel-head strong {
  color: #0f172a;
  font-size: 14px;
}

.schema-panel-head span {
  color: #64748b;
  font-size: 12px;
}

.schema-columns-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.schema-columns-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 12px;
  color: #334155;
}

.schema-columns-table th,
.schema-columns-table td {
  padding: 8px 10px;
  text-align: left;
  border-bottom: 1px solid rgba(148, 163, 184, 0.15);
  border-right: 1px solid rgba(148, 163, 184, 0.12);
  vertical-align: top;
}

.schema-columns-table th:last-child,
.schema-columns-table td:last-child {
  border-right: 0;
}

.schema-columns-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: #f1f5f9;
  color: #0f172a;
  font-weight: 600;
}

.schema-columns-table tbody tr:nth-child(even) td {
  background: #fbfdff;
}

.schema-col-name {
  font-weight: 600;
  color: #0f172a;
}

.schema-flag-group {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.schema-flag {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  line-height: 1.2;
  color: #0f172a;
  background: #e2e8f0;
}

.schema-flag-pk {
  background: #fef3c7;
  color: #92400e;
}

.schema-flag-unique {
  background: #dcfce7;
  color: #166534;
}

.schema-flag-not-null {
  background: #fee2e2;
  color: #991b1b;
}

.schema-flag-muted {
  background: #f1f5f9;
  color: #64748b;
}

.schema-side-panel {
  min-width: 0;
}

.schema-side-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.schema-side-block h4 {
  margin: 0 0 8px;
  color: #0f172a;
  font-size: 13px;
}

.schema-chip-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.schema-chip-list li {
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid rgba(148, 163, 184, 0.24);
  background: #f8fafc;
  color: #334155;
  font-size: 12px;
}

.schema-index-list-v2 {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.schema-index-item-v2 {
  padding: 10px;
  border-radius: 10px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  background: #f8fafc;
}

.schema-index-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.schema-index-top strong {
  color: #0f172a;
  font-size: 13px;
}

.schema-index-item-v2 p {
  margin: 8px 0 0;
  color: #475569;
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}

.schema-muted {
  margin: 0;
  color: #94a3b8;
  font-size: 12px;
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.history-item {
  padding: 14px;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 10px;
  background: #f8fafc;
}

.history-head,
.history-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.history-head p {
  margin: 4px 0 0;
  color: #64748b;
}

.export-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  color: #475569;
  font-size: 12px;
}

.history-sql {
  margin: 12px 0;
  padding: 12px;
  overflow: auto;
  border-radius: 12px;
  background: #0f172a;
  color: #dbeafe;
  font-family: 'SFMono-Regular', 'Menlo', monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.status-bar {
  padding: 8px 12px;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 10px;
  background: #fff;
  color: #475569;
  font-size: 12px;
}

.status-left,
.status-right {
  flex-wrap: wrap;
}

.status-left span:not(:first-child),
.status-right span {
  padding: 2px 8px;
  border-radius: 999px;
  background: #f1f5f9;
}

.empty-card {
  display: grid;
  place-items: center;
  min-height: 420px;
}

.empty-state {
  display: grid;
  justify-items: center;
  gap: 10px;
  max-width: 560px;
  text-align: center;
}

.empty-actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: center;
}

.empty-icon {
  font-size: 56px;
  color: #64748b;
}

.icon-button {
  flex-shrink: 0;
}

:deep(.n-tree-node-content) {
  min-width: 0;
}

.table-browser-scroll :deep(.n-scrollbar-container),
.table-browser-scroll :deep(.n-scrollbar-content) {
  min-height: 100%;
}

:deep(.tree-label) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
}

:deep(.tree-main) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:deep(.tree-meta) {
  flex-shrink: 0;
  font-size: 11px;
  color: #94a3b8;
  text-transform: uppercase;
}

@media (max-width: 1280px) {
  .exec-summary,
  .schema-metrics,
  .schema-content {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 900px) {
  .layout-header {
    gap: 10px;
    padding: 12px;
  }

  .toolbar-bar,
  .connection-actions,
  .card-header,
  .drawer-actions,
  .status-bar {
    flex-wrap: wrap;
  }
}
</style>
