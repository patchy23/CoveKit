import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'
import type { UiSize } from './types'

/** shadcn-vue 风格的类名合并入口，业务组件不直接依赖实现细节。 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * 下拉浮层外壳类（UiSelect / UiCombobox / UiTabsOverflow 共用）。
 * z-[220] 是全局浮层基线：必须高于 UiModal 遮罩 z-[180]（11 号文定稿），新浮层组件勿用 z-50。
 * 各组件再自行叠加定位（fixed/popper）与宽度约束。
 */
export const UI_FLOATING_PANEL_CLASS =
  'z-[220] overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark'

/** 选择器选项的内边距与字号随控件档位（UiSelect / UiCombobox 共用） */
export function uiOptionSizeClass(size: UiSize): string {
  if (size === 'xs') return 'py-xs text-caption'
  if (size === 'sm') return 'py-[6px] text-body-sm'
  if (size === 'lg') return 'py-[9px] text-body'
  return 'py-[7px] text-body'
}

/** 控件附属文本（label、说明等）的字号随控件档位（UiField 等共用） */
export function uiLabelSizeClass(size: UiSize): string {
  if (size === 'xs') return 'text-caption'
  if (size === 'sm') return 'text-body-sm'
  return 'text-body'
}
