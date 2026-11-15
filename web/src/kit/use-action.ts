import { useState } from 'react'
import { ApiError } from './api'

export interface Action<A extends unknown[]> {
  /** resolves to whether the work succeeded */
  run: (...args: A) => Promise<boolean>
  busy: boolean
  /** the failure that belongs to no field of this form */
  error: unknown
  /** the server's complaint about one field, by the server's name for it */
  fieldError: (field: string) => string | null
  clear: () => void
}

/** Sorts a failure into "about this field" or "about the whole action".
 *  A complaint about a field the form does not have is about the whole
 *  action, so it is never swallowed. */
export function placeError(error: unknown, fields: readonly string[]): { field: string | null; general: unknown } {
  if (error instanceof ApiError && error.code === 'invalid' && error.field && fields.includes(error.field)) {
    return { field: error.field, general: undefined }
  }
  return { field: null, general: error }
}

/** Runs something that changes the server, one run at a time. `fields` are
 *  the server-side names of the fields the form shows. */
export function useAction<A extends unknown[]>(
  work: (...args: A) => Promise<void>,
  fields: readonly string[] = [],
): Action<A> {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(undefined)

  const run = async (...args: A) => {
    setBusy(true)
    setError(undefined)
    try {
      await work(...args)
      return true
    } catch (caught) {
      setError(caught)
      return false
    } finally {
      setBusy(false)
    }
  }

  const placed = placeError(error, fields)
  return {
    run,
    busy,
    error: placed.general,
    fieldError: (field) => (placed.field === field && error instanceof ApiError ? error.message : null),
    clear: () => setError(undefined),
  }
}
