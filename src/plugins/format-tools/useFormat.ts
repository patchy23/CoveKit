/**
 * JSON 格式化与校验纯函数已下沉到 `@/core/format/json`
 * （编辑器组件与插件共用同一实现，避免两份逻辑各自漂移）。
 * 本文件仅作转发，插件内的引用路径与单测保持不变。
 */
export * from '@/core/format/json'
