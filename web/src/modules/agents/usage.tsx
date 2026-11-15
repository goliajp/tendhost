import { useMemo, useState } from 'react'
import { Loading, Problem } from '@/kit/ui'
import { useResource } from '@/kit/use-resource'
import { agentsApi, type AgentKind } from './api'
import { compact, lastDays, seriesModels, stacks, totals, type DayStack } from './rules'

const SPANS = [7, 30, 90] as const
const MAX_SERIES = 5
const COLOURS = ['var(--series-1)', 'var(--series-2)', 'var(--series-3)', 'var(--series-4)', 'var(--series-5)']
const HEIGHT = 120

/** Token usage of one agent on one host: a bar per day split by model,
 *  and the totals of the span. */
export function Usage({ hostId, agent }: { hostId: string; agent: AgentKind }) {
  const [days, setDays] = useState<(typeof SPANS)[number]>(30)
  const usage = useResource(`usage:${hostId}:${agent}:${days}`, () => agentsApi.usage(hostId, agent, days), 60_000)

  const view = useMemo(() => {
    if (!usage.data) return null
    const models = seriesModels(usage.data, MAX_SERIES)
    return { models, bars: stacks(usage.data, lastDays(new Date(), days), models), sum: totals(usage.data) }
  }, [usage.data, days])

  return (
    <div>
      <div className="mb-2 flex flex-wrap items-baseline justify-between gap-3">
        <h4 className="m-0 font-medium">Tokens per day</h4>
        <div role="group" aria-label="Span" className="flex gap-1">
          {SPANS.map((span) => (
            <button
              key={span}
              type="button"
              aria-pressed={span === days}
              onClick={() => setDays(span)}
              className={`rounded-[5px] border px-2.5 py-0.5 text-[0.87rem] ${
                span === days ? 'border-ink bg-ink text-ground' : 'border-rule text-muted hover:border-muted'
              }`}
            >
              {span} days
            </button>
          ))}
        </div>
      </div>
      {usage.error ? <Problem error={usage.error} retry={usage.reload} /> : null}
      {view === null ? (
        usage.loading ? (
          <Loading what="usage" />
        ) : null
      ) : view.models.length === 0 ? (
        <p className="text-muted">No session was recorded in these {days} days.</p>
      ) : (
        <>
          <Bars bars={view.bars} />
          <ul className="m-0 mt-2 flex list-none flex-wrap gap-x-5 gap-y-1 p-0 text-[0.87rem]">
            {view.models.map((model, index) => (
              <li key={model} className="inline-flex items-center gap-1.5">
                <span aria-hidden className="inline-block h-2.5 w-2.5 rounded-[2px]" style={{ background: COLOURS[index] }} />
                {model}
              </li>
            ))}
          </ul>
          <p className="mt-1 text-[0.87rem] text-muted">
            Bars count tokens sent and received. Cache traffic is in the totals only; it would dwarf the rest.
          </p>
          <dl className="tabular m-0 mt-3 grid grid-cols-2 gap-x-6 gap-y-1 sm:grid-cols-4">
            {(
              [
                ['Input', view.sum.input],
                ['Output', view.sum.output],
                ['Cache read', view.sum.cacheRead],
                ['Cache write', view.sum.cacheWrite],
              ] as const
            ).map(([name, value]) => (
              <div key={name}>
                <dt className="text-[0.87rem] text-muted">{name}</dt>
                <dd className="m-0 text-[1.1rem] font-medium" title={`${value.toLocaleString('en')} tokens`}>
                  {compact(value)}
                </dd>
              </div>
            ))}
          </dl>
        </>
      )}
    </div>
  )
}

function Bars({ bars }: { bars: DayStack[] }) {
  const [hover, setHover] = useState<DayStack | null>(null)
  const top = Math.max(1, ...bars.map((bar) => bar.total))
  const last = bars[bars.length - 1]
  const read = hover ?? last
  return (
    <figure className="m-0">
      <figcaption className="tabular mb-1 text-[0.87rem] text-muted">
        {read ? `${read.day}: ${compact(read.total)} tokens` : ''}
        {hover ? '' : ' (latest day; point at a bar for another)'}
      </figcaption>
      <svg
        role="img"
        aria-label={`Tokens per day from ${bars[0]?.day} to ${last?.day}; the busiest day had ${compact(top)}`}
        viewBox={`0 0 ${bars.length} ${HEIGHT}`}
        preserveAspectRatio="none"
        className="block h-[120px] w-full border-b border-rule"
        onPointerLeave={() => setHover(null)}
      >
        {bars.map((bar, x) => {
          let y = HEIGHT
          return (
            <g key={bar.day} onPointerEnter={() => setHover(bar)}>
              <rect x={x} y={0} width={1} height={HEIGHT} fill="transparent" />
              {bar.parts.map((part, index) => {
                const height = (part / top) * HEIGHT
                y -= height
                return <rect key={index} x={x + 0.12} y={y} width={0.76} height={height} fill={COLOURS[index]} />
              })}
            </g>
          )
        })}
      </svg>
      <div className="tabular mt-0.5 flex justify-between text-[0.75rem] text-muted">
        <span>{bars[0]?.day}</span>
        <span>{last?.day}</span>
      </div>
    </figure>
  )
}
