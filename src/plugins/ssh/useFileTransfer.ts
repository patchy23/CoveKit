/**
 * SSH 文件传输事件状态：统一处理进度、完成/失败反馈与取消订阅。
 */
import { onMounted, onUnmounted, ref } from "vue";
import { useUiStore } from "@/stores/ui";
import { onTransferProgress } from "./ipc";

/** 订阅文件传输事件，并在上传完成后触发目录刷新。 */
export function useFileTransfer(
  currentConnectionId: () => string | undefined,
  onUploadDone: () => void
) {
  const ui = useUiStore();
  const transferStatus = ref("");
  let unlisten: (() => void) | null = null;
  let disposed = false;

  onMounted(async () => {
    try {
      const stop = await onTransferProgress((progress) => {
        if (progress.connectionId !== currentConnectionId()) return;
        const upload = progress.transferId.startsWith("up-");
        if (!progress.done) {
          const percent =
            progress.total > 0
              ? Math.min(100, Math.round((progress.transferred / progress.total) * 100))
              : 0;
          transferStatus.value = `${upload ? "上传" : "下载"} ${percent}%`;
          return;
        }

        transferStatus.value = "";
        const name =
          progress.remotePath.split("/").pop() || progress.localPath.split(/[\\/]/).pop() || "文件";
        if (progress.error) {
          ui.toast(`文件传输失败：${progress.error}`);
        } else {
          ui.toast(`${upload ? "上传" : "下载"}完成：${name}`);
          if (upload) onUploadDone();
        }
      });
      if (disposed) stop();
      else unlisten = stop;
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }
  });

  onUnmounted(() => {
    disposed = true;
    unlisten?.();
  });
  return transferStatus;
}
