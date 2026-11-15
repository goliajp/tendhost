import { Link } from 'react-router'
import { bytes, count, text } from '@/kit/format'
import { Slot } from '@/kit/module'
import { useCan } from '@/kit/session'
import { LinkButton, Loading, PageHeader, Problem } from '@/kit/ui'
import { useResource } from '@/kit/use-resource'
import { hostsApi, type Host } from './api'
import { byName } from './rules'

const REFRESH_MS = 10_000

export function HostsPage() {
  const canAdd = useCan('admin')
  const hosts = useResource('hosts', hostsApi.list, REFRESH_MS)
  const sorted = hosts.data ? [...hosts.data].sort(byName) : undefined

  return (
    <>
      <PageHeader
        title="Hosts"
        lead={sorted && sorted.length > 0 ? <Slot name="hosts.summary" hostIds={sorted.map((h) => h.id)} /> : undefined}
        actions={canAdd && sorted && sorted.length > 0 ? <LinkButton to="/hosts/add" tone="primary">Add a host</LinkButton> : undefined}
      />
      {hosts.error ? (
        <div className="mb-6">
          <Problem error={hosts.error} retry={hosts.reload} />
        </div>
      ) : null}
      {sorted === undefined ? (
        hosts.loading ? (
          <Loading what="hosts" />
        ) : null
      ) : sorted.length === 0 ? (
        <NoHosts canAdd={canAdd} />
      ) : (
        <HostTable hosts={sorted} />
      )}
    </>
  )
}

function NoHosts({ canAdd }: { canAdd: boolean }) {
  return (
    <div className="max-w-[65ch] border-l-4 border-rule py-1 pl-5">
      <p className="text-[1.1rem] font-medium">No machine reports to this server yet</p>
      {canAdd ? (
        <>
          <p className="mt-1 mb-4 text-muted">
            Adding one takes two commands on that machine. It only makes outbound connections to this server, so it
            can sit behind NAT and needs no open port.
          </p>
          <LinkButton to="/hosts/add" tone="primary">
            Add the first host
          </LinkButton>
        </>
      ) : (
        <p className="mt-1 text-muted">An admin or the owner can add one.</p>
      )}
    </div>
  )
}

// one grid for the header and every row, so columns line up; below `md`
// a row stacks and the header is dropped
const GRID = 'md:grid md:grid-cols-[minmax(11rem,1.1fr)_minmax(10rem,1.6fr)_minmax(9rem,1.2fr)_4rem_6rem] md:gap-x-6'

function HostTable({ hosts }: { hosts: Host[] }) {
  return (
    <div role="table" aria-label="Hosts" className="border-y border-rule">
      <div role="row" className={`${GRID} hidden border-b border-rule py-2 text-[0.87rem] text-muted`}>
        <span role="columnheader">State</span>
        <span role="columnheader">Name</span>
        <span role="columnheader">System</span>
        <span role="columnheader" className="text-right">
          CPUs
        </span>
        <span role="columnheader" className="text-right">
          Memory
        </span>
      </div>
      <div role="rowgroup" className="divide-y divide-rule">
        {hosts.map((host) => (
          <div role="row" key={host.id} className={`${GRID} items-baseline py-3`}>
            <div role="cell" className="text-[0.93rem]">
              <Slot name="host.state" hostId={host.id} />
            </div>
            <div role="cell" className="min-w-0">
              <Link to={`/hosts/${host.id}`} className="font-medium text-ink underline decoration-rule underline-offset-4 hover:decoration-ink">
                {host.name}
              </Link>
              {host.labels.length > 0 ? (
                <span className="ml-2 text-[0.87rem] text-muted">{host.labels.join(', ')}</span>
              ) : null}
            </div>
            <div role="cell" className="text-[0.93rem] text-muted">
              {text(host.facts.os)} {host.facts.os_version}, {text(host.facts.arch)}
            </div>
            <div role="cell" className="tabular text-[0.93rem] md:text-right">
              <span className="text-muted md:hidden">CPUs </span>
              {count(host.facts.cpu_count)}
            </div>
            <div role="cell" className="tabular text-[0.93rem] md:text-right">
              <span className="text-muted md:hidden">Memory </span>
              {bytes(host.facts.memory_bytes)}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
