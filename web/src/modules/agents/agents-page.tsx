import { Link } from 'react-router'
import { useCan } from '@/kit/session'
import { LinkButton, Loading, PageHeader, Problem, Section } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { AgentBlock } from './agent-block'
import { agentsApi, type AgentKind } from './api'
import { byHost, trackedAfter } from './rules'

const REFRESH_MS = 15_000

export function AgentsPage() {
  const canChange = useCan('admin')
  const views = useResource('agents', agentsApi.overview, REFRESH_MS)
  const hosts = useResource('agents:hosts', agentsApi.hosts, REFRESH_MS)
  const groups = views.data && hosts.data ? byHost(views.data, hosts.data) : undefined
  const error = views.error ?? hosts.error

  const stop = useAction(async (hostId: string, agent: AgentKind) => {
    await agentsApi.setTracked(hostId, trackedAfter(views.data ?? [], hostId, agent, false))
    await views.reload()
  })

  return (
    <>
      <PageHeader
        title="Coding agents"
        lead="Claude Code, Codex, Gemini CLI and Kimi Code on your hosts: their accounts, how much of each limit is used, and tokens per day"
        actions={
          canChange && groups && groups.length > 0 ? (
            <LinkButton to="/agents/track" tone="primary">
              Track agents on a host
            </LinkButton>
          ) : undefined
        }
      />
      {error ? (
        <div className="mb-6">
          <Problem error={error} retry={() => void Promise.all([views.reload(), hosts.reload()])} />
        </div>
      ) : null}
      {stop.error ? (
        <div className="mb-6">
          <Problem error={stop.error} />
        </div>
      ) : null}
      {groups === undefined ? (
        views.loading || hosts.loading ? (
          <Loading what="agents" />
        ) : null
      ) : groups.length === 0 ? (
        <NothingFound anyHost={(hosts.data?.length ?? 0) > 0} />
      ) : (
        <>
          <p className="mb-8 max-w-[70ch] text-[0.93rem] text-muted">
            tendhostd reads the files the agents themselves keep on the host: session logs and settings. It reads
            no credential, token or key, and sends none. Until an agent is tracked, this server knows only that it
            is installed and how many accounts it has.
          </p>
          {groups.map((group) => (
            <Section
              key={group.host.id}
              title={group.host.name}
              aside={
                <Link
                  to={`/hosts/${group.host.id}`}
                  className="text-[0.87rem] text-muted underline decoration-rule underline-offset-4 hover:text-ink"
                >
                  Open the host
                </Link>
              }
            >
              <div className="divide-y divide-rule">
                {group.agents.map((view) => (
                  <AgentBlock
                    key={view.agent}
                    view={view}
                    canChange={canChange}
                    busy={stop.busy}
                    onStop={() => void stop.run(view.host_id, view.agent)}
                  />
                ))}
              </div>
            </Section>
          ))}
        </>
      )}
    </>
  )
}

function NothingFound({ anyHost }: { anyHost: boolean }) {
  return (
    <div className="max-w-[65ch] border-l-4 border-rule py-1 pl-5">
      <p className="text-[1.1rem] font-medium">
        {anyHost ? 'No coding agent was found on any host' : 'No host is enrolled yet'}
      </p>
      {anyHost ? (
        <div className="mt-1 space-y-2 text-muted">
          <p>
            tendhostd looks for Claude Code, Codex, Gemini CLI and Kimi Code in the home directory of the user it
            runs as, or in every home directory when it runs as root. An agent used by another user than the
            daemon’s is not seen.
          </p>
          <p>
            A host enrolled with <code className="font-mono text-[0.87rem]">--no-agents</code> tells this server
            nothing about agents. A host reports what it finds at its next sync, within 30 seconds of starting.
          </p>
        </div>
      ) : (
        <>
          <p className="mt-1 mb-4 text-muted">
            Agents are found on hosts. Add the machine you use them on first; it reports what it finds by itself.
          </p>
          <LinkButton to="/hosts/add" tone="primary">
            Add a host
          </LinkButton>
        </>
      )}
    </div>
  )
}
