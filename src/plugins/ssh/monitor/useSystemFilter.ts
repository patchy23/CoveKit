import { computed } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'

/** 两个列表分别记住选择，随当前数据空间设置保存。 */
export function useSystemFilter(kind: 'services' | 'processes') {
  const settings = useSettingsStore()
  const ui = useUiStore()
  const key = kind === 'services' ? 'hideSystemServices' : 'hideSystemProcesses'
  return computed({
    get: () => settings.getToolSetting<boolean>('ssh', key, true) !== false,
    set: (value: boolean) => {
      void settings.setToolSetting('ssh', key, value).catch((error) => {
        ui.toast(`保存系统项筛选失败：${error}`)
      })
    },
  })
}
