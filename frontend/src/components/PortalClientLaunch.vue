<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import type { ClientLaunchOption } from '../utils/portalClientLaunch'

defineProps<{
  options: ClientLaunchOption[]
  hint: string
  copied: boolean
}>()
const emit = defineEmits<{ copy: [] }>()
const dialog = ref<HTMLDialogElement | null>(null)
const attempted = ref(false)
onBeforeUnmount(() => dialog.value?.close())

function markAttempt() {
  attempted.value = true
  dialog.value?.close()
}
</script>

<template>
  <div class="portal-launch-control">
    <a v-if="options.length === 1" class="button button-accent portal-primary-action"
      :href="options[0]!.href" @click="markAttempt">打开应用</a>
    <button v-else class="button button-accent portal-primary-action" type="button" @click="dialog?.showModal()">
      {{ options.length ? '选择应用' : '导入说明' }}
    </button>
    <p v-if="attempted" class="portal-launch-feedback" role="status">
      请在系统提示中允许打开应用，并在应用内确认导入。若没有打开，请检查应用是否已安装，或复制链接到应用中添加订阅。
    </p>
    <Teleport to="body">
    <dialog ref="dialog" class="portal-launch-dialog" aria-label="订阅导入" @click="event => { if (event.target === dialog) dialog?.close() }">
      <header class="modal-header">
        <h3>{{ options.length ? '选择已安装的客户端' : '手动导入订阅' }}</h3>
        <button class="button button-ghost" type="button" @click="dialog?.close()">关闭</button>
      </header>
      <p>{{ hint }}</p>
      <p v-if="options.length">浏览器无法可靠检测已安装的应用。请选择已安装的客户端，并允许系统打开它。</p>
      <div class="portal-launch-options">
        <a v-for="option in options" :key="option.href" class="button button-accent"
          :href="option.href" @click="markAttempt">打开 {{ option.label }}</a>
      </div>
      <button class="button button-ghost" type="button" @click="emit('copy')">{{ copied ? '已复制' : '复制订阅链接' }}</button>
    </dialog>
    </Teleport>
  </div>
</template>

<style scoped>
.portal-launch-control { display: contents; }
.portal-launch-feedback { order: 2; flex-basis: 100%; grid-column: 1 / -1; font-size: .8rem; line-height: 1.6; }
.portal-launch-dialog { width: min(32rem, calc(100vw - 2rem)); box-sizing: border-box; max-height: 85dvh; overflow: auto; border: 1px solid #c8dfe2; border-radius: 20px; padding: 24px; color: #17333b; background: #f8fcfd; }
.portal-launch-dialog::backdrop { background: #102f3b80; }
.portal-launch-dialog p { line-height: 1.7; }
.portal-launch-options { display: grid; gap: 10px; margin: 20px 0; }
:global([data-theme="dark"] .portal-launch-dialog) { color: #e4f2f4; background: #17333b; border-color: #39616b; }
</style>
