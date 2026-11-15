// The one place that talks http. Every module's api file goes through it.

export type ErrorCode =
  | 'not_found'
  | 'conflict'
  | 'invalid'
  | 'unauthenticated'
  | 'forbidden'
  | 'throttled'
  | 'internal'
  | 'unreachable'

export class ApiError extends Error {
  readonly status: number
  readonly code: ErrorCode
  /** for `invalid`: the form field the server names */
  readonly field: string | null

  constructor(status: number, code: ErrorCode, message: string, field: string | null = null) {
    super(message)
    this.status = status
    this.code = code
    this.field = field
  }
}

/** `invalid` messages are `<field>: <reason>`; the two halves are shown apart */
export function splitInvalid(message: string): { field: string | null; reason: string } {
  const at = message.indexOf(': ')
  if (at <= 0 || /\s/.test(message.slice(0, at))) return { field: null, reason: message }
  return { field: message.slice(0, at), reason: message.slice(at + 2) }
}

export function toApiError(status: number, body: unknown): ApiError {
  const error = (body as { error?: { code?: string; message?: string } } | null)?.error
  if (!error?.code || !error.message) {
    return new ApiError(status, 'internal', `The server answered ${status} without an explanation`)
  }
  if (error.code === 'invalid') {
    const { field, reason } = splitInvalid(error.message)
    return new ApiError(status, 'invalid', reason, field)
  }
  return new ApiError(status, error.code as ErrorCode, error.message)
}

const ROOT = '/api/v1'
const unauthenticatedListeners = new Set<() => void>()

/** called when a request that should have carried a session comes back 401 */
export function onUnauthenticated(listener: () => void): () => void {
  unauthenticatedListeners.add(listener)
  return () => {
    unauthenticatedListeners.delete(listener)
  }
}

async function request<T>(method: string, path: string, body?: unknown, quiet401 = false): Promise<T> {
  let response: Response
  try {
    response = await fetch(ROOT + path, {
      method,
      credentials: 'same-origin',
      headers: body === undefined ? undefined : { 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
  } catch {
    throw new ApiError(0, 'unreachable', 'The server cannot be reached')
  }
  if (response.status === 204) return undefined as T
  const raw = await response.text()
  let parsed: unknown = null
  try {
    parsed = raw ? JSON.parse(raw) : null
  } catch {
    parsed = null
  }
  if (!response.ok) {
    const error = toApiError(response.status, parsed)
    if (error.code === 'unauthenticated' && !quiet401) unauthenticatedListeners.forEach((l) => l())
    throw error
  }
  return parsed as T
}

export const api = {
  get: <T>(path: string) => request<T>('GET', path),
  /** a GET whose 401 is an answer, not a lost session */
  probe: <T>(path: string) => request<T>('GET', path, undefined, true),
  post: <T>(path: string, body?: unknown, quiet401 = false) => request<T>('POST', path, body, quiet401),
  patch: <T>(path: string, body: unknown) => request<T>('PATCH', path, body),
  put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
  delete: (path: string) => request<void>('DELETE', path),
}

/** what to show a person for any thrown value */
export function explain(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.code === 'forbidden') return 'Your role does not allow this'
    if (error.code === 'invalid' && error.field) return `${error.field}: ${error.message}`
    return error.message
  }
  return error instanceof Error ? error.message : String(error)
}
