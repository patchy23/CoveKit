# ADR-0008 · 应用内拖拽用 pointer 事件自实现

- 状态：已采纳
- 日期：2026-09-14
- 批次：ssh-202609-001

## 处境

Tauri 的 `dragDropEnabled` 默认开启，而 OS 文件拖入窗口正依赖它。但它同时把窗口注册成 OLE 拖放目标，结果是**应用内 HTML5 拖拽（`draggable` / `dragstart` / `drop`）被吞掉**——2026-09-05 做 SSH 分组拖拽时实测确认，表现为拖动无反应、事件不触发。

## 决定

窗口级保留 `dragDropEnabled`，供 OS 文件拖入使用；**应用内部的一切拖拽一律用 `pointerdown` / `pointermove` / `pointerup` 自实现**，不依赖 HTML5 拖放 API（参照 `src/plugins/ssh/useServerGroups.ts` 的既有实现）。

## 代价与边界

拖动阈值、幽灵元素、落点高亮、自动滚动都要自己写，比 HTML5 拖放多做一层。禁用 `dragDropEnabled` 不是选项——那会牺牲 OS 文件拖入。若将来 Tauri 提供两者并存的能力，再评估是否回到原生 API。
