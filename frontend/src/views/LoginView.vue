<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { extractApiError } from '../api/client'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import { useI18n } from '../i18n'
import { useAuthStore } from '../store/auth'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()
const { t } = useI18n()

const form = reactive({
  username: 'admin',
  password: 'admin123456',
})

const loading = ref(false)
const errorMessage = ref('')

async function submit() {
  loading.value = true
  errorMessage.value = ''

  try {
    await auth.login(form)
    if (auth.user?.must_change_credentials) {
      await router.replace('/change-credentials')
      return
    }
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
      <div class="auth-topbar">
        <span class="eyebrow">{{ t('secureConsole') }}</span>
        <LanguageSwitch />
      </div>
      <h1 class="hero-title">{{ t('loginTitle') }}</h1>
      <p class="hero-copy">
        {{ t('loginCopy') }}
      </p>

      <form class="form-grid" @submit.prevent="submit">
        <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>

        <div>
          <label class="field-label" for="username">{{ t('username') }}</label>
          <input id="username" v-model.trim="form.username" class="input" autocomplete="username" />
        </div>

        <div>
          <label class="field-label" for="password">{{ t('password') }}</label>
          <input
            id="password"
            v-model="form.password"
            class="input"
            type="password"
            autocomplete="current-password"
          />
        </div>

        <button class="button button-accent" type="submit" :disabled="loading">
          {{ loading ? t('loggingIn') : t('enterAdmin') }}
        </button>

        <div class="hint">
          {{ t('loginHintDefault') }}
        </div>
      </form>
    </section>
  </div>
</template>
