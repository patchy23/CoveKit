/**
 * core 层格式化与校验纯函数（各插件与编辑器组件共用同一实现）
 *
 * 目录约定：本目录（`src/core/format/`）与同名单文件 `src/core/format.ts` **不可共存**——
 * 两者同时存在时模块解析优先命中单文件，`@/core/format` 只能拿到单文件的导出
 * （2026-09-11 实测：`@/core/format` 只返回了 formatBytes）。原单文件内容已移入 `bytes.ts`。
 *
 * 引用建议：需要全部能力时用 `@/core/format`；只用一个子模块时直接写
 * `@/core/format/json` 这类显式路径，便于 tree-shaking 与按需加载。
 */
export { formatBytes } from './bytes'
export { formatJson, isValidJson, minifyJson, positionToLineCol } from './json'
export type { FormatError, FormatResult } from './json'
export { formatXml, isValidXml, minifyXml } from './xml'
export type { XmlResult } from './xml'
export { formatSql } from './sql'
