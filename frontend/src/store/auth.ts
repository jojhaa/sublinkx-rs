import { defineStore } from 'pinia'
import { loginRequest, fetchMeRequest, type LoginPayload } from '../api/auth'

export interface AuthUser {
  user_id: number
  username: string
  nickname: string
  role: string
  status: string
}

const TOKEN_KEY = 'sublinkx_rs_token'

export const useAuthStore = defineStore('auth', {
  state: () => ({
    token: localStorage.getItem(TOKEN_KEY) ?? '',
    user: null as AuthUser | null,
  }),
  actions: {
    async login(payload: LoginPayload) {
      const response = await loginRequest(payload)
      this.token = response.data.access_token
      localStorage.setItem(TOKEN_KEY, this.token)
      await this.fetchMe()
    },
    async fetchMe() {
      const response = await fetchMeRequest()
      this.user = response.data
      return response.data
    },
    clearAuth() {
      this.token = ''
      this.user = null
      localStorage.removeItem(TOKEN_KEY)
    },
  },
})

export { TOKEN_KEY }
