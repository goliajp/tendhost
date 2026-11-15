import { describe, expect, it } from 'vitest'
import type { Sample } from './api'
import { diskUse, linear, mounts, nearest, niceCeil, pathOf, pointsFor, segments, seriesOf, typicalGap } from './series'

const every = (n: number, step: number, v = (i: number) => i) =>
  Array.from({ length: n }, (_, i) => ({ t: i * step, v: v(i) }))

describe('pointsFor', () => {
  it('asks for a point per three pixels, within bounds', () => {
    expect(pointsFor(600)).toBe(200)
    expect(pointsFor(90)).toBe(60)
    expect(pointsFor(5000)).toBe(400)
    expect(pointsFor(0)).toBe(60)
  })
})

describe('segments', () => {
  it('keeps regular reporting as one line', () => {
    expect(segments(every(10, 30))).toHaveLength(1)
  })

  it('breaks the line where the host was silent', () => {
    const points = [...every(5, 30), ...every(5, 30).map((p) => ({ t: p.t + 600, v: p.v }))]
    const parts = segments(points)
    expect(parts.map((part) => part.length)).toEqual([5, 5])
  })

  it('does not break on a single late report', () => {
    const points = [0, 30, 60, 125, 155].map((t) => ({ t, v: 1 }))
    expect(segments(points)).toHaveLength(1)
  })

  it('handles none and one', () => {
    expect(segments([])).toEqual([])
    expect(segments([{ t: 1, v: 1 }])).toEqual([[{ t: 1, v: 1 }]])
    expect(typicalGap([{ t: 1, v: 1 }])).toBe(0)
  })
})

describe('niceCeil', () => {
  it('rounds up to 1, 2 or 5 times a power of ten', () => {
    expect([0, 0.3, 1, 1.2, 4.9, 5, 7, 64, 100].map(niceCeil)).toEqual([1, 0.5, 1, 2, 5, 5, 10, 100, 100])
  })
})

describe('drawing arithmetic', () => {
  it('maps a domain onto a range, either direction', () => {
    const y = linear(0, 10, 100, 0)
    expect([y(0), y(5), y(10)]).toEqual([100, 50, 0])
    expect(linear(3, 3, 7, 9)(3)).toBe(7)
  })

  it('writes a path through the points', () => {
    const path = pathOf([{ t: 0, v: 0 }, { t: 10, v: 5 }], (t) => t * 2, (v) => 10 - v)
    expect(path).toBe('M0.0,10.0L20.0,5.0')
  })

  it('finds the point nearest in time', () => {
    const points = every(4, 10)
    expect(nearest(points, 14)?.t).toBe(10)
    expect(nearest(points, 16)?.t).toBe(20)
    expect(nearest([], 1)).toBeNull()
  })
})

describe('reading samples', () => {
  const sample = (at: string, disks: Sample['disks']): Sample => ({
    at,
    load1: 1,
    memory_used_bytes: 2,
    uptime_secs: 3,
    disks,
  })
  const samples = [
    sample('2026-01-01T00:00:00Z', [{ mount: '/', total_bytes: 200, used_bytes: 50 }]),
    sample('2026-01-01T00:00:30Z', [
      { mount: '/data', total_bytes: 0, used_bytes: 0 },
      { mount: '/', total_bytes: 200, used_bytes: 100 },
    ]),
  ]

  it('lists every mount once', () => {
    expect(mounts(samples)).toEqual(['/', '/data'])
  })

  it('gives disk use in percent, and no value for an absent or empty disk', () => {
    expect(diskUse(samples[0]!, '/')).toBe(25)
    expect(diskUse(samples[0]!, '/data')).toBeNull()
    expect(diskUse(samples[1]!, '/data')).toBeNull()
  })

  it('skips samples that have no value for the series', () => {
    const series = seriesOf(samples, (s) => diskUse(s, '/data'))
    expect(series).toEqual([])
    expect(seriesOf(samples, (s) => s.load1)).toHaveLength(2)
  })
})
