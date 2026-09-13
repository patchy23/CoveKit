/**
 * 框架长任务视图（可靠性 T11-1/T11-2）
 *
 * 后端是权威：这里只做镜像，进入界面拉一次全量、之后按事件增量更新。
 * 为什么要有它：任务状态原来只存在于触发它的那个组件的局部 ref 里，
 * 离开设置页就丢失，回来既看不到进度也不知道上次失败原因。
 *
 * 容量口径：已完成任务与后端一样只保留最近 20 条（后端已截断，这里再兜一次，
 * 避免事件乱序时前端比后端还多）。
 *
 * 取消口径：框架长任务当前都不可中途取消（快照里 `cancellable` 为 false 并带原因），
 * 因此这里不提供取消动作——需要取消的阶段（更新下载/安装）由 `stores/update` 自己管。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { createScope, type Scope } from '@/core/lifecycle'
import { invokeCommand } from '@/core/ipc/ipc'
import { FRAMEWORK_TASK_EVENT, type TaskList, type TaskSnapshot } from '@/core/ipc/contracts'

/** 已完成任务在前端保留的条数（与后端 FINISHED_HISTORY_CAP 对齐） */
export const FINISHED_KEEP = 20

/** 是否为终态（终态任务从活跃移到已完成，且不再接受变更） */
export function isTerminal(task: TaskSnapshot): boolean {
  return task.state === 'succeeded' || task.state === 'failed'
}

export const useTasksStore = defineStore('tasks', () => {
  /** 进行中的任务（登记顺序） */
  const active = ref<TaskSnapshot[]>([])
  /** 已结束的任务（最近在前） */
  const finished = ref<TaskSnapshot[]>([])
  /** 全量拉取失败原因（同步问题要说出来，不能假装没有任务） */
  const syncError = ref('')

  const activeCount = computed(() => active.value.length)
  /** 全部任务：活跃在前、已完成在后（界面一次列全） */
  const tasks = computed(() => [...active.value, ...finished.value])

  const scope: Scope = createScope('store.tasks')
  let started = false

  /** 应用一条快照：按 id 归位，终态进已完成并裁到上限 */
  function applySnapshot(snapshot: TaskSnapshot): void {
    const inActive = active.value.findIndex((task) => task.id === snapshot.id)
    const inFinished = finished.value.findIndex((task) => task.id === snapshot.id)

    if (isTerminal(snapshot)) {
      if (inActive >= 0) active.value.splice(inActive, 1)
      if (inFinished >= 0) finished.value.splice(inFinished, 1)
      finished.value.unshift(snapshot)
      while (finished.value.length > FINISHED_KEEP) finished.value.pop()
      return
    }

    if (inFinished >= 0) finished.value.splice(inFinished, 1)
    if (inActive >= 0) active.value.splice(inActive, 1, snapshot)
    else active.value.push(snapshot)
  }

  /** 全量对齐（进入界面或事件漏收后调用） */
  async function refresh(): Promise<void> {
    try {
      const list = await invokeCommand<Record<string, never>, TaskList>('framework_tasks')
      active.value = list.active
      finished.value = list.finished.slice(0, FINISHED_KEEP)
      syncError.value = ''
    } catch (reason) {
      syncError.value = reason instanceof Error ? reason.message : String(reason)
    }
  }

  /** 订阅事件并首次拉取；重复调用无副作用（多个入口都会调） */
  async function start(): Promise<void> {
    if (started) return
    started = true
    await scope.listenEvent<TaskSnapshot>(FRAMEWORK_TASK_EVENT, (snapshot) => {
      applySnapshot(snapshot)
    })
    await refresh()
  }

  /** 停止订阅并释放（单 tab 关闭时调用；应用退出不需要） */
  async function stop(): Promise<void> {
    await scope.dispose()
    started = false
  }

  /** 某任务当前的快照（按 id 查，找不到返回 null） */
  function find(id: string): TaskSnapshot | null {
    return tasks.value.find((task) => task.id === id) ?? null
  }

  return {
    active,
    finished,
    tasks,
    syncError,
    activeCount,
    start,
    stop,
    refresh,
    find,
  }
})
