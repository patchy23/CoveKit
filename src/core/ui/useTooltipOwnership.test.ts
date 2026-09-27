import { effectScope, watchEffect } from 'vue'
import { expect, it } from 'vitest'
import { useTooltipOwnership } from './useTooltipOwnership'

it('大列表切换提示只通知新旧目标，重复经过当前目标不再通知', () => {
  const scope = effectScope()
  let notifications = 0
  const owners = scope.run(() =>
    Array.from({ length: 1_000 }, () => {
      const owner = useTooltipOwnership()
      watchEffect(() => void owner.isOwner.value, {
        flush: 'sync',
        onTrigger: () => notifications++,
      })
      return owner
    })
  )!
  try {
    owners[0].claim()
    expect(notifications).toBe(1)
    notifications = 0
    owners[999].claim()
    expect(notifications).toBe(2)
    expect(owners.filter((owner) => owner.isOwner.value)).toEqual([owners[999]])
    owners[0].release()
    owners[999].claim()
    expect(notifications).toBe(2)
    expect(owners[999].isOwner.value).toBe(true)
  } finally {
    scope.stop()
  }
})

it('旧目标卸载不关闭新目标，已卸载目标的迟到回调不能再取得归属', () => {
  const firstScope = effectScope()
  const secondScope = effectScope()
  const first = firstScope.run(useTooltipOwnership)!
  const second = secondScope.run(useTooltipOwnership)!
  try {
    first.claim()
    second.claim()
    firstScope.stop()
    first.claim()
    expect(first.isOwner.value).toBe(false)
    expect(second.isOwner.value).toBe(true)
    secondScope.stop()
    expect(second.isOwner.value).toBe(false)
    second.claim()
    expect(second.isOwner.value).toBe(false)
  } finally {
    firstScope.stop()
    secondScope.stop()
  }
})
