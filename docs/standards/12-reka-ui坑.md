# 12 · Reka UI 原语坑

> **常青文档**（2026-09-13 从 AI 助手技能库迁入仓库，正文未改）。
> UiSelect / UiCombobox / 模态 / 浮层 z 序的已知陷阱与排查配方。
>
> 维护：改了本文件描述的行为，就改这里——不要只改 AI 助手侧的副本。

# Reka UI 原语坑（core/ui 封装之下的行为）

`@/core/ui` 的组件是 shadcn-vue 源码模式 + Reka UI 无样式原语的薄封装。封装挡不住下面这些**原语级**约束——它们只在运行时炸，lint / build / 单测全绿。

## SelectItem / ComboboxItem 的 value 不能是空字符串

`reka-ui` 的 `SelectItem` 在 `value === ''` 时直接 `throw`（空串被它保留表示「清空选择」）：

```js
// node_modules/reka-ui/dist/SelectItem.js
if (props.value === "") throw new Error("A <SelectItem /> must have a value prop that is not an empty string. ...")
```

抛错发生在 **SelectContent 渲染时**，所以故障表现极具误导性：

- 触发器完全正常——文案显示、有箭头、`role=combobox` 在无障碍树里也在；
- 点它**弹层永远不出现**（渲染即崩），AX/SOM 树里查不到任何下拉/选项节点；
- 看起来就是「交互没接上 / 点不动」，极易误判成事件被吞、被遮挡或 `disabled` 命中。

### 规则

- 传给 `UiSelect` / `UiCombobox` 的**每一项 value 都必须非空**。
- 「不选 / 跟随默认 / 不使用」这类语义**用非空哨兵值**，不要用 `''`：

  ```ts
  /** 「跟随默认」在下拉里的哨兵值：reka 的 SelectItem 禁止空字符串 value */
  export const FOLLOW_DEFAULT_VALUE = '__follow_default__'

  /** 绑定 id → 下拉选中值（未绑定显示「跟随默认」） */
  export function toSelectValue(boundId: string | undefined): string {
    return boundId === undefined || boundId === '' ? FOLLOW_DEFAULT_VALUE : boundId
  }

  /** 下拉选中值 → 绑定 id（哨兵值回写为空串 = 解除绑定） */
  export function toBoundId(selectValue: string): string {
    return selectValue === FOLLOW_DEFAULT_VALUE ? '' : selectValue
  }
  ```

- **对外契约保持不变**：哨兵值只活在下拉这一层，`emit` 出去和落库的仍是 `''` / 真实 id。在选中回调里做反映射；漏了这步会把 `'__none__'` 当成真实 id 存进去，等于写出新 bug。
- 换算抽成**纯函数 + 单测**（仓库硬要求）：未绑定/空串都映射到哨兵、哨兵回写为空串，再补一条**断言下拉里每一项 value 都非空**——这条是唯一能防回归的。
- 公共组件里已有先例可循：`CredentialPicker.vue` 的「新建」用 `__create__` 哨兵，同类语义照这个模式走。

### 排查配方（症状：下拉点不开 / 选了没反应）

1. 先分清是「压根没弹出」还是「弹出但选中不生效」：点一下再抓 AX/SOM 树。**没有下拉/选项节点 = 没弹出**（渲染期抛错）；有节点但值不变 = 选中没回写。
2. 没弹出 → 立刻 grep 该下拉的 options 构造找空串：

   ```bash
   grep -rn "value: ''" src/
   ```

   注意区分 `xxx.value = ''`（清空表单，无关）和真正的 **option 字面量**。
3. **全仓一并扫**：这个坑不挑组件。同一个 bug 曾在插件私有组件和公共 `core/ui/CredentialPicker.vue` 里各有一处——只修用户报的那处等于没修。
4. 想确认断言存在，直接看装好的包：`node_modules/reka-ui/dist/SelectItem.js`。

### 归因纪律

不要在还没排除渲染期抛错的情况下，把「合成点击（computer_use）打不开原生下拉」当成结论。
把注入方式当原因会放过真 bug；把工具当坏掉会写进技能库变成长期误判。先修掉最可能的抛错再复验。

## 同类约束

- **Select 与 Tooltip 的定位上下文不能错套**：禁止 `TooltipRoot → SelectTrigger`，否则 SelectTrigger 的 PopperAnchor 会登记到 Tooltip 的 PopperRoot，Select 浮层拿不到锚点，表现为箭头已展开、选项停在 `translate(0, -200%)` 的测量位置，外部点击只收起看不见的列表。公共 UiSelect 使用 `SelectTrigger as-child → UiTooltip → button`，按钮显式透传 disabled。测试必须检查浮层完成定位，不能只检查 aria-expanded 和选项 DOM 存在。

- **reka 模态弹窗会把 `body` 置 `pointer-events: none`**：任何渲染在模态之上的自定义浮层（右键菜单、浮层面板）必须显式 `pointer-events-auto`，否则它显示正常但点不动。
- **浮层 z 序是分层约定的**（`UiModal` `z-[180]`、`UiContextMenu` `z-[200]`、`SelectContent` `z-[220]`）：新加浮层先看这几个档位再定 z-index，别随手写 `z-50`。
- **公共浮层通常 Portal 到 body**：先核对实际渲染目标，再检查层级、裁剪和渲染错误；不能仅凭组件类型排除 overflow 或断言一定是异常。
