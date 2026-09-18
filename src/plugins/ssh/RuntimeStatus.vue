<script setup lang="ts">
/** 服务、容器及编排状态的统一展示；操作判断继续使用原始状态。 */
import { computed } from 'vue'
import { UiBadge } from '@/core/ui'
import { runtimeStatus } from './runtimeStatus'
const props = withDefaults(defineProps<{ status: string; busy?: boolean }>(), { busy: false })
const display = computed(() =>
  props.busy ? { label: '执行中', tone: 'warning' as const } : runtimeStatus(props.status)
)
</script>

<template>
  <UiBadge size="xs" :tone="display.tone" class="whitespace-nowrap font-sans">
    {{ display.label }}
  </UiBadge>
</template>
