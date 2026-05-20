<script setup lang="ts">
import { computed, h } from 'vue'
import {
  NTree,
  NButton,
  NSpin,
} from 'naive-ui'
import type { TreeOption } from 'naive-ui'
import type { TreeNode } from '@/types/database'
import { useConnectionsStore } from '@/stores/connections'

const props = defineProps<{
  nodes: TreeNode[]
  expandedKeys: string[]
  selectedKey?: string | null
  loading?: boolean
}>()

const emit = defineEmits<{
  expand: [keys: string[]]
  select: [key: string]
  'new-connection': []
}>()

const store = useConnectionsStore()

function nodeIcon(node: TreeNode): string {
  switch (node.kind) {
    case 'connection':
      return '🗄️'
    case 'database':
      return '📁'
    case 'schema':
      return '📂'
    case 'table':
      return '📋'
    case 'column':
      return '│'
    default:
      return ''
  }
}

const treeData = computed<TreeOption[]>(() =>
  props.nodes.map(node => ({
    key: node.id,
    label: node.name,
    isLeaf: node.kind === 'column',
    prefix: () =>
      h('span', { class: 'tree-node-icon' }, nodeIcon(node)),
    children: node.children?.map(child => ({
      key: child.id,
      label: child.name,
      isLeaf: child.kind === 'column',
      prefix: () =>
        h('span', { class: 'tree-node-icon' }, nodeIcon(child)),
    })),
  }))
)

function onExpand(keys: string[]) {
  emit('expand', keys)
}

function onSelect(keys: string[]) {
  if (keys.length === 0) return
  emit('select', keys[0])
}
</script>

<template>
  <div class="connection-tree">
    <div class="tree-header">
      <span class="tree-title">连接浏览器</span>
      <NButton text size="small" @click="emit('new-connection')">
        + 新建
      </NButton>
    </div>
    <NSpin :show="loading">
      <NTree
        :data="treeData"
        :expanded-keys="expandedKeys"
        :selected-keys="selectedKey ? [selectedKey] : []"
        selectable
        :block-line="true"
        @update:expanded-keys="onExpand"
        @update:selected-keys="onSelect"
      />
    </NSpin>
  </div>
</template>

<style scoped>
.connection-tree {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.tree-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 10px;
  border-bottom: 1px solid var(--db-border);
  flex-shrink: 0;
}
.tree-title {
  font-weight: 600;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--n-text-color-3);
}
.tree-node-icon {
  font-size: 13px;
  line-height: 1;
  margin-right: 4px;
  width: 16px;
  display: inline-block;
  text-align: center;
}
</style>
