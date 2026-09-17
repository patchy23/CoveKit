<script setup lang="ts">
import UiTooltip from './UiTooltip.vue'
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from 'reka-ui'
import UiIcon from './UiIcon.vue'

const props = withDefaults(
  defineProps<{
    open: boolean
    title?: string
    description?: string
    width?: string
    size?: 'sm' | 'md' | 'lg' | 'xl'
    closeOnBackdrop?: boolean
  }>(),
  { title: '', description: '', width: '', size: 'md', closeOnBackdrop: false }
)

const sizeWidth = {
  sm: 'min(420px, 92vw)',
  md: 'min(560px, 92vw)',
  lg: 'min(760px, 94vw)',
  xl: 'min(1040px, 96vw)',
}

const emit = defineEmits<{ close: [] }>()

function onOpenChange(value: boolean) {
  if (!value) emit('close')
}

function onPointerDownOutside(event: Event) {
  if (!props.closeOnBackdrop) event.preventDefault()
}
</script>

<template>
  <DialogRoot :open="open" @update:open="onOpenChange">
    <DialogPortal>
      <DialogOverlay
        class="ui-modal-overlay fixed inset-0 z-[180] bg-[rgba(16,20,28,0.45)] backdrop-blur-[3px]"
      />
      <div class="pointer-events-none fixed inset-0 z-[180] grid place-items-center p-md">
        <DialogContent
          class="ui-modal-panel pointer-events-auto relative"
          :style="{ width: width || sizeWidth[size] }"
          @pointer-down-outside="onPointerDownOutside"
        >
          <!-- 右上角关闭（遮罩点击默认已禁用，所有弹窗必须有可见出口） -->
          <UiTooltip content="关闭">
            <button
              type="button"
              class="absolute right-[14px] top-[14px] grid h-[26px] w-[26px] place-items-center rounded-[6px] text-text-muted transition-colors hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
              aria-label="关闭"
              @click="emit('close')"
            >
              <UiIcon name="x" :size="12" />
            </button>
          </UiTooltip>
          <header v-if="title || description || $slots.header" class="mb-[18px] pr-[30px]">
            <slot name="header">
              <DialogTitle
                class="text-card-title font-semibold text-primary dark:text-primary-dark"
              >
                {{ title }}
              </DialogTitle>
              <DialogDescription
                v-if="description"
                class="mt-xs text-body-sm text-secondary dark:text-secondary-dark"
              >
                {{ description }}
              </DialogDescription>
            </slot>
          </header>
          <slot />
          <footer v-if="$slots.footer" class="mt-[20px] flex justify-end gap-sm">
            <slot name="footer" />
          </footer>
        </DialogContent>
      </div>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
.ui-modal-overlay[data-state='open'] {
  animation: ui-modal-fade-in 0.18s ease;
}
.ui-modal-overlay[data-state='closed'] {
  animation: ui-modal-fade-out 0.18s ease;
}
.ui-modal-panel[data-state='open'] {
  animation: ui-modal-panel-in 0.18s ease;
}
.ui-modal-panel[data-state='closed'] {
  animation: ui-modal-panel-out 0.18s ease;
}
@keyframes ui-modal-fade-in {
  from {
    opacity: 0;
  }
}
@keyframes ui-modal-panel-in {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.98);
  }
}
@keyframes ui-modal-fade-out {
  to {
    opacity: 0;
  }
}
@keyframes ui-modal-panel-out {
  to {
    opacity: 0;
    transform: translateY(10px) scale(0.98);
  }
}
</style>
