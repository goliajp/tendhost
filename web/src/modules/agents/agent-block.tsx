import { ConfirmButton } from '@/kit/confirm'
import { ago, dateTime, parseTime } from '@/kit/format'
import { LinkButton, StateTag } from '@/kit/ui'
import { useNow } from '@/kit/use-resource'
import type { AgentView, LimitReading } from './api'
import { agentName, offers } from './rules'
import { Usage } from './usage'

const NOT_KEPT = 'does not keep that where tendhostd can read it'

/** Everything known about one agent on one host. */
export function AgentBlock({
  view,
  canChange,
  busy,
  onStop,
}: {
  view: AgentView
  canChange: boolean
  busy: boolean
  onStop: () => void
}) {
  const name = agentName(view.agent)
  const accounts = `${view.account_count} ${view.account_count === 1 ? 'account' : 'accounts'}`

  return (
    <article className="py-5 first:pt-2">
      <header className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2">
        <div>
          <h3 className="m-0 inline text-[1.05rem] font-semibold">{name}</h3>
          <span className="ml-3 text-[0.93rem] text-muted">{accounts}</span>
          <span className="ml-3 text-[0.93rem]">
            <StateTag tone={view.tracked ? 'live' : 'quiet'}>{view.tracked ? 'Tracked' : 'Not tracked'}</StateTag>
          </span>
        </div>
        {canChange ? (
          view.tracked ? (
            <ConfirmButton
              label="Stop tracking"
              question={`Stop tracking ${name} here? Its accounts, limit readings and usage are deleted from this server`}
              confirmLabel="Stop and delete"
              busy={busy}
              onConfirm={onStop}
            />
          ) : (
            <LinkButton to={`/agents/track?host=${encodeURIComponent(view.host_id)}`}>Track it</LinkButton>
          )
        ) : null}
      </header>

      {!view.tracked ? (
        <p className="mt-2 max-w-[65ch] text-[0.93rem] text-muted">
          Only that it is installed, and how many accounts it has, is known. {offers(view)}.
        </p>
      ) : (
        <div className="mt-4 grid gap-x-10 gap-y-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]">
          <div className="space-y-5">
            <div>
              <h4 className="m-0 mb-1 font-medium">Accounts</h4>
              {view.accounts.length === 0 ? (
                <p className="text-[0.93rem] text-muted">
                  Waiting for the host to report them; it answers at its next sync, within 30 seconds.
                </p>
              ) : (
                <ul className="m-0 list-none space-y-1 p-0">
                  {view.accounts.map((account) => (
                    <li key={account.dir} className="text-[0.93rem]">
                      {account.label}
                      <span className="text-muted">
                        {account.plan ? `, plan ${account.plan}` : ''}{' '}
                        <span className="font-mono text-[0.8rem] break-all">{account.dir}</span>
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <div>
              <h4 className="m-0 mb-1 font-medium">Limits</h4>
              {!view.supports_limits ? (
                <p className="text-[0.93rem] text-muted">
                  {name} {NOT_KEPT}.
                </p>
              ) : view.limits ? (
                <Limits reading={view.limits} shared={view.account_count > 1} />
              ) : (
                <p className="text-[0.93rem] text-muted">
                  No reading yet. One appears after the next session of {name} on this host.
                </p>
              )}
            </div>
          </div>
          <div>
            {view.supports_usage ? (
              <>
                <Usage hostId={view.host_id} agent={view.agent} />
                {view.account_count > 1 ? (
                  <p className="mt-2 max-w-[65ch] text-[0.87rem] text-muted">
                    Accounts that share one session store, the usual layout for several accounts, cannot be
                    told apart in it: usage is for {name} on this host as a whole.
                  </p>
                ) : null}
              </>
            ) : (
              <>
                <h4 className="m-0 mb-1 font-medium">Tokens per day</h4>
                <p className="text-[0.93rem] text-muted">
                  {name} {NOT_KEPT}.
                </p>
              </>
            )}
          </div>
        </div>
      )}
    </article>
  )
}

function Limits({ reading, shared }: { reading: LimitReading; shared: boolean }) {
  const now = useNow(30_000)
  return (
    <div className="space-y-3">
      {reading.windows.map((window) => {
        const percent = Math.round(window.used_ratio * 100)
        const high = window.used_ratio >= 0.9
        const resets = parseTime(window.resets_at)
        return (
          <div key={window.name}>
            <div className="tabular flex items-baseline justify-between gap-3 text-[0.93rem]">
              <span>
                {window.name} window{reading.plan ? <span className="text-muted">, plan {reading.plan}</span> : null}
              </span>
              <span className={high ? 'font-medium text-down' : 'font-medium'}>{percent}% used</span>
            </div>
            <div
              role="meter"
              aria-label={`${window.name} window used`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percent}
              className="mt-1 h-2 overflow-hidden rounded-[2px] bg-rule"
            >
              <div className={`h-full ${high ? 'bg-down' : 'bg-ink'}`} style={{ width: `${Math.min(100, percent)}%` }} />
            </div>
            <p className="tabular mt-1 text-[0.87rem] text-muted">
              {resets ? `Resets ${dateTime(resets)}` : 'The reading does not say when it resets'}
            </p>
          </div>
        )
      })}
      <p className="tabular text-[0.87rem] text-muted">
        Read {ago(parseTime(reading.observed_at), now)}, at {dateTime(parseTime(reading.observed_at))}. A reading is
        taken from a session, so it is as old as the last one.
        {shared ? ' When several accounts share one session store, it belongs to whichever account ran that session.' : ''}
      </p>
    </div>
  )
}
