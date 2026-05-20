<script setup lang="ts">
import { computed } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'

import AppLayout from '@/components/layout/AppLayout.vue'

const osThemeRef = useOsTheme()
const theme = computed(() => (osThemeRef.value === 'dark' ? darkTheme : null))
</script>

<template>
  <n-config-provider :theme="theme">
    <AppLayout />
  </n-config-provider>
</template>
