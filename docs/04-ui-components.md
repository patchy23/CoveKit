# 公共前端组件库

公共组件统一位于 `src/core/ui/`，业务插件只能从 `@/core/ui` 导入通用控件。组件库以 `DESIGN.md` token 为视觉单一事实源，不在插件中复制按钮、表单、页签、弹窗和状态色组合。

## 1. 组件清单

| 组件 | 用途 | 关键约定 |
| --- | --- | --- |
| `UiButton` | 按钮与链接按钮 | `primary / secondary / ghost / danger`；支持 loading、sm、block |
| `UiInput` | 单行输入 | 原生属性透传；支持 `v-model.trim`、`v-model.number`、invalid |
| `UiTextarea` | 多行输入 | 支持 resize 与 invalid |
| `UiSelect` | 下拉选择 | 自动上下翻转；支持禁用选项和业务颜色映射 |
| `UiField` | 字段布局 | 统一标签、必填、说明、错误信息 |
| `UiTabs` | 页签 | `pill` 用于视图切换，`line` 用于工作区功能切换 |
| `UiPanel` | 内容面板 | 统一边框、背景、标题、说明、操作区和内边距 |
| `UiBadge` | 状态徽标 | 仅使用已有语义 tone |
| `UiAlert` | 行内反馈 | info、success、warning、danger |
| `UiToolbar` | 操作栏 | 统一操作间距，可启用 sticky |
| `UiEmptyState` | 空状态 | 标题、说明、图标与可选操作 |
| `UiModal` | 弹窗壳 | 统一 Esc、遮罩关闭、宽度、标题和 footer |
| `UiCheckbox / UiRadioGroup / UiSwitch` | 选择控件 | 复选、互斥选择与功能开关 |
| `UiSearchInput / UiIconButton` | 高频操作 | 搜索清空与纯图标动作 |
| `UiTable / UiPagination` | 数据页面 | compact/default/comfortable 表格与受控分页 |
| `UiProgress / UiSkeleton / UiSpinner` | 加载反馈 | 进度、占位骨架和局部加载 |
| `UiAvatar / UiKbd / UiDivider` | 信息元素 | 头像、快捷键与内容分隔 |

`CodeViewer`、`LineNumberTextarea`、`ConfirmDialog`、`InputDialog`、`ContextMenu` 属于复合公共组件，继续保留在同一目录。

## 2. 尺寸体系

基础控件统一使用 `size="xs | sm | md | lg"`：

| 尺寸 | 控件高度 | 典型场景 |
| --- | --- | --- |
| `xs` | 24px | 表格行内操作、状态筛选、超紧凑工具栏 |
| `sm` | 28px | SSH/DNS/数据库列表、侧栏、分页 |
| `md` | 36px | 普通工具页、标准表单和操作栏 |
| `lg` | 42px | 舒适表单、首次引导和重点操作 |

`UiTable` 使用独立的 `density="compact | default | comfortable"`；数据密集工具不要通过缩小整个页面字号来获得紧凑效果。

## 3. 使用规则

```vue
<script setup lang="ts">
import { UiButton, UiField, UiInput, UiPanel } from '@/core/ui'
</script>

<template>
  <UiPanel title="连接设置" description="配置只保存在本机。">
    <UiField label="主机" required>
      <UiInput v-model.trim="host" class="font-mono" placeholder="127.0.0.1" />
    </UiField>
    <UiButton variant="primary" :loading="saving" @click="save">保存</UiButton>
  </UiPanel>
</template>
```

- 业务页不再直接使用 `btn-*`、`field-*` 组合类；这些类是组件内部实现细节。
- 原生按钮仅保留在表格行内图标、窗口控制等高度定制场景；可复用的文字操作必须用 `UiButton`。
- 字段错误通过 `UiField.error + UiInput.invalid` 表达，异步或跨字段反馈使用 `UiAlert` 或 toast。
- 页签不能在业务页重新拼 active 类；选择器不能退回原生 `select`。
- 新增视觉变体前先更新 `DESIGN.md`，再扩展公共组件 API。

## 4. 验收页面

应用工具列表中的“组件实验室”是独立交互测试页，覆盖按钮、表单、徽标、页签、提示、空状态和弹窗。修改公共组件时必须同时检查亮色与深色主题、禁用态、加载态和窄窗口表现。
