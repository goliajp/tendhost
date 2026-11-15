import { useCallback, useEffect, useRef, useState } from 'react'

export interface Resource<T> {
  /** the last value that loaded; kept while a refresh is in flight, so a
   *  page that polls does not blank out between answers */
  data: T | undefined
  /** the last failure; cleared by the next success */
  error: unknown
  /** true until the first answer, of either kind */
  loading: boolean
  reload: () => Promise<void>
}

/** Loads with `load`, again whenever `key` changes, and every `everyMs`
 *  while the tab is visible. */
export function useResource<T>(key: string, load: () => Promise<T>, everyMs?: number): Resource<T> {
  const [state, setState] = useState<{ key: string; data: T | undefined; error: unknown; loading: boolean }>({
    key,
    data: undefined,
    error: undefined,
    loading: true,
  })
  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  // answers that arrive for a key no longer shown are dropped
  const current = useRef(key)

  const reload = useCallback(async () => {
    const asked = key
    try {
      const data = await loadRef.current()
      if (current.current === asked) setState({ key: asked, data, error: undefined, loading: false })
    } catch (error) {
      if (current.current === asked) setState((s) => ({ key: asked, data: s.key === asked ? s.data : undefined, error, loading: false }))
    }
  }, [key])

  useEffect(() => {
    current.current = key
    void reload()
    if (!everyMs) return
    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') void reload()
    }, everyMs)
    return () => clearInterval(timer)
  }, [key, reload, everyMs])

  if (state.key !== key) return { data: undefined, error: undefined, loading: true, reload }
  return { data: state.data, error: state.error, loading: state.loading, reload }
}

/** the current time, re-read every `everyMs`; for text such as "12 s ago" */
export function useNow(everyMs = 1000): Date {
  const [now, setNow] = useState(() => new Date())
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), everyMs)
    return () => clearInterval(timer)
  }, [everyMs])
  return now
}
