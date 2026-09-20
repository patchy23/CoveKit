<script setup lang="ts">
/**
 * ToolHost · 工具页签宿主（可靠性 T10-6）
 *
 * 一个工具崩了不该带走整个窗口：加载失败与渲染异常都收在这里，显示「哪一步失败 + 重试」。
 * 重试会重建组件实例——异步组件失败后必须换新实例才能重新 import 并重新执行。
 *
 * 为什么要有加载态：工具组件是 `defineAsyncComponent`（按需分包），
 * 首次打开要读一次分包，没有加载态就是一段无从判断的白屏。
 */
import { computed, defineAsyncComponent, h, onErrorCaptured, ref, type Component } from 'vue'
import UiSpinner from '@/core/ui/UiSpinner.vue'

const props = defineProps<{
  /** 工具 id（错误归属与日志前缀） */
  toolId: string
  /** 工具显示名（错误面板给人看） */
  title: string
  /** 组件加载器（注册表提供） */
  loader: () => Promise<{ default: Component }>
}>()

/** 加载态（首次打开：分包读取期间显示，避免白屏）；转圈统一走 UiSpinner */
const LoadingView: Component = () =>
  h(
    'div',
    {
      class:
        'flex h-full items-center justify-center gap-[8px] text-body-sm text-text-muted dark:text-text-muted-dark',
    },
    [h(UiSpinner, { size: 'md', label: '正在加载' }), h('span', null, '正在加载…')]
  )

/** 实例代数：自增即强制重建组件（重试路径） */
const generation = ref(0)
/** 渲染期异常（错误边界捕获；不静默，界面必须可见） */
const renderError = ref<string | null>(null)
/** 组件加载失败原因（异步组件错误面板展示） */
const loadError = ref<string | null>(null)

/** 加载失败面板（带重试；重试重建实例） */
function createLoadErrorView(): Component {
  return () =>
    h(
      'div',
      { class: 'flex h-full flex-col items-center justify-center gap-[10px] px-md text-center' },
      [
        h(
          'p',
          { class: 'text-body font-medium dark:text-primary-dark' },
          `${props.title} 加载失败`
        ),
        h(
          'p',
          {
            class:
              'select-text max-w-[560px] break-all text-body-sm text-text-muted dark:text-text-muted-dark',
          },
          loadError.value ?? '未能载入工具组件'
        ),
        h(
          'button',
          {
            class:
              'h-[32px] rounded-md border border-border px-[14px] text-body-sm text-secondary transition-colors hover:text-primary dark:border-border-dark dark:text-secondary-dark dark:hover:text-primary-dark',
            onClick: retry,
          },
          '重试'
        ),
      ]
    )
}

/** 异步组件：随代数重建（重试即换新实例） */
const frame = computed<Component>(() => {
  const current = generation.value
  return defineAsyncComponent({
    loader: props.loader,
    loadingComponent: LoadingView,
    errorComponent: createLoadErrorView(),
    delay: 120,
    timeout: 30000,
    onError: (error, retryLoad, fail) => {
      loadError.value = describe(error)
      console.error(`[tool:${props.toolId}] 组件加载失败（第 ${current + 1} 次）`, error)
      // 第一次失败自动重试一次（分包读取偶发失败很常见），再失败就交给错误面板
      if (current === 0) retryLoad()
      else fail()
    },
  })
})

/** 渲染期错误边界：工具内部抛错只影响这个页签 */
onErrorCaptured((error, _instance, info) => {
  renderError.value = `${describe(error)}（${info}）`
  console.error(`[tool:${props.toolId}] 渲染失败`, error)
  return false
})

/** 重试：清错误 + 重建实例 */
function retry() {
  loadError.value = null
  renderError.value = null
  generation.value += 1
}

/** 错误描述归一化 */
function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
</script>

<template>
  <div class="h-full min-h-0">
    <!-- 渲染期错误：给原因 + 重试，不让整个工作区一起空白 -->
    <div
      v-if="renderError"
      class="flex h-full flex-col items-center justify-center gap-[10px] px-md text-center"
    >
      <p class="text-body font-medium text-danger-strong dark:text-danger-dark">
        {{ title }} 出错了
      </p>
      <p
        class="select-text max-w-[560px] break-all text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ renderError }}
      </p>
      <button
        class="h-[32px] rounded-md border border-border px-[14px] text-body-sm text-secondary transition-colors hover:text-primary dark:border-border-dark dark:text-secondary-dark dark:hover:text-primary-dark"
        @click="retry"
      >
        重试
      </button>
    </div>
    <component :is="frame" v-else :key="generation" />
  </div>
</template>
