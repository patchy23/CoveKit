/** 本工具 IPC，公共源获取与空间持久化均由 Rust 承接。 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { NewsCache, NewsFeeds } from './contracts'
export const ipc = {
  fetch: (): Promise<NewsFeeds> => invokeCommand('codex_news_fetch'),
  load: (): Promise<unknown> => invokeCommand('codex_news_load'),
  save: (value: NewsCache): Promise<void> => invokeCommand('codex_news_save', { value }),
}
