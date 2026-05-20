<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { NButton, NTooltip } from 'naive-ui'

const props = defineProps<{
  modelValue: string
  connectionId?: string | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  execute: [sql: string]
  stop: []
}>()

const isExecuting = ref(false)

onMounted(() => {
  // TODO: Monaco Editor 懒加载
})

onBeforeUnmount(() => {
  // cleanup
})

watch(() => props.modelValue, (newVal) => {
  // sync with editor
})

function runQuery(): void {
  if (props.modelValue.trim()) {
    emit('execute', props.modelValue)
  }
}

function stopQuery(): void {
  emit('stop')
}
</script>

<template>
  <div class="sql-editor">
    <div class="editor-toolbar">
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton
            size="small"
            @click="runQuery"
            :loading="isExecuting"
            quaternary
          >
            运行
          </NButton>
        </template>
        <span>运行 (Ctrl+Enter)</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton
            size="small"
            quaternary
            @click="stopQuery"
          >
            停止
          </NButton>
        </template>
        <span>停止</span>
      </NTooltip>
    </div>
    <textarea
      :value="modelValue"
      @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
      class="sql-textarea"
      placeholder="输入 SQL 查询..."
    />
  </div>
</template>

<style scoped>
.sql-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  border-bottom: 1px solid var(--n-border-color);
}
.editor-toolbar {
  display: flex;
  gap: 4px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--n-border-color);
  background: var(--n-color);
}
.sql-textarea {
  flex: 1;
  min-height: 0;
  padding: 12px;
  border: none;
  resize: none;
  font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 14px;
  line-height: 1.6;
  background: var(--n-color);
  color: var(--n-text-color);
  outline: none;
}
</style>
