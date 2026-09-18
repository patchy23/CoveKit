import { describe, expect, it } from 'vitest'
import { fingerprint, fromRecord, newDraft, requestPayload, toRecord } from './requestDraft'
import type { ApiRecord } from './contracts'

describe('接口草稿与请求构建', () => {
  it('保留重复参数和请求头、忽略禁用行、正确处理 URL fragment 与 DELETE body', () => {
    const draft = newDraft('http')
    draft.method = 'DELETE'
    draft.url = 'https://example.invalid/api?original=1#section'
    draft.params = [
      { id: '1', key: 'q', value: '中文' },
      { id: '2', key: 'q', value: 'two' },
      { id: '3', key: 'disabled', value: 'x', enabled: false },
    ]
    draft.headers = [
      { id: '4', key: 'X-Test', value: 'a' },
      { id: '5', key: 'X-Test', value: 'b' },
      { id: '6', key: 'Content-Type', value: 'application/custom' },
    ]
    draft.bodyMode = 'json'
    draft.body = '{"remove":true}'
    const payload = requestPayload(draft)
    expect(payload.url).toBe(
      'https://example.invalid/api?original=1&q=%E4%B8%AD%E6%96%87&q=two#section'
    )
    expect(payload.headers).toEqual([
      ['X-Test', 'a'],
      ['X-Test', 'b'],
      ['Content-Type', 'application/custom'],
    ])
    expect(payload.body).toBe(draft.body)
  })

  it('SSE 支持 POST JSON，补充 Accept 且保持临时认证只用于发送', () => {
    const draft = newDraft('sse')
    draft.url = 'https://example.invalid/events'
    draft.method = 'POST'
    draft.bodyMode = 'json'
    draft.body = '{"stream":true}'
    draft.auth = { mode: 'bearer', credentialId: '', secret: 'test-only-token' }
    expect(requestPayload(draft).headers).toContainEqual(['Accept', 'text/event-stream'])
    expect(requestPayload(draft).auth?.secret).toBe('test-only-token')
    expect(toRecord(draft, '流式接口', '测试').options).not.toContain('test-only-token')
    draft.body = '{'
    expect(() => requestPayload(draft)).toThrow('JSON')
  })

  it('旧记录兼容、表单编码和行 ID 不污染脏标记', () => {
    const old: ApiRecord = {
      id: 1,
      type: 'http',
      name: '旧接口',
      method: 'POST',
      url: 'https://example.invalid',
      params: '[]',
      headers: '[{"name":"X-Test","value":"legacy"}]',
      bodyMode: 'raw',
      body: '原文',
      groupName: '',
      options: '{}',
      updatedAt: '',
    }
    const draft = fromRecord(old)
    expect(draft.bodyMode).toBe('text')
    expect(draft.headers[0].key).toBe('X-Test')
    const before = fingerprint(draft)
    draft.headers[0].id = 'different'
    expect(fingerprint(draft)).toBe(before)
    draft.bodyMode = 'form'
    draft.form = [{ id: '1', key: 'text', value: 'a & b' }]
    expect(requestPayload(draft).body).toBe('text=a%20%26%20b')
    const record = toRecord(draft, '表单', '分组')
    expect(fromRecord({ ...old, ...record, type: 'http' }).form[0].value).toBe('a & b')
  })

  it('损坏配置和错误协议不能伪装成可发送请求', () => {
    const draft = newDraft('ws')
    draft.url = 'https://example.invalid'
    expect(() => requestPayload(draft)).toThrow('ws:')
    const record = {
      ...toRecord(newDraft('http'), '测试', ''),
      id: 1,
      type: 'http',
      updatedAt: '',
      params: 'not json',
    } as ApiRecord
    expect(() => fromRecord(record)).toThrow()
  })
})
