// The pieces every page is built from. No page defines its own button.

import {
  useEffect,
  useId,
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from 'react'
import { Link } from 'react-router'
import { explain } from './api'

type Tone = 'primary' | 'plain' | 'danger'

const TONES: Record<Tone, string> = {
  primary: 'bg-live text-surface border-live hover:opacity-90',
  plain: 'bg-surface text-ink border-rule hover:border-muted',
  danger: 'bg-surface text-down border-down hover:bg-down-soft',
}

const CONTROL =
  'inline-flex items-center justify-center rounded-[5px] border px-3.5 py-1.5 text-[0.93rem] font-medium ' +
  'disabled:cursor-not-allowed disabled:opacity-50 transition-colors'

export function Button({
  tone = 'plain',
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { tone?: Tone }) {
  return <button type="button" {...props} className={`${CONTROL} ${TONES[tone]} ${className}`} />
}

export function LinkButton({ to, tone = 'plain', children }: { to: string; tone?: Tone; children: ReactNode }) {
  return (
    <Link to={to} className={`${CONTROL} ${TONES[tone]} no-underline`}>
      {children}
    </Link>
  )
}

const INPUT =
  'block w-full rounded-[5px] border border-rule bg-surface px-3 py-1.5 text-ink placeholder:text-muted ' +
  'aria-[invalid=true]:border-down'

/** A labelled control. `error` sits under the control it belongs to. */
export function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string
  hint?: string
  error?: string | null
  children: (props: { id: string; 'aria-invalid': boolean; 'aria-describedby': string | undefined }) => ReactNode
}) {
  const id = useId()
  const describedBy = error ? `${id}-error` : hint ? `${id}-hint` : undefined
  return (
    <div className="space-y-1">
      <label htmlFor={id} className="block text-[0.93rem] font-medium">
        {label}
      </label>
      {children({ id, 'aria-invalid': Boolean(error), 'aria-describedby': describedBy })}
      {error ? (
        <p id={`${id}-error`} role="alert" className="text-[0.87rem] text-down">
          {error}
        </p>
      ) : hint ? (
        <p id={`${id}-hint`} className="text-[0.87rem] text-muted">
          {hint}
        </p>
      ) : null}
    </div>
  )
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`${INPUT} ${props.className ?? ''}`} />
}

export function Textarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={`${INPUT} ${props.className ?? ''}`} />
}

/** A state in words, with the one mark that separates good, bad and
 *  undecided at a glance. */
export function StateTag({ tone, children }: { tone: 'live' | 'down' | 'quiet'; children: ReactNode }) {
  const mark =
    tone === 'live' ? 'bg-live border-live' : tone === 'down' ? 'bg-transparent border-down' : 'bg-transparent border-rule'
  return (
    <span className="inline-flex items-baseline gap-2">
      <span aria-hidden className={`inline-block h-2.5 w-2.5 translate-y-[1px] rounded-[2px] border-2 ${mark}`} />
      <span className={tone === 'down' ? 'text-down' : ''}>{children}</span>
    </span>
  )
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={`${INPUT} ${props.className ?? ''}`} />
}

export function Notice({ tone, children }: { tone: 'down' | 'live' | 'quiet'; children: ReactNode }) {
  const style =
    tone === 'down'
      ? 'border-down bg-down-soft'
      : tone === 'live'
        ? 'border-live bg-live-soft'
        : 'border-rule bg-surface'
  return (
    <div role={tone === 'down' ? 'alert' : 'status'} className={`rounded-[5px] border-l-4 px-4 py-3 ${style}`}>
      {children}
    </div>
  )
}

/** A failed load or action, in the server's words. */
export function Problem({ error, retry }: { error: unknown; retry?: () => void }) {
  return (
    <Notice tone="down">
      <p>{explain(error)}</p>
      {retry ? (
        <Button className="mt-2" onClick={retry}>
          Try again
        </Button>
      ) : null}
    </Notice>
  )
}

export function PageHeader({ title, lead, actions }: { title: string; lead?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="mb-8 flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
      <div className="min-w-0">
        <h1 className="text-[1.75rem] leading-tight font-semibold tracking-[-0.01em]">{title}</h1>
        {lead ? <p className="mt-1 max-w-[65ch] text-muted">{lead}</p> : null}
      </div>
      {actions ? <div className="flex flex-wrap gap-2">{actions}</div> : null}
    </header>
  )
}

/** A titled part of a page. `ruled={false}` leaves out the line under the
 *  title, for content that begins with a ruled list of its own. */
export function Section({
  title,
  aside,
  ruled = true,
  children,
}: {
  title: string
  aside?: ReactNode
  ruled?: boolean
  children: ReactNode
}) {
  return (
    <section className="mt-10 first:mt-0">
      <div
        className={`mb-3 flex flex-wrap items-baseline justify-between gap-3 ${ruled ? 'border-b border-rule pb-2' : ''}`}
      >
        <h2 className="text-[1.1rem] font-semibold">{title}</h2>
        {aside}
      </div>
      {children}
    </section>
  )
}

export function Loading({ what }: { what: string }) {
  return <p className="text-muted">Loading {what}</p>
}

/** Copies `value`; says so for two seconds. */
export function CopyButton({ value, label = 'Copy' }: { value: string; label?: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  useEffect(() => {
    if (state === 'idle') return
    const timer = setTimeout(() => setState('idle'), 2000)
    return () => clearTimeout(timer)
  }, [state])
  return (
    <Button
      onClick={() => {
        navigator.clipboard.writeText(value).then(
          () => setState('copied'),
          () => setState('failed'),
        )
      }}
    >
      {state === 'copied' ? 'Copied' : state === 'failed' ? 'Select and copy by hand' : label}
    </Button>
  )
}

/** A command or a secret, shown whole, with its copy button. */
export function Copyable({ value, label }: { value: string; label: string }) {
  return (
    <div className="flex flex-wrap items-start gap-2">
      <pre
        aria-label={label}
        className="min-w-0 flex-1 overflow-x-auto rounded-[5px] border border-rule bg-ground px-3 py-2 font-mono text-[0.87rem] leading-relaxed whitespace-pre-wrap break-all"
      >
        {value}
      </pre>
      <CopyButton value={value} />
    </div>
  )
}

/** A rule-separated list of rows; the page decides what a row holds. */
export function Rows({ children }: { children: ReactNode }) {
  return <ul className="m-0 list-none divide-y divide-rule border-y border-rule p-0">{children}</ul>
}

/** One numbered step of a wizard. The number is the order to do them in. */
export function WizardStep({ n, title, active, children }: { n: number; title: string; active: boolean; children: ReactNode }) {
  return (
    <li className="grid grid-cols-[2rem_1fr] gap-x-3" aria-current={active ? 'step' : undefined}>
      <span
        className={`tabular flex h-7 w-7 items-center justify-center rounded-full border text-[0.87rem] font-medium ${
          active ? 'border-live bg-live text-surface' : 'border-rule text-muted'
        }`}
      >
        {n}
      </span>
      <div className="min-w-0">
        <h2 className={`mb-3 text-[1.1rem] font-semibold ${active ? '' : 'text-muted'}`}>{title}</h2>
        {children}
      </div>
    </li>
  )
}

export function Forbidden() {
  return (
    <Notice tone="down">
      <p>Your role does not allow this page. Ask an admin or the owner for a higher role.</p>
    </Notice>
  )
}
