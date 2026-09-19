<script lang="ts">
/**
 * UiIcon · 全局通用图标统一出口（@lucide/vue 封装）
 * 职责：业务/核心组件一律通过 name 引用图标，禁止再手写内联 SVG；
 * 换图标库时只需改本文件的注册表与渲染，调用方零改动。
 * 尺寸/线宽：size、strokeWidth 透传 lucide；颜色继承 currentColor（用 class 控制）。
 * 例外（保持自定义，不走本组件）：
 *  - 工具品牌图标注册表 src/features/ui/AppIcon.vue（视觉资产）
 *  - 数据库对象图标 src/plugins/database/DbObjectIcon.vue（自绘套装）
 *  - 数据库品牌 logo（src/assets/db-icons 图片资源）
 *  - 运行/停止等定稿主按钮图形、SSH 监控图表（非图标语义）
 */
import {
  AlignLeft,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  ChevronUp,
  Copy,
  Download,
  Ellipsis,
  Eye,
  EyeOff,
  Folder,
  GripVertical,
  FolderUp,
  Info,
  LayoutGrid,
  LoaderCircle,
  Minus,
  Package,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Save,
  Search,
  SkipForward,
  Square,
  Star,
  Trash2,
  Upload,
  X,
} from '@lucide/vue'

/** 已注册的通用图标（key 为语义名，新增图标在此加一行） */
export const UI_ICONS = {
  search: Search,
  'chevron-down': ChevronDown,
  'chevron-right': ChevronRight,
  'chevron-up': ChevronUp,
  'chevrons-left': ChevronsLeft,
  'chevrons-right': ChevronsRight,
  x: X,
  check: Check,
  minus: Minus,
  /** 旋转等待（需调用方加 animate-spin） */
  loading: LoaderCircle,
  refresh: RefreshCw,
  /** 四宫格（查看结构） */
  grid: LayoutGrid,
  play: Play,
  /** 停止（实心方块；启停按钮用） */
  stop: Square,
  /** 全部执行（播放 + 竖线） */
  'play-all': SkipForward,
  /** SQL 格式化 */
  format: AlignLeft,
  save: Save,
  copy: Copy,
  download: Download,
  /** 更多/溢出（横向三点） */
  dots: Ellipsis,
  /** 新建/添加 */
  plus: Plus,
  /** 客户端/可执行文件（frpc 客户端管理入口） */
  package: Package,
  /** 书签/收藏 */
  star: Star,
  /** 后退（返回上一次位置） */
  'arrow-left': ArrowLeft,
  /** 前进/指向右侧（双栏传输方向等） */
  'arrow-right': ArrowRight,
  /** 上级目录 */
  'arrow-up': ArrowUp,
  /** 上传 */
  upload: Upload,
  /** 上传目录 */
  'folder-up': FolderUp,
  /** 重命名/编辑 */
  pencil: Pencil,
  trash: Trash2,
  /** 目录（列表栏脚、路径展示） */
  folder: Folder,
  square: Square,
  'grip-vertical': GripVertical,
  /** 信息提示（空状态与说明文案） */
  info: Info,
  /** 秘密字段显示/隐藏（眼睛切换） */
  eye: Eye,
  'eye-off': EyeOff,
} as const

export type UiIconName = keyof typeof UI_ICONS
</script>

<script setup lang="ts">
withDefaults(defineProps<{ name: UiIconName; size?: number; strokeWidth?: number | string }>(), {
  size: 14,
  strokeWidth: 2,
})
</script>

<template>
  <component :is="UI_ICONS[name]" :size="size" :stroke-width="strokeWidth" aria-hidden="true" />
</template>
