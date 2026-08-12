/**
 * 插件引导 · 各插件副作用导入即完成注册（registerTool 各自 manifest）
 * 新增插件 = 建 src/plugins/<id>/ 目录 + 此处加一行，框架与既有插件零改动。
 * 插件间禁止互相 import（公共能力走 @/core/ui 与 @/core/ipc）。
 */
import '@/plugins/json-formatter'
import '@/plugins/xml-formatter'
import '@/plugins/ts-converter'
import '@/plugins/http-ws'
import '@/plugins/sqlite'
import '@/plugins/hosts'
import '@/plugins/dns'
import '@/plugins/ssh'
import '@/plugins/tts'
import '@/plugins/component-lab'
