import apiClient, { type PaginationMeta } from './client'

export interface TemplateItem {
  id: number
  name: string
  kind: string
  content: string
  is_builtin: boolean
  created_at: string
  updated_at: string
}

interface TemplateListResponse {
  code: string
  data: TemplateItem[]
  pagination: PaginationMeta
  kind_counts: Array<{ kind: string; count: number }>
}

export interface TemplateListParams {
  page?: number
  page_size?: number
  kind?: string
}

interface TemplateResponse {
  code: string
  data: TemplateItem
}

export interface TemplatePayload {
  name: string
  kind: string
  content: string
}

export async function listTemplates(params: TemplateListParams = {}) {
  const { data } = await apiClient.get<TemplateListResponse>('/api/v1/templates', { params })
  return data
}

export async function createTemplate(payload: TemplatePayload) {
  const { data } = await apiClient.post<TemplateResponse>('/api/v1/templates', payload)
  return data
}

export async function updateTemplate(id: number, payload: TemplatePayload) {
  const { data } = await apiClient.put<TemplateResponse>(`/api/v1/templates/${id}`, payload)
  return data
}

export async function deleteTemplate(id: number) {
  const { data } = await apiClient.delete<{ code: string; message: string }>(`/api/v1/templates/${id}`)
  return data
}
