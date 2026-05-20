<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import {
  NDataTable,
  NTag,
  NSpin,
  NEmpty,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import { useConnectionsStore } from '@/stores/connections'

const props = defineProps<{
  visible: boolean
  tableNodeId: string | null
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

const store = useConnectionsStore()

const columns = computed<DataTableColumns<any>>(() => {
  if (!props.tableNodeId) return []
  const tableNode = (() => {
    for (const db of store.treeNodes) {
      for (const t of (db.children || [])) {
        if (t.id === props.tableNodeId) return t
      }
    }
    return null
  })()
  const cols = (tableNode?.children || []).filter((c: any) => c.kind === 'column')
  return cols.map((col: any) => ({
    title: col.name,
    key: col.name,
    width: 160,
    resizable: true,
    render() {
      const parts: any[] = [col.name]
      if (col.dataType) {
        parts.push(h(NTag, { size: 'small', style: 'margin-left:6px' }, { default: () => col.dataType }))
      }
      return parts
    },
  }))
})

const rows = computed(() => {
  if (!props.tableNodeId) return []
  for (const db of store.treeNodes) {
    for (const t of (db.children || [])) {
      if (t.id === props.tableNodeId) return (t.children || []).filter((c: any) => c.kind === 'column')
    }
  }
  return []
})

function onClose() {
  emit('update:visible', false)
}
</script>

<template>
  <NDrawer
    :show="visible"
    :width="400"
    placement="right"
    :native-scrollbar="false"
    @update:show="onClose"
  >
    <NDrawerContent
      :title="tableNodeId ? (() => { const m = tableNodeId.match(/-tbl-(.+)$/); return m ? m[1] : '表详情' })() : '表详情'"
      :native-scrollbar="false"
      closable
    >
      <NSpin :show="store.loading">
        <template v-if="tableNodeId && columns.length > 0">
          <NDataTable
            :columns="columns"
            :data="rows"
            :row-key="(row: any) => row.name"
            :max-height="'calc(100vh - 260px)'"
            :striped="true"
            :bordered="false"
            size="small"
          />
        </template>
        <NEmpty v-else description="请选择一张表以查看列信息" />
      </NSpin>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped>
</style>
