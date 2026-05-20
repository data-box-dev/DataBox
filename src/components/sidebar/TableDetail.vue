<script setup lang="ts">
import { ref, watch, computed, h } from 'vue'
import {
  NDrawer,
  NDrawerContent,
  NDataTable,
  NTag,
  NSpace,
  NText,
  NSpin,
  NEmpty,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import type { ColumnSchema, TableSchema } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

const props = defineProps<{
  visible: boolean
  tableNodeId: string | null
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

const schema = ref<TableSchema | null>(null)
const loading = ref(false)

watch(
  () => props.visible,
  async (visible) => {
    if (visible && props.tableNodeId) {
      await loadSchema()
    } else if (!visible) {
      schema.value = null
    }
  },
)

watch(
  () => props.tableNodeId,
  async (nodeId) => {
    if (props.visible && nodeId) {
      await loadSchema()
    }
  },
)

async function loadSchema() {
  if (!props.tableNodeId) return
  // Parse: "${connId}-db-${dbName}-tbl-${tableName}"
  const match = props.tableNodeId.match(/^(.+)-db-(.+)-tbl-(.+)$/)
  if (!match) return
  const [, connId, , tableName] = match

  loading.value = true
  try {
    schema.value = await tauriCommands.describeTable(connId, '', tableName)
  } catch {
    schema.value = null
  } finally {
    loading.value = false
  }
}

const columns = computed<DataTableColumns<ColumnSchema>>(() => {
  if (!schema.value) return []
  return [
    {
      title: '列名',
      key: 'name',
      width: 160,
      resizable: true,
      render(row) {
        const parts = [
          h(NText, { depth: row.isPrimaryKey ? 1 : 3 }, { default: () => row.name }),
        ]
        if (row.isPrimaryKey) {
          parts.push(h(NTag, { size: 'tiny', type: 'warning', bordered: false }, { default: () => 'PK' }))
        }
        if (row.isUnique) {
          parts.push(h(NTag, { size: 'tiny', type: 'info', bordered: false }, { default: () => 'UQ' }))
        }
        return h(NSpace, { size: 4, align: 'center' }, { default: () => parts })
      },
    },
    {
      title: '类型',
      key: 'dataType',
      width: 120,
      render(row) {
        return h(NTag, { size: 'small', type: 'default' }, { default: () => row.dataType })
      },
    },
    {
      title: '可空',
      key: 'nullable',
      width: 70,
      render(row) {
        return row.nullable ? 'YES' : 'NO'
      },
    },
    {
      title: '默认值',
      key: 'defaultValue',
      render(row) {
        return row.defaultValue || '—'
      },
    },
    {
      title: '备注',
      key: 'comment',
      render(row) {
        return row.comment || '—'
      },
    },
  ]
})

function onClose() {
  emit('update:visible', false)
}
</script>

<template>
  <NDrawer
    :show="visible"
    :width="420"
    placement="right"
    :native-scrollbar="false"
    @update:show="onClose"
  >
    <NDrawerContent
      :title="schema?.name || '表详情'"
      :native-scrollbar="false"
      closable
    >
      <NSpin :show="loading">
        <template v-if="schema">
          <div class="table-detail">
            <div class="detail-section">
              <div class="detail-title">列信息 ({{ schema.columns.length }})</div>
              <NDataTable
                :columns="columns"
                :data="schema.columns"
                :row-key="(col: ColumnSchema) => col.name"
                :max-height="'calc(100vh - 280px)'"
                :striped="true"
                :bordered="false"
                :single-line="false"
              />
            </div>
            <div v-if="schema.indexes.length > 0" class="detail-section">
              <div class="detail-title">索引 ({{ schema.indexes.length }})</div>
              <div
                v-for="idx in schema.indexes"
                :key="idx.name"
                class="index-item"
              >
                <NSpace :size="6" align="center">
                  <NTag
                    v-if="idx.isPrimary"
                    size="tiny"
                    type="warning"
                    :bordered="false"
                  >
                    PK
                  </NTag>
                  <NTag
                    v-if="idx.isUnique"
                    size="tiny"
                    type="info"
                    :bordered="false"
                  >
                    UQ
                  </NTag>
                  <span class="index-name">{{ idx.name }}</span>
                  <span class="index-cols">
                    ({{ idx.columns.join(', ') }})
                  </span>
                </NSpace>
              </div>
            </div>
          </div>
        </template>
        <NEmpty v-else description="未找到表信息" />
      </NSpin>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped>
.table-detail {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
}
.detail-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.detail-title {
  font-weight: 600;
  font-size: 13px;
  color: var(--n-text-color-1);
  padding-bottom: 4px;
  border-bottom: 1px solid var(--n-border-color);
}
.index-item {
  padding: 4px 0;
  font-size: 13px;
}
.index-name {
  font-weight: 500;
}
.index-cols {
  color: var(--n-text-color-3);
  font-size: 12px;
}
</style>
