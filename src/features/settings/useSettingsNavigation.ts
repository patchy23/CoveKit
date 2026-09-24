import { onMounted, onUnmounted, ref, type Ref } from 'vue'

/** 目录跟随实际滚动位置；内容尺寸变化和滚动到底时也重新定位。 */
export function useSettingsNavigation(viewport: Ref<HTMLElement | null>) {
  const activeSection = ref('appearance')
  let observer: ResizeObserver | undefined
  let stopScroll: (() => void) | undefined

  function syncSection() {
    const root = viewport.value
    if (!root) return
    const sections = Array.from(root.querySelectorAll<HTMLElement>('[data-settings-section]'))
    if (!sections.length) return
    const bottom =
      root.scrollHeight > root.clientHeight &&
      root.scrollTop + root.clientHeight >= root.scrollHeight - 2
    const top = root.getBoundingClientRect().top + 24
    const current = bottom
      ? sections[sections.length - 1]
      : (sections.filter((section) => section.getBoundingClientRect().top <= top).at(-1) ??
        sections[0])
    activeSection.value = current.dataset.settingsSection!
  }

  function goToSection(id: string) {
    const root = viewport.value
    const section = Array.from(
      root?.querySelectorAll<HTMLElement>('[data-settings-section]') ?? []
    ).find((element) => element.dataset.settingsSection === id)
    if (!root || !section) return
    root.scrollTo({
      top:
        root.scrollTop +
        section.getBoundingClientRect().top -
        root.getBoundingClientRect().top -
        14,
      behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches
        ? 'instant'
        : 'smooth',
    })
  }

  onMounted(() => {
    const root = viewport.value
    if (!root) return
    root.addEventListener('scroll', syncSection, { passive: true })
    stopScroll = () => root.removeEventListener('scroll', syncSection)
    observer = new ResizeObserver(syncSection)
    observer.observe(root)
    if (root.firstElementChild) observer.observe(root.firstElementChild)
    syncSection()
  })
  onUnmounted(() => {
    stopScroll?.()
    observer?.disconnect()
  })
  return { activeSection, goToSection }
}
