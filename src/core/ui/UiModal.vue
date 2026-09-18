<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import UiTooltip from './UiTooltip.vue'
import { computed } from 'vue'
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
    /**
     * 自由宽度（如 'min(460px, 92vw)'），优先于 size；
     * size='full'（工作台型）时作为面板最大宽度（缺省 min(920px, 94vw)）。
     * 仅响应式宽度档位给不了时使用，常规弹窗一律走 size 档位。
     */
    width?: string
    /**
     * 尺寸档位；full 为工作台型弹窗（代码编辑器/终端）：高度撑满遮罩留白、
     * 内容区弹性伸缩，header/#header 与 #footer 固定不滚。
     */
    size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
    closeOnBackdrop?: boolean
  }>(),
  { title: '', description: '', width: '', size: 'md', closeOnBackdrop: false }
)

const sizeWidth: Record<'sm' | 'md' | 'lg' | 'xl', string> = {
  sm: 'min(420px, 92vw)',
  md: 'min(560px, 92vw)',
  lg: 'min(760px, 94vw)',
  xl: 'min(1040px, 96vw)',
}

const isFull = computed(() => props.size === 'full')

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
      <div
        class="pointer-events-none fixed inset-0 z-[180] grid place-items-center"
        :class="isFull ? 'p-[40px]' : 'p-md'"
      >
        <!-- full 档内容自控滚动（内部编辑器/终端有自己的滚动域），不包 UiScrollArea -->
        <UiScrollArea v-if="!isFull" as-child axis="vertical">
          <DialogContent
            class="ui-modal-panel pointer-events-auto relative"
            :style="{ width: width || sizeWidth[size as 'sm' | 'md' | 'lg' | 'xl'] }"
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
        </UiScrollArea>
        <DialogContent
          v-else
          class="ui-modal-panel pointer-events-auto relative flex !max-h-none w-full flex-col !overflow-hidden !rounded-lg !p-0"
          :style="{ maxWidth: width || 'min(920px, 94vw)' }"
          @pointer-down-outside="onPointerDownOutside"
        >
          <UiTooltip content="关闭">
            <button
              type="button"
              class="absolute right-[10px] top-[7px] z-10 grid h-[26px] w-[26px] place-items-center rounded-[6px] text-text-muted transition-colors hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
              aria-label="关闭"
              @click="emit('close')"
            >
              <UiIcon name="x" :size="12" />
            </button>
          </UiTooltip>
          <!-- full 档：header 固定、默认插槽弹性、footer 固定 -->
          <header v-if="title || description || $slots.header" class="shrink-0">
            <slot name="header">
              <div
                class="flex items-center gap-[10px] border-b border-border px-[16px] py-[10px] dark:border-border-dark"
              >
                <DialogTitle
                  class="text-card-title font-semibold text-primary dark:text-primary-dark"
                >
                  {{ title }}
                </DialogTitle>
                <DialogDescription
                  v-if="description"
                  class="text-body-sm text-secondary dark:text-secondary-dark"
                >
                  {{ description }}
                </DialogDescription>
              </div>
            </slot>
          </header>
          <div class="flex min-h-0 flex-1 flex-col">
            <slot />
          </div>
          <footer
            v-if="$slots.footer"
            class="flex shrink-0 justify-end gap-sm border-t border-border px-[16px] py-[10px] dark:border-border-dark"
          >
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
