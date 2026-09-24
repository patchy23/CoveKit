<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import UiTooltip from './UiTooltip.vue'
import { computed, useSlots } from 'vue'
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
    /** 普通弹窗可固定在视口上方，内容变化只向下伸缩；full 档忽略此项。 */
    placement?: 'center' | 'top'
    closeOnBackdrop?: boolean
  }>(),
  { title: '', description: '', width: '', size: 'md', placement: 'center', closeOnBackdrop: false }
)

const sizeWidth: Record<'sm' | 'md' | 'lg' | 'xl', string> = {
  sm: 'min(420px, 92vw)',
  md: 'min(560px, 92vw)',
  lg: 'min(760px, 94vw)',
  xl: 'min(1040px, 96vw)',
}

const isFull = computed(() => props.size === 'full')
const topAligned = computed(() => !isFull.value && props.placement === 'top')

const emit = defineEmits<{ close: [] }>()

const slots = useSlots()

/** 是否有头部（决定内容区是否需要补上 padding） */
const hasHeader = computed(() => !!(props.title || props.description || slots.header))

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
        class="pointer-events-none fixed inset-0 z-[180] grid justify-items-center"
        :class="
          isFull
            ? 'items-center p-[40px]'
            : topAligned
              ? 'items-start px-md pb-[16px] pt-[clamp(16px,8vh,64px)]'
              : 'items-center p-md'
        "
      >
        <!--
          非 full 档：面板本身永不滚动——header/footer 固定，只有内容区滚动。
          （面板整体滚动会把标题与操作按钮一起滚走，且滚动条贴着面板外缘像「外部滚动条」。）
        -->
        <DialogContent
          v-if="!isFull"
          class="ui-modal-panel pointer-events-auto relative flex flex-col !overflow-hidden !p-0"
          :style="{
            width: width || sizeWidth[size as 'sm' | 'md' | 'lg' | 'xl'],
            maxHeight: topAligned ? 'calc(100dvh - clamp(16px, 8vh, 64px) - 16px)' : undefined,
          }"
          @pointer-down-outside="onPointerDownOutside"
        >
          <!-- 右上角关闭（遮罩点击默认已禁用，所有弹窗必须有可见出口） -->
          <UiTooltip content="关闭">
            <button
              type="button"
              class="absolute right-[14px] top-[14px] z-10 grid h-[26px] w-[26px] place-items-center rounded-[6px] text-text-muted transition-colors hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
              aria-label="关闭"
              @click="emit('close')"
            >
              <UiIcon name="x" :size="12" />
            </button>
          </UiTooltip>
          <header v-if="hasHeader" class="mb-[18px] shrink-0 px-xl pt-xl pr-[44px]">
            <slot name="header">
              <DialogTitle
                class="text-card-title font-semibold text-primary dark:text-primary-dark"
              >
                {{ title }}
              </DialogTitle>
              <DialogDescription
                v-if="description"
                class="select-text mt-xs text-body-sm text-secondary dark:text-secondary-dark"
              >
                {{ description }}
              </DialogDescription>
            </slot>
          </header>
          <UiScrollArea
            class="min-h-0 flex-1 px-xl"
            :class="{ 'pt-xl': !hasHeader, 'pb-xl': !$slots.footer }"
            axis="vertical"
          >
            <slot />
          </UiScrollArea>
          <footer
            v-if="$slots.footer"
            class="mt-[20px] flex shrink-0 justify-end gap-sm px-xl pb-xl"
          >
            <slot name="footer" />
          </footer>
        </DialogContent>
        <DialogContent
          v-else
          class="ui-modal-panel pointer-events-auto relative flex h-full !max-h-none w-full flex-col !overflow-hidden !rounded-lg !p-0"
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
                  class="select-text text-body-sm text-secondary dark:text-secondary-dark"
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
