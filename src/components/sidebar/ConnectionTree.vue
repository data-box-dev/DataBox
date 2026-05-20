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

const treeData = computed<TreeOption[]>(() =>
  props.nodes.map((node) => ({
    key: node.id,
    label: node.name,
    isLeaf: node.kind === 'column',
    children: node.children?.map((child) => ({
      key: child.id,
      label: child.name,
      isLeaf: child.kind === 'column',
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
        :checkable="false"
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
  padding: 8px 12px;
  border-bottom: 1px solid var(--n-border-color);
  flex-shrink: 0;
}
.tree-title {
  font-weight: 600;
  font-size: 14px;
}
</style>
