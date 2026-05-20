<script setup lang="ts">
import { ref, shallowRef, onMounted, onBeforeUnmount, watch } from 'vue'
import { NButton, NTooltip } from 'naive-ui'
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'

const props = defineProps<{
  modelValue: string
  connectionId?: string | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  execute: [sql: string]
  stop: []
}>()

const containerRef = ref<HTMLElement>()
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor>()
const isExecuting = ref(false)

onMounted(() => {
  if (!containerRef.value) return

  editor.value = monaco.editor.create(containerRef.value, {
    value: props.modelValue,
    language: 'sql',
    theme: 'vs',
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    lineNumbers: 'on',
    automaticLayout: true,
    fontSize: 14,
    padding: { top: 8 },
  })

  editor.value.onDidChangeModelContent(() => {
    emit('update:modelValue', editor.value!.getValue())
  })

  // Ctrl+Enter: 执行当前查询
  editor.value.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
    const sql = editor.value!.getValue()
    if (sql.trim()) {
      emit('execute', sql)
    }
  })
})

onBeforeUnmount(() => {
  editor.value?.dispose()
})

watch(() => props.modelValue, (newVal) => {
  if (editor.value && newVal !== editor.value.getValue()) {
    editor.value.setValue(newVal)
  }
})

function runQuery(): void {
  const sql = editor.value?.getValue() || ''
  if (sql.trim()) {
    emit('execute', sql)
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
    <div ref="containerRef" class="editor-container" />
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
.editor-container {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
</style>
