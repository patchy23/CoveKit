# 公共组件 API 备忘

> **何时读**：用到某个公共组件、要确认它的 props / emits / 事件名 / 返回值时。
> **本文讲什么**：`@/core/ui` 下各组件的真实签名与用法样例（不是设计意图，是实际接口）。
> **维护**：公共组件契约改变时同步对应条目，历史说明不能代替实际接口。
> 契约红线（哪些组件必须用、禁止原生控件）见 [`11-插件UI开发约定.md`](11-插件UI开发约定.md)。

---

## 组件真实 API 备忘

- **UiEmptyState 紧凑空状态**：子表、展开详情等局部区域传 `compact`，使用正文小字号、常规字重与 64px 最小高度；默认仍为页面空状态的标题字号和 160px 最小高度。组件固定使用界面字体，避免继承表格的数据字体。

- **展开行的视觉层级**：`UiTableExpandableRow` 使用缩进的 neutral 底衬与 surface 内容区、细边框和小圆角，避免详情与相邻主行斑马纹混在一起；不叠加阴影。子表推荐 `UiTable density="compact"`，不要使用 comfortable 放大次级信息；表单和反馈内容自行添加内部间距，表格可直接铺满内容区。

- **UiTableExpandableRow 表格行展开**：从 `@/core/ui` 导入，放在 `UiTable` 的 `tbody` 内；组件生成主行和跨列详情行，不增加表格外包装。必填 `label`、`columns`（总列数，包含组件生成的首列）和受控 `v-model:expanded`；`disabled` 禁止切换。`#label` 自定义首列名称，默认插槽放其余 `UiTableCell`，并提供 `{ toggle, expanded, detailsId }` 供操作列复用；`#details` 放子表格、表单或状态提示。展开按钮支持原生键盘操作并带展开状态及详情区域关联。默认首次展开挂载、收起卸载；`keepMounted` 启用后首次展开才挂载，收起保留输入和实例，隐藏期间的轮询等副作用由业务管理。单行或多行展开由父层决定，加载、错误、空状态也由业务负责。组件是多根行片段，不依赖根节点 class 透传；父子 `UiTable` 的密度、条纹和悬停样式各自独立。组件实验室的“表格行展开”展示上述行为。

- **右键菜单更新不先卸载**：公共 ContextMenu 不在右键按下时关闭；业务 contextmenu 已处理时保留当前实例更新内容，新公共菜单挂载才接管并关闭旧实例。未处理的外部右键、普通外部左键仍关闭；捕获监听兼容业务 stopPropagation，窗口内公共菜单保持唯一。焦点恢复使用 preventScroll，避免带动页面滚动。

- **Tooltip 触发器引用与关闭**：插槽保持单个触发元素，组件内部隔离 Reka 对缓存 VNode 的修改，提示文字为空再恢复时保留原始元素/ref；关闭直接卸载浮层，避免过渡状态残留。触发元素本身不会随提示开关重建。

- **UiScrollArea 公共滚动区**：从 `@/core/ui` 导入，`axis` 为 `vertical`（默认）、`horizontal` 或 `both`；`theme` 为 `auto`（默认，继承主题）、`light` 或 `dark`。默认生成 div，`as` 可指定元素；已有滚动元素用 `as-child`，如 `<UiScrollArea as-child axis="both"><div ref="viewport" class="h-full">内容</div></UiScrollArea>`，保留原元素、引用与事件。循环 key 和条件分支放到组件上，不再单独写 `overflow-auto` 等滚动类。`managed` 仅提供公共标记与主题，供 xterm、CodeMirror 和原生文本框保留内部 overflow 管理；深色终端设 `theme="dark"`。所有滚动条视觉只在 `scrollbars.css` 定义，使用原生拖动与键盘行为，容器仍需按布局约束宽高。

- **悬停入口已统一**：`UiCheckbox`、`UiSwitch` 的 `title` 也已接入 `UiTooltip`，保留键盘聚焦提示；其他元素显式包装 `UiTooltip`。原生节点和未接入的组件禁止再传 `title`，由 `tooltipUnification.test.ts` 扫描全仓模板。弹窗、面板和空状态等真正的标题保留。包装循环/条件元素时把 `v-for`、`key`、`v-if/v-else` 放在 `UiTooltip` 上；不增加 DOM 容器，保留表格层级和元素引用。空提示只禁用提示，不重建触发元素；仅鼠标无按键的悬停事件被隔离，拖拽移动继续传播。

- **UiTooltip 公共悬停提示**：从 `@/core/ui` 导入，`content` 为纯文字，默认插槽只放一个触发元素；`side` 默认 `bottom`，空间不足时自动翻转；`delayDuration` 默认 400ms，`disabled` 可关闭提示。复用 Reka 的定位、键盘聚焦与 Esc 关闭，自动避让视口，Portal 层级 240，适配浅深色且不增加布局包裹。提示内容允许按最大宽度换行，不放交互控件；全局最多一个，离开立即关闭，鼠标穿透且文字不可选中，需复制的内容另设明确入口。`UiButton` 的 `title`、`UiIconButton` 的 `label/title` 和 `UiSelect` 的 `title` 已自动接入，不再输出原生 `title`。其他元素用 `<UiTooltip content="说明"><span tabindex="0">内容</span></UiTooltip>`，移除触发元素原生 `title`，图标按钮保留 `aria-label`。原生禁用按钮不可键盘聚焦，必要原因应同时显示在表单说明中。

- **ContextMenu 是声明式**：按路径导入，`:x :y :items` + item 的 `onClick` 回调 + `@close`，父组件用 `v-if="menu"` 控制显隐；`label` 默认“操作菜单”，分隔项 `{ label: '', separator: true }`，选项支持 `disabled/danger`。`size="md | sm"`，组件按实际尺寸自动避让视口并保留 8px 边距，不再要求调用方估算菜单宽高；过高接入公共滚动区。打开聚焦首个可用项，↑↓/Home/End 导航，Enter/空格执行，Esc/Tab 关闭并归还原焦点；执行动作前也先关闭并归还焦点，外部点击不抢焦点。弹窗内使用 FocusScope 暂停底层焦点约束，菜单内指针事件不触发底层弹窗遮罩关闭。
- **菜单宽度按内容自适应**：ContextMenu 的 sm 最小 140px、最大 320px，md 最小 168px、最大 340px；同时受视口宽度限制，文字单行截断，禁止业务侧复制定位与滚动逻辑。提示说明允许换行，不能套用动作菜单的单行规则。
- **UiInput/UiSelect 的 `update:modelValue` 是 `string | number`**：写入 string 型 state 必须 `String(v)` 转换，否则 vue-tsc TS2322/TS2769。
- **UiTree 树节点交互语义（2026-08-16 用户定稿，取代旧「表点击开数据页签」）**：**单击任何节点仅选中**（同步活动连接，不开页签）；**双击**：连接节点（depth 0）= 离线即连接（`onTreeOpen` 必须显式处理连接节点，只处理叶子会出现「双击不连接、右键才能连」）；可展开节点折叠/展开，不可展开叶子触发 `open` 事件（UiTree 新增 emit：表/视图→结构页签，Redis 键→键详情）；打开数据页签只在右键菜单「查看数据」。右键按层级出菜单。tab id 带连接前缀（`data-<connId>-<table>` / `structure-<connId>-<table>`）防跨连接同名表冲突。
- **UiIcon 与 AppIcon 是两套独立图标表**：UiIcon（`core/ui`，@lucide/vue 封装）是小工具图标固定集合（search/grid/copy/play/chevrons/x/refresh/eye/eye-off/dots(横向三点，更多/溢出) 等，TS 字面量枚举，用错名字直接 TS2322，新增图标 = 注册表加一行）；AppIcon（`features/ui`）是工具大图标表（lock/db/gear/sliders 等）。用图标前先 grep 对应表确认名字存在，勿跨表引用。**注册新图标前先确认 lucide 真有这个导出**：`ls node_modules/@lucide/vue/dist/esm/icons/ | grep '^<kebab-name>'`（文件名即 kebab 名，`arrow-left-to-line.mjs` → 导出 `ArrowLeftToLine`），确认后再在注册表加一行并补中文用途注释。
- **UiButton / UiIconButton 禁用与尺寸**：UiButton 显式支持 `disabled?: boolean`（裸 `disabled` 和 `:disabled` 均可）；`loading` 同样禁止激活。`as="a"` 时禁用会移除 href 和键盘入口并阻止点击，恢复后还原属性。UiIconButton 将 disabled 透传到 UiButton；xs=24、sm=28、md=36、lg=42px 正方形，响应运行中的 size 变化。
- **UiRadioGroup 互斥筛选**：`variant="radio"` 为默认圆形单选；`variant="chips"` 为横向可换行筛选按钮，四档 size、options 与受控 modelValue 共用。组必须提供 name，建议 aria-label；options 为 `{ value, label, description?, disabled? }`，chips 显示 label。支持单项/整组禁用及方向键选择；描述较长的表单继续使用 radio 变体。凭证类型筛选已接入 chips。
- **UiPanel 折叠**：`collapsible` 启用，`defaultOpen` 只控制初始状态；标题入口支持 Enter/空格，aria-expanded/aria-controls 描述状态和内容关联。内容通过 v-show 保留，折叠不销毁输入与子组件。
- **UiProgress 进度**：`value` 默认 0、`max` 默认 100，有限 value 钳制在 0..max；无效 value 按 0、非正或无效 max 按 100 展示，百分比与 aria 值一致。`indeterminate` 表示总量未知，不提供 aria-valuenow 或百分比，保留 label，动画遵循减少动画设置。
- **密码框定稿（2026-08-16 三轮纠偏后）**：一律 `UiInput type="password"`——组件自带输入框内眼睛切换明文，**禁止在 label 行/输入框右上角另加眼睛按钮**（会与自带眼睛重复）；**禁止加 `font-mono`**（掩码圆点要与普通输入框视觉一致，font-mono 只给明文数据如 AKID/私钥全文）；WebView2 原生 reveal 眼睛由 main.css `.field-input::-ms-reveal { display: none }` 全局隐藏（否则与组件眼睛重合）。私钥等多行秘密不脱敏，UiTextarea 明文可编辑。UiIcon 的 eye/eye-off 注册保留给非输入框场景（如「显示密钥」列表切换）。
- **UiSearchInput 不得加回 `type="search"`**：WebView2 原生清除按钮 × 会与组件自带清空按钮重复（双 X 实测被用户报 bug）；同理新输入类组件不要依赖浏览器原生控件装饰（搜索清除/密码眼睛都走组件自绘）。
- **Tauri 窗口内 HTML5 拖拽不可用（2026-09-05 SSH 分组拖拽实测）**：`dragDropEnabled` 默认 true 把窗口注册成 OLE 拖放目标（OS 文件拖入走 `getCurrentWebview().onDragDropEvent`，SSH 文件上传靠它），副作用是吞掉应用内 HTML5 DnD——合成 DragEvent 正常、真实鼠标拖拽无效。内部拖拽（如列表行拖到分组）用 **pointer 事件自实现**（`useGroupDrag` 模式：pointerdown + 6px 阈值 + window pointermove/pointerup + elementFromPoint 命中 `[data-group-drop]` + Esc 取消 + 跟随指针的浮动标签）；不能为了内部拖拽关掉 dragDropEnabled（会弄丢 OS 文件拖入的路径）。**pointerup 必须做无中间帧兜底**：down 与 up 之间若丢了全部 pointermove（合成输入、极快甩拖），active 永不置位 → 整次拖拽静默丢弃且无任何报错；onPointerUp 先按 up 相对 down 位移 ≥ 阈值补激活，再走投放判定（2026-09-10 双栏文件拖拽实测，背景注入一次 move 都没有）。**pointer 拖拽激活后必须锁浏览器文本选择**：pointer 自实现拖拽不拦默认行为时，拖过路径上的文字会被选中（用户报「拖拽上传下载时会选中拖动区域的文本内容」）——越过阈值激活那一刻 `document.body.style.userSelect='none'` 并 `getSelection().removeAllRanges()` 清掉已开始的选区，拖拽结束（pointerup/Esc/取消）恢复原值；只锁真拖拽期间，单击/双击选词、行内复制不受影响。
- **WebView2 原生右键菜单必须运行期屏蔽（2026-09-09 用户报「原生菜单干扰程序内右键菜单」）**：浏览器默认菜单（复制/粘贴/刷新/检查元素）会与自绘 ContextMenu 叠加。**JS `contextmenu` preventDefault 无效**——WebView2 默认菜单是 COM 层 UI（页面事件取消不影响，只有 `AreDefaultContextMenusEnabled` 能关）。Tauri 2.11 零依赖可达：`WebviewWindow.with_webview(|wv| ...)` 拿到 `PlatformWebview`（Windows 专属 `controller()` 方法），`controller.CoreWebView2()?.Settings()?.SetAreDefaultContextMenusEnabled(false)`——方法都是 webview2-com 类型的固有方法（该 crate 已被 tauri-runtime-wry 引入且全局开启 windows 相关 feature），**无需新增 Cargo 依赖也无需 import 路径**；`#[cfg(windows)]` 包住、setup 里遍历 `app.webview_windows()`。副作用：输入框原生剪切/粘贴菜单一并消失（Ctrl+C/V 不受影响）。排查思路：窗口级浮层问题先翻 wry/tauri 源码的 `PlatformSpecificWebViewAttributes` 与 `PlatformWebview` 方法表，别自己造轮子。
- **自定义滚动条必须连 corner 一起覆盖（2026-09-10 用户报「横向滚动条右侧白方块」）**：`::-webkit-scrollbar` 只自定义 thumb/track 时，横向+纵向滚动条交汇的右下角仍是浏览器默认纯白方块（scrollbar-corner）——浅色主题下格外刺眼；补 `::-webkit-scrollbar-corner { background: transparent }`（main.css unlayered 全局区，深浅色通吃，所有表格/列表一次受益）。排查引导：任何「滚动条旁莫名色块/残留 UI」先查 ::-webkit-scrollbar 系列伪元素是否覆盖全。
- **SFC scoped 样式暗色覆盖两连坑（2026-09-05 UiTable 斑马纹暗色白黑相间实测）**：① 组件 `<style scoped>` 里 `:global([data-theme='dark']) .x :deep(...)` 混写选择器会被 Vue SFC 编译**静默丢弃**——规则根本不进样式表，且无任何报错；② 暗色覆盖若写进 main.css 的 `@layer components`，按 CSS 层叠规则**整层输给**组件 scoped（unlayered）样式，同优先级还输在文档顺序。正解：暗色覆盖写 main.css **unlayered 全局区** + `html[data-theme='dark']` 前缀提优先级（参 542 行 tree-line 区段与 UiTable 覆盖段注释）；验证手法 = 浏览器查 styleSheets 里规则是否存在 + 切暗色后 getComputedStyle 实测。SSH 表格没事是因为它用 Tailwind `dark:` 类（@custom-variant 生成，不走 scoped）。
- **页签条三件套（2026-08-16 用户定稿：页签永不换行 + 溢出收纳公共化）**：① CSS 红线：`.ui-tab` 带 `whitespace-nowrap`/`shrink-0`，文字套 `.ui-tab-label`（max-w-160 truncate），`.ui-tabs` 是 `overflow-hidden` 不再 `overflow-x-auto`——「页签名字无论何时只能一行」。② 溢出用公共 **UiTabsOverflow**（`@/core/ui`）：触发器是**横向三点 dots 图标**（Ellipsis，UiIcon 注册名 `dots`；初版 chevron+数量徽标被用户否——「常规是三个点图标」），下拉样式对齐 UiSelect，逐行悬停 X 关闭，点外收起，emits `select/close`。③ composable **useTabsOverflow**（容器 ref + items + active + reserved + widthOpts{extra,maxLabel} → visible/hidden）：按**每个页签文字实际宽度**估算（CJK 14px/其余 8px + extra 固定宽，文字 capped）动态决定可见个数——固定单宽估算（如一律 190px）窄窗口只剩 3 个被用户否。两个硬约束：**容器必须 `overflow-hidden`**（否则 scrollWidth 不可测）；**估算必须配实测反馈**——纯估算偏小会出现「没显示完但没出三点」被裁掉，watchPostEffect 渲染后测 scrollWidth>clientWidth 即 trim+1 逐步多收直至收敛（宽度/页签数变化重置 trim）。**第三个硬约束（2026-09-05 实测）：UiTabsOverflow 下拉面板必须 Teleport 到 body 用 fixed 定位**——面板若绝对定位在页签条内，会被容器的 overflow-hidden 整体裁掉，症状是「点三点没反应」（其实开了但看不见）；触发器右缘对齐面板右缘。数据库工作台与 ToolWorkspace 都已接入同一套；ToolWorkspace 旧的自写「···」下拉已删，勿再造私有溢出菜单。
- **components.test.ts 的原生控件/表格禁令只扫描 `src/plugins/**/*.vue`**：`src/features/**`、`src/core/**` 下的框架级页面（SettingsPage、凭证管理面板等）不被契约测试强制，但仍须自觉走 Ui* 组件与语义字体 token。
- **UiCombobox（2026-09-05 新增，`@/core/ui`）**：可输入搜索的下拉选择（reka Combobox 封装）——触发框输入即过滤（匹配 label + keywords，大小写不敏感）、↑↓ 高亮、Enter 选中；样式对齐 UiSelect（`ui-control-*` 档位 + z-[220] 浮层）。props：`modelValue / options{value,label,keywords} / placeholder / searchPlaceholder / emptyText / size`；选项末尾可放「+ 新建」哨兵项由调用方拦截（ServerForm 的 `__create__` 模式）。选项多（>8 个）或需要搜索时用它，不要用 UiSelect 硬撑。
- **UiListRow（2026-09-05 新增，`@/core/ui`）**：列表行壳——统一 size（sm=28px/md=34px）+ hover/选中态 + indent 缩进 + cursor 语义（default/pointer/grab），内容全走默认插槽，事件（click/dblclick/contextmenu/pointerdown）直接挂组件上透传根元素。普通列表行复用此壳；需要排序或层级时使用下述 UiSortableList / UiTree，SSH 和接口侧栏已接入。组件实验室 NavigationShowcase 有示例。
- **含反斜杠的字符串 prop 禁止走 HTML 属性直传**（2026-09-08 本地面包屑不分层实测）：`separator="\\"` 传入的是两个字符的反斜杠，与 JS 里单 `'\\'` 不相等且**编译无警告**——必须 `:separator="'\\'"` 绑定表达式。排查信号：组件行为像「没收到 prop」但模板里明明写了。
- **Vue 多根片段静默丢 class 透传（2026-09-06 FileBrowser 列表「显示不全」实测）**：组件 template 有多个根节点时，父组件传的 `class="min-w-0 flex-1"` 等 attrs 被 Vue **静默丢弃**（仅控制台 warning），flex 尺寸丢失 → 列表区塌陷空白。拆子组件时必须包单根 `div.flex.h-full.min-h-0.flex-col` 再谈内部布局。同类：给行元素挂 `:data-selected` 自定义属性但全仓没有对应 CSS = 选中态哑的（SSH 文件双栏两侧都踩过），选中态用真实 class 绑定或 UiListRow 的 `active` prop。
- **Vue 模板内联事件处理器不支持多语句**（2026-09-05 vite 编译报错白屏实测；2026-09-09 在 ConfirmDialog `@confirm="emit(...); x = false; emit('close')"` 又踩一次——**「确认后连环动作」是高发形态**，写到弹窗 confirm 处理器时就要警觉）：分号/换行多语句直接被 vue compiler 拒绝（vite-error-overlay 报 Error parsing JavaScript expression）。多步操作一律抽成方法（如 `onConfirmed()`），模板只留单调用。
- **右键菜单/弹窗内操作 xterm 后必须归还焦点**（2026-09-05 实测）：菜单点击把焦点带离 xterm 隐藏输入框 → 粘贴后光标消失、键盘无响应；TerminalTab 复制/全选/粘贴三个动作的 finally 统一 `term?.focus()`。新增终端类组件照此自检。
- **凭证选择 UI 定稿（2026-09-05 两轮纠偏）**：工具侧选凭证 = **认证方式/来源下拉加「凭证」档 + 下方独立的 UiCombobox 可搜索凭证下拉**（参考 ssh/ServerForm）。两个被否方案勿回潮：① 独立「凭证库」字段与认证方式并列（初版 CredentialPicker 形态，被嫌割裂）；② 凭证条目直接混进认证方式下拉（vault:<id> 选项）——用户原话「认证方式里面加一个凭证类型，然后下面提供一个凭证的可输入搜索的下拉框」。配套行为：选中凭证后手工输入框隐藏；凭证档未选凭证时提交拦截；引用失效/凭证库加载失败**红字直接提示**（原 CredentialPicker 静默 catch 加载失败 → 「下拉是空的毫无反馈」实测 bug）；凭证类型不符由后端连接时明确报错（apply_vault_credential 模式）。



## 可排序列表与可拖拽树

- `UiSortableList`：`items: UiListItem[]`，每项有稳定 `id`、`label`，可选 `description/kind/badge/muted/disabled/draggable`。默认允许排序，无父子层级。
- `UiTree`：`items: UiTreeItem[]` 是深度优先排列的**可见节点**，在列表项基础上增加 `depth/expandable/expanded/loading`。默认不拖动，启用时传 `draggable`。空目录显式设置 `expandable`，不要根据子项数量判定目录。兼容数据库工具原有展开事件。
- 两者共用 `modelValue` 单选、`rowHeight` 22/24/28/32、`dragHandle`、`disabled/busy/loading/error/filtered/emptyText/label`。过滤中禁拖；busy 禁止重复操作；失败不改变传入数据。搜索、工具栏和业务菜单由页面组合。
- `move` 只发请求 `{ id, targetId, position }`，position 为 before/after/inside；targetId 为 null 表示根层末尾。业务 `canDrop(move)` 附加约束，组件默认拒绝自身、后代及禁用节点。业务负责原子保存并更新 items，不能只改显示顺序。
- 事件：`select(item)`、`update:modelValue(id)`、`toggle(item)`、`open(item)`、`contextmenu(item,event)`、`blankContextmenu(event)`、`retry()`。展开由业务持有，节点身份不随改名或移动改变。
- 插槽：`icon/label/row/suffix` 均提供 item，row 另提供 selected；empty 可自定义空态。suffix 中的按钮不会选择或拖动行，不将按钮嵌入另一按钮。
- 方向键导航，树左右展开/返回父项；Alt+上下移动顺序，树 Alt+左右移出/移入前一个目录。拖动超过阈值才启动；Esc、失焦、指针取消与卸载均清理；悬停目录延迟展开，边缘自动滚动。
- 插入线和拖动预览不占布局空间，不在开始拖动后往列表中插入“移至根目录”行。组件实验室 `CollectionShowcase` 演示列表、树、空目录、禁用、长列表、加载、失败重试及模拟保存失败。
