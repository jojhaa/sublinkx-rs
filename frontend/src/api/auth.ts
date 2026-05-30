import apiClient from './client'
import type { AuthUser } from '../store/auth'

export interface LoginPayload {
  username: string
  password: string
}

interface LoginApiResponse {
  code: string
  data: {
    access_token: string
    token_type: string
    expires_in_hours: number
  }
}

interface MeApiResponse {
  code: string
  data: AuthUser
}

export async function loginRequest(payload: LoginPayload) {
  const { data } = await apiClient.post<LoginApiResponse>('/api/v1/auth/login', payload)
  return data
}

export async function fetchMeRequest() {
  const { data } = await apiClient.get<MeApiResponse>('/api/v1/auth/me')
  return data
}
