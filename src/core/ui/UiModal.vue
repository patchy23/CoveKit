<script setup lang="ts">
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from 'reka-ui'

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
          class="ui-modal-panel pointer-events-auto"
          :style="{ width: width || sizeWidth[size] }"
          @pointer-down-outside="onPointerDownOutside"
        >
          <header v-if="title || description || $slots.header" class="mb-[18px]">
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
