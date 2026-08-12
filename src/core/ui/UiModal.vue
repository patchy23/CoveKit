<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'

withDefaults(
  defineProps<{
    open: boolean
    title?: string
    description?: string
    width?: string
    closeOnBackdrop?: boolean
  }>(),
  { title: '', description: '', width: 'min(560px, 92vw)', closeOnBackdrop: true }
)

const emit = defineEmits<{ close: [] }>()

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') emit('close')
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <Teleport to="body">
    <Transition name="ui-modal">
      <div
        v-if="open"
        class="fixed inset-0 z-[180] grid place-items-center bg-[rgba(16,20,28,0.45)] p-md backdrop-blur-[3px]"
        @mousedown.self="closeOnBackdrop && emit('close')"
      >
        <section class="ui-modal-panel" :style="{ width }" role="dialog" aria-modal="true">
          <header v-if="title || description || $slots.header" class="mb-[18px]">
            <slot name="header">
              <h2 class="text-card-title font-semibold text-primary dark:text-primary-dark">
                {{ title }}
              </h2>
              <p
                v-if="description"
                class="mt-xs text-body-sm text-secondary dark:text-secondary-dark"
              >
                {{ description }}
              </p>
            </slot>
          </header>
          <slot />
          <footer v-if="$slots.footer" class="mt-[20px] flex justify-end gap-sm">
            <slot name="footer" />
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.ui-modal-enter-active,
.ui-modal-leave-active {
  transition: opacity 0.18s ease;
}
.ui-modal-enter-active .ui-modal-panel,
.ui-modal-leave-active .ui-modal-panel {
  transition: transform 0.18s ease;
}
.ui-modal-enter-from,
.ui-modal-leave-to {
  opacity: 0;
}
.ui-modal-enter-from .ui-modal-panel,
.ui-modal-leave-to .ui-modal-panel {
  transform: translateY(10px) scale(0.98);
}
</style>
