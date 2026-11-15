import { useEffect, useState } from 'react'
import { Link, useSearchParams } from 'react-router'
import { Button, LinkButton, Loading, Notice, PageHeader, Problem, WizardStep as Step } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { agentsApi, type AgentKind, type AgentView } from './api'
import { agentName, byHost, detailArrived, offers } from './rules'

const POLL_MS = 3000
/** a sync comes every 30 s; past this the host is probably not reporting */
const GIVE_UP_MS = 75_000

type Stage = { at: 'host' } | { at: 'choose'; hostId: string } | { at: 'waiting' | 'done' | 'late'; hostId: string; wanted: AgentKind[] }

export function TrackPage() {
  const [params] = useSearchParams()
  const views = useResource('agents', agentsApi.overview, POLL_MS)
  const hosts = useResource('agents:hosts', agentsApi.hosts)
  const groups = views.data && hosts.data ? byHost(views.data, hosts.data) : undefined

  const asked = params.get('host')
  const [stage, setStage] = useState<Stage>(asked ? { at: 'choose', hostId: asked } : { at: 'host' })
  const [picked, setPicked] = useState<AgentKind[] | null>(null)

  const group = 'hostId' in stage ? groups?.find((g) => g.host.id === stage.hostId) : undefined
  const chosen = picked ?? group?.agents.filter((a) => a.tracked).map((a) => a.agent) ?? []

  const save = useAction(async (hostId: string, wanted: AgentKind[]) => {
    await agentsApi.setTracked(hostId, wanted)
    await views.reload()
    setStage({ at: wanted.length === 0 ? 'done' : 'waiting', hostId, wanted })
  }, ['tracked'])

  // the wait ends when the host has answered, or after it clearly has not
  const waiting = stage.at === 'waiting' ? stage : null
  const arrived = waiting !== null && views.data !== undefined && detailArrived(views.data, waiting.hostId, waiting.wanted)
  useEffect(() => {
    if (!waiting) return
    if (arrived) {
      setStage({ ...waiting, at: 'done' })
      return
    }
    const timer = setTimeout(() => setStage({ ...waiting, at: 'late' }), GIVE_UP_MS)
    return () => clearTimeout(timer)
  }, [waiting, arrived])

  const error = views.error ?? hosts.error
  const step = stage.at === 'host' ? 1 : stage.at === 'choose' ? 2 : 3

  return (
    <>
      <p className="mb-2 text-[0.93rem]">
        <Link to="/agents" className="text-muted underline decoration-rule underline-offset-4 hover:text-ink">
          Coding agents
        </Link>
      </p>
      <PageHeader
        title="Track agents on a host"
        lead="Three steps: pick the host, choose which of the agents found there to track, and see what it reports"
      />
      {error ? (
        <div className="mb-6">
          <Problem error={error} />
        </div>
      ) : null}
      {groups === undefined ? (
        <Loading what="what the hosts found" />
      ) : (
        <ol className="m-0 max-w-[46rem] list-none space-y-10 p-0">
          <Step n={1} title="Pick the host" active={step === 1}>
            {stage.at === 'host' ? (
              groups.length === 0 ? (
                <Notice tone="quiet">
                  <p>
                    No host has reported a coding agent.{' '}
                    <Link to="/agents" className="text-ink underline underline-offset-4">
                      See why that can be
                    </Link>
                  </p>
                </Notice>
              ) : (
                <ul className="m-0 list-none space-y-2 p-0">
                  {groups.map((g) => (
                    <li key={g.host.id}>
                      <button
                        type="button"
                        onClick={() => {
                          setPicked(null)
                          setStage({ at: 'choose', hostId: g.host.id })
                        }}
                        className="w-full rounded-[5px] border border-rule px-4 py-3 text-left hover:border-muted"
                      >
                        <span className="block font-medium">{g.host.name}</span>
                        <span className="block text-[0.93rem] text-muted">
                          {g.agents.map((a) => `${agentName(a.agent)}${a.tracked ? ' (tracked)' : ''}`).join(', ')}
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              )
            ) : (
              <p className="text-muted">
                {group?.host.name ?? 'The host'}
                {stage.at === 'choose' ? (
                  <>
                    {' '}
                    <button type="button" className="underline underline-offset-4" onClick={() => setStage({ at: 'host' })}>
                      Pick another
                    </button>
                  </>
                ) : null}
              </p>
            )}
          </Step>

          <Step n={2} title="Choose the agents to track" active={step === 2}>
            {stage.at === 'choose' ? (
              group === undefined ? (
                <Notice tone="quiet">
                  <p>This host has reported no coding agent.</p>
                </Notice>
              ) : (
                <form
                  onSubmit={(event) => {
                    event.preventDefault()
                    void save.run(stage.hostId, [...chosen].sort())
                  }}
                >
                  <p className="mb-3 max-w-[65ch] text-[0.93rem] text-muted">
                    This is what {group.host.name} found. Before an agent is tracked, only its kind and its number
                    of accounts are known here.
                  </p>
                  <fieldset className="m-0 space-y-3 border-0 p-0">
                    <legend className="sr-only">Agents</legend>
                    {group.agents.map((agent) => (
                      <Choice
                        key={agent.agent}
                        agent={agent}
                        checked={chosen.includes(agent.agent)}
                        onChange={(on) =>
                          setPicked(on ? [...chosen, agent.agent] : chosen.filter((kind) => kind !== agent.agent))
                        }
                      />
                    ))}
                  </fieldset>
                  <div className="mt-4 max-w-[65ch] space-y-1 text-[0.93rem] text-muted">
                    <p>
                      tendhostd reads the session logs and settings the agent keeps on that host. It reads no
                      credential, token or key.
                    </p>
                    <p>Unticking a tracked agent deletes what was collected about it from this server.</p>
                  </div>
                  {save.error ? (
                    <div className="mt-4">
                      <Problem error={save.error} />
                    </div>
                  ) : null}
                  <Button type="submit" tone="primary" className="mt-4" disabled={save.busy}>
                    {save.busy ? 'Saving' : chosen.length === 0 ? 'Track none of them' : 'Track the ticked agents'}
                  </Button>
                </form>
              )
            ) : stage.at !== 'host' ? (
              <p className="text-muted">{stage.wanted.length === 0 ? 'None' : stage.wanted.map(agentName).join(', ')}</p>
            ) : null}
          </Step>

          <Step n={3} title="See what the host reports" active={step === 3}>
            {stage.at === 'waiting' ? (
              <p role="status" className="text-muted">
                Waiting for {group?.host.name ?? 'the host'} to answer. It does at its next sync, within 30 seconds;
                this page moves on by itself
              </p>
            ) : stage.at === 'late' ? (
              <>
                <Notice tone="down">
                  <p>
                    {group?.host.name ?? 'The host'} has not answered in over a minute. The choice is saved and it
                    will report when its daemon next syncs; check that the host is online.
                  </p>
                </Notice>
                <div className="mt-4">
                  <LinkButton to="/agents">Back to coding agents</LinkButton>
                </div>
              </>
            ) : stage.at === 'done' ? (
              <>
                <Notice tone="live">
                  {stage.wanted.length === 0 ? (
                    <p>No agent is tracked on {group?.host.name ?? 'this host'} now. What was collected is deleted.</p>
                  ) : (
                    <ul className="m-0 list-none space-y-2 p-0">
                      {stage.wanted.map((kind) => {
                        const view = group?.agents.find((a) => a.agent === kind)
                        return view ? <Arrived key={kind} view={view} /> : null
                      })}
                    </ul>
                  )}
                </Notice>
                <div className="mt-4 flex flex-wrap gap-2">
                  <LinkButton to="/agents" tone="primary">
                    Open coding agents
                  </LinkButton>
                  <Button
                    onClick={() => {
                      setPicked(null)
                      setStage({ at: 'host' })
                    }}
                  >
                    Track agents on another host
                  </Button>
                </div>
              </>
            ) : null}
          </Step>
        </ol>
      )}
    </>
  )
}

function Choice({ agent, checked, onChange }: { agent: AgentView; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className={`flex cursor-pointer gap-3 rounded-[5px] border px-4 py-3 ${checked ? 'border-live bg-surface' : 'border-rule'}`}>
      <input
        type="checkbox"
        className="mt-1.5 accent-(--live)"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span>
        <span className="block font-medium">
          {agentName(agent.agent)}
          <span className="font-normal text-muted">
            , {agent.account_count} {agent.account_count === 1 ? 'account' : 'accounts'}
            {agent.tracked ? ', tracked now' : ''}
          </span>
        </span>
        <span className="block text-[0.93rem] text-muted">{offers(agent)}.</span>
      </span>
    </label>
  )
}

function Arrived({ view }: { view: AgentView }) {
  const name = agentName(view.agent)
  const accounts = view.accounts.map((a) => (a.plan ? `${a.label} (plan ${a.plan})` : a.label)).join(', ')
  const limits = !view.supports_limits
    ? `${name} keeps no limit readings where tendhostd can read them`
    : view.limits
      ? view.limits.windows.map((w) => `${w.name} window ${Math.round(w.used_ratio * 100)}% used`).join(', ')
      : 'no limit reading yet; one appears after its next session'
  return (
    <li>
      <span className="font-medium">{name}</span>: {accounts || 'no account reported'}; {limits}
    </li>
  )
}
