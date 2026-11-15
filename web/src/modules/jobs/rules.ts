import { dateTime, parseTime } from '@/kit/format'
import type { Job, RunState, Schedule } from './api'

export const RUN_STATE: Record<RunState, { name: string; meaning: string; tone: 'live' | 'down' | 'quiet' }> = {
  pending: { name: 'Waiting', meaning: 'Waiting for the host to collect it at its next report', tone: 'quiet' },
  running: { name: 'Running', meaning: 'Handed to the host; it has not reported the end yet', tone: 'quiet' },
  succeeded: { name: 'Succeeded', meaning: 'The command exited with status 0', tone: 'live' },
  failed: {
    name: 'Failed',
    meaning: 'The command exited with a status other than 0, or the host refused to run it',
    tone: 'down',
  },
  timed_out: { name: 'Timed out', meaning: 'The host stopped the command at the job’s time limit', tone: 'down' },
  missed: { name: 'Missed', meaning: 'The host did not collect it within 6 hours, so it was never started', tone: 'down' },
  lost: {
    name: 'Lost',
    meaning: 'Handed to the host, which never reported how it ended; it may or may not have finished',
    tone: 'down',
  },
}

export function isOver(state: RunState): boolean {
  return state !== 'pending' && state !== 'running'
}

const DAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
const two = (n: number) => String(n).padStart(2, '0')
const number = (field: string, min: number, max: number): number | null =>
  /^\d+$/.test(field) && Number(field) >= min && Number(field) <= max ? Number(field) : null
const step = (field: string): number | null => {
  const match = /^\*\/(\d+)$/.exec(field)
  return match && Number(match[1]) > 0 ? Number(match[1]) : null
}

/** `1-5` → Monday to Friday; `0,6` → Sunday and Saturday; null when the
 *  field is not a plain day, range or list of days. */
function daysOfWeek(field: string): string | null {
  const day = (text: string) => {
    const n = number(text, 0, 7)
    return n === null ? null : DAYS[n % 7]!
  }
  const range = /^(\d)-(\d)$/.exec(field)
  if (range) {
    const [from, to] = [day(range[1]!), day(range[2]!)]
    return from && to ? `${from} to ${to}` : null
  }
  const names = field.split(',').map(day)
  if (names.some((name) => name === null)) return null
  return names.length === 1 ? names[0]! : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`
}

const FIELDS: [string, number, number][] = [
  ['minute', 0, 59],
  ['hour', 0, 23],
  ['day of the month', 1, 31],
  ['month', 1, 12],
  ['day of the week', 0, 7],
]

/** What is plainly wrong with a cron expression: the number of fields, or
 *  a number outside its field's range. Null when nothing is; the server
 *  has the last word on the rest. */
export function cronProblem(expr: string): string | null {
  const fields = expr.trim().split(/\s+/)
  if (fields.length !== 5 || fields[0] === '') return 'A cron expression has five fields'
  for (const [index, field] of fields.entries()) {
    const [name, min, max] = FIELDS[index]!
    // a number after `/` is a step, not a value of the field
    const values = field.split(',').flatMap((part) => (part.split('/')[0] ?? '').match(/\d+/g) ?? [])
    if (values.some((value) => Number(value) < min || Number(value) > max)) {
      return `The ${name} is from ${min} to ${max}`
    }
  }
  return null
}

/** A five-field cron expression in words, for the shapes people write
 *  most; any other valid shape is spelled out field by field. Null when
 *  it does not have five fields. Cron times are UTC. */
export function cronInWords(expr: string): string | null {
  const fields = expr.trim().split(/\s+/)
  if (fields.length !== 5 || fields[0] === '') return null
  const [minute, hour, dayOfMonth, month, dayOfWeek] = fields as [string, string, string, string, string]
  const anyDay = dayOfMonth === '*' && month === '*' && dayOfWeek === '*'
  const m = number(minute, 0, 59)
  const h = number(hour, 0, 23)

  if (anyDay && minute === '*' && hour === '*') return 'Every minute'
  if (anyDay && hour === '*' && step(minute)) return `Every ${step(minute)} minutes`
  if (anyDay && hour === '*' && m !== null) return `Every hour, at minute ${m}`
  if (anyDay && m === 0 && step(hour)) return `Every ${step(hour)} hours, on the hour (UTC)`
  if (m !== null && h !== null && month === '*') {
    const time = `${two(h)}:${two(m)} UTC`
    if (dayOfMonth === '*' && dayOfWeek === '*') return `Every day at ${time}`
    const days = dayOfMonth === '*' ? daysOfWeek(dayOfWeek) : null
    if (days) return `On ${days} at ${time}`
    const date = dayOfWeek === '*' ? number(dayOfMonth, 1, 31) : null
    if (date !== null) return `On day ${date} of every month at ${time}`
  }
  const part = (field: string, unit: string) => (field === '*' ? `every ${unit}` : `${unit} ${field}`)
  return [
    `At ${part(minute, 'minute')}`,
    part(hour, 'hour'),
    part(dayOfMonth, 'day of the month'),
    part(month, 'month'),
    `${part(dayOfWeek, 'day of the week')} (UTC)`,
  ].join(', ')
}

export function scheduleInWords(schedule: Schedule): string {
  switch (schedule.kind) {
    case 'manual':
      return 'Only when someone runs it'
    case 'once':
      return `Once, at ${dateTime(parseTime(schedule.at))}`
    case 'cron':
      return cronInWords(schedule.expr) ?? schedule.expr
  }
}

/** When the job next runs, or why no time can be given. */
export function nextRun(job: Pick<Job, 'enabled' | 'next_run_at' | 'schedule'>): string {
  if (!job.enabled) return 'Not while disabled'
  if (job.next_run_at) return dateTime(parseTime(job.next_run_at))
  return job.schedule.kind === 'once' ? 'Has run' : 'When someone runs it'
}

/** `45 s`, `3 min 4 s`, `2 h 5 min` */
export function span(seconds: number): string {
  const s = Math.max(0, Math.round(seconds))
  if (s < 60) return `${s} s`
  if (s < 3600) return s % 60 === 0 ? `${s / 60} min` : `${Math.floor(s / 60)} min ${s % 60} s`
  const minutes = Math.floor((s % 3600) / 60)
  return minutes === 0 ? `${Math.floor(s / 3600)} h` : `${Math.floor(s / 3600)} h ${minutes} min`
}

/** How long a run took; null until it has both ends. */
export function runSeconds(started: string | null, finished: string | null): number | null {
  const [from, to] = [parseTime(started), parseTime(finished)]
  return from && to ? (to.getTime() - from.getTime()) / 1000 : null
}

/** The value of a `datetime-local` input as the instant it names in the
 *  reader's time zone; null when empty or not a time. */
export function localInputToIso(value: string): string | null {
  return parseTime(value)?.toISOString() ?? null
}
