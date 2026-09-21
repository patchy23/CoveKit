import { expect, it } from 'vitest'
import { parseComposeContainers } from './composeContainers'

const parse = (stdout: string) => parseComposeContainers({ exitCode: 0, stdout, stderr: 'warning' })

it('逐行 JSON 保留完整 ID、镜像、暂停状态和本次运行时长', () => {
  const rows = parse(
    [
      JSON.stringify({
        ID: 'a'.repeat(64),
        Name: 'web',
        Image: 'web:latest',
        State: 'running',
        Status: 'Up 15 seconds (healthy)',
        RunningFor: '2 days ago',
        Ports: '0.0.0.0:8080->80/tcp',
      }),
      JSON.stringify({
        ID: 'b'.repeat(64),
        Name: 'db',
        Image: 'postgres',
        State: 'paused',
        Status: 'Up 2 hours (Paused)',
        Ports: '',
      }),
    ].join('\r\n\r\n')
  )
  expect(rows.map((row) => row.name)).toEqual(['db', 'web'])
  expect(rows[0]).toMatchObject({ status: 'paused', uptime: '2 hours' })
  expect(rows[1]).toMatchObject({
    id: 'a'.repeat(64),
    image: 'web:latest',
    uptime: '15 seconds',
    ports: '0.0.0.0:8080->80/tcp',
  })
})

it('旧版 JSON 数组支持 Publishers，缺少镜像和运行时长时明确显示未知', () => {
  const rows = parse(
    JSON.stringify(
      [
        {
          ID: 'one',
          Name: 'app',
          State: 'running',
          RunningFor: '3 days ago',
          Publishers: [
            { URL: '0.0.0.0', TargetPort: 80, PublishedPort: 8080, Protocol: 'tcp' },
            { URL: '::', TargetPort: 80, PublishedPort: 8080, Protocol: 'tcp' },
            { URL: '', TargetPort: 53, PublishedPort: 0, Protocol: 'udp' },
          ],
        },
        {
          ID: 'two',
          Name: 'stopped',
          State: 'exited',
          Status: 'Exited (0) 1 hour ago',
          Publishers: null,
        },
      ],
      null,
      2
    )
  )
  expect(rows[0]).toMatchObject({
    image: '—',
    uptime: '—',
    ports: '0.0.0.0:8080->80/tcp, [::]:8080->80/tcp, 53/udp',
  })
  expect(rows[1]).toMatchObject({ status: 'exited', uptime: '—', ports: '' })
})

it.each(['', '\r\n ', '[]'])('空的成功结果才显示零容器：%j', (raw) => {
  expect(parse(raw)).toEqual([])
})

it.each([
  'permission denied',
  '{',
  '{}',
  'null',
  '[null]',
  '[{"ID":"a","Name":"app"}]',
  '{"ID":"a","Name":"app","State":"running"}\ninvalid',
])('损坏或不完整结果不伪装成空列表：%s', (raw) => {
  expect(() => parse(raw)).toThrow(/Compose 容器/)
})

it('非零退出码优先于 stdout，保留错误详情', () => {
  expect(() =>
    parseComposeContainers({ exitCode: 1, stdout: '[]', stderr: 'permission denied' })
  ).toThrow('permission denied')
})

it('错误端口字段不能被静默忽略', () => {
  expect(() =>
    parse(
      JSON.stringify({
        ID: 'a',
        Name: 'app',
        State: 'running',
        Publishers: [{ TargetPort: 80, PublishedPort: 'bad', Protocol: 'tcp' }],
      })
    )
  ).toThrow('端口数值异常')
})
