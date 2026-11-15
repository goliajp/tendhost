# tendhost

A control plane you host yourself: bring your machines under one dashboard,
and the coding-agent accounts on them (Claude Code, Codex, Gemini CLI, Kimi
Code) with their limits and usage. Built for one person or a small team with
up to a few dozen machines.

One server binary with an embedded store, no external database. Managed
machines run a small daemon that only makes outbound connections, so they
can sit behind NAT and the server never needs SSH access to them.

> **Status: early.** The server and the host daemon work from the command
> line and the api: first-run setup, users and tokens, enrolling hosts, their
> facts, measurements and online state, the audit log. Jobs, agent accounts,
> alerts and the web dashboard are not there yet. Nothing is released; build
> from source.

## What works today

- First-run setup: the server prints a one-time setup token, and whoever
  holds it creates the owner account
- Users with three roles (`owner`, `admin`, `viewer`), password sign-in,
  sessions
- Api tokens, each limited to a role no higher than its maker's
- Hosts: an admin opens an enrollment (a token valid for 15 minutes, good
  for one machine); `tendhostd enroll` on the machine joins it; `tendhostd
  run` then reports its facts (os, architecture, cpus, memory) and, every
  30 seconds, load, memory and disk use
- Online state: a host not heard from for 90 seconds is shown offline
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

## Add a host

Open an enrollment on the server (`$TOKEN` is an api token of an admin):

```sh
curl -s -X POST localhost:7080/api/v1/enrollments -H "Authorization: Bearer $TOKEN"
```

On the machine to add, within 15 minutes:

```sh
cargo build --release -p tendhost-hostd      # produces target/release/tendhostd
tendhostd enroll --server http://<server>:7080 --token <enrollment token>
tendhostd run
```

`enroll` writes the host's credential to `/etc/tendhost/hostd.toml` when run
as root and to `~/.config/tendhost/hostd.toml` otherwise (`--config` or
`TENDHOSTD_CONFIG` to choose), readable by its owner only. `--name` sets the
name it appears under; the default is the hostname.

`tendhostd` only makes outbound requests to the server and opens no port. A
host enrolls in one of two modes, chosen on the host: reporting only (the
default), or `--manage`, which will also let the server run commands on it
once jobs exist. The mode is kept in the host's own configuration file; the
server cannot change it.

When a host is removed on the server its daemon exits with status 78.

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
| POST | `/enrollments` | admin | open an enrollment; the token is shown once |
| GET | `/hosts` | signed in | every host with its facts |
| GET | `/hosts/{id}` | signed in | |
| PATCH | `/hosts/{id}` | admin | rename (`name`) and/or relabel (`labels`) |
| DELETE | `/hosts/{id}` | admin | remove; its credential and measurements go with it |
| GET | `/presence` | signed in | last contact and online state per host |
| GET | `/hosts/{id}/samples` | signed in | measurements, oldest first; `?since=<time>`, default the last hour; kept 7 days |
| GET | `/audit` | admin | newest first; `?before=<seq>&limit=<n>` |

`GET /healthz` answers `ok` without a credential.

## Security

- The server stores only SHA-256 fingerprints of setup tokens, sessions,
  api tokens, enrollment tokens and host credentials, and argon2id hashes
  of passwords
- A host's credential opens the host protocol only, never the api
- The session cookie is `HttpOnly` and `SameSite=Strict`
- By default the server listens on loopback only; put it behind a TLS
  reverse proxy before exposing it, and point hosts at the `https://`
  address: the host credential travels in a request header

## License

MIT or Apache-2.0, at your option.
