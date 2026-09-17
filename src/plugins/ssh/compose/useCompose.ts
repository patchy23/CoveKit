/** Compose 页状态：异步结果按连接和编辑版本隔离，草稿只保留在当前工作区内存。 */
import { computed, ref, watch } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useToolLifecycle } from '@/core/lifecycle'
import type { ComposeAction, ComposeOutput, ComposeProject, ServerConnection } from '../contracts'
import { ipc } from '../ipc'
import { COMPOSE_TEMPLATE, mergeComposeProjects, validateComposeDraft } from './composeProjects'

export function useCompose(
  connection: () => ServerConnection | undefined,
  profileId: () => string,
  workspaceId: string
) {
  const settings = useSettingsStore()
  const {
    dirty: lifecycleDirty,
    running,
    scope,
  } = useToolLifecycle('ssh', { owner: `ssh.compose.${workspaceId}` })
  const remote = ref<ComposeProject[]>([])
  const sessionProjects = ref<ComposeProject[]>([])
  const selected = ref<ComposeProject | null>(null)
  const filePath = ref('')
  const draftName = ref('')
  const content = ref('')
  const baseline = ref('')
  const mtime = ref<number>()
  const isNew = ref(false)
  const loaded = ref(false)
  const loading = ref(false)
  const listing = ref(false)
  const saving = ref(false)
  const action = ref<ComposeAction | null>(null)
  const error = ref('')
  const listError = ref('')
  const notice = ref('')
  const output = ref<ComposeOutput | null>(null)
  let generation = 0
  let listGeneration = 0
  const connected = computed(() => connection()?.status === 'connected')
  const dirty = computed(() => loaded.value && (isNew.value || content.value !== baseline.value))
  const busy = computed(() => saving.value || action.value !== null)
  const remembered = () =>
    settings.getToolSetting<Record<string, ComposeProject[]>>('ssh', 'composeProjects', {})
  const projects = computed(() =>
    mergeComposeProjects(remote.value, [
      ...(remembered()[profileId()] ?? []),
      ...sessionProjects.value,
    ])
  )
  watch(
    dirty,
    (value) => {
      lifecycleDirty.value = value
    },
    { immediate: true }
  )
  watch(
    busy,
    (value) => {
      running.value = value
    },
    { immediate: true }
  )
  const current = (id: string | undefined, version: number) =>
    !scope.disposed && connection()?.sessionId === id && generation === version

  async function refresh() {
    const id = connection()?.sessionId
    if (scope.disposed || !id || !connected.value) return
    const version = ++listGeneration
    listing.value = true
    try {
      const result = await ipc.sshComposeList(id)
      if (scope.disposed || id !== connection()?.sessionId || version !== listGeneration) return
      remote.value = result
      listError.value = ''
    } catch (e) {
      if (!scope.disposed && id === connection()?.sessionId && version === listGeneration)
        listError.value = String(e)
    } finally {
      if (version === listGeneration) listing.value = false
    }
  }

  async function remember(project: ComposeProject) {
    const id = connection()?.sessionId
    const version = generation
    const list = sessionProjects.value.filter((item) => item.name !== project.name)
    sessionProjects.value = [...list, project]
    const all = remembered()
    const existing = all[profileId()] ?? []
    if (
      existing.some(
        (item) =>
          item.name === project.name &&
          JSON.stringify(item.configFiles) === JSON.stringify(project.configFiles)
      )
    )
      return
    try {
      await settings.setToolSetting('ssh', 'composeProjects', {
        ...all,
        [profileId()]: [...existing.filter((item) => item.name !== project.name), project],
      })
    } catch (e) {
      if (current(id, version)) notice.value = `本地项目路径记录失败：${e}。本次会话仍可使用。`
    }
  }

  async function open(project: ComposeProject, path = project.configFiles[0] ?? '') {
    if (busy.value) return
    const id = connection()?.sessionId
    const version = ++generation
    selected.value = project
    filePath.value = path
    isNew.value = false
    loaded.value = false
    loading.value = false
    content.value = baseline.value = ''
    mtime.value = undefined
    error.value = notice.value = ''
    output.value = null
    if (!id || !connected.value || !path) {
      error.value = !path
        ? 'Docker 未提供配置文件路径，无法编辑或执行编排操作'
        : '请先恢复 SSH 连接'
      return
    }
    loading.value = true
    try {
      const result = await ipc.sshEditOpen(id, path)
      if (!current(id, version)) return
      if (!result.ok) throw new Error(result.error ?? '读取配置失败')
      content.value = baseline.value = result.content
      mtime.value = result.modifiedAt
      loaded.value = true
      await remember(project)
    } catch (e) {
      if (current(id, version)) error.value = String(e)
    } finally {
      if (current(id, version)) loading.value = false
    }
  }

  function create() {
    if (busy.value) return
    generation += 1
    selected.value = null
    filePath.value = draftName.value = ''
    content.value = COMPOSE_TEMPLATE
    baseline.value = ''
    mtime.value = undefined
    isNew.value = loaded.value = true
    loading.value = false
    error.value = notice.value = ''
    output.value = null
  }

  async function save() {
    const id = connection()?.sessionId
    if (!id || !connected.value || busy.value || !loaded.value) return
    const path = filePath.value.trim()
    const text = content.value
    const version = generation
    const wasNew = isNew.value
    const project = wasNew
      ? { name: draftName.value.trim(), status: '已保存 · 尚未部署', configFiles: [path] }
      : selected.value
    if (!project) return
    error.value = notice.value = ''
    if (!wasNew && mtime.value === undefined) {
      error.value = '文件版本未知，请保留草稿后重新读取配置再保存'
      return
    }
    if (wasNew) {
      error.value = validateComposeDraft(project.name, path)
      if (!error.value && projects.value.some((p) => p.name === project.name))
        error.value = '该项目名已存在，请使用其他名称'
      if (error.value) return
    }
    saving.value = true
    try {
      if (wasNew) await ipc.sshComposeCreate({ connectionId: id, remotePath: path, content: text })
      else {
        const result = await ipc.sshEditSave(id, path, text, mtime.value)
        if (!result.ok)
          throw new Error(
            result.conflict
              ? '远端文件已修改，本次未覆盖。请复制保留草稿后重新读取，再合并修改。'
              : (result.error ?? '保存失败')
          )
      }
      if (!current(id, version)) return
      selected.value = project
      filePath.value = path
      isNew.value = false
      baseline.value = text
      notice.value = '配置已保存，尚未应用；可先校验，再启动 / 更新。'
      await remember(project)
      // 重新读取版本；失败时禁止再次保存，避免持有错误的冲突基线。
      if (!current(id, version)) return
      mtime.value = undefined
      const saved = await ipc.sshEditOpen(id, path)
      if (!current(id, version)) return
      if (!saved.ok || saved.content !== text)
        throw new Error('保存后远端内容发生变化，请重新读取后再编辑')
      mtime.value = saved.modifiedAt
    } catch (e) {
      if (current(id, version)) error.value = String(e)
    } finally {
      saving.value = false
    }
  }

  async function run(next: ComposeAction) {
    const id = connection()?.sessionId
    const project = selected.value
    if (
      !id ||
      !connected.value ||
      !project ||
      !project.configFiles.length ||
      busy.value ||
      dirty.value
    )
      return
    const version = generation
    action.value = next
    error.value = notice.value = ''
    output.value = null
    try {
      const result = await ipc.sshComposeAction({ connectionId: id, project, action: next })
      if (!current(id, version)) return
      output.value = result
      if (result.exitCode !== 0) error.value = `操作失败，退出码 ${result.exitCode}`
      else notice.value = next === 'config' ? 'Compose 配置校验通过' : '操作完成'
      await refresh()
    } catch (e) {
      if (current(id, version)) error.value = String(e)
    } finally {
      action.value = null
    }
  }

  watch(
    () => connection()?.sessionId,
    () => {
      generation += 1
      loading.value = false
      // 保留草稿，旧会话文件版本不再适合作为保存基线。
      mtime.value = undefined
      if (loaded.value && !isNew.value) error.value = '连接已变化，请保留草稿后重新读取配置再保存'
      remote.value = []
      void refresh()
    },
    { immediate: true }
  )

  return {
    projects,
    selected,
    filePath,
    draftName,
    content,
    isNew,
    loaded,
    loading,
    listing,
    saving,
    action,
    error,
    listError,
    notice,
    output,
    connected,
    dirty,
    busy,
    mtime,
    refresh,
    open,
    create,
    save,
    run,
  }
}
