import { defineStore } from 'pinia'
import {
  changeCredentialsRequest,
  fetchMeRequest,
  loginRequest,
  type ChangeCredentialsPayload,
  type LoginPayload,
} from '../api/auth'

export interface AuthUser {
  user_id: number
  username: string
  nickname: string
  role: string
  status: string
  must_change_credentials: boolean
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
      this.user = response.data.user
      localStorage.setItem(TOKEN_KEY, this.token)
    },
    async changeCredentials(payload: ChangeCredentialsPayload) {
      const response = await changeCredentialsRequest(payload)
      this.user = response.data
      return response.data
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
