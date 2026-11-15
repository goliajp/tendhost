import { useMemo, useRef, useState } from 'react'
import { bytes } from '@/kit/format'
import { Loading, Problem, Section } from '@/kit/ui'
import { useResource } from '@/kit/use-resource'
import { telemetryApi } from './api'
import { LineChart } from './chart'
import { diskUse, mounts, pointsFor, seriesOf } from './series'

const RANGES = [
  { id: '1h', label: '1 hour', ms: 3600_000 },
  { id: '24h', label: '24 hours', ms: 24 * 3600_000 },
  { id: '7d', label: '7 days', ms: 7 * 24 * 3600_000 },
] as const

type RangeId = (typeof RANGES)[number]['id']

const REFRESH_MS = 30_000

/** Fills `host.sections`: load, memory and each disk over a chosen range. */
export function HostCharts({ hostId }: { hostId: string }) {
  const holder = useRef<HTMLDivElement | null>(null)
  const [rangeId, setRangeId] = useState<RangeId>('1h')
  const range = RANGES.find((r) => r.id === rangeId)!
  // the window is fixed at each load, so the charts and their data agree
  const [window_, setWindow] = useState(() => ({ from: Date.now() - range.ms, to: Date.now() }))

  const samples = useResource(
    `samples:${hostId}:${rangeId}`,
    async () => {
      const to = Date.now()
      const from = to - range.ms
      // two charts side by side on a wide page, one on a narrow one
      const width = holder.current?.clientWidth ?? 0
      const loaded = await telemetryApi.samples(hostId, new Date(from), pointsFor(width >= 900 ? width / 2 : width))
      setWindow({ from, to })
      return loaded
    },
    REFRESH_MS,
  )

  const data = samples.data
  const load = useMemo(() => seriesOf(data ?? [], (s) => s.load1), [data])
  const memory = useMemo(() => seriesOf(data ?? [], (s) => s.memory_used_bytes), [data])
  const disks = useMemo(
    () => mounts(data ?? []).map((mount) => ({ mount, points: seriesOf(data ?? [], (s) => diskUse(s, mount)) })),
    [data],
  )

  return (
    <Section
      title="Measurements"
      aside={
        <div role="group" aria-label="Range" className="flex gap-1">
          {RANGES.map((r) => (
            <button
              key={r.id}
              type="button"
              aria-pressed={r.id === rangeId}
              onClick={() => setRangeId(r.id)}
              className={`rounded-[5px] border px-2.5 py-0.5 text-[0.87rem] ${
                r.id === rangeId ? 'border-ink bg-ink text-ground' : 'border-rule text-muted hover:border-muted'
              }`}
            >
              {r.label}
            </button>
          ))}
        </div>
      }
    >
      <div ref={holder} />
      {samples.error ? (
        <div className="mb-4">
          <Problem error={samples.error} retry={samples.reload} />
        </div>
      ) : null}
      {data === undefined ? (
        samples.loading ? (
          <Loading what="measurements" />
        ) : null
      ) : (
        <div className="grid gap-x-10 gap-y-8 lg:grid-cols-2">
          <LineChart title="Load, 1 minute" points={load} format={(v) => v.toFixed(2)} {...window_} />
          <LineChart title="Memory in use" points={memory} format={(v) => bytes(v)} unit={2 ** 30} {...window_} />
          {disks.map((disk) => (
            <LineChart
              key={disk.mount}
              title={`Disk ${disk.mount}`}
              points={disk.points}
              format={(v) => `${v.toFixed(0)}%`}
              ceiling={100}
              {...window_}
            />
          ))}
        </div>
      )}
      <p className="mt-4 text-[0.87rem] text-muted">
        A break in a line is a stretch in which the host did not report. Measurements are kept for 7 days.
      </p>
    </Section>
  )
}
