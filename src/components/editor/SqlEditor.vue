<script setup lang="ts">
import { ref, watch } from 'vue'
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

watch(() => props.modelValue, syncLineCount)

function syncLineCount() {
  const lines = props.modelValue.split('\n').length
  lineCount.value = Math.max(lines, 1)
}

function getLineNumbers(): string {
  let result = ''
  for (let i = 1; i <= lineCount.value; i++) {
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

  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault()
    runQuery()
    return
  }

  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'Enter') {
    e.preventDefault()
    runAllQueries()
    return
  }

  if ((e.ctrlKey || e.metaKey) && e.key === 'Home') {
    e.preventDefault()
    textarea.setSelectionRange(0, 0)
    return
  }

  if ((e.ctrlKey || e.metaKey) && e.key === 'End') {
    e.preventDefault()
    const len = textarea.value.length
    textarea.setSelectionRange(len, len)
    return
  }

  if (e.key === 'Tab') {
    e.preventDefault()
    const start = textarea.selectionStart
    const end = textarea.selectionEnd
    const value = textarea.value
    const newValue = value.substring(0, start) + '  ' + value.substring(end)
    emit('update:modelValue', newValue)
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
          <NButton size="small" @click="runQuery" :loading="isExecuting" quaternary>
            运行
          </NButton>
        </template>
        <span>运行当前语句 (Ctrl+Enter)</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" @click="runAllQueries" :loading="isExecuting" quaternary>
            全部执行
          </NButton>
        </template>
        <span>执行所有语句 (Ctrl+Shift+Enter)</span>
      </NTooltip>
      <NTooltip placement="bottom">
        <template #trigger>
          <NButton size="small" quaternary @click="stopQuery">
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
        spellcheck="false"
        autocomplete="off"
        autocorrect="off"
        autocapitalize="off"
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
  background: var(--db-bg-editor);
}
.editor-toolbar {
  display: flex;
  gap: 2px;
  padding: 2px 6px;
  border-bottom: 1px solid var(--db-border);
  background: var(--db-bg-toolbar);
  flex-shrink: 0;
}
.editor-body {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
  position: relative;
  font-family: 'JetBrains Mono', 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 13px;
}
.line-numbers {
  flex-shrink: 0;
  width: 42px;
  padding: 10px 4px 10px 0;
  text-align: right;
  color: var(--n-text-color-3);
  line-height: 1.55;
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
  line-height: 1.55;
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
  opacity: 0.6;
}
</style>
