import type { UiTone } from '@/core/ui/types'

/** SSH 运行对象共用展示语义；保留未知状态，不能将未知值归为已停止。 */
export function runtimeStatus(status: string): { label: string; tone: UiTone } {
  const value = status.trim()
  const states: Record<string, [string, UiTone]> = {
    active: ['运行中', 'success'],
    running: ['运行中', 'success'],
    inactive: ['已停止', 'neutral'],
    exited: ['已停止', 'neutral'],
    stopped: ['已停止', 'neutral'],
    created: ['未启动', 'neutral'],
    paused: ['已暂停', 'warning'],
    activating: ['启动中', 'warning'],
    deactivating: ['停止中', 'warning'],
    reloading: ['重载中', 'warning'],
    restarting: ['重启中', 'warning'],
    removing: ['移除中', 'warning'],
    refreshing: ['更新中', 'warning'],
    maintenance: ['维护中', 'warning'],
    failed: ['失败', 'danger'],
    dead: ['异常', 'danger'],
    运行中: ['运行中', 'success'],
    部分运行: ['部分运行', 'warning'],
    已停止: ['已停止', 'neutral'],
    未部署: ['未部署', 'neutral'],
    已暂停: ['已暂停', 'warning'],
    重启中: ['重启中', 'warning'],
    异常: ['异常', 'danger'],
  }
  const matched = Object.prototype.hasOwnProperty.call(states, value) ? states[value] : undefined
  return matched
    ? { label: matched[0], tone: matched[1] }
    : { label: value || '未知', tone: 'neutral' }
}
