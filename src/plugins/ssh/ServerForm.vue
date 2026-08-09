<script setup lang="ts">
/**
 * ServerForm · 添加/编辑服务器弹窗
 * 表单：名称 / host / port / 用户名 / 认证方式 / 密码/密钥 / 备注
 */
import { reactive, watch } from "vue";
import type { ServerProfile, AuthMethod } from "./contracts";
import Select from "@/features/ui/Select.vue";

const props = defineProps<{
  profile: ServerProfile | null;
}>();

const emit = defineEmits<{
  (
    e: "save",
    p: ServerProfile,
    creds: { password?: string; privateKey?: string; passphrase?: string }
  ): void;
  (e: "cancel"): void;
  (e: "error", msg: string): void;
}>();

const form = reactive({
  id: "",
  name: "",
  host: "",
  port: 22,
  username: "",
  authMethod: "password" as AuthMethod,
  password: "",
  privateKey: "",
  passphrase: "",
  remark: "",
});

watch(
  () => props.profile,
  (p) => {
    // 凭证永不回填到表单；每次打开都清空，避免上一次输入泄漏到另一台服务器。
    form.password = "";
    form.privateKey = "";
    form.passphrase = "";
    if (p) {
      form.id = p.id;
      form.name = p.name;
      form.host = p.host;
      form.port = p.port;
      form.username = p.username;
      form.authMethod = p.authMethod;
      form.remark = p.remark ?? "";
    } else {
      form.id = "";
      form.name = "";
      form.host = "";
      form.port = 22;
      form.username = "";
      form.authMethod = "password";
      form.remark = "";
    }
  },
  { immediate: true }
);

function submit() {
  // 必填校验：名称/主机/用户名任一为空则提示并中止（UI-014）
  if (!form.name.trim()) {
    emit("error", "请输入服务器名称");
    return;
  }
  if (!form.host.trim()) {
    emit("error", "请输入主机地址");
    return;
  }
  if (!form.username.trim()) {
    emit("error", "请输入登录用户名");
    return;
  }
  if (!Number.isInteger(form.port) || form.port < 1 || form.port > 65535) {
    emit("error", "端口必须是 1 到 65535 之间的整数");
    return;
  }
  const p: ServerProfile = {
    id: form.id || `profile-${Date.now()}`,
    name: form.name.trim(),
    host: form.host.trim(),
    port: form.port,
    username: form.username.trim(),
    authMethod: form.authMethod,
    remark: form.remark.trim() || undefined,
    lastConnectedAt: props.profile?.lastConnectedAt,
  };
  emit("save", p, {
    password: form.password.trim() || undefined,
    privateKey: form.privateKey.trim() || undefined,
    passphrase: form.passphrase.trim() || undefined,
  });
}
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-[150] grid place-items-center bg-black/30"
      @click.self="emit('cancel')"
    >
      <div
        class="w-[420px] rounded-lg border border-border bg-surface p-[18px] shadow-[0_16px_48px_rgba(16,24,40,0.25)] dark:border-border-dark dark:bg-surface-dark"
      >
        <h3 class="mb-[14px] text-card-title font-medium text-primary dark:text-primary-dark">
          {{ props.profile ? "编辑服务器" : "添加服务器" }}
        </h3>

        <div class="space-y-[10px]">
          <div>
            <label class="field-label mb-[4px]">名称</label>
            <input v-model="form.name" class="field-input" placeholder="如：生产服务器" />
          </div>

          <div class="grid grid-cols-2 gap-[10px]">
            <div>
              <label class="field-label mb-[4px]">主机</label>
              <input v-model="form.host" class="field-input font-mono" placeholder="192.168.1.1" />
            </div>
            <div>
              <label class="field-label mb-[4px]">端口</label>
              <input v-model.number="form.port" type="number" class="field-input" />
            </div>
          </div>

          <div>
            <label class="field-label mb-[4px]">用户名</label>
            <input v-model="form.username" class="field-input" placeholder="root" />
          </div>

          <div>
            <label class="field-label mb-[4px]">认证方式</label>
            <Select
              :model-value="form.authMethod"
              :options="[
                { value: 'password', label: '密码' },
                { value: 'privateKey', label: '私钥' },
                { value: 'privateKeyWithPassphrase', label: '私钥 + Passphrase' },
              ]"
              @update:model-value="form.authMethod = $event as AuthMethod"
            />
          </div>

          <div v-if="form.authMethod === 'password'">
            <label class="field-label mb-[4px]">密码</label>
            <input v-model="form.password" type="password" class="field-input" />
          </div>

          <div v-if="form.authMethod !== 'password'">
            <label class="field-label mb-[4px]">私钥内容</label>
            <textarea
              v-model="form.privateKey"
              class="field-textarea font-mono text-body-sm"
              rows="4"
              placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
            />
          </div>

          <div v-if="form.authMethod === 'privateKeyWithPassphrase'">
            <label class="field-label mb-[4px]">Passphrase</label>
            <input v-model="form.passphrase" type="password" class="field-input" />
          </div>

          <div>
            <label class="field-label mb-[4px]">备注（可选）</label>
            <input v-model="form.remark" class="field-input" placeholder="用途说明" />
          </div>
        </div>

        <div class="mt-[16px] flex justify-end gap-[8px]">
          <button class="btn-ghost" @click="emit('cancel')">取消</button>
          <button class="btn-primary" @click="submit">保存</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
