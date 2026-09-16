# 用浏览器驱动验证 Vue UI 的坑

> **何时读**：用 Playwright / CDP / 浏览器工具实走查 Vue 页面、点不动某个控件时。
> **本文讲什么**：浏览器驱动与 reka-ui 传送门、测试环境、选择器相关的实测坑与配方。
> **体量**：约 4.2 千字符。
> **出处**：原 `11-插件UI开发约定.md` 的「浏览器驱动验证 Vue UI 的坑」节，正文一字未改。
> 测试任务书执行规范见 [`15-测试与走查.md`](15-测试与走查.md)。

---

## 浏览器驱动验证 Vue UI 的坑（本会话实测）

- **实测前先确认 dev 进程与窗口活着**：`ps -W | grep -i patchybox`（空 = 进程没了）+ `curl -s -o /dev/null -w '%{http_code}' http://localhost:1420/`（`000` = vite 已死）。dev 退出后所有 capture/click 都是空转，还会拿着过期界面状态判断「改动没生效」。后台重启 `pnpm tauri dev`，等输出出现 `Running `target\debug\patchybox.exe`` 再操作（vite ready 只代表前端就绪，窗口还没起）
- **dev端口占用**：核对PID、命令行、工作目录与启动归属。没有应用窗口不能证明服务无人使用；只停止本任务启动且确认可清理的实例，未知归属不强杀，已有合适开发服务可协调复用。
- **用户要「跑起来我测、带后台黑窗口看日志」时**：`background=true` 跑 `export PATH="/c/Users/patchy/AppData/Roaming/npm:$PATH"; pnpm tauri dev`，配 `notify=["Running"]`（匹配 dev 打印的就绪行，`Running` 开头那行），日志留在会话里随时 poll 读，别重定向到文件（用户会在终端标签里自己看）；报告里给 PID 与就绪证据，并提醒改前端走 HMR、改 Rust 我会重编
- **每次交互后重新 querySelector**：Vue 重渲染会替换 DOM 节点，旧引用上 dispatchEvent 静默无效；且状态更新在 nextTick，断言要包 `setTimeout(…, 150)` 或 Promise
- **只发 `.click()` 经常静默无效**（2026-09-05 实测）：对挂 Vue 组件事件的按钮要发完整事件序列 `mousedown(bubbles) + click(bubbles)`；断言前等 nextTick
- **browser_console 表达式约束**：多行带注释的 async IIFE 会报 "Unexpected end of input"——压成单行、去注释、用 `function(){}` 而非箭头函数
- treeitem 的 `textContent` 含图标字符/SVG 无文本——匹配节点文字用 `startsWith`/`endsWith` 而非 `===`
- HMR 大改后会整页刷新丢状态，console 查不到元素就先重新 navigate + 重新点开工具页
- **真实 Tauri 窗口可视化验证已打通（2026-09-08 实测）**：`computer_use list_windows` 找到 patchybox.exe 的 window_id（标题是应用名如 "Hekara"，另有一个 `com.patchy23.patchybox-siw` 辅助窗不用管），再 `capture(mode='vision', pid, window_id)` 即得窗口截图 + vision 分析——能直接验证「列表是否渲染/布局是否塌陷」这类 IPC 依赖的页面（浏览器 localhost:1420 测不了的）。layout 类 bug（显示不全/挤压）优先这条路，别盲改 CSS。**需要点击/操作时改用 `capture(mode='som')` 而非 vision**：som 返回带编号的可点元素 + AX 树（vision 模式永远报 0 interactable elements，逼你盲猜坐标空转好几轮）；SOM 元素 bounds 是**原生桌面坐标**（≈截图像素 ×1.11），点击直接用 element 编号，坐标只兜底；**坐标换算**：bounds↔click coordinate 相差窗口 native 原点（capture 里 Document bounds 的 x/y），互转用 ± 原点，别按截图宽高比例猜。**2026-09-12 补**：窗口 native 尺寸大于截图时还要除以缩放系数（实测截图 1455×945 / native 1500×1000 ≈ **1.031**），完整换算式 `screenshot = (native - 窗口原点) / scale`，由此算出的坐标与 UIA 实际 invoke 点误差 ≤1px（连续 4 次点击全部命中预期元素）；`coordinate=` 传的是**命中判定**——只要落进元素矩形内就由 UIA 精确 invoke 到元素中心，不必自己精算中心，取矩形内略偏中的位置即可。**拖拽/pointer 事件验证必须 `delivery_mode='foreground'`**：background 投递走 PostMessage/UIA（拖拽降级为 pen 合成注入），WebView2 的 pointer 事件通道收不到——症状是拖拽静默无反应、代码看着没问题；foreground SendInput 有 20 步中间采样才能触发 pointerdown/move/up（2026-09-10 双栏拖拽实测）。**自绘浮层菜单（ContextMenu）的验证路线（2026-09-12 修正上一条结论）**：`action: "right_click"` 加 coordinate 能在 WebView2 里真的触发 `@contextmenu` 菜单（background 投递即可，不必 foreground），**打开后再 `capture(mode='som')`，UiButton 渲染的菜单项会作为 Button 节点出现在 AX 树里**（可直接读全部文案、按坐标点中，实测「重命名/复制一份/编辑备注/在资源管理器中显示/删除」五项一次全读到）。只有当菜单项确实没进 AX 树时，才退回截图 + vision_analyze 问「X 项的像素坐标」再点。som 截断的元素 label 全量在 `C:\Users\patchy\AppData\Local\hermes\cache\computer_use\elements_*.json`，search_files 可查。**窗口不在前台时整个注入通道可能失效**：background Posted click 无任何效果、foreground 也被系统前台窗口顶回（foreground_unavailable）——此时若待验证对象是纯逻辑（如右键菜单项组装），直接写单测断言数据（useFileContextMenu.test.ts 模式：openMenu 驱动 + menuItems 逐项断言）替代 UI 实测，别在注入上反复消耗轮次。
- **有 WebView2 CDP 通道时优先用它，别走坐标点击（2026-09-16 实测）**：`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9222' pnpm tauri dev` 起应用后，`curl -s http://127.0.0.1:9222/json/list` 拿到 WebView 页目标的 `webSocketDebuggerUrl`，用 Python `websockets` 跑 `Runtime.evaluate`（`awaitPromise=True`、`returnByValue=True`）即可按文本/DOM 选择器驱动并读回状态，能 await 轮询等界面到位、能连续跑上百轮开关；页内还能 `await import('/src/plugins/<id>/ipc.ts')` 直接调插件 IPC 造夹具状态（vite dev 按路径服务模块）。取数钩子见[15-测试与走查](15-测试与走查.md)。

- **面板类改动看截图先确认连接状态**：连接页签的状态点变红 = 会话已断（长时间空闲或整页刷新都会断），断开态下文件面板本就空白——别把空面板当成「改动把渲染搞坏了」的回归证据；vite 输出里该文件的 HMR update 无报错即可先按通过处理，重连后再做视觉确认
- **dev 运行中新增 `*.test.ts` 会触发 vite 整页 reload**（新文件被 vite 当新依赖处理，实测）：用户的活体状态（打开的页签、已建立的连接）被重置回工具首页。用户正在 dev 里配合测试时别顺手补测试文件；确需补就先说明「会刷新应用」，或等这轮测试结束再补
- **直接往插件数据目录落盘的手工假条目不会出现在列表里**：列表走 IPC + 挂载时刷新，页面不重新挂载就看不到新文件，右键/点选验证也拿不到它。要验证列表内的交互（右键菜单、行内按钮）时**直接用列表里已有的条目**——菜单行为与条目内容无关，拿现有档案验证即可；要造数据就走应用自己的新建入口使其入库。别为了逼出新文件反复折腾刷新/重启 dev。

- **改选区/装饰这类「看得见但摸不着」的样式后，用真实鼠标拖选验证**：JS 里 `getSelection().addRange()` 走不到 CodeMirror 自己的鼠标选区路径（它监听 pointer 并画自绘层）。用 CDP 派发三段真实鼠标事件——`Input.dispatchMouseEvent` 的 `mousePressed` → `mouseMoved`（带 `buttons=1`）→ `mouseReleased`，坐标取 `.cm-line` 的 `getBoundingClientRect()`；再读 `.cm-selectionLayer .cm-selectionBackground` 的 `getComputedStyle().backgroundColor`，与设计值逐位对齐，并**聚焦与失焦两种状态各测一次**（两态可能命中不同规则）。
- **用户说「还是看不见 / 一样的啊」时，去读他们截图的像素，别只猜颜色**：视觉通道不可用时用 PIL 做直方图（`Counter(im.getdata()).most_common()`）——目标色以大面积出现（本次选区色占位图 23% 像素）说明改动**已生效、问题在浓度**；出现的是库内置色（如 `#d7d4f0`）则说明**选择器被压过**。两种结论对应完全不同的修法，比反复调值省数轮。

