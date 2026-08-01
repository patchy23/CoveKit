/**
 * Presentation 双载体（架构 §4.2）
 * modal=轻量弹窗（第一批全部工具）；workspace=全内容区工作台（第二批 HTTP/WS、DB 预留）。
 * 工具不感知载体：框架按 manifest.presentation 路由挂载。
 */
import type { Presentation } from "@/core/registry/types";

export type { Presentation };

/** 载体路由：按工具声明返回挂载方式（第一批 modal，workspace 为接口 + 骨架） */
export function resolvePresentation(p: Presentation): "modal" | "workspace" {
  return p;
}

/** 打开工具的载荷 */
export interface OpenToolPayload {
  id: string;
}
