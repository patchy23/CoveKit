/**
 * 下载速率估算（纯函数，便于单测）
 *
 * 用滑动时间窗口，而不是「累计字节 ÷ 总耗时」：平均值在链路变慢时仍显示虚高的数字，
 * 用户看到的速率与实际进度对不上；一旦下载卡住，平均值只会缓慢下降，完全看不出卡死。
 * 窗口内样本不足（刚开始下载）时返回 null，让调用方保留上一次显示值，避免闪一个假数字。
 */

/** 单个采样点：时刻（毫秒）+ 累计接收字节 */
export interface SpeedSample {
  /** 采样时刻（毫秒时间戳） */
  at: number
  /** 该时刻的累计接收字节数 */
  received: number
}

/** 滑动窗口长度（毫秒）：越长越平滑，越短越灵敏 */
const WINDOW_MS = 2000

/**
 * 计算窗口内的平均速率
 *
 * @param samples 按时间升序的采样点（含当前点）
 * @param now 当前时刻（毫秒）
 * @returns 速率（字节/秒）；样本不足或时间跨度为零时返回 null；窗口内无新增字节时返回 0
 */
export function estimateSpeed(samples: SpeedSample[], now: number): number | null {
  const inWindow = samples.filter((sample) => now - sample.at <= WINDOW_MS)
  if (inWindow.length < 2) return null
  const first = inWindow[0]
  const last = inWindow[inWindow.length - 1]
  const elapsedMs = last.at - first.at
  if (elapsedMs <= 0) return null
  const deltaBytes = last.received - first.received
  // 没有新增字节说明传输停滞，如实返回 0（前端会显示 0 B/s，用户才知道卡住了）
  if (deltaBytes <= 0) return 0
  return (deltaBytes * 1000) / elapsedMs
}

/**
 * 追加采样点并裁剪掉窗口外的旧样本
 *
 * 同一毫秒内的多个事件只保留最后一个：那些点的时间跨度为 0，会让速率算出无穷大。
 *
 * @param samples 已有的采样点
 * @param sample 新采样点
 * @returns 新的采样点数组（不修改入参）
 */
export function pushSample(samples: SpeedSample[], sample: SpeedSample): SpeedSample[] {
  const kept = samples.filter((item) => item.at < sample.at)
  return [...kept, sample].filter((item) => sample.at - item.at <= WINDOW_MS)
}
