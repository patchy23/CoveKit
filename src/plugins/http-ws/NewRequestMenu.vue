<script setup lang="ts">
/** 协议只在创建时选择，已打开页签不提供类型切换。 */
import { UiButton, UiDropdownMenu, UiIcon, UiIconButton } from '@/core/ui'
import type { ApiKind } from './contracts'
defineProps<{ compact?: boolean }>()
defineEmits<{ create: [kind: ApiKind] }>()
</script>
<template>
  <UiDropdownMenu
    size="sm"
    :items="[
      { value: 'http', label: 'HTTP 请求' },
      { value: 'sse', label: 'SSE 订阅' },
      { value: 'ws', label: 'WebSocket 连接' },
    ]"
    @select="$emit('create', $event as ApiKind)"
  >
    <template #trigger
      ><UiIconButton v-if="compact" label="新建接口" size="xs"
        ><UiIcon name="plus" :size="14" /></UiIconButton
      ><UiButton v-else variant="ghost" size="xs" title="新建接口"
        ><UiIcon name="plus" :size="14" />新建</UiButton
      ></template
    >
  </UiDropdownMenu>
</template>
