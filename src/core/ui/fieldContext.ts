import type { ComputedRef, InjectionKey } from 'vue'

/**
 * UiField 向插槽内控件提供的关联上下文：
 * 控件把 controlId 落到自身 id、describedById 落到 aria-describedby，
 * 这样 UiField 的 label/error/description 才能被读屏正确关联。
 * 未包在 UiField 里的控件 inject 到 undefined，行为不变。
 */
export interface UiFieldContext {
  controlId: string
  describedById: ComputedRef<string | undefined>
}

export const uiFieldContextKey: InjectionKey<UiFieldContext> = Symbol('ui-field-context')
