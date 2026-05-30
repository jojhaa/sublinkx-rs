const DEFAULT_PAGE_SIZE = 20

export function readStoredPageSize(key: string, options: number[], fallback = DEFAULT_PAGE_SIZE) {
  const stored = Number(window.localStorage.getItem(key))
  return options.includes(stored) ? stored : fallback
}

export function storePageSize(key: string, value: number) {
  window.localStorage.setItem(key, String(value))
}
