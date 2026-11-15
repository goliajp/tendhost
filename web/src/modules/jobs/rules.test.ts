import { describe, expect, it } from 'vitest'
import { cronInWords, cronProblem, isOver, localInputToIso, nextRun, runSeconds, scheduleInWords, span } from './rules'

describe('cronInWords', () => {
  it('reads the common shapes as a sentence', () => {
    expect(cronInWords('* * * * *')).toBe('Every minute')
    expect(cronInWords('*/5 * * * *')).toBe('Every 5 minutes')
    expect(cronInWords('15 * * * *')).toBe('Every hour, at minute 15')
    expect(cronInWords('0 */6 * * *')).toBe('Every 6 hours, on the hour (UTC)')
    expect(cronInWords('30 3 * * *')).toBe('Every day at 03:30 UTC')
    expect(cronInWords('0 9 * * 1-5')).toBe('On Monday to Friday at 09:00 UTC')
    expect(cronInWords('0 9 * * 0,6')).toBe('On Sunday and Saturday at 09:00 UTC')
    expect(cronInWords('0 9 * * 7')).toBe('On Sunday at 09:00 UTC')
    expect(cronInWords('5 0 1 * *')).toBe('On day 1 of every month at 00:05 UTC')
  })

  it('spells out any other five-field expression instead of guessing', () => {
    expect(cronInWords('*/10 8-18 * 1,7 1-5')).toBe(
      'At minute */10, hour 8-18, every day of the month, month 1,7, day of the week 1-5 (UTC)',
    )
    expect(cronInWords('0 9 1 * 1')).toBe('At minute 0, hour 9, day of the month 1, every month, day of the week 1 (UTC)')
  })

  it('is null for anything but five fields', () => {
    expect(cronInWords('* * * *')).toBeNull()
    expect(cronInWords('0 * * * * *')).toBeNull()
    expect(cronInWords('   ')).toBeNull()
  })
})

describe('cronProblem', () => {
  it('names the field whose number is out of range', () => {
    expect(cronProblem('61 * * * *')).toBe('The minute is from 0 to 59')
    expect(cronProblem('0 24 * * *')).toBe('The hour is from 0 to 23')
    expect(cronProblem('0 0 0 * *')).toBe('The day of the month is from 1 to 31')
    expect(cronProblem('0 0 1 13 *')).toBe('The month is from 1 to 12')
    expect(cronProblem('0 0 * * 1-8')).toBe('The day of the week is from 0 to 7')
  })

  it('counts the fields', () => {
    expect(cronProblem('* * * *')).toBe('A cron expression has five fields')
    expect(cronProblem('')).toBe('A cron expression has five fields')
  })

  it('does not take a step for a value, and passes what is in range', () => {
    expect(cronProblem('*/90 * * * *')).toBeNull()
    expect(cronProblem('0-59/15 0,12 1,15 1-12 0,7')).toBeNull()
  })
})

describe('scheduleInWords', () => {
  it('covers the three kinds', () => {
    expect(scheduleInWords({ kind: 'manual' })).toBe('Only when someone runs it')
    expect(scheduleInWords({ kind: 'cron', expr: '30 3 * * *' })).toBe('Every day at 03:30 UTC')
    expect(scheduleInWords({ kind: 'once', at: new Date(2026, 0, 5, 9, 3, 7).toISOString() })).toBe(
      'Once, at 2026-01-05 09:03:07',
    )
  })
})

describe('span', () => {
  it('uses the two largest units that say something', () => {
    expect([0, 45, 60, 184, 3600, 7500].map(span)).toEqual(['0 s', '45 s', '1 min', '3 min 4 s', '1 h', '2 h 5 min'])
  })
})

describe('runSeconds', () => {
  it('needs both ends', () => {
    expect(runSeconds('2026-01-01T00:00:00Z', '2026-01-01T00:00:02.500Z')).toBe(2.5)
    expect(runSeconds('2026-01-01T00:00:00Z', null)).toBeNull()
    expect(runSeconds(null, null)).toBeNull()
  })
})

describe('isOver', () => {
  it('is false only while a result can still arrive', () => {
    expect(isOver('pending')).toBe(false)
    expect(isOver('running')).toBe(false)
    expect((['succeeded', 'failed', 'timed_out', 'missed', 'lost'] as const).every(isOver)).toBe(true)
  })
})

describe('localInputToIso', () => {
  it('reads the input as local time and gives the instant', () => {
    expect(localInputToIso('2026-01-05T09:03')).toBe(new Date(2026, 0, 5, 9, 3).toISOString())
    expect(localInputToIso('')).toBeNull()
  })
})

describe('nextRun', () => {
  const manual = { kind: 'manual' } as const
  it('gives the time, or the reason there is none', () => {
    const at = new Date(2026, 0, 5, 9, 3, 7).toISOString()
    expect(nextRun({ enabled: true, next_run_at: at, schedule: { kind: 'cron', expr: '* * * * *' } })).toBe(
      '2026-01-05 09:03:07',
    )
    expect(nextRun({ enabled: false, next_run_at: null, schedule: { kind: 'cron', expr: '* * * * *' } })).toBe(
      'Not while disabled',
    )
    expect(nextRun({ enabled: true, next_run_at: null, schedule: manual })).toBe('When someone runs it')
    expect(nextRun({ enabled: true, next_run_at: null, schedule: { kind: 'once', at } })).toBe('Has run')
  })
})
