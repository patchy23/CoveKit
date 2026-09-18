/**
 * 数据包文件对话框（sync L2）
 *
 * 为什么有这一层：导出选保存位置、导入选数据包两处都在挑 `.pbdata` 文件，
 * 之前各处直接调 `plugin-dialog`，过滤器与错误语义各写一遍——取消、失败、用户手输
 * 不带扩展名的文件名三种情况的处理很容易漏（漏了就表现为「点了没反应」）。
 *
 * 语义约定：
 * 1. 统一 `.pbdata` 过滤器与建议文件名；用户手输的名字缺扩展名时补上（不覆盖其它扩展名）；
 * 2. **取消不是错误**：返回 `null` 且不提示；调用方据此静默留在原步即可；
 * 3. **失败必须可见**：对话框本身出错时提示用户（toast）并返回 `null`，不静默吞掉；
 * 4. 对话框与提示都可注入：单测替换掉 Tauri 插件与全局 toast，生产用默认实现。
 */
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog'
import type { OpenDialogOptions, SaveDialogOptions } from '@tauri-apps/plugin-dialog'
import { i18n } from '@/i18n'

/** 数据包扩展名（与 Rust 侧 `package.rs` 的容器约定一致） */
export const PB_DATA_EXTENSION = 'pbdata'

/** 文件名禁用字符（Windows 与 macOS 取交集；控制字符另按码点判断） */
const ILLEGAL_NAME_CHARS = '\\/:*?"<>|'

export interface FileDialogPort {
  save: (options: SaveDialogOptions) => Promise<string | null>
  open: (options: OpenDialogOptions) => Promise<string | null>
  /** 失败提示（生产 = 全局 toast） */
  notify: (message: string) => void
}

export interface PickPathOptions {
  /** 对话框标题（缺省由系统给默认标题） */
  title?: string
  /** 初始目录或文件名 */
  defaultPath?: string
}

export interface FileDialogApi {
  /** 选保存位置：取消或失败均返回 null（失败已提示） */
  pickSavePath: (options?: PickPathOptions) => Promise<string | null>
  /** 选已存在的数据包：取消或失败均返回 null（失败已提示） */
  pickOpenPath: (options?: PickPathOptions) => Promise<string | null>
}

/** 补上 `.pbdata` 扩展名（已有任意扩展名时原样保留，不猜用户意图） */
export function ensurePackExtension(path: string): string {
  const trimmed = path.trim()
  if (!trimmed) return trimmed
  const name = trimmed.slice(trimmed.lastIndexOf('/') + 1)
  if (name.includes('.')) return trimmed
  return `${trimmed}.${PB_DATA_EXTENSION}`
}

/**
 * 默认保存文件名：`covekit-<空间名>-<YYYYMMDD>.pbdata`。
 *
 * 为什么不用固定名：多份数据包放进同一目录会互相覆盖，用户还得自己改名；空间名与导出日期
 * 是他在文件管理器里唯一认得出的线索。空间名先剔除文件系统禁用字符与控制字符（按码点判断，
 * 不用正则控制字符类），空白折成短横线；剔除后为空则退回 `data`，不造出纯日期文件名。
 */
export function defaultPackFileName(spaceName: string, at: Date): string {
  const cleaned = [...spaceName]
    .filter((char) => char.charCodeAt(0) > 0x1f && !ILLEGAL_NAME_CHARS.includes(char))
    .join('')
    .trim()
    .replace(/\s+/g, '-')
  const pad = (value: number): string => String(value).padStart(2, '0')
  const stamp = `${at.getFullYear()}${pad(at.getMonth() + 1)}${pad(at.getDate())}`
  return `covekit-${cleaned || 'data'}-${stamp}.${PB_DATA_EXTENSION}`
}

/** 默认失败提示：全局 toast（动态导入避免 core → stores 的初始化顺序耦合） */
function defaultNotify(message: string): void {
  void import('@/stores/ui').then(({ useUiStore }) => useUiStore().toast(message))
}

function packFilter(): { name: string; extensions: string[] } {
  return {
    name: i18n.global.t('settings.dataManagement.dialogFilterName'),
    extensions: [PB_DATA_EXTENSION],
  }
}

function failureMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}

export function createFileDialog(port: Partial<FileDialogPort> = {}): FileDialogApi {
  const save = port.save ?? dialogSave
  const open = port.open ?? dialogOpen
  const notify = port.notify ?? defaultNotify

  async function pickSavePath(options: PickPathOptions = {}): Promise<string | null> {
    try {
      const picked = await save({
        title: options.title ?? i18n.global.t('settings.dataManagement.pickSaveTitle'),
        defaultPath: options.defaultPath,
        filters: [packFilter()],
      })
      if (typeof picked !== 'string') return null
      return ensurePackExtension(picked)
    } catch (reason) {
      notify(
        i18n.global.t('settings.dataManagement.pickSaveFailed', {
          message: failureMessage(reason),
        })
      )
      return null
    }
  }

  async function pickOpenPath(options: PickPathOptions = {}): Promise<string | null> {
    try {
      const picked = await open({
        title: options.title ?? i18n.global.t('settings.dataManagement.pickOpenTitle'),
        defaultPath: options.defaultPath,
        multiple: false,
        directory: false,
        filters: [packFilter()],
      })
      if (typeof picked !== 'string') return null
      return picked
    } catch (reason) {
      notify(
        i18n.global.t('settings.dataManagement.pickOpenFailed', {
          message: failureMessage(reason),
        })
      )
      return null
    }
  }

  return { pickSavePath, pickOpenPath }
}

/** 生产单例（组件直接用；需要替身的场景走 `createFileDialog`） */
export const fileDialog = createFileDialog()
