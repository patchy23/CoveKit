# 编辑器基座全维度对比：CodeMirror 6（现用）vs Monaco Editor（2026-09-12）

> 用途：回答「要不要换成 Monaco」这类问题，避免重复调研；作者会话：编辑器组件打磨。
> 结论先行：**维持 CodeMirror 6**，Monaco 唯一实质优势（语言服务级智能提示）不在本项目需求内。

---

## 1. 测量方法（可复现）

| 项 | 做法 |
| -- | ---- |
| Monaco 版本 | `monaco-editor@0.56.0`（npm registry `latest`，2026-09-12 查） |
| 体积 | npm registry `dist.unpackedSize` + jsdelivr `data.jsdelivr.com/v1/packages/npm/...?structure=flat` 文件清单聚合 |
| 传输量 | 真实 Chromium 加载 CDN 版 Monaco，读 `performance.getEntriesByType('resource')` 的 `transferSize` 求和 |
| 性能 | 同一浏览器：`monaco.editor.create` / `UiCodeEditor` 挂载同文档，记同步耗时 + 双 `requestAnimationFrame` 首帧 |
| 内存 | `performance.memory.usedJSHeapSize` 在「模块加载完成 → 建编辑器」前后的差值（同一口径） |
| 测试文档 | 1 万行 JSON（187,783 字符）与 10 万行 JSON（2,077,783 字符），内容一致 |
| 环境 | Windows + 真实 Chromium；我们这边 dev（未压缩）；Monaco 用 CDN 的 min 版；Monaco 关闭 minimap |

> 注意：我们这边是 dev 未压缩构建，Monaco 是压缩版 —— 即便在这个对我们不利的口径下，差距仍然显著。

---

## 2. 体积

| 维度 | 我们（CM6） | Monaco 0.56 | 倍数 |
| ---- | ----------- | ----------- | ---- |
| npm 解包 | 官方包按需（`view` 1.2MB + `state` 0.4MB + 扩展按需） | **93.4MB**（1909 文件） | 约 200× |
| 我们的构建产物 | `vendor-editor` **417.81KB / gzip 135.62KB** | `min/` 23.3MB（其中 `ts.worker` 13MB） | 55× |
| 实际传输（可用集） | **135.62KB gzip**（运行时 + 常用语言包） | **1141KB gzip**（22 个请求） | 8.4× |
| 语言包加载 | 按需独立 chunk（如 `erlang-*.js` 8.10KB） | 语言服务 worker：json 0.84MB / css 1.00MB / ts 6.43MB（min） | — |

Monaco 93.4MB 的构成：`dev/` 42.6MB（未压缩，运行时不用）+ `esm/` 27.0MB + `min/` 23.3MB；按关键词聚合 `typescript` 30.4MB、`worker` 49.9MB。

---

## 3. 性能（同文档、同口径）

| 场景 | 我们（CM6） | Monaco | 倍数 |
| ---- | ----------- | ------ | ---- |
| 1 万行 · 同步创建（热） | **11.8ms** | 22.2ms（首测含惰性初始化 146.5ms） | 1.9× |
| 1 万行 · 首帧（双 rAF） | **15.4ms** | 36.3ms | 2.4× |
| 10 万行 · 同步创建 | **32.7ms** | 94.3ms | 2.9× |
| 10 万行 · 首帧 | **38.1ms** | 173.6ms | 4.6× |
| 首屏加载 | dev 冷启动 65.1ms / 首帧 123.2ms（本地） | 1564ms（CDN，含网络；loader 单项 828ms） | — |

---

## 4. 内存与 DOM

| 维度 | 我们（CM6） | Monaco |
| ---- | ----------- | ------ |
| 同文档（1 万行 JSON）堆增量 | **0.47MB**（Vue + CM + 实例 + 文档） | 4.3MB（monaco 惰性初始化 + 实例 + 文档） |
| 空实例增量（历史记录） | 224KB / 实例（预算 <10MB） | 未单测 |
| 视口 DOM 行数 | 36 `.cm-line`（固定） | 56 `.view-line` |
| 虚拟化 | 有（只渲染视口） | 有 |

两者都做了视口虚拟化，DOM 不随行数增长；差异在运行时基础开销。

---

## 5. 功能覆盖（对照任务书 §4 的 22 项）

| 能力 | 我们 | Monaco |
| ---- | ---- | ------ |
| 1 语法高亮 / 2 行号 / 3 当前行 / 4 撤销重做 / 5 缩进 / 6 括号 / 7 折叠 / 8 多光标 / 9 列选择 / 10 查找替换 / 11 跳转行 / 12 选中词高亮 / 16 只读 / 17 明暗主题 / 19 右键菜单 / 20 快捷键 / 21 变更钩子 / 22 大文件降级 | 全部自研落地并验收 | 原生覆盖 |
| 18 状态栏 | 自绘（行列/选中/语言/缩进/长度） | **无内建**（VS Code 的状态栏属工作台外壳，需自建） |
| 13 格式化 | JSON / XML / SQL（复用仓库纯函数 + 自研 SQL 格式化） | JSON / CSS / HTML 内建，**XML 与 SQL 无**（需第三方扩展） |
| 14 语法错误标记 | JSON / XML / SQL 基础（行内波浪线 + 中文提示） | JSON / TS / CSS / HTML 内建，**XML 与 SQL 无** |
| 15 补全 | schema / 关键字 / 词法（注入式） | **语言服务级（唯一实质优势）** |
| 界面文案 | 全中文自绘（查找面板、状态栏、右键菜单） | 需引入本地化包，自绘部分仍要自己写 |
| 主题贴合 `DESIGN.md` | 直接吃 CSS 变量（`--cm-*`），暗色用变量覆盖 | `defineTheme` 自定义 token 规则表，与 tokens 体系需手工对齐 |

---

## 6. 工程集成（Tauri 侧，关键差异）

| 维度 | 我们（CM6） | Monaco |
| ---- | ----------- | ------ |
| web worker | **不需要** | 语言服务必须走 worker |
| CSP | 无额外放宽（无 worker / 无 eval） | 需 `blob:` 放宽（Tauri 默认收紧） |
| 构建接入 | 普通 Vite 动态 import | 需 `vite-plugin-monaco-editor` 或手写 worker 入口（社区有卡壳案例） |
| 单 exe 便携交付 | 前端资源内嵌，增量 135.62KB gzip | 内嵌后体积按 MB 计 |
| 离线 | 天然离线 | 天然离线（但资源体积大） |

---

## 7. 生态与维护

| 维度 | 我们（CM6） | Monaco |
| ---- | ----------- | ------ |
| star | 7.8k（dev 聚合仓库） | 46.7k |
| npm 周下载 | `view` 1062 万 | 627 万 |
| 维护形态 | 多个官方小包，按需升级 | 单一大包，升级需整体替换 |
| 已知弱项 | LSP / IntelliSense 生态弱（官方 `lsp-client` 已归档） | 体积 / worker / CSP / 主题体系 |
| 同类采用者 | Obsidian、Replit、Joplin | VS Code 及需要完整语言服务的 IDE 类产品 |

**反方向证据**：Azure LogicAppsUX《Monaco to CodeMirror Migration Design》（2025-01-14，Approved）把 Monaco 整体迁到 CodeMirror 6，理由为性能与体积。

---

## 8. Monaco 的优势与「什么情况下才该换」

Monaco 实质优于我们的只有一处：**语言服务级智能提示**（TypeScript 类型检查与重构、JSON Schema 校验与路径补全、CSS/HTML 语义检查）。这是它 30MB TypeScript 编译器换来的能力，本项目明确不需要（任务书 §4「本轮不做」）。

真要换的信号（出现任意一条再评估）：需要项目级 TypeScript 类型推断与跨文件重构；需要 JSON Schema 驱动的表单级校验；需要与 VS Code 扩展生态互通。届时也是「新增一个 IDE 型工具」而非替换现有编辑器基座。

---

## 9. 复现步骤

```bash
# 体积
curl -s https://registry.npmjs.org/monaco-editor/latest | python -c "import json,sys; d=json.load(sys.stdin); print(d['version'], d['dist']['unpackedSize'])"
curl -s "https://data.jsdelivr.com/v1/packages/npm/monaco-editor@0.56.0?structure=flat"

# 性能 / 内存：在项目根放一个引用 CDN 的临时 HTML（vite dev 会直接服务），
# 加载 https://cdn.jsdelivr.net/npm/monaco-editor@0.56.0/min/vs/loader.js，
# 用 require(['vs/editor/editor.main']) 后 create 同文档，读 performance.now() 与 performance.memory。
# 我们这边同样挂 UiCodeEditor，双 rAF 记首帧。
```
