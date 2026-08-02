<script setup lang="ts">
/**
 * SQLite 数据库 · 打开连接 + 表浏览 + SQL 执行 + 结果表格
 */
import { computed, ref } from "vue";
import type { DbQueryResult } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import { useUiStore } from "@/stores/ui";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";
import { describeResult, displayCell, fileName } from "./useSqlite";

const ui = useUiStore();

const dbPath = ref("");
const connected = ref(false);
const tables = ref<string[]>([]);
const activeTable = ref("");
const sql = ref("SELECT * FROM users LIMIT 100;");
const result = ref<DbQueryResult | null>(null);
const busy = ref(false);
const status = ref("");

const dbName = computed(() => fileName(dbPath.value));

async function openDb() {
  if (!dbPath.value.trim()) {
    ui.toast("请输入数据库文件路径");
    return;
  }
  busy.value = true;
  status.value = "";
  try {
    const r = await ipc.dbOpen(dbPath.value.trim());
    if (!r.ok) {
      ui.toast(r.error ?? "打开失败");
      return;
    }
    connected.value = true;
    tables.value = r.tables;
    activeTable.value = "";
    result.value = null;
    sql.value = tables.value.length ? `SELECT * FROM ${tables.value[0]} LIMIT 100;` : "";
    status.value = `已连接 ${dbName.value}（${r.tables.length} 张表）`;
  } catch (e) {
    ui.toast("打开失败：" + (e instanceof Error ? e.message : String(e)));
  } finally {
    busy.value = false;
  }
}

async function closeDb() {
  await ipc.dbClose().catch(() => {});
  connected.value = false;
  tables.value = [];
  activeTable.value = "";
  result.value = null;
  status.value = "";
}

async function refreshTables() {
  try {
    tables.value = await ipc.dbTables();
  } catch (e) {
    ui.toast("刷新失败：" + (e instanceof Error ? e.message : String(e)));
  }
}

async function browseTable(t: string) {
  activeTable.value = t;
  busy.value = true;
  try {
    result.value = await ipc.dbQueryTable(t, 200);
  } catch (e) {
    ui.toast("查询失败：" + (e instanceof Error ? e.message : String(e)));
  } finally {
    busy.value = false;
  }
}

async function runSql() {
  if (!sql.value.trim()) return;
  busy.value = true;
  try {
    result.value = await ipc.dbExecute(sql.value);
    if (!result.value.ok) {
      ui.toast(result.value.error ?? "执行失败");
    }
  } catch (e) {
    ui.toast("执行失败：" + (e instanceof Error ? e.message : String(e)));
  } finally {
    busy.value = false;
  }
}

function formatCell(v: string): string {
  const d = displayCell(v);
  return d.length > 200 ? `${d.slice(0, 200)}…` : d;
}
</script>

<template>
  <div class="flex max-w-[1100px] flex-col gap-[12px]">
    <!-- 固定连接栏（滚动时始终可见） -->
    <div class="sticky-toolbar">
      <input
        v-model="dbPath"
        class="field-input min-w-[280px] flex-1 font-mono"
        placeholder="数据库文件路径，如 C:\data\app.db"
        spellcheck="false"
        :disabled="connected"
        @keyup.enter="openDb"
      />
      <button v-if="!connected" class="btn-primary shrink-0" :disabled="busy" @click="openDb">
        {{ busy ? "打开中…" : "打开 / 新建" }}
      </button>
      <button v-else class="btn-secondary shrink-0" @click="closeDb">断开</button>
      <span class="truncate text-body-sm text-text-muted dark:text-text-muted-dark">{{
        status
      }}</span>
    </div>

    <template v-if="connected">
      <div class="flex min-h-[380px] gap-[12px]">
        <!-- 表列表 -->
        <aside class="w-[200px] shrink-0">
          <div class="mb-[6px] flex items-center justify-between">
            <label class="field-label">表（{{ tables.length }}）</label>
            <button class="btn-ghost" title="刷新" @click="refreshTables">刷新</button>
          </div>
          <div class="flex max-h-[420px] flex-col gap-[4px] overflow-y-auto pr-[4px]">
            <button
              v-for="t in tables"
              :key="t"
              class="rounded-md px-[10px] py-[7px] text-left text-body transition-colors"
              :class="
                activeTable === t
                  ? 'bg-tertiary-soft font-medium text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
                  : 'text-secondary hover:bg-neutral hover:text-primary dark:text-secondary-dark dark:hover:bg-neutral-dark dark:hover:text-primary-dark'
              "
              @click="browseTable(t)"
            >
              <span class="truncate font-mono">{{ t }}</span>
            </button>
            <p
              v-if="!tables.length"
              class="px-[10px] py-[8px] text-body-sm text-text-muted dark:text-text-muted-dark"
            >
              暂无表
            </p>
          </div>
        </aside>

        <!-- 主区：SQL + 结果 -->
        <div class="flex min-w-0 flex-1 flex-col gap-[10px]">
          <div>
            <label class="mb-[6px] field-label">SQL</label>
            <div class="flex items-start gap-[8px]">
              <LineNumberTextarea v-model="sql" min-height="120px" class="flex-1" />
              <div class="flex flex-col gap-[6px]">
                <button class="btn-primary shrink-0" :disabled="busy" @click="runSql">执行</button>
                <button class="btn-ghost shrink-0" @click="sql = ''">清空</button>
              </div>
            </div>
          </div>

          <div class="flex items-center justify-between">
            <label class="field-label">结果</label>
            <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
              {{
                result
                  ? describeResult(
                      result.isQuery,
                      result.isQuery ? result.rows.length : result.rowsAffected
                    )
                  : ""
              }}
            </span>
          </div>

          <!-- 结果表格 -->
          <div
            class="min-h-[160px] flex-1 overflow-auto rounded-md border border-border dark:border-border-dark"
          >
            <table
              v-if="result && result.ok && result.columns.length"
              class="w-full border-collapse text-left"
            >
              <thead class="sticky top-0 bg-surface-muted dark:bg-surface-muted-dark">
                <tr>
                  <th
                    v-for="c in result.columns"
                    :key="c"
                    class="border-b border-border px-[10px] py-[7px] text-body-sm font-medium text-secondary dark:border-border-dark dark:text-secondary-dark"
                  >
                    {{ c }}
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(row, ri) in result.rows"
                  :key="ri"
                  class="odd:bg-surface-muted/40 dark:odd:bg-surface-muted-dark/40"
                >
                  <td
                    v-for="(cell, ci) in row"
                    :key="ci"
                    class="max-w-[280px] truncate border-b border-border/60 px-[10px] py-[6px] font-mono text-body-sm text-primary dark:border-border-dark/60 dark:text-primary-dark"
                    :title="cell"
                  >
                    <span
                      :class="
                        cell === 'NULL' ? 'italic text-text-muted dark:text-text-muted-dark' : ''
                      "
                    >
                      {{ formatCell(cell) }}
                    </span>
                  </td>
                </tr>
                <tr v-if="!result.rows.length">
                  <td
                    :colspan="result.columns.length"
                    class="px-[10px] py-[14px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
                  >
                    0 行
                  </td>
                </tr>
              </tbody>
            </table>
            <p
              v-else-if="result && !result.ok"
              class="px-[12px] py-[14px] font-mono text-body-sm text-tertiary-strong dark:text-tertiary-dark"
            >
              {{ result.error }}
            </p>
            <p
              v-else-if="result && !result.isQuery"
              class="px-[12px] py-[14px] text-body-sm text-text-muted dark:text-text-muted-dark"
            >
              执行成功，影响 {{ result.rowsAffected }} 行
            </p>
            <p
              v-else
              class="px-[12px] py-[14px] text-body-sm text-text-muted dark:text-text-muted-dark"
            >
              执行 SQL 查看结果（SELECT / PRAGMA / DML）
            </p>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
