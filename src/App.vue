<script setup lang="ts">
import { darkTheme, useOsTheme } from 'naive-ui'
import { computed } from 'vue'
import AppLayout from '@/components/layout/AppLayout.vue'

const osThemeRef = useOsTheme()
const theme = computed(() => (osThemeRef.value === 'dark' ? darkTheme : null))
</script>

<template>
  <n-config-provider :theme="theme">
    <AppLayout />
  </n-config-provider>
</template>
<style scoped>
</style>
