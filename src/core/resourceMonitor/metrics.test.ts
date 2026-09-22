import { beforeEach, expect, it } from 'vitest'
import {
  beginMeasuredCommand,
  configureToolMetrics,
  countScopeResource,
  setMonitorCommands,
  toolMetricSnapshot,
} from './metrics'
beforeEach(() => {
  configureToolMetrics([])
  setMonitorCommands([
    { name: 'ssh_example', toolId: 'ssh' },
    { name: 'db_example', toolId: 'database' },
  ])
})
it('仅统计所选工具，取消后迟到结果不污染新会话', () => {
  configureToolMetrics(['ssh'])
  expect(beginMeasuredCommand('db_example')).toBeUndefined()
  const late = beginMeasuredCommand('ssh_example')!
  expect(toolMetricSnapshot('ssh').inFlight).toBe(1)
  configureToolMetrics([])
  configureToolMetrics(['ssh'])
  late(true)
  expect(toolMetricSnapshot('ssh').failures).toBe(0)
  const finish = beginMeasuredCommand('ssh_example')!
  finish(true)
  finish(true)
  expect(toolMetricSnapshot('ssh')).toMatchObject({
    requests: 1,
    failures: 1,
    inFlight: 0,
    completed: 1,
  })
})
it('晚开启监测仍能读取现有资源，释放后回到基线', () => {
  countScopeResource('existing-tool', 'scopes', 1)
  countScopeResource('existing-tool', 'listeners', 2)
  configureToolMetrics(['existing-tool'])
  expect(toolMetricSnapshot('existing-tool')).toMatchObject({ scopes: 1, listeners: 2 })
  countScopeResource('existing-tool', 'listeners', -2)
  countScopeResource('existing-tool', 'scopes', -1)
  expect(toolMetricSnapshot('existing-tool')).toMatchObject({ scopes: 0, listeners: 0 })
})
