<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  NPopover,
  NButton,
  NInput,
  NSpace,
  NTag,
  NText,
  NEmpty,
  NScrollbar,
} from 'naive-ui'
import { useSavedQueriesStore } from '@/stores/savedQueries'
import { useQueryStore } from '@/stores/query'

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

const props = defineProps<{
  visible: boolean
}>()

const savedStore = useSavedQueriesStore()
const queryStore = useQueryStore()

const showSaveForm = ref(false)
const queryName = ref('')

const isOpen = computed(() => props.visible)

function onSaveConfirm() {
  if (!queryStore.editorContent.trim()) return
  const name = queryName.value.trim() || `查询 ${new Date().toLocaleString('zh-CN')}`
  savedStore.add(name, queryStore.editorContent)
  queryName.value = ''
  showSaveForm.value = false
}

function onLoad(item: { id: string; sql: string }) {
  queryStore.setEditorContent(item.sql)
  emit('update:visible', false)
}

function onDelete(id: string) {
  savedStore.remove(id)
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleString('zh-CN')
}
</script>

<template>
  <NPopover
    :show="isOpen"
    placement="bottom-start"
    :keep-alive-on-hide="true"
    trigger="click"
    @update:show="(v: boolean) => !v && emit('update:visible', false)"
  >
    <template #trigger>
      <slot />
    </template>
    <div class="saved-queries-panel">
      <div class="sq-header">
        <NText depth="2" class="sq-title">已保存的查询</NText>
        <NButton
          v-if="!showSaveForm"
          size="tiny"
          quaternary
          @click="showSaveForm = true"
        >
          + 保存当前
        </NButton>
      </div>
      <div v-if="showSaveForm" class="sq-save-form">
        <NInput
          v-model:value="queryName"
          size="small"
          placeholder="查询名称（留空自动生成）"
          @keyup.enter="onSaveConfirm"
        />
        <NSpace :size="4">
          <NButton size="tiny" type="primary" @click="onSaveConfirm" :disabled="!queryStore.editorContent.trim()">
            保存
          </NButton>
          <NButton size="tiny" @click="showSaveForm = false">
            取消
          </NButton>
        </NSpace>
      </div>
      <NScrollbar class="sq-list">
        <NEmpty v-if="savedStore.sorted.length === 0" description="暂无已保存的查询" size="small" />
        <div
          v-for="item in savedStore.sorted"
          :key="item.id"
          class="sq-item"
          @click="onLoad(item)"
        >
          <div class="sq-item-name">{{ item.name }}</div>
          <div class="sq-item-meta">
            <NTag size="tiny" :bordered="false" type="default">
              {{ formatTime(item.updatedAt) }}
            </NTag>
            <NButton
              size="tiny"
              quaternary
              text
              @click.stop="onDelete(item.id)"
              class="sq-item-del"
            >
              删除
            </NButton>
          </div>
        </div>
      </NScrollbar>
    </div>
  </NPopover>
</template>

<style scoped>
.saved-queries-panel {
  width: 300px;
  max-height: 360px;
  display: flex;
  flex-direction: column;
}
.sq-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--n-border-color);
  flex-shrink: 0;
}
.sq-title {
  font-weight: 600;
  font-size: 13px;
}
.sq-save-form {
  padding: 8px 12px;
  border-bottom: 1px solid var(--n-border-color);
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
}
.sq-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
.sq-item {
  padding: 8px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--n-divider-color);
  transition: background 0.15s;
}
.sq-item:hover {
  background: var(--n-color-hover);
}
.sq-item-name {
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sq-item-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 4px;
}
.sq-item-del {
  opacity: 0;
  transition: opacity 0.15s;
}
.sq-item:hover .sq-item-del {
  opacity: 1;
}
</style>
