/**
 * 文件页双栏右键菜单装配（从 FileManagerTab 拆出，300 行红线）
 * 远程菜单（useFileContextMenu）与本地菜单（useLocalContextMenu）的动作接线集中在此。
 */
import type { RemoteFile } from '../contracts'
import type { useFileSelection } from './useFileSelection'
import { useFileContextMenu } from './useFileContextMenu'
import { useLocalContextMenu } from './useLocalContextMenu'

type Selection = ReturnType<typeof useFileSelection>

export function useFileManagerMenus(deps: {
  remoteSel: Selection
  localSel: Selection
  selectedRemoteFiles: () => RemoteFile[]
  selectedLocalFiles: () => RemoteFile[]
  refreshRemote: () => void
  refreshLocal: () => void
  currentDir: () => string
  /* 远程动作 */
  requestNewFile: () => void
  requestMkdir: () => void
  upload: (dir: string, selectDirectory?: boolean) => Promise<void>
  download: (file: RemoteFile) => void
  requestBatchDownload: (files: RemoteFile[]) => void
  openFile: (file: RemoteFile) => Promise<void>
  requestRename: (file: RemoteFile) => void
  requestDelete: (file: RemoteFile) => void
  requestBatchDeleteRemote: (files: RemoteFile[]) => void
  onChmod: (file: RemoteFile) => void
  onBookmark: (dir: RemoteFile) => void
  /* 本地上传/批量 */
  uploadLocalPaths: (paths: string[], remoteDir?: string) => Promise<void>
  requestBatchUpload: (files: RemoteFile[]) => void
  /* 本地动作 */
  requestLocalNewFile: () => void
  requestLocalMkdir: () => void
  requestLocalRename: (file: RemoteFile) => void
  requestLocalDelete: (file: RemoteFile) => void
  requestBatchDeleteLocal: (files: RemoteFile[]) => void
}) {
  const { menu, menuItems, openMenu } = useFileContextMenu({
    selection: deps.selectedRemoteFiles,
    isSelected: (path) => deps.remoteSel.selectedPaths.value.has(path),
    selectOnly: (file) => deps.remoteSel.selectOnly(file.path),
    refresh: deps.refreshRemote,
    newFile: deps.requestNewFile,
    mkdir: deps.requestMkdir,
    upload: (target) => void deps.upload(target?.isDir ? target.path : deps.currentDir()),
    uploadDirectory: (target) =>
      void deps.upload(target?.isDir ? target.path : deps.currentDir(), true),
    download: (file) => deps.download(file),
    batchDownload: (files) => deps.requestBatchDownload(files),
    edit: (file) => void deps.openFile(file),
    rename: deps.requestRename,
    chmod: deps.onChmod,
    addBookmark: deps.onBookmark,
    deleteFile: deps.requestDelete,
    batchDelete: (files) => deps.requestBatchDeleteRemote(files),
  })

  const {
    menu: localMenu,
    menuItems: localMenuItems,
    openMenu: openLocalMenu,
  } = useLocalContextMenu({
    selection: deps.selectedLocalFiles,
    isSelected: (path) => deps.localSel.selectedPaths.value.has(path),
    selectOnly: (file) => deps.localSel.selectOnly(file.path),
    refresh: deps.refreshLocal,
    newFile: deps.requestLocalNewFile,
    mkdir: deps.requestLocalMkdir,
    upload: (files) =>
      files.length > 1
        ? deps.requestBatchUpload(files)
        : void deps.uploadLocalPaths(
            files.map((f) => f.path),
            deps.currentDir()
          ),
    rename: deps.requestLocalRename,
    deleteFile: deps.requestLocalDelete,
    batchDelete: (files) => deps.requestBatchDeleteLocal(files),
  })

  return { menu, menuItems, openMenu, localMenu, localMenuItems, openLocalMenu }
}
