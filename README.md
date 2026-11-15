# tendhost

A control plane you host yourself: bring your machines under one dashboard,
and the coding-agent accounts on them (Claude Code, Codex, Gemini CLI, Kimi
Code) with their limits and usage. Built for one person or a small team with
up to a few dozen machines.

One server binary with an embedded store, no external database. Managed
machines run a small daemon that only makes outbound connections, so they
can sit behind NAT and the server never needs SSH access to them.

> **Status: early.** The server starts, walks first-run setup, and manages
> users, sessions, api tokens and the audit log. Hosts, jobs, agent accounts,
> alerts and the web dashboard are not there yet. Nothing is released; build
> from source.

## What works today

- First-run setup: the server prints a one-time setup token, and whoever
  holds it creates the owner account
- Users with three roles (`owner`, `admin`, `viewer`), password sign-in,
  sessions
- Api tokens, each limited to a role no higher than its maker's
- An audit log of every change, with who made it

## Run it

Requires a stable Rust toolchain.

```sh
cargo run -p tendhost-server
```

It listens on `127.0.0.1:7080`, keeps its data in `./data`, and on first
start prints the setup token:

```sh
curl -s localhost:7080/api/v1/setup \
  -H 'content-type: application/json' \
  -d '{"setup_token":"<token>","name":"you","password":"<at least 10 characters>"}' \
  -c cookies.txt

curl -s localhost:7080/api/v1/session -b cookies.txt
```

## Configuration

| variable | default | |
|---|---|---|
| `TENDHOST_LISTEN` | `127.0.0.1:7080` | address to listen on |
| `TENDHOST_DATA` | `./data` | data directory |
| `TENDHOST_PUBLIC_URL` | | the address people and hosts reach the server at, when it is behind a proxy; an `https://` value marks the session cookie `Secure` |
| `RUST_LOG` | `info` | log filter |

## API

Under `/api/v1`. Sign in with the session cookie or `Authorization: Bearer
<api token>`. Errors are `{"error": {"code", "message"}}`.

| method | path | needs | |
|---|---|---|---|
| GET | `/setup` | | `{"needed": bool}` |
| POST | `/setup` | setup token | create the owner; signs in |
| POST | `/session` | | sign in with `name` and `password` |
| GET | `/session` | signed in | the current user |
| DELETE | `/session` | | sign out |
| GET, POST | `/users` | admin | list / create (`name`, `password`, `role`) |
| DELETE | `/users/{id}` | admin | remove; an owner only by an owner, never yourself |
| GET, POST | `/tokens` | admin | list / create (`name`, `role`); the secret is shown once |
| DELETE | `/tokens/{id}` | admin | revoke |
| GET | `/audit` | admin | newest first; `?before=<time>&limit=<n>` |

`GET /healthz` answers `ok` without a credential.

## Security

- The server stores only SHA-256 fingerprints of setup tokens, sessions and
  api tokens, and argon2id hashes of passwords
- The session cookie is `HttpOnly` and `SameSite=Strict`
- By default the server listens on loopback only; put it behind a TLS
  reverse proxy before exposing it

## License

MIT or Apache-2.0, at your option.
