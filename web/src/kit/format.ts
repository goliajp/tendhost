// Unknown is `null` and renders as a dash. Zero is a value and renders as 0.

export const DASH = '—'

const UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']

export function bytes(value: number | null | undefined): string {
  if (value === null || value === undefined) return DASH
  let n = value
  let unit = 0
  while (n >= 1024 && unit < UNITS.length - 1) {
    n /= 1024
    unit += 1
  }
  const decimals = unit === 0 || n >= 100 ? 0 : 1
  return `${n.toFixed(decimals)} ${UNITS[unit]}`
}

export function text(value: string | null | undefined): string {
  return value === null || value === undefined || value === '' ? DASH : value
}

export function count(value: number | null | undefined): string {
  return value === null || value === undefined ? DASH : String(value)
}

/** how long ago, in the largest unit that fits: `12 s`, `4 min`, `3 h`, `2 d` */
export function ago(then: Date | null, now: Date): string {
  if (then === null) return DASH
  const seconds = Math.max(0, Math.floor((now.getTime() - then.getTime()) / 1000))
  if (seconds < 60) return `${seconds} s ago`
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} h ago`
  return `${Math.floor(seconds / 86400)} d ago`
}

/** time left as `m:ss`; `0:00` once passed */
export function countdown(until: Date, now: Date): string {
  const seconds = Math.max(0, Math.ceil((until.getTime() - now.getTime()) / 1000))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}

export function parseTime(value: string | null | undefined): Date | null {
  if (!value) return null
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}

const two = (n: number) => String(n).padStart(2, '0')

/** `2026-01-05 09:03:07` in the reader's time zone: one form for every
 *  locale, sortable by eye */
export function dateTime(value: Date | null): string {
  if (value === null) return DASH
  return `${value.getFullYear()}-${two(value.getMonth() + 1)}-${two(value.getDate())} ${clock(value, true)}`
}

/** `09:03`, or `09:03:07` with seconds */
export function clock(value: Date, seconds = false): string {
  return `${two(value.getHours())}:${two(value.getMinutes())}${seconds ? `:${two(value.getSeconds())}` : ''}`
}

/** `01-05 09:03`: for spans longer than a day, where the hour alone is ambiguous */
export function dayClock(value: Date): string {
  return `${two(value.getMonth() + 1)}-${two(value.getDate())} ${clock(value)}`
}
