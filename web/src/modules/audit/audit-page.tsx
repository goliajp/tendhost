import { useState } from 'react'
import { dateTime, parseTime } from '@/kit/format'
import { useSession } from '@/kit/session'
import { Button, Loading, PageHeader, Problem } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { PAGE, auditApi, type Entry } from './api'
import { describe, reachesStart, who } from './describe'

export function AuditPage() {
  const { principal } = useSession()
  const newest = useResource('audit', () => auditApi.entries())
  // pages older than the newest one, in the order they were asked for
  const [older, setOlder] = useState<Entry[][]>([])

  const more = useAction(async (before: number) => {
    const page = await auditApi.entries(before)
    setOlder((pages) => [...pages, page])
  })
  const refresh = useAction(async () => {
    setOlder([])
    await newest.reload()
  })

  const entries = newest.data ? [...newest.data, ...older.flat()] : null
  const lastPage = older.length > 0 ? older[older.length - 1]!.length : (newest.data?.length ?? 0)
  const oldest = entries?.[entries.length - 1]
  const failure = newest.error ?? more.error ?? refresh.error
  const busy = more.busy || refresh.busy

  return (
    <>
      <PageHeader
        title="Audit log"
        lead="Every change made on this server and who made it, newest first. Entries are never edited or removed"
        actions={
          <Button disabled={busy} onClick={() => void refresh.run()}>
            Load the newest
          </Button>
        }
      />
      {failure ? (
        <div className="mb-6">
          <Problem error={failure} />
        </div>
      ) : null}
      {entries === null ? (
        newest.loading ? (
          <Loading what="the audit log" />
        ) : null
      ) : (
        <>
          <ol className="m-0 list-none divide-y divide-rule border-y border-rule p-0">
            {entries.map((entry) => (
              <li key={entry.seq} className="py-3">
                <details className="group">
                  <summary className="grid cursor-pointer list-none grid-cols-[4.5rem_1fr] gap-x-3 sm:grid-cols-[4.5rem_11.5rem_1fr]">
                    <span className="tabular text-muted">
                      <span
                        aria-hidden
                        className="mr-1.5 inline-block text-[0.65rem] transition-transform group-open:rotate-90"
                      >
                        ▶
                      </span>
                      {entry.seq}
                    </span>
                    <time dateTime={entry.at} className="tabular col-start-2 text-[0.93rem] text-muted sm:col-start-auto">
                      {dateTime(parseTime(entry.at))}
                    </time>
                    <span className="col-start-2 min-w-0 sm:col-start-auto">
                      <span className="font-medium">{who(entry.actor, principal.id)}</span>
                      <span className="text-muted">: </span>
                      {describe(entry)}
                    </span>
                  </summary>
                  <dl className="mt-3 ml-[5.25rem] grid grid-cols-[5rem_1fr] gap-x-4 gap-y-1 text-[0.87rem]">
                    <dt className="text-muted">Event</dt>
                    <dd className="m-0 font-mono break-all">{entry.topic}</dd>
                    <dt className="text-muted">Actor</dt>
                    <dd className="m-0 font-mono break-all">
                      {entry.actor.kind}
                      {'id' in entry.actor ? ` ${entry.actor.id}` : ''}
                    </dd>
                    <dt className="text-muted">Details</dt>
                    <dd className="m-0">
                      <pre className="m-0 overflow-x-auto font-mono whitespace-pre-wrap break-all">
                        {JSON.stringify(entry.payload, null, 2)}
                      </pre>
                    </dd>
                  </dl>
                </details>
              </li>
            ))}
          </ol>
          {entries.length === 0 ? <p className="mt-4 text-muted">Nothing has been recorded yet.</p> : null}
          {reachesStart(entries, lastPage, PAGE) ? (
            entries.length > 0 ? (
              <p className="mt-4 text-[0.93rem] text-muted">This is the beginning of the log.</p>
            ) : null
          ) : (
            <Button className="mt-4" disabled={busy} onClick={() => oldest && void more.run(oldest.seq)}>
              {more.busy ? 'Loading older entries' : 'Show older entries'}
            </Button>
          )}
        </>
      )}
    </>
  )
}
