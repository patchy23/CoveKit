/**
 * 折叠 gutter 的箭头标记
 *
 * 替换 CodeMirror 默认的 `⌄` / `›` 文本字符（12px 字号下笔画位置不可控，观感差）。
 * 换成 inline SVG 细线 chevron：与项目其余图标同一套笔画观感（圆头、1.5px 描边），
 * 展开态朝下、折叠态朝右，颜色与悬停态由 theme.ts 的折叠 gutter 样式控制。
 */

/** 展开态：chevron-down */
const CHEVRON_DOWN_PATH = 'M3.5 5.75 8 10.25l4.5-4.5'

/** 折叠态：chevron-right */
const CHEVRON_RIGHT_PATH = 'M5.75 3.5 10.25 8l-4.5 4.5'

/**
 * 创建折叠标记元素
 *
 * @param open 该行当前是否处于展开态（可折叠未折叠为 true，已折叠为 false）
 */
export function createFoldMarker(open: boolean): HTMLElement {
  const wrapper = document.createElement('span')
  wrapper.className = 'cm-fold-marker'
  const path = open ? CHEVRON_DOWN_PATH : CHEVRON_RIGHT_PATH
  wrapper.innerHTML = `<svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="${path}" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`
  return wrapper
}
