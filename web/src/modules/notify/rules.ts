import type { Security, Target, TargetView } from './api'

export type Kind = Target['kind']

export const KIND_NAME: Record<Kind, string> = { webhook: 'Webhook', ntfy: 'ntfy', email: 'Email' }

export const KIND_MEANING: Record<Kind, string> = {
  webhook: 'Posts {"title", "body", "source": "tendhost"} as JSON to an address of yours',
  ntfy: 'Posts to an ntfy topic, on ntfy.sh or your own server, for a push message on your phone',
  email: 'Sends a mail through an SMTP server you name',
}

export const SECURITY: Record<Security, { name: string; port: string }> = {
  tls: { name: 'TLS from the start', port: '465' },
  starttls: { name: 'STARTTLS', port: '587' },
  none: { name: 'None, for a relay on the same machine', port: '25' },
}

/** Where a channel sends, in one line, with secrets as set or not set. */
export function describe(target: TargetView): string {
  switch (target.kind) {
    case 'webhook':
      return target.url
    case 'ntfy':
      return `${target.url}, ${target.has_token ? 'with an access token' : 'no access token'}`
    case 'email': {
      const login = target.username
        ? `as ${target.username}${target.has_password ? '' : ', no password'}`
        : 'without signing in'
      return `to ${target.to} from ${target.from}, through ${target.host}:${target.port} (${SECURITY[target.security].name.toLowerCase()}), ${login}`
    }
  }
}

/** The create form: every field as typed. */
export interface Form {
  kind: Kind
  url: string
  token: string
  host: string
  port: string
  security: Security
  username: string
  password: string
  from: string
  to: string
}

export const EMPTY: Form = {
  kind: 'webhook',
  url: '',
  token: '',
  host: '',
  port: '465',
  security: 'tls',
  username: '',
  password: '',
  from: '',
  to: '',
}

const orNull = (value: string) => (value.trim() === '' ? null : value.trim())

/** The form as the target to create. An empty optional field is absent,
 *  not an empty string. The password is kept exactly as typed. */
export function toTarget(form: Form): Target {
  switch (form.kind) {
    case 'webhook':
      return { kind: 'webhook', url: form.url.trim() }
    case 'ntfy':
      return { kind: 'ntfy', url: form.url.trim(), token: orNull(form.token) }
    case 'email':
      return {
        kind: 'email',
        host: form.host.trim(),
        port: Number(form.port),
        security: form.security,
        username: orNull(form.username),
        password: form.password === '' ? null : form.password,
        from: form.from.trim(),
        to: form.to.trim(),
      }
  }
}

/** The port a security choice usually goes with; kept when the port box
 *  holds something the user chose themselves. */
export function portAfter(previous: Security, next: Security, port: string): string {
  return port === SECURITY[previous].port || port === '' ? SECURITY[next].port : port
}
