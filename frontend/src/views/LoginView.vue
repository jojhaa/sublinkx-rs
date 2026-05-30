<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { extractApiError } from '../api/client'
import { useAuthStore } from '../store/auth'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()

const form = reactive({
  username: 'admin',
  password: 'change-me-now',
})

const loading = ref(false)
const errorMessage = ref('')

async function submit() {
  loading.value = true
  errorMessage.value = ''

  try {
    await auth.login(form)
    const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : '/'
    await router.replace(redirect)
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="auth-page">
    <section class="auth-card">
      <span class="eyebrow">Secure Console</span>
      <h1 class="hero-title">登录控制台</h1>
      <p class="hero-copy">
        使用管理员账号进入多协议订阅工作台。登录后可以管理节点、订阅、模板和客户端导出。
      </p>

      <form class="form-grid" @submit.prevent="submit">
        <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>

        <div>
          <label class="field-label" for="username">用户名</label>
          <input id="username" v-model.trim="form.username" class="input" autocomplete="username" />
        </div>

        <div>
          <label class="field-label" for="password">密码</label>
          <input
            id="password"
            v-model="form.password"
            class="input"
            type="password"
            autocomplete="current-password"
          />
        </div>

        <button class="button button-accent" type="submit" :disabled="loading">
          {{ loading ? '正在登录...' : '进入后台' }}
        </button>

        <div class="hint">
          默认账号来自后端环境变量 <code>BOOTSTRAP_ADMIN_USERNAME</code> 和
          <code>BOOTSTRAP_ADMIN_PASSWORD</code>。
        </div>
      </form>
    </section>
  </div>
</template>
