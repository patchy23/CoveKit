<script setup lang="ts">
/**
 * 编辑器组件展示页
 *
 * 覆盖四档形态：完整编辑器、轻量输入区（minimal）、只读查看器、语言自动识别矩阵。
 * 明暗高亮随应用主题切换（高亮色走 --cm-* 变量，无需重建实例）。
 */
import { ref } from 'vue'
import { UiBadge, UiCodeEditor, UiPanel, UiSelect, type EditorCursorInfo } from '@/core/ui'

/** 主编辑器语言选择：auto 表示按 filename='config.json' 自动识别 */
const language = ref('auto')
const languageOptions = [
  { value: 'auto', label: '自动识别' },
  { value: 'json', label: 'JSON' },
  { value: 'sql', label: 'SQL' },
  { value: 'plaintext', label: '纯文本' },
]

const fullSample = ref(`{
  "tool": "patchyBox",
  "version": "0.9.0",
  "features": ["format", "http", "database", "ssh"],
  "limits": { "maxFileSize": 5242880, "highlight": 524288 },
  "nested": { "enabled": true, "retries": 3, "ratio": 0.75 }
}`)

/** 光标信息（验证 cursor 回调与状态栏数据源） */
const cursor = ref<EditorCursorInfo>({ line: 1, column: 1, selected: 0 })

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
</script>

<template>
  <div class="flex flex-col gap-md">
    <UiPanel
      title="代码编辑器"
      description="完整档：语法高亮、行号、当前行、括号匹配、折叠、多光标、缩进参考线；语言包按需加载，明暗随应用主题。"
    >
      <template #actions>
        <UiSelect v-model="language" :options="languageOptions" size="sm" />
      </template>

      <UiCodeEditor
        v-model="fullSample"
        :language="language"
        filename="config.json"
        height="240px"
        @cursor="cursor = $event"
      />

      <p class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark">
        光标：第 {{ cursor.line }} 行 · 第 {{ cursor.column }} 列 · 选中 {{ cursor.selected }} 字符
        （切换语言或只读走 Compartment 热重配置，不重建实例）
      </p>
    </UiPanel>

    <UiPanel
      title="轻量输入区（minimal）"
      description="替代原「行号 + textarea」场景：保留行号、缩进、撤销重做与基础键位，不装折叠 / 括号闭合 / 列选择，降低实例开销。"
    >
      <UiCodeEditor v-model="minimalSample" mode="minimal" filename="config.xml" height="180px" />
    </UiPanel>

    <UiPanel
      title="只读查看器"
      description="readonly 即查看器（取代原 CodeViewer）：可选性保留、行号与折叠可用、不可编辑。"
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
  </div>
</template>
