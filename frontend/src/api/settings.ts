import apiClient from './client'

export interface AppSettings {
  latency_auto_enabled: boolean
  latency_interval_minutes: number
  latency_core_path: string
  latency_test_url: string
  latency_timeout_secs: number
}

interface SettingsResponse {
  code: string
  data: AppSettings
}

export async function getSettings() {
  const { data } = await apiClient.get<SettingsResponse>('/api/v1/settings')
  return data
}

export async function updateSettings(payload: AppSettings) {
  const { data } = await apiClient.put<SettingsResponse>('/api/v1/settings', payload)
  return data
}
