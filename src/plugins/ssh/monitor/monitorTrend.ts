/** 监控曲线按真实采样时间定位；不足三分钟时保留左侧空白，不拉伸成完整历史。 */
export const TREND_WINDOW_MS = 180_000

export function monitorTrendPath(
  samples: { timestamp: number; value: number }[],
  end: number,
  max: number
): string {
  if (!Number.isFinite(max) || max <= 0) return ''
  let connected = false
  return samples
    .map(({ timestamp, value }) => {
      if (!Number.isFinite(value) || timestamp < end - TREND_WINDOW_MS || timestamp > end) {
        connected = false
        return ''
      }
      const x = ((timestamp - end + TREND_WINDOW_MS) / TREND_WINDOW_MS) * 600
      const y = 100 - (Math.max(0, Math.min(value, max)) / max) * 100
      const point = `${connected ? 'L' : 'M'}${x.toFixed(1)},${y.toFixed(1)}`
      connected = true
      return point
    })
    .filter(Boolean)
    .join(' ')
}
