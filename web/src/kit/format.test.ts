import { describe, expect, it } from 'vitest'
import { ago, bytes, clock, count, countdown, dateTime, dayClock, parseTime, text } from './format'

const at = (iso: string) => new Date(iso)

describe('unknown versus zero', () => {
  it('renders unknown as a dash and zero as zero', () => {
    expect(bytes(null)).toBe('—')
    expect(bytes(0)).toBe('0 B')
    expect(count(undefined)).toBe('—')
    expect(count(0)).toBe('0')
    expect(text('')).toBe('—')
  })
})

describe('bytes', () => {
  it('uses binary units with one decimal below 100', () => {
    expect(bytes(1023)).toBe('1023 B')
    expect(bytes(1024)).toBe('1.0 KiB')
    expect(bytes(64 * 2 ** 30)).toBe('64.0 GiB')
    expect(bytes(512 * 2 ** 30)).toBe('512 GiB')
  })
})

describe('ago', () => {
  const now = at('2026-01-01T12:00:00Z')
  it('picks the largest unit that fits', () => {
    expect(ago(at('2026-01-01T11:59:48Z'), now)).toBe('12 s ago')
    expect(ago(at('2026-01-01T11:56:00Z'), now)).toBe('4 min ago')
    expect(ago(at('2026-01-01T09:00:00Z'), now)).toBe('3 h ago')
    expect(ago(at('2025-12-30T12:00:00Z'), now)).toBe('2 d ago')
  })
  it('does not go negative when the clocks disagree', () => {
    expect(ago(at('2026-01-01T12:00:05Z'), now)).toBe('0 s ago')
  })
  it('is a dash when never', () => {
    expect(ago(null, now)).toBe('—')
  })
})

describe('countdown', () => {
  it('counts minutes and seconds and stops at zero', () => {
    const now = at('2026-01-01T12:00:00Z')
    expect(countdown(at('2026-01-01T12:15:00Z'), now)).toBe('15:00')
    expect(countdown(at('2026-01-01T12:00:09Z'), now)).toBe('0:09')
    expect(countdown(at('2026-01-01T11:59:00Z'), now)).toBe('0:00')
  })
})

describe('parseTime', () => {
  it('returns null for what is not a time', () => {
    expect(parseTime('nonsense')).toBeNull()
    expect(parseTime(null)).toBeNull()
    expect(parseTime('2026-01-01T00:00:00Z')?.toISOString()).toBe('2026-01-01T00:00:00.000Z')
  })
})

describe('times of day', () => {
  // built from local parts, so the test holds in any time zone
  const moment = new Date(2026, 0, 5, 9, 3, 7)
  it('writes one form for every locale, zero-padded', () => {
    expect(dateTime(moment)).toBe('2026-01-05 09:03:07')
    expect(clock(moment)).toBe('09:03')
    expect(dayClock(moment)).toBe('01-05 09:03')
    expect(dateTime(null)).toBe('—')
  })
})
