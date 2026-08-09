/** SSH 文件页右键菜单：按空白、目录、文件动态组装操作。 */
import { computed, ref } from "vue";
import type { ContextMenuItem } from "@/core/ui/ContextMenu.vue";
import type { RemoteFile } from "./contracts";
import { canEditRemoteFile } from "./useSsh";

interface FileMenuActions {
  refresh: () => void;
  upload: () => void;
  download: (file: RemoteFile) => void;
  edit: (file: RemoteFile) => void;
  rename: (file: RemoteFile) => void;
  select: (file: RemoteFile) => void;
}

/** 管理文件列表右键菜单坐标、目标和动态操作项。 */
export function useFileContextMenu(actions: FileMenuActions) {
  const menu = ref<{ target: RemoteFile | null; x: number; y: number } | null>(null);

  function openMenu(event: MouseEvent, target: RemoteFile | null) {
    event.preventDefault();
    if (target) actions.select(target);
    const width = 150;
    const itemCount = target?.isDir ? 2 : target ? 4 : 2;
    menu.value = {
      target,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - itemCount * 36 - 16)),
    };
  }

  const menuItems = computed<ContextMenuItem[]>(() => {
    const target = menu.value?.target ?? null;
    if (!target) {
      return [
        { label: "刷新", onClick: actions.refresh },
        { label: "上传", onClick: actions.upload },
      ];
    }
    if (target.isDir) {
      return [
        { label: "刷新", onClick: actions.refresh },
        { label: "重命名", onClick: () => actions.rename(target) },
      ];
    }
    const items: ContextMenuItem[] = [
      { label: "上传", onClick: actions.upload },
      { label: "下载", onClick: () => actions.download(target) },
    ];
    if (canEditRemoteFile(target)) {
      items.push({ label: "编辑", onClick: () => actions.edit(target) });
    }
    items.push({ label: "重命名", onClick: () => actions.rename(target) });
    return items;
  });

  return { menu, menuItems, openMenu };
}
