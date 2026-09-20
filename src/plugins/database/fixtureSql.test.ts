import { expect, it } from 'vitest'
import { fixtureSql } from './fixtureSql'
it('夹具拒绝对象注入、无限数量和未实现的方言', () => {
  expect(() => fixtureSql('mysql', 'x; DROP TABLE y', 10)).toThrow()
  expect(() => fixtureSql('postgresql', 'sample', 1001)).toThrow()
  expect(() => fixtureSql('oracle', 'sample', 10)).toThrow()
  expect(fixtureSql('sqlite', 'sample', 3)).toContain('WHERE n < 3')
})
