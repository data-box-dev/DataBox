<script setup lang="ts">
import { ref } from 'vue'
import { ClipboardList, History, Settings } from '@vicons/tabler'

const siderWidth = ref<string>('300px')
const updateSiderWidth = (newWidth: string) => {
  siderWidth.value = newWidth
}

</script>

<template>
  <n-layout>
    <!-- 头部 -->
    <n-layout-header bordered>
      <n-flex
        class="h-100%"
        data-tauri-drag-region
        justify="end"
        align="center"
        size="small"
      >
        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="square-button" size="small" quaternary>
              <n-icon :component="ClipboardList" size="25px" />
            </n-button>
          </template>
          <span>保存记录</span>
        </n-tooltip>
        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="square-button" size="small" quaternary>
              <n-icon :component="History" size="25px" />
            </n-button>
          </template>
          <span>执行记录</span>
        </n-tooltip>
        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="square-button mr-1.5" size="small" quaternary>
              <n-icon :component="Settings" size="25px" />
            </n-button>
          </template>
          <span>系统设置</span>
        </n-tooltip>
      </n-flex>
    </n-layout-header>
    <!-- 下方左右布局 -->
    <n-layout has-sider >
      <n-split
        min="80px"
        :size="siderWidth"
        @update:size="updateSiderWidth"
        resize-trigger-size="1">
        <template #1>
          <!-- 左侧边栏 -->
          <n-layout-sider
            class="transition-none"
            bordered
            :width="siderWidth"
            :native-scrollbar="false"
          >
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
          </n-layout-sider>
        </template>
        <template #2>
          <!-- 右侧内容区 -->
          <n-layout-content bordered :native-scrollbar="false">
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
            <n-h2>海淀桥</n-h2>
          </n-layout-content>
        </template>
      </n-split>
    </n-layout>
  </n-layout>
</template>

<style lang="scss" scoped>
.n-layout-header {
  height: 40px;
}

.n-layout-sider {
  height: calc(100vh - 40px);
}

.n-layout-content {
  height: calc(100vh - 40px);
}

.square-button {
  width: 30px;
  height: 30px;
  padding: 0;
}
</style>
