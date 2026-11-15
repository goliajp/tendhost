// The arithmetic behind the charts, kept apart from drawing.

import type { Sample } from './api'

export interface Point {
  /** unix milliseconds */
  t: number
  v: number
}

/** At most `max` points over `[from, to]`: the window is cut into equal
 *  spans and each span that holds samples becomes their mean. A span with
 *  no sample yields no point, so silence stays visible as a gap. */
export function downsample(points: readonly Point[], from: number, to: number, max: number): Point[] {
  if (points.length <= max) return [...points]
  const span = (to - from) / max
  const sums = new Map<number, { t: number; v: number; n: number }>()
  for (const point of points) {
    const index = Math.min(max - 1, Math.max(0, Math.floor((point.t - from) / span)))
    const sum = sums.get(index) ?? { t: 0, v: 0, n: 0 }
    sum.t += point.t
    sum.v += point.v
    sum.n += 1
    sums.set(index, sum)
  }
  return [...sums.entries()]
    .sort(([a], [b]) => a - b)
    .map(([, sum]) => ({ t: sum.t / sum.n, v: sum.v / sum.n }))
}

/** The median distance in time between neighbouring points; 0 for fewer
 *  than two. */
export function typicalGap(points: readonly Point[]): number {
  const gaps = points.slice(1).map((point, i) => point.t - points[i]!.t)
  if (gaps.length === 0) return 0
  gaps.sort((a, b) => a - b)
  return gaps[Math.floor(gaps.length / 2)]!
}

/** Splits the points wherever neighbours are further apart than two and a
 *  half typical gaps: the host was not reporting there, and a line across
 *  would claim values nobody measured. */
export function segments(points: readonly Point[]): Point[][] {
  const limit = typicalGap(points) * 2.5
  const out: Point[][] = []
  for (const point of points) {
    const last = out[out.length - 1]
    const previous = last?.[last.length - 1]
    if (last && previous && (limit === 0 || point.t - previous.t <= limit)) last.push(point)
    else out.push([point])
  }
  return out
}

/** The smallest of 1, 2, 5 × 10ⁿ that is not below `value`; 1 for 0. */
export function niceCeil(value: number): number {
  if (value <= 0) return 1
  const power = 10 ** Math.floor(Math.log10(value))
  for (const step of [1, 2, 5, 10]) {
    if (step * power >= value) return step * power
  }
  return 10 * power
}

export function linear(d0: number, d1: number, r0: number, r1: number): (value: number) => number {
  const width = d1 - d0
  return (value) => (width === 0 ? r0 : r0 + ((value - d0) / width) * (r1 - r0))
}

/** An svg path through the points of one segment. */
export function pathOf(segment: readonly Point[], x: (t: number) => number, y: (v: number) => number): string {
  return segment.map((p, i) => `${i === 0 ? 'M' : 'L'}${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`).join('')
}

/** The point nearest in time to `t`; null when there is none. */
export function nearest(points: readonly Point[], t: number): Point | null {
  let best: Point | null = null
  for (const point of points) {
    if (best === null || Math.abs(point.t - t) < Math.abs(best.t - t)) best = point
  }
  return best
}

export function seriesOf(samples: readonly Sample[], pick: (sample: Sample) => number | null): Point[] {
  const points: Point[] = []
  for (const sample of samples) {
    const t = Date.parse(sample.at)
    const v = pick(sample)
    if (!Number.isNaN(t) && v !== null) points.push({ t, v })
  }
  return points
}

/** Every mount that appears in the samples, in name order. */
export function mounts(samples: readonly Sample[]): string[] {
  return [...new Set(samples.flatMap((sample) => sample.disks.map((disk) => disk.mount)))].sort()
}

/** Percent of a disk in use in one sample; null when the sample does not
 *  mention the mount. */
export function diskUse(sample: Sample, mount: string): number | null {
  const disk = sample.disks.find((d) => d.mount === mount)
  if (!disk || disk.total_bytes === 0) return null
  return (disk.used_bytes / disk.total_bytes) * 100
}
