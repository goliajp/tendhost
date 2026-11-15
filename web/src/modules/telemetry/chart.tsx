import { useEffect, useRef, useState } from 'react'
import { DASH, clock, dayClock } from '@/kit/format'
import { linear, nearest, niceCeil, pathOf, segments, type Point } from './series'

const HEIGHT = 150
const PAD = { top: 8, right: 8, bottom: 20, left: 60 }

function useWidth(): [React.RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement | null>(null)
  const [width, setWidth] = useState(0)
  useEffect(() => {
    const element = ref.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => setWidth(entry?.contentRect.width ?? 0))
    observer.observe(element)
    return () => observer.disconnect()
  }, [])
  return [ref, width]
}

const stamp = (t: number, withDate: boolean) => (withDate ? dayClock(new Date(t)) : clock(new Date(t)))

/** One measurement over time. The line breaks where the host did not
 *  report. The figure beside the title is the latest value, or the value
 *  under the pointer. */
export function LineChart({
  title,
  points,
  from,
  to,
  format,
  ceiling,
  unit = 1,
}: {
  title: string
  points: Point[]
  from: number
  to: number
  format: (value: number) => string
  /** a fixed top of the scale; by default it follows the data */
  ceiling?: number
  /** the size of the unit the scale is read in, so that its ticks are
   *  round in that unit: 2 ** 30 for a scale shown in GiB */
  unit?: number
}) {
  const [ref, width] = useWidth()
  const [hover, setHover] = useState<Point | null>(null)
  const shown = points
  const top = ceiling ?? niceCeil(Math.max(0, ...shown.map((p) => p.v)) / unit) * unit
  const x = linear(from, to, PAD.left, Math.max(PAD.left + 1, width - PAD.right))
  const y = linear(0, top, HEIGHT - PAD.bottom, PAD.top)
  const latest = shown[shown.length - 1] ?? null
  const read = hover ?? latest
  const longRange = to - from > 36 * 3600 * 1000

  return (
    <figure className="m-0">
      <figcaption className="mb-1 flex flex-wrap items-baseline justify-between gap-x-4">
        <span className="font-medium">{title}</span>
        <span className="tabular text-[0.93rem]">
          {read ? format(read.v) : DASH}
          <span className="text-muted">
            {read ? ` at ${stamp(read.t, longRange)}` : ''}
          </span>
        </span>
      </figcaption>
      <div ref={ref}>
        {width > 0 ? (
          <svg
            width={width}
            height={HEIGHT}
            role="img"
            aria-label={`${title}: ${latest ? `latest ${format(latest.v)}` : 'no measurements in this range'}`}
            className="block touch-none"
            onPointerMove={(event) => {
              const box = event.currentTarget.getBoundingClientRect()
              const t = from + ((event.clientX - box.left - PAD.left) / (width - PAD.left - PAD.right)) * (to - from)
              setHover(nearest(shown, t))
            }}
            onPointerLeave={() => setHover(null)}
          >
            {[0, 0.5, 1].map((fraction) => (
              <g key={fraction}>
                <line
                  x1={PAD.left}
                  x2={width - PAD.right}
                  y1={y(top * fraction)}
                  y2={y(top * fraction)}
                  stroke="var(--rule)"
                  strokeWidth={1}
                />
                <text
                  x={PAD.left - 6}
                  y={y(top * fraction) + 4}
                  textAnchor="end"
                  fontSize={11}
                  fill="var(--muted)"
                  className="tabular"
                >
                  {format(top * fraction)}
                </text>
              </g>
            ))}
            <text x={PAD.left} y={HEIGHT - 4} fontSize={11} fill="var(--muted)">
              {stamp(from, longRange)}
            </text>
            <text x={width - PAD.right} y={HEIGHT - 4} fontSize={11} fill="var(--muted)" textAnchor="end">
              {stamp(to, longRange)}
            </text>
            {segments(shown).map((segment, index) =>
              segment.length === 1 ? (
                <circle key={index} cx={x(segment[0]!.t)} cy={y(segment[0]!.v)} r={1.5} fill="var(--ink)" />
              ) : (
                <path
                  key={index}
                  d={pathOf(segment, x, y)}
                  fill="none"
                  stroke="var(--ink)"
                  strokeWidth={1.5}
                  strokeLinejoin="round"
                />
              ),
            )}
            {hover ? (
              <>
                <line x1={x(hover.t)} x2={x(hover.t)} y1={PAD.top} y2={HEIGHT - PAD.bottom} stroke="var(--muted)" strokeWidth={1} />
                <circle cx={x(hover.t)} cy={y(hover.v)} r={3.5} fill="var(--live)" />
              </>
            ) : null}
            {shown.length === 0 ? (
              <text x={(PAD.left + width - PAD.right) / 2} y={HEIGHT / 2} textAnchor="middle" fontSize={13} fill="var(--muted)">
                No measurements in this range
              </text>
            ) : null}
          </svg>
        ) : (
          <div style={{ height: HEIGHT }} />
        )}
      </div>
    </figure>
  )
}
