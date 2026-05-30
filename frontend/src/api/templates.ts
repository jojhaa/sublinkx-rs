import apiClient from './client'

export interface TemplateItem {
  id: number
  name: string
  kind: string
  content: string
  created_at: string
  updated_at: string
}

interface TemplateListResponse {
  code: string
  data: TemplateItem[]
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

export async function listTemplates() {
  const { data } = await apiClient.get<TemplateListResponse>('/api/v1/templates')
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
