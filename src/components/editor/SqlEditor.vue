<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import { NButton, NTooltip } from 'naive-ui'

const props = defineProps<{
  modelValue: string
  connectionId?: string | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  execute: [sql: string]
  'execute-multi': [sql: string]
  stop: []
}>()

const isExecuting = ref(false)
const textareaRef = ref<HTMLTextAreaElement | null>(null)
const lineCount = ref(1)

onMounted(() => {
  // TODO: Monaco Editor 懒加载
})

onBeforeUnmount(() => {
  // cleanup
})

watch(() => props.modelValue, () => {
  syncLineCount()
})

function syncLineCount() {
  const lines = props.modelValue.split('\n').length
  lineCount.value = Math.max(lines, 1)
}

function getLineNumbers(): string {
  const count = lineCount.value
  let result = ''
  for (let i = 1; i <= count; i++) {
    result += i + '\n'
  }
  return result
}

function runQuery(): void {
  if (props.modelValue.trim()) {
    emit('execute', props.modelValue)
  }
}

function runAllQueries(): void {
  if (props.modelValue.trim()) {
    emit('execute-multi', props.modelValue)
  }
}

function stopQuery(): void {
  emit('stop')
}

function onKeyDown(e: KeyboardEvent): void {
  const textarea = e.target as HTMLTextAreaElement

  // Ctrl+Enter / Cmd+Enter → run current statement
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault()
    runQuery()
    return
  }

  // Ctrl+Shift+Enter / Cmd+Shift+Enter → run all
  if (
    (e.ctrlKey || e.metaKey) &&
    e.shiftKey &&
    e.key === 'Enter'
  ) {
    e.preventDefault()
    runAllQueries()
    return
  }

  // Ctrl+Home → go to start
  if ((e.ctrlKey || e.metaKey) && e.key === 'Home') {
    e.preventDefault()
    textarea.setSelectionRange(0, 0)
    return
  }

  // Ctrl+End → go to end
  if ((e.ctrlKey || e.metaKey) && e.key === 'End') {
    e.preventDefault()
    const len = textarea.value.length
    textarea.setSelectionRange(len, len)
    return
  }

  // Tab → insert 2 spaces
  if (e.key === 'Tab') {
    e.preventDefault()
    const start = textarea.selectionStart
    const end = textarea.selectionEnd
    const value = textarea.value
    const newValue = value.substring(0, start) + '  ' + value.substring(end)
    emit('update:modelValue', newValue)
    // Move cursor after inserted spaces
    requestAnimationFrame(() => {
      textarea.selectionStart = textarea.selectionEnd = start + 2
    })
  }
}

function onScroll(this: HTMLTextAreaElement) {
  const lineEl = this.parentElement?.querySelector('.line-numbers pre')
  if (lineEl) {
    lineEl.scrollTop = this.scrollTop
  }
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
            @click="runAllQueries"
            :loading="isExecuting"
            quaternary
          >
            全部执行
          </NButton>
        </template>
        <span>执行所有语句 (Ctrl+Shift+Enter)</span>
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
    <div class="editor-body">
      <div class="line-numbers" aria-hidden="true">
        <pre>{{ getLineNumbers() }}</pre>
      </div>
      <textarea
        ref="textareaRef"
        :value="modelValue"
        @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
        @keydown="onKeyDown"
        @scroll="onScroll"
        @select="syncLineCount"
        class="sql-textarea"
        placeholder="输入 SQL 查询...&#10;&#10;Ctrl+Enter 运行当前语句&#10;Ctrl+Shift+Enter 执行所有语句&#10;Tab 插入缩进"
      />
    </div>
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
  flex-shrink: 0;
}
.editor-body {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
  position: relative;
}
.line-numbers {
  flex-shrink: 0;
  width: 40px;
  padding: 12px 4px 12px 0;
  text-align: right;
  color: var(--n-text-color-3);
  font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 14px;
  line-height: 1.6;
  user-select: none;
  border-right: 1px solid var(--n-border-color);
  background: var(--n-color);
  overflow: hidden;
}
.line-numbers pre {
  margin: 0;
  line-height: inherit;
}
.sql-textarea {
  flex: 1;
  min-width: 0;
  padding: 12px;
  border: none;
  resize: none;
  font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 14px;
  line-height: 1.6;
  background: var(--n-color);
  color: var(--n-text-color);
  outline: none;
  tab-size: 2;
  overflow: auto;
  white-space: pre;
  overflow-wrap: normal;
}
</style>
