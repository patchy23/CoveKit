import { onScopeDispose, ref, watch, type Ref } from 'vue'
import type { RemoteFile } from '../contracts'
import { formatBytes, formatTime } from '../connection/useSsh'

/** 大目录按帧测量完整模型；离屏文件也参与列宽，不依赖当前可见行。 */
export function useFileColumnWidths(
  root: Ref<HTMLElement | null>,
  files: () => RemoteFile[],
  local: boolean
) {
  const widths = ref<number[]>([])
  let frame = 0
  let generation = 0
  let probe: HTMLDivElement | undefined
  let cache = new WeakMap<RemoteFile, { signature: string; widths: number[] }>()
  let styleSignature = ''
  let expiry: ReturnType<typeof setTimeout> | undefined
  const minimum = local ? [48, 90, 132] : [160, 76, 118, 92, 88]
  function stop() {
    clearTimeout(expiry)
    expiry = undefined
    generation++
    cancelAnimationFrame(frame)
    probe?.remove()
    probe = undefined
  }
  function scan() {
    stop()
    const source = files()
    if (source.length <= 200) {
      widths.value = []
      return
    }
    const cells = root.value?.querySelectorAll<HTMLElement>('tbody tr[data-file-row] td')
    if (!cells?.length) return
    const styles = [...cells].slice(0, minimum.length).map((cell) => {
      const style = getComputedStyle(cell)
      return {
        font: style.font,
        letterSpacing: style.letterSpacing,
        fontVariantNumeric: style.fontVariantNumeric,
        padding: (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0),
      }
    })
    const nextSignature = JSON.stringify(styles)
    if (styleSignature !== nextSignature) {
      cache = new WeakMap()
      styleSignature = nextSignature
    }
    const measured = [...minimum]
    const request = generation
    let index = 0
    probe = document.createElement('div')
    probe.setAttribute('aria-hidden', 'true')
    probe.style.cssText =
      'position:fixed;visibility:hidden;pointer-events:none;left:0;top:0;width:max-content;contain:layout style;'
    document.body.append(probe)
    function chunk() {
      if (request !== generation || !probe) return
      const started = performance.now()
      do {
        const pending: { file: RemoteFile; signature: string; elements: HTMLSpanElement[] }[] = []
        // 成批设置文字后统一读取布局，避免每个字段各触发一次同步布局。
        for (let n = 0; n < 32 && index < source.length; n++, index++) {
          const file = source[index]!
          const signature = JSON.stringify([
            file.name,
            file.isDir,
            file.size,
            file.modifiedAt,
            file.permissions,
            file.owner,
          ])
          const previous = cache.get(file)
          if (previous?.signature === signature) {
            previous.widths.forEach((width, column) => {
              measured[column] = Math.max(measured[column]!, width)
            })
            continue
          }
          const values = [
            file.name,
            file.isDir ? '-' : formatBytes(file.size),
            formatTime(file.modifiedAt),
          ]
          if (!local) values.push(file.permissions, file.owner)
          const elements = values.map((value, column) => {
            const element = document.createElement('span')
            const style = styles[column]!
            element.style.cssText = 'display:inline-block;white-space:pre;width:max-content;'
            element.style.font = style.font
            element.style.letterSpacing = style.letterSpacing
            element.style.fontVariantNumeric = style.fontVariantNumeric
            if (column === 0 && file.isDir) element.style.fontWeight = '500'
            element.textContent = value
            if (column === 0) {
              const icon = document.createElement('span')
              icon.textContent = file.isDir ? '📁' : '📄'
              icon.style.marginRight = '6px'
              element.prepend(icon)
            }
            probe!.append(element)
            return element
          })
          pending.push({ file, signature, elements })
        }
        for (const item of pending) {
          const sizes = item.elements.map((element, column) =>
            // 远程名称沿用 max-w-0 和 truncate 的弹性列；其它列及本地名称保留完整宽度。
            column === 0 && !local
              ? minimum[0]!
              : Math.ceil(element.getBoundingClientRect().width + styles[column]!.padding)
          )
          cache.set(item.file, { signature: item.signature, widths: sizes })
          sizes.forEach((width, column) => {
            measured[column] = Math.max(measured[column]!, width)
          })
        }
        probe.replaceChildren()
      } while (index < source.length && performance.now() - started < 4)
      widths.value = [...measured]
      if (index < source.length) frame = requestAnimationFrame(chunk)
      else {
        probe.remove()
        probe = undefined
        // 列宽数字继续用于布局；闲置的逐文件测量副本可以重建，不保留模型之外的长期缓存。
        expiry = setTimeout(
          () => {
            cache = new WeakMap()
            expiry = undefined
          },
          10 * 60 * 1000
        )
      }
    }
    frame = requestAnimationFrame(chunk)
  }
  watch([root, files], scan, { immediate: true, flush: 'post' })
  function changedFont() {
    styleSignature = ''
    scan()
  }
  document.fonts?.addEventListener('loadingdone', changedFont)
  window.addEventListener('resize', scan)
  onScopeDispose(() => {
    stop()
    document.fonts?.removeEventListener('loadingdone', changedFont)
    window.removeEventListener('resize', scan)
  })
  return widths
}
