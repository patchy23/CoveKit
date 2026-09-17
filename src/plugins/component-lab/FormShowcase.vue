<script setup lang="ts">
import { reactive, ref } from 'vue'
import {
  UiCheckbox,
  UiField,
  UiInput,
  UiPanel,
  UiRadioGroup,
  UiSearchInput,
  UiSelect,
  UiSwitch,
  UiTextarea,
} from '@/core/ui'
import type { UiSize } from '@/core/ui'

const sizes: UiSize[] = ['xs', 'sm', 'md', 'lg']
const inputs = reactive<Record<UiSize, string>>({
  xs: '紧凑输入',
  sm: '小号输入',
  md: '标准输入',
  lg: '大号输入',
})
const selects = reactive<Record<UiSize, string>>({
  xs: 'sqlite',
  sm: 'mysql',
  md: 'postgres',
  lg: 'postgres',
})
const checks = reactive<Record<UiSize, boolean>>({ xs: true, sm: true, md: false, lg: true })
const switches = reactive<Record<UiSize, boolean>>({ xs: true, sm: false, md: true, lg: false })
const search = ref('component')
const textarea = ref('多行输入适合脚本、配置、请求体和备注。')
const radio = ref('system')
const filter = ref('sqlite')
const advancedName = ref('折叠后保留输入')

const options = [
  { value: 'sqlite', label: 'SQLite' },
  { value: 'mysql', label: 'MySQL' },
  { value: 'postgres', label: 'PostgreSQL' },
  { value: 'disabled', label: '不可用选项', disabled: true },
]
const themeOptions = [
  { value: 'system', label: '跟随系统', description: '自动匹配操作系统外观。' },
  { value: 'light', label: '浅色', description: '始终使用明净浅色主题。' },
  { value: 'dark', label: '深色', description: '始终使用深色主题。' },
]
</script>

<template>
  <UiPanel
    title="输入与选择"
    description="所有基础输入统一支持 xs / sm / md / lg，尺寸可按页面密度选择。"
  >
    <div class="grid gap-md lg:grid-cols-2">
      <div class="flex flex-col gap-sm">
        <UiField v-for="size in sizes" :key="size" :label="`输入框 ${size}`" :size="size">
          <UiInput v-model="inputs[size]" :size="size" />
        </UiField>
      </div>
      <div class="flex flex-col gap-sm">
        <UiField v-for="size in sizes" :key="size" :label="`选择器 ${size}`" :size="size">
          <UiSelect v-model="selects[size]" :size="size" :options="options" />
        </UiField>
      </div>
    </div>
    <div class="mt-md grid gap-md lg:grid-cols-2">
      <UiField label="搜索输入"><UiSearchInput v-model="search" clearable /></UiField>
      <UiField label="错误状态" error="当前输入不符合格式要求">
        <UiInput model-value="invalid value" invalid />
      </UiField>
    </div>
    <UiField label="多行输入" class="mt-md">
      <UiTextarea v-model="textarea" size="sm" rows="3" />
    </UiField>
  </UiPanel>

  <div class="grid gap-md lg:grid-cols-2">
    <UiPanel title="复选框与开关" description="适合筛选项、功能开关和批量选择。">
      <div class="grid grid-cols-2 gap-md">
        <div class="flex flex-col gap-md">
          <UiCheckbox
            v-for="size in sizes"
            :key="size"
            v-model="checks[size]"
            :size="size"
            :label="`复选框 ${size}`"
          />
          <UiCheckbox
            :model-value="false"
            indeterminate
            label="部分选中"
            description="用于树形与批量选择。"
          />
        </div>
        <div class="flex flex-col gap-md">
          <UiSwitch
            v-for="size in sizes"
            :key="size"
            v-model="switches[size]"
            :size="size"
            :label="`开关 ${size}`"
          />
          <UiSwitch :model-value="false" disabled label="禁用状态" />
        </div>
      </div>
    </UiPanel>

    <UiPanel title="单选组" description="用于少量互斥选项；较多选项使用 UiSelect。">
      <UiRadioGroup v-model="radio" name="theme-preview" :options="themeOptions" />
    </UiPanel>
  </div>

  <UiPanel title="互斥筛选" description="筛选结果使用单选语义；方向键切换，禁用项跳过。">
    <div class="flex flex-col gap-md">
      <UiRadioGroup
        v-for="size in sizes"
        :key="size"
        v-model="filter"
        :name="`filter-${size}`"
        variant="chips"
        :size="size"
        :options="options"
        :aria-label="`数据库筛选 ${size}`"
      />
    </div>
  </UiPanel>

  <UiPanel title="更多配置" description="支持 Enter 与空格折叠，内容状态保留。" collapsible>
    <UiField label="配置名称"><UiInput v-model="advancedName" /></UiField>
  </UiPanel>
</template>
