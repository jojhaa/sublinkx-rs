import apiClient from './client'

export interface AppSettings {
  public_base_url: string
  latency_auto_enabled: boolean
  latency_interval_minutes: number
  latency_concurrency: number
  latency_core_path: string
  latency_test_url: string
  latency_timeout_secs: number
  ip_probe_auto_enabled: boolean
  ip_probe_interval_minutes: number
  ip_probe_after_upstream_import: boolean
  country_detection_auto_enabled: boolean
  country_detection_interval_minutes: number
  risk_enforcement_enabled: boolean
  connectivity_default_target: 'system_default' | 'cloudflare_204' | 'google_204'
  connectivity_default_rounds: 1 | 3 | 5
  connectivity_sync_last_latency: boolean
  public_export_cache_ttl_seconds: number
  public_export_ip_limit_per_minute: number
  public_export_global_limit_per_minute: number
  mihomo_country_load_min_nodes: number
  mihomo_country_fallback_min_nodes: number
}

interface SettingsResponse {
  code: string
  data: AppSettings
}

export interface MihomoCoreStatus {
  os: string
  arch: string
  supported: boolean
  installed: boolean
  path: string | null
  version: string | null
  message: string
}

export interface MihomoCoreDownloadResult {
  os: string
  arch: string
  version: string
  asset_name: string
  path: string
  size: number
}

interface MihomoCoreStatusResponse {
  code: string
  data: MihomoCoreStatus
}

interface MihomoCoreDownloadResponse {
  code: string
  data: MihomoCoreDownloadResult
}

export async function getSettings() {
  const { data } = await apiClient.get<SettingsResponse>('/api/v1/settings')
  return data
}

export async function updateSettings(payload: AppSettings) {
  const { data } = await apiClient.put<SettingsResponse>('/api/v1/settings', payload)
  return data
}

export async function getMihomoCoreStatus() {
  const { data } = await apiClient.get<MihomoCoreStatusResponse>('/api/v1/settings/mihomo-core')
  return data
}

export async function downloadMihomoCore() {
  const { data } = await apiClient.post<MihomoCoreDownloadResponse>('/api/v1/settings/mihomo-core/download')
  return data
}
