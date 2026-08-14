<script setup lang="ts">
/**
 * 数据库对象图标（自绘 stroke SVG，12px）
 * 说明：替代旧的 Unicode 字形（◈ ◇ ▦ ◫ ⚿ ▸），样式与工作台内嵌图标一致
 * 约定：viewBox 24、stroke=currentColor、stroke-width 1.8、圆角端点
 * kind 取值：
 *  - database / schema / group（未细分分组兜底）
 *  - group-tables / group-views / group-funcs / group-events / group-seqs
 *    group-procs / group-packages / group-synonyms / group-indexes / group-triggers / group-keys
 *  - table / view / key（叶子对象）
 */
import { computed } from 'vue'

const props = defineProps<{ kind?: string }>()

/** 每个 kind 对应一组 SVG path（含虚线等附加属性） */
interface IconShape {
  paths: string[]
  dashed?: boolean
}

const SHAPES: Record<string, IconShape> = {
  // 数据库：圆柱体
  database: {
    paths: [
      'M12 8.25c4.14 0 7.5-1.23 7.5-2.75S16.14 2.75 12 2.75 4.5 3.98 4.5 5.5 7.86 8.25 12 8.25Z',
      'M4.5 5.5v13c0 1.52 3.36 2.75 7.5 2.75s7.5-1.23 7.5-2.75v-13',
      'M4.5 12c0 1.52 3.36 2.75 7.5 2.75s7.5-1.23 7.5-2.75',
    ],
  },
  // schema：带标题栏的容器（命名空间）
  schema: {
    paths: [
      'M4 6.5A2.5 2.5 0 0 1 6.5 4h11A2.5 2.5 0 0 1 20 6.5v11a2.5 2.5 0 0 1-2.5 2.5h-11A2.5 2.5 0 0 1 4 17.5Z',
      'M4 9.75h16',
      'M7 7.1h.01',
    ],
  },
  // 分组兜底：文件夹
  group: {
    paths: [
      'M4 6.75A1.75 1.75 0 0 1 5.75 5h3.4l1.9 2.25h7.2A1.75 1.75 0 0 1 20 9v8.25A1.75 1.75 0 0 1 18.25 19H5.75A1.75 1.75 0 0 1 4 17.25Z',
    ],
  },
  // 表：网格
  table: {
    paths: [
      'M4.5 6A1.5 1.5 0 0 1 6 4.5h12A1.5 1.5 0 0 1 19.5 6v12a1.5 1.5 0 0 1-1.5 1.5H6A1.5 1.5 0 0 1 4.5 18Z',
      'M4.5 9.25h15M4.5 14h15M9.75 9.25v10.25M14.25 9.25v10.25',
    ],
  },
  // 视图：内部虚线的网格（虚拟表）
  view: {
    paths: [
      'M4.5 6A1.5 1.5 0 0 1 6 4.5h12A1.5 1.5 0 0 1 19.5 6v12a1.5 1.5 0 0 1-1.5 1.5H6A1.5 1.5 0 0 1 4.5 18Z',
      'M4.5 9.25h15',
    ],
    dashed: true, // 内部网格线用虚线
  },
  // 函数：花括号
  function: {
    paths: [
      'M8.75 4.75c-1.8 0-2.4.95-2.4 2.35v1.85c0 1.15-.7 1.9-1.6 1.9.9 0 1.6.75 1.6 1.9v1.85c0 1.4.6 2.35 2.4 2.35M15.25 4.75c1.8 0 2.4.95 2.4 2.35V8.95c0 1.15.7 1.9 1.6 1.9-.9 0-1.6.75-1.6 1.9v1.85c0 1.4-.6 2.35-2.4 2.35',
    ],
  },
  // 事件：日历
  event: {
    paths: [
      'M4.75 7A1.75 1.75 0 0 1 6.5 5.25h11A1.75 1.75 0 0 1 19.25 7v10.5a1.75 1.75 0 0 1-1.75 1.75h-11a1.75 1.75 0 0 1-1.75-1.75Z',
      'M4.75 10.25h14.5M8.5 3.5v3.5M15.5 3.5v3.5',
      'M12 14.25h.01',
    ],
  },
  // 序列：递增长短线
  sequence: {
    paths: ['M5 19.25v-5M9.5 19.25v-8M14 19.25v-11M18.5 19.25v-14'],
  },
  // 存储过程：文档 + 行
  procedure: {
    paths: [
      'M6 4.75h9L18.5 8.25V18A1.25 1.25 0 0 1 17.25 19.25H6A1.25 1.25 0 0 1 4.75 18V6A1.25 1.25 0 0 1 6 4.75Z',
      'M15 4.75V8.5h3.5',
      'M8 12.25h6M8 15.25h4',
    ],
  },
  // 包：立体盒子
  package: {
    paths: [
      'M4.75 7.75 12 4l7.25 3.75v8.5L12 20l-7.25-3.75Z',
      'M4.75 7.75 12 11.5l7.25-3.75M12 11.5V20',
    ],
  },
  // 同义词：双链环
  synonym: {
    paths: [
      'M10.2 13.8a3.1 3.1 0 0 0 4.4 0l2.6-2.6a3.1 3.1 0 0 0-4.4-4.4l-1.3 1.3',
      'M13.8 10.2a3.1 3.1 0 0 0-4.4 0l-2.6 2.6a3.1 3.1 0 0 0 4.4 4.4l1.3-1.3',
    ],
  },
  // 索引：递增柱条
  index: {
    paths: ['M4.75 19.25h14.5', 'M7.25 19.25v-6M12 19.25v-9.5M16.75 19.25V6.5'],
  },
  // 触发器：闪电
  trigger: {
    paths: ['M13.25 3.5 6 13.25h4.75L10.5 20.5 18 10.75h-4.75Z'],
  },
  // 键：钥匙
  key: {
    paths: [
      'M8.25 18.25a3.75 3.75 0 1 0 0-7.5 3.75 3.75 0 0 0 0 7.5Z',
      'M11 11.75 19.5 3.25M15.75 7l2.75 2.75M13.25 9.5l2.25 2.25',
    ],
  },
}

/** group-xxx 分组复用对应叶子对象的图标 */
const GROUP_ALIAS: Record<string, string> = {
  'group-tables': 'table',
  'group-views': 'view',
  'group-funcs': 'function',
  'group-events': 'event',
  'group-seqs': 'sequence',
  'group-procs': 'procedure',
  'group-packages': 'package',
  'group-synonyms': 'synonym',
  'group-indexes': 'index',
  'group-triggers': 'trigger',
  'group-keys': 'key',
}

const shape = computed<IconShape>(() => {
  const kind = props.kind ?? ''
  return SHAPES[kind] ?? SHAPES[GROUP_ALIAS[kind] ?? ''] ?? SHAPES.group
})

/** 视图网格的内部虚线（仅 view 使用） */
const VIEW_DASHED = ['M4.5 14h15M9.75 9.25v10.25M14.25 9.25v10.25']
</script>

<template>
  <svg
    class="h-[12px] w-[12px] shrink-0"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-for="d in shape.paths" :key="d" :d="d" />
    <template v-if="shape.dashed">
      <path v-for="d in VIEW_DASHED" :key="d" :d="d" stroke-dasharray="2.2 2" />
    </template>
  </svg>
</template>
