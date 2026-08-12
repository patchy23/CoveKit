/** 公共控件尺寸：紧凑数据页到舒适表单页统一使用这一组层级。 */
export type UiSize = 'xs' | 'sm' | 'md' | 'lg'

/** 公共语义色，不允许业务组件自行扩展字面色值。 */
export type UiTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info' | 'purple'

/**
 * 内容字体语义。组件负责字号与字重，业务只声明内容是什么。
 * 中英文具体字形由 patchyBox Sans / Mono 的 unicode-range 自动选择。
 */
export type UiContentKind = 'text' | 'technical' | 'numeric' | 'status' | 'action' | 'code'
