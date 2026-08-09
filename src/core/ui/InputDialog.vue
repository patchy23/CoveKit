<script setup lang="ts">
/** InputDialog · 基于 BaseModal 的项目统一单行输入弹窗。 */
import { nextTick, ref, watch } from "vue";
import BaseModal from "@/features/ui/BaseModal.vue";

const props = defineProps<{
  open: boolean;
  title: string;
  label: string;
  initialValue?: string;
  confirmLabel?: string;
}>();

const emit = defineEmits<{
  (event: "confirm", value: string): void;
  (event: "close"): void;
}>();

const value = ref("");
const input = ref<HTMLInputElement | null>(null);

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    value.value = props.initialValue ?? "";
    await nextTick();
    input.value?.focus();
    input.value?.select();
  },
  { immediate: true }
);

function submit() {
  const trimmed = value.value.trim();
  if (trimmed) emit("confirm", trimmed);
}
</script>

<template>
  <BaseModal :open="open" width="min(420px, 92vw)" @close="emit('close')">
    <h3 class="mb-[14px] text-card-title font-medium text-primary dark:text-primary-dark">
      {{ title }}
    </h3>
    <label class="field-label" for="action-dialog-input">{{ label }}</label>
    <input
      id="action-dialog-input"
      ref="input"
      v-model="value"
      class="field-input mb-[20px]"
      spellcheck="false"
      @keyup.enter="submit"
    />
    <div class="flex justify-end gap-[8px]">
      <button class="btn-ghost" @click="emit('close')">取消</button>
      <button class="btn-primary" :disabled="!value.trim()" @click="submit">
        {{ confirmLabel ?? "确定" }}
      </button>
    </div>
  </BaseModal>
</template>
