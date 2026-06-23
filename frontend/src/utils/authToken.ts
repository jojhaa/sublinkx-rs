const TOKEN_KEY = 'sublinkx_rs_token'
const CSRF_KEY = 'sublinkx_rs_csrf'

function storageAvailable(storage: Storage | undefined): storage is Storage {
  return typeof storage !== 'undefined'
}

export function readAuthToken(): string {
  if (typeof window === 'undefined') {
    return ''
  }

  const sessionToken = storageAvailable(window.sessionStorage)
    ? window.sessionStorage.getItem(TOKEN_KEY)
    : null
  if (sessionToken) {
    return sessionToken
  }

  const legacyToken = storageAvailable(window.localStorage)
    ? window.localStorage.getItem(TOKEN_KEY)
    : null
  if (legacyToken) {
    writeAuthToken(legacyToken)
    window.localStorage.removeItem(TOKEN_KEY)
    return legacyToken
  }

  return ''
}

export function readCsrfToken(): string {
  if (typeof window === 'undefined' || !storageAvailable(window.sessionStorage)) {
    return ''
  }

  return window.sessionStorage.getItem(CSRF_KEY) ?? ''
}

export function writeCsrfToken(token: string) {
  if (typeof window === 'undefined' || !storageAvailable(window.sessionStorage)) {
    return
  }

  window.sessionStorage.setItem(CSRF_KEY, token)
}

export function writeAuthToken(token: string) {
  if (typeof window === 'undefined') {
    return
  }

  if (storageAvailable(window.sessionStorage)) {
    window.sessionStorage.setItem(TOKEN_KEY, token)
  }
  if (storageAvailable(window.localStorage)) {
    window.localStorage.removeItem(TOKEN_KEY)
  }
}

export function clearAuthToken() {
  if (typeof window === 'undefined') {
    return
  }

  if (storageAvailable(window.sessionStorage)) {
    window.sessionStorage.removeItem(TOKEN_KEY)
    window.sessionStorage.removeItem(CSRF_KEY)
  }
  if (storageAvailable(window.localStorage)) {
    window.localStorage.removeItem(TOKEN_KEY)
  }
}
