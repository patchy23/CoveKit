/**
 * useHostKeyQueue · 主机密钥人工确认队列所有者
 *
 * 职责：把后端并发首连时推送的多个主机密钥确认请求排队（单值会被覆盖，导致先到的请求永远挂起），
 * 并把队首请求暴露给界面；应答按「先来先答」出队，卸载时对积压请求统一取消，
 * 避免后端握手回调挂到超时。
 */
import { computed, ref } from 'vue'
import { ipc } from '../ipc'
import type { HostKeyVerifyRequest } from '../contracts'

/** 主机密钥确认决策（与后端 `ssh_host_key_respond` 契约一致） */
export type HostKeyDecision = 'trustOnce' | 'trustSave' | 'cancel' | 'replace'

export function useHostKeyQueue() {
  /** 待确认请求（FIFO） */
  const queue = ref<HostKeyVerifyRequest[]>([])
  /** 队首请求：界面一次只展示一个确认弹窗 */
  const hostKeyRequest = computed(() => queue.value[0] ?? null)

  /** 入队一条确认请求（后端事件回调） */
  function enqueue(request: HostKeyVerifyRequest) {
    queue.value.push(request)
  }

  /** 应答队首确认请求；队列为空时不做任何事（晚到的用户点击） */
  async function respondHostKey(decision: HostKeyDecision) {
    const request = queue.value.shift()
    if (!request) return
    await ipc.sshHostKeyRespond({ requestId: request.requestId, decision }).catch(() => undefined)
  }

  /** 卸载清理：积压请求统一取消并清空队列 */
  function cancelAll() {
    for (const pending of queue.value) {
      void ipc
        .sshHostKeyRespond({ requestId: pending.requestId, decision: 'cancel' })
        .catch(() => undefined)
    }
    queue.value = []
  }

  return { hostKeyRequest, enqueue, respondHostKey, cancelAll }
}
