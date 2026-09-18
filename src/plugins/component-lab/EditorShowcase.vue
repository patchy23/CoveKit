<script setup lang="ts">
/**
 * 编辑器组件展示页
 *
 * 覆盖：完整档（校验 / 格式化 / 查找替换 / 状态栏 / 右键菜单）、轻量档、只读查看器、
 * 语言自动识别矩阵、大文件降级（512KB 阈值）。
 * 明暗高亮随应用主题切换（高亮色走 --cm-* 变量，无需重建实例）。
 */
import { ref } from 'vue'
import {
  UiBadge,
  UiButton,
  UiCodeDiff,
  UiCodeEditor,
  UiPanel,
  UiSelect,
  type EditorContextMenuPayload,
  type EditorCursorInfo,
} from '@/core/ui'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()

/** 完整档编辑器引用（命令式 API 演示：格式化） */
const fullEditor = ref<InstanceType<typeof UiCodeEditor> | null>(null)

/** 主编辑器语言选择：auto 表示按 filename='config.json' 自动识别 */
const language = ref('auto')
const languageOptions = [
  { value: 'auto', label: '自动识别' },
  { value: 'json', label: 'JSON' },
  { value: 'sql', label: 'SQL' },
  { value: 'plaintext', label: '纯文本' },
]

const fullSample = ref(`{
  "tool": "CoveKit",
  "version": "0.9.0",
  "features": ["format", "http", "database", "ssh"],
  "limits": { "maxFileSize": 5242880, "highlight": 524288 },
  "nested": { "enabled": true, "retries": 3, "ratio": 0.75 }
}`)

/** 光标信息（验证 cursor 回调与状态栏数据源） */
const cursor = ref<EditorCursorInfo>({ line: 1, column: 1, selected: 0 })

/** 查找替换演示：SQL 文本，Ctrl+F / Ctrl+H / Ctrl+G 可直接试 */
const searchSample = ref(`SELECT s.id,
       s.host_name,
       s.port,
       COUNT(l.id) AS conn_count
  FROM ssh_server s
  LEFT JOIN conn_log l ON l.server_id = s.id
 WHERE s.enabled = 1
   AND s.host_name LIKE '%prod%'
 GROUP BY s.id, s.host_name, s.port
 ORDER BY conn_count DESC
 LIMIT 20;`)

/** 校验演示：缺逗号 + 多逗号 + 未闭合括号，用于观察波浪线与中文提示 */
const brokenJson = ref(`{
  "tool": "CoveKit"
  "limits": { "maxFileSize": 5242880, }
  "features": ["format", "http"],
}`)

/** 大文件降级演示：约 560KB（超过 512KB 阈值 → 关闭高亮 / 折叠 / 补全） */
const largeSample = ref(buildLargeSample())

/** 差异对比样例：远端当前内容 vs 本地编辑内容 */
const diffOriginal = `{
  "tool": "CoveKit",
  "version": "1.0.0",
  "editor": "legacy-textarea",
  "limits": { "maxFileSize": 1048576 }
}`

const diffModified = `{
  "tool": "CoveKit",
  "version": "1.0.0",
  "editor": "codemirror-6",
  "limits": { "maxFileSize": 5242880 },
  "features": ["search", "format", "lint"]
}`

/** 右键菜单演示：记录最后一次触发的行号与行文本 */
const lastMenu = ref<EditorContextMenuPayload | null>(null)

const minimalSample = ref(`<?xml version="1.0" encoding="UTF-8"?>
<config>
  <server host="10.0.0.12" port="8080" />
  <retry times="3" interval="500ms" />
  <logging level="info" path="/var/log/app.log" />
</config>`)

const readonlySample = ref(`SELECT s.id,
       s.host_name,
       COUNT(l.id) AS conn_count
  FROM ssh_server s
  LEFT JOIN conn_log l ON l.server_id = s.id
 WHERE s.enabled = 1
 GROUP BY s.id, s.host_name
 ORDER BY conn_count DESC
 LIMIT 20;`)

/** 语言自动识别矩阵：不传 language，仅凭 filename 识别 */
const detectSamples = [
  {
    filename: 'Dockerfile',
    hint: 'Dockerfile → 容器指令高亮',
    content: `FROM node:20-alpine
WORKDIR /app
COPY package.json ./
RUN pnpm install --frozen-lockfile
EXPOSE 3000
CMD ["node", "server.js"]`,
  },
  {
    filename: 'deploy.sh',
    hint: 'deploy.sh → Shell 高亮',
    content: `#!/usr/bin/env bash
set -euo pipefail
HOST="\${1:-10.0.0.12}"
echo "deploying to $HOST"
ssh "root@$HOST" 'systemctl restart api'`,
  },
  {
    filename: 'values.yaml',
    hint: 'values.yaml → YAML 高亮',
    content: `replicaCount: 2
image:
  repository: registry.local/api
  tag: "1.4.2"
resources:
  limits: { cpu: "500m", memory: 512Mi }`,
  },
  {
    filename: '/etc/hosts',
    hint: '/etc/hosts → 纯文本（无高亮）',
    content: `127.0.0.1   localhost
::1         localhost
10.0.0.12   api.internal
10.0.0.13   db.internal`,
  },
]

/** 生成约 560KB 文本（8000 行 × 约 70 字符） */
function buildLargeSample(): string {
  const lines: string[] = []
  for (let i = 1; i <= 8000; i += 1) {
    lines.push(`{"seq": ${i}, "tool": "CoveKit", "event": "sync", "size": ${i * 37}, "ok": true},`)
  }
  return lines.join('\n')
}

/** 格式化：成功与失败都有可见反馈（失败走 error 事件语义，这里直接提示） */
function onFormat(): void {
  const ok = fullEditor.value?.format() ?? false
  ui.toast(ok ? '已格式化（可 Ctrl+Z 撤回）' : '格式化失败：内容不是合法 JSON')
}
</script>

<template>
  <div class="flex flex-col gap-md">
    <UiPanel
      title="代码编辑器"
      description="完整档：语法高亮、行号、当前行、括号匹配、折叠、多光标、缩进参考线、状态栏；语言包按需加载，明暗随应用主题。"
    >
      <template #actions>
        <UiSelect v-model="language" :options="languageOptions" size="sm" />
        <UiButton size="sm" @click="onFormat">格式化</UiButton>
      </template>

      <UiCodeEditor
        ref="fullEditor"
        v-model="fullSample"
        :language="language"
        filename="config.json"
        status-bar
        height="240px"
        @cursor="cursor = $event"
      />

      <p class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark">
        光标：第 {{ cursor.line }} 行 · 第 {{ cursor.column }} 列 · 选中 {{ cursor.selected }} 字符
        （切换语言或只读走 Compartment 热重配置，不重建实例）
      </p>
    </UiPanel>

    <UiPanel
      title="查找替换 / 跳转行"
      description="Ctrl+F 查找、Ctrl+H 替换、Ctrl+G 跳转行、Ctrl+S 触发 save 事件、Esc 关闭浮层。面板统计与跳转同源（正则 / 区分大小写 / 全词），在编辑器内右键会带出行号与行文本。"
    >
      <UiCodeEditor
        v-model="searchSample"
        filename="query.sql"
        status-bar
        height="220px"
        @contextmenu="lastMenu = $event"
      />
      <p class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark">
        <template v-if="lastMenu">
          右键：第 {{ lastMenu.line }} 行 · {{ lastMenu.lineText.trim().slice(0, 56) }}
        </template>
        <template v-else>在编辑器内右键可查看行号与行文本（contextmenu 事件）</template>
      </p>
    </UiPanel>

    <UiPanel
      title="语法校验"
      description="JSON 用官方解析器、XML/SVG 用 DOMParser、SQL 做括号与引号配对检查；错误以波浪线 + 中文提示呈现，位置精确到行列。"
    >
      <template #actions>
        <UiBadge tone="neutral" size="xs">含错误示例</UiBadge>
      </template>
      <UiCodeEditor v-model="brokenJson" language="json" status-bar height="180px" />
    </UiPanel>

    <UiPanel
      title="大文件降级"
      description="超过 512KB 关闭语法高亮、折叠与补全；超过 5MB 强制只读并提示。下面示例约 560KB，状态栏右侧会显示降级提示。"
    >
      <template #actions>
        <UiBadge tone="neutral" size="xs">约 560KB</UiBadge>
      </template>
      <UiCodeEditor
        v-model="largeSample"
        filename="bulk.json"
        status-bar
        height="180px"
        @error="ui.toast($event)"
      />
    </UiPanel>

    <UiPanel
      title="轻量输入区（minimal）"
      description="替代原「行号 + textarea」场景：保留行号、缩进、撤销重做与基础键位，不装折叠 / 括号闭合 / 列选择，降低实例开销。"
    >
      <UiCodeEditor v-model="minimalSample" mode="minimal" filename="config.xml" height="180px" />
    </UiPanel>

    <UiPanel
      title="只读查看器"
      description="readonly 即查看器（取代原 CodeViewer）：可选性保留、行号与折叠可用、不可编辑，查找仍可用。"
    >
      <template #actions>
        <UiBadge tone="neutral" size="xs">只读</UiBadge>
      </template>
      <UiCodeEditor
        v-model="readonlySample"
        readonly
        filename="query.sql"
        :fold-gutter="false"
        height="200px"
      />
    </UiPanel>

    <UiPanel
      title="语言自动识别"
      description="不传 language，仅凭 filename 判定；未识别的扩展名回退纯文本，不高亮也不报错。"
    >
      <div class="grid grid-cols-2 gap-sm">
        <div v-for="sample in detectSamples" :key="sample.filename" class="flex flex-col gap-xs">
          <span class="text-label-caps text-text-muted dark:text-text-muted-dark">
            {{ sample.hint }}
          </span>
          <UiCodeEditor
            v-model="sample.content"
            mode="minimal"
            :filename="sample.filename"
            :line-numbers="false"
            height="150px"
          />
        </div>
      </div>
    </UiPanel>
    <UiPanel
      title="代码差异（L3）"
      description="基于 @codemirror/merge：左右对照与内联两种形态，顶部给出新增/删除行数。SSH 远程文件保存冲突时用同一组件对比「远端当前」与「本地编辑」。"
    >
      <UiCodeDiff
        :original="diffOriginal"
        :modified="diffModified"
        filename="config.json"
        height="200px"
      />
    </UiPanel>
  </div>
</template>
