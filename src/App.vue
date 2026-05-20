<script setup lang="ts">
import { computed } from 'vue'
import { darkTheme, useOsTheme, NConfigProvider } from 'naive-ui'

import AppLayout from '@/components/layout/AppLayout.vue'

const osThemeRef = useOsTheme()
const theme = computed(() => (osThemeRef.value === 'dark' ? darkTheme : null))
</script>

<template>
  <NConfigProvider :theme="theme">
    <AppLayout />
  </NConfigProvider>
</template>
