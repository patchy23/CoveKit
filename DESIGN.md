---
version: alpha
name: ToolKit
description: 明净浅色的桌面工具箱——暖橙强调、卡片化浏览、办公友好。
colors:
  primary: "#191D23"
  secondary: "#5C6672"
  tertiary: "#F0562C"
  tertiary-strong: "#C2410C"
  tertiary-soft: "#FDEEE7"
  neutral: "#F4F5F7"
  surface: "#FFFFFF"
  surface-muted: "#FAFBFC"
  border: "#E5E8EC"
  border-strong: "#D3D8DE"
  text-muted: "#98A2AD"
  on-tertiary: "#FFFFFF"
  success: "#12B76A"
  success-strong: "#067647"
  success-soft: "#E7F8F0"
  on-tertiary-dark: "#0F1216"
  primary-dark: "#E8EDF2"
  secondary-dark: "#9AA7B5"
  neutral-dark: "#0F1216"
  surface-dark: "#161B21"
  surface-muted-dark: "#1B2129"
  border-dark: "#232A34"
  border-strong-dark: "#303946"
  text-muted-dark: "#66727F"
  tertiary-dark: "#FF7A4D"
  tertiary-soft-dark: "#3A2116"
  success-dark: "#34D399"
  success-soft-dark: "#0F2E22"
typography:
  h1:
    fontFamily: "patchyBox Sans"
    fontSize: 17px
    fontWeight: 700
    letterSpacing: "-0.02em"
  h2:
    fontFamily: "patchyBox Sans"
    fontSize: 14px
    fontWeight: 700
    letterSpacing: "-0.01em"
  card-title:
    fontFamily: "patchyBox Sans"
    fontSize: 14.5px
    fontWeight: 700
    letterSpacing: "-0.01em"
  body-md:
    fontFamily: "patchyBox Sans"
    fontSize: 13px
    lineHeight: 1.5
  body-sm:
    fontFamily: "patchyBox Sans"
    fontSize: 12px
    lineHeight: 1.55
  label-caps:
    fontFamily: "patchyBox Sans"
    fontSize: 10.5px
    fontWeight: 600
    letterSpacing: "0.1em"
rounded:
  sm: 8px
  md: 10px
  lg: 14px
  xl: 16px
  full: 9999px
spacing:
  xs: 4px
  sm: 8px
  md: 14px
  lg: 22px
  xl: 26px
components:
  button-primary:
    backgroundColor: "{colors.tertiary-strong}"
    textColor: "{colors.on-tertiary}"
    rounded: "{rounded.sm}"
    padding: 11px
  button-primary-hover:
    backgroundColor: "{colors.tertiary}"
  button-ghost:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.secondary}"
    rounded: "{rounded.sm}"
    padding: 11px
  button-ghost-hover:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.primary}"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    rounded: "{rounded.lg}"
    padding: 18px
  nav-item:
    textColor: "{colors.secondary}"
    rounded: "{rounded.sm}"
    padding: 9px
  nav-item-active:
    backgroundColor: "{colors.tertiary-soft}"
    textColor: "{colors.tertiary-strong}"
  searchbox:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.primary}"
    rounded: "{rounded.sm}"
    padding: 10px
  chip:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.secondary}"
    rounded: "{rounded.full}"
    padding: 6px
  chip-active:
    backgroundColor: "{colors.tertiary-soft}"
    textColor: "{colors.tertiary-strong}"
  tag:
    backgroundColor: "{colors.success-soft}"
    textColor: "{colors.success-strong}"
    rounded: "{rounded.full}"
    padding: 3px
  toast:
    backgroundColor: "#1C2129"
    textColor: "#FFFFFF"
    rounded: "{rounded.sm}"
    padding: 10px
  card-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.primary-dark}"
    rounded: "{rounded.lg}"
    padding: 18px
  button-primary-dark:
    backgroundColor: "{colors.tertiary-dark}"
    textColor: "{colors.on-tertiary-dark}"
    rounded: "{rounded.sm}"
    padding: 11px
  nav-item-dark:
    textColor: "{colors.secondary-dark}"
    rounded: "{rounded.sm}"
    padding: 9px
  nav-item-active-dark:
    backgroundColor: "{colors.tertiary-soft-dark}"
    textColor: "{colors.tertiary-dark}"
  chip-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.secondary-dark}"
    rounded: "{rounded.full}"
    padding: 6px
  chip-active-dark:
    backgroundColor: "{colors.tertiary-soft-dark}"
    textColor: "{colors.tertiary-dark}"
---

## Overview

ToolKit 是一个面向大众用户的桌面工具箱：明净、亲和、低学习成本。整体基调是"工具商店"——用户浏览分类、扫读卡片、点开即用。暖橙（#F0562C）是唯一的交互驱动色，点缀在米白底上，传递专注而不刺眼的效率感。

## Colors

- **Primary (#191D23):** 正文、标题、卡片文字。深墨色，接近纯黑但不刺眼。
- **Secondary (#5C6672):** 次级文字、导航项、描述。在白色背景上对比度 5.8:1，满足 WCAG AA。
- **Tertiary (#F0562C):** 交互驱动色——选中态、图标、HOT 标签、强调文字。大面积使用会稀释信号，仅用于状态与焦点。
- **Tertiary-strong (#C2410C):** 主按钮底色。白字对比度 5.2:1，满足 AA（#F0562C 仅 3.5:1，只用于大号元素与图标）。
- **Tertiary-soft (#FDEEE7):** 选中态背景、图标底色。橙色信号的低对比载体。
- **Neutral / Surface / Border:** 页面背景 #F4F5F7、卡片 #FFFFFF、描边 #E5E8EC 的三级灰阶，构成卡片悬浮感。
- **Success (#12B76A):** 仅用于"热门/完成"等正反馈标签，不参与布局。
- **Dark mode:** 所有语义色有对应 `-dark` 变体（中性灰阶反转、强调色提亮为 #FF7A4D 以适配深底）。

## Typography

`patchyBox Sans` 全站统一：拉丁字符通过 unicode-range 使用本地 Inter Variable，CJK 字符通过独立 unicode-range 使用 PingFang SC / Microsoft YaHei UI / Noto Sans SC，并按 Light、Regular、Bold 真实字面映射字重；禁止依赖 Windows FontLink 或浏览器合成粗体渲染中文。层级靠字重与字号，不换字体族。卡片标题 14.5px/700 与正文 12px 形成明确的主次；`label-caps` 用于侧栏分类小标与页面小标题，强调间距不强调字号。技术字段使用 `patchyBox Mono` 组合字体：拉丁字符通过 unicode-range 使用 Cascadia Code / Consolas，CJK 字符通过独立 unicode-range 使用 PingFang SC / Microsoft YaHei；禁止依赖 Windows FontLink 或 generic monospace 渲染中文，任何位置都不得出现宋体类字形。

## Layout

间距基线 4px。卡片网格 gap 用 `md`（14px），页面内边距用 `xl`（26px），区块间距 `lg`（22px）。卡片最小宽 228px，`auto-fill` 自适应列数，保证 1360px 窗口下 5 列、缩窗平滑降列。

## Elevation & Depth

- 卡片静态：1px 描边 + 极浅投影 `0 1px 2px rgba(16,24,40,.04)`，悬浮时抬升 3px 并加深投影，配合顶部 3px 橙色渐变线作为"可点击"信号。
- 弹窗：`0 12px 40px rgba(16,24,40,.14)`，打开时 180ms 上浮淡入。
- 深色模式投影加深，但保持同样的层级逻辑。

## Shapes

圆角克制：交互元素 `sm`（8px），卡片 `lg`（14px），弹窗 `xl`（16px），胶囊标签 `full`。窗口本体 16px 圆角 + 1px 描边，作为桌面"悬浮窗口"的视觉锚点。

## Components

- `button-primary` 每屏至多一个；主按钮白字必须落在 tertiary-strong 上，不能直接使用 tertiary。
- `card` 是工具列表的默认载体：图标（44px 圆角方块、tertiary-soft 底）+ 标题 + 一行描述 + 标签行。
- `nav-item` 侧栏导航，选中态用 tertiary-soft 底 + **tertiary-strong 字**（13px 小字在浅橙底上需 ≥4.5:1，tertiary 本身仅 3.1:1）；计数徽标随选中态反色。
- `chip` 用于最近使用与筛选标签，选中态同 nav-item-active 逻辑。
- `tag` 用于 HOT/完成标签：success-soft 底 + success-strong 字（10.5px 小字需 ≥4.5:1，纯 success 绿仅 2.4:1）。
- `toast` 固定深色，全主题通用，出现在视口底部居中。
- `code-editor` 的配色分两类，都是 `--cm-*`，**不进本文件色板**：语法高亮（`--cm-keyword` / `-property` / `-variable` / `-string` / `-number` / `-tag` / `-function` / `-operator` / `-punct` / `-comment`，近 GitHub 调色板）与编辑器装饰（`--cm-gutter-bg` / `-active-line` / `-active-gutter-bg` / `-active-gutter-fg` / `-match-bg` / `-match-border`，取 tertiary 的极淡透明度变体；`-indent-guide` / `-indent-guide-active` / `-fold-marker` / `-fold-marker-hover` 取本文件 border-strong / text-muted / secondary 同名语义值）。两类都由 `src/assets/styles/main.css` 的浅色/深色同名区块维护：同名覆盖即完成主题切换，无需重建编辑器实例；编辑器容器外观（边框、背景、字号、行高）仍走上方语义 token。
- 深色模式组件使用 `-dark` 后缀变体（`card-dark`、`nav-item-active-dark`、`chip-active-dark`…），深色底上的橙色文字对比度 ~5.8:1，无需加深。

## Do's and Don'ts

- **Do** 使用 token 引用（`{colors.tertiary}`）而非字面十六进制。
- **Do** 保留一个 accent 色；新状态色先扩展调色板再使用。
- **Don't** 在白色背景上用 tertiary 承载小号白字（对比度不足），一律用 tertiary-strong。
- **Don't** 为每个卡片加投影——描边 + 浅投影是默认，深投影只属于悬浮与弹窗。
- **Don't** 嵌套组件变体。`button-primary-hover` 是平级键，不是子键。
- **Don't** 引入调色板之外的色值；深色模式必须走 `-dark` 变体。
