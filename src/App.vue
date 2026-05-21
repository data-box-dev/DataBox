<script setup lang="ts">
import { computed } from 'vue'
import { darkTheme } from 'naive-ui'

import AppLayout from '@/components/layout/AppLayout.vue'

const theme = computed(() => darkTheme)
</script>

<template>
  <NConfigProvider :theme="theme">
    <AppLayout />
  </NConfigProvider>
</template>
