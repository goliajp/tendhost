# tendhost

A control plane you host yourself: bring your machines under one dashboard,
and the coding agents on them (Claude Code, Codex, Gemini CLI, Kimi Code)
with their accounts, usage and limits. Built for one person or a small team
with up to a few dozen machines.

- **One server binary**, with an embedded store and the dashboard inside
  it. No external database
- **Hosts only connect outward.** Each managed machine runs a small daemon,
  `tendhostd`, that reports to the server and opens no port. Machines can
  sit behind NAT, and the server never needs SSH access to them
- **The host decides what the server may do.** A host enrolls either to
  report only, or to also run commands; the choice is kept on the host and
  the server cannot change it
- **No credentials are read.** For coding agents, `tendhostd` reads the
  session logs and account metadata the agents keep for themselves, never
  a login token or a keychain

> **Status: early (0.1).** Everything described here works and is tested,
> but the project is young: expect rough edges, and read
> [Security](#security) before exposing a server to the internet.

## What it does

- **Hosts**: a guided wizard gives a new machine one command to run; the
  host then appears with its system, CPUs and memory, shows online or
  offline, and charts load, memory and disk use (kept 7 days)
- **Jobs**: shell commands on hosts that allow them, by hand, once at a
  time, or on a cron schedule, with every run's state, exit code and output
- **Coding agents**: which agents are installed on which host and with how
  many accounts; for the ones you choose to track, the signed-in account,
  token usage per day and model, and limit windows where the agent keeps
  them locally
- **Alerts**: a host gone silent, a disk nearly full, an agent near its
  limit, a failed job; sent to a webhook, an ntfy topic or by email, when
  the condition begins and when it ends
- **Access**: an owner, admins and viewers; api tokens; an audit log of
  every change with who made it
- **A command line**, `tendhost`, for all of the above

## Run the server

With Docker:

```sh
docker run -d --name tendhost -p 7080:7080 -v tendhost-data:/data ghcr.io/goliajp/tendhost:latest
docker logs tendhost        # shows the setup token
```

Or download `tendhost-server-<platform>` from the
[releases](https://github.com/goliajp/tendhost/releases) and run it:

```sh
TENDHOST_LISTEN=0.0.0.0:7080 ./tendhost-server
```

Open `http://<server>:7080`. The first visit asks for the **setup token**
the server printed when it started, and creates the owner account. The
token works once.

Hosts must be able to reach the server at the address you open the
dashboard with. Behind a reverse proxy, set `TENDHOST_PUBLIC_URL` to the
public address.

### Configuration

| variable | default | |
|---|---|---|
| `TENDHOST_LISTEN` | `127.0.0.1:7080` (`0.0.0.0:7080` in the image) | address to listen on |
| `TENDHOST_DATA` | `./data` (`/data` in the image) | data directory; back this up |
| `TENDHOST_PUBLIC_URL` | | the address people and hosts reach the server at, when it differs from the listen address; an `https://` value marks the session cookie `Secure` |
| `RUST_LOG` | `info` | log filter |

## Add a host

In the dashboard, **Add host** walks through it. It comes down to one
command on the machine (Linux or macOS), valid for 15 minutes and for one
machine:

```sh
curl -fsSL https://<server>/install.sh | sh -s -- --token <enrollment token>
```

The script downloads `tendhostd` for the machine from this project's
releases, checks it against its published checksum, enrolls the machine,
and starts the daemon as a service (systemd on Linux, launchd on macOS).

Options after `--token <token>`:

| option | |
|---|---|
| `--manage` | also let the server run commands on this host; without it the host only reports |
| `--no-agents` | tell the server nothing about the coding agents on this host |
| `--name <name>` | the name to appear under; default: the hostname |

By hand, with the binary already on the machine:

```sh
tendhostd enroll --server https://<server> --token <enrollment token> [--manage]
tendhostd install      # run as a service from now on; or `tendhostd run` in a terminal
```

`enroll` writes the host's credential to `/etc/tendhost/hostd.toml` when
run as root and to `~/.config/tendhost/hostd.toml` otherwise (`--config` or
`TENDHOSTD_CONFIG` to choose), readable by its owner only. `tendhostd
uninstall` removes the service and leaves the host enrolled.

On Linux `install` needs root and creates `tendhostd.service`. On macOS it
creates a LaunchDaemon when run as root and a LaunchAgent of the current
user otherwise.

What a host does afterwards:

- every 30 seconds it sends load, memory and disk use, and collects the
  commands waiting for it
- what it could not send is kept, on disk across restarts, and sent when
  the server is reachable again (up to a day's worth)
- when the host is removed on the server, the daemon exits with status 78
  and its service is not restarted

## Jobs

A job is a shell command on one host that enrolled with `--manage`. It runs
with `sh -c`, as the user the daemon runs as (root for a system service),
in its own process group; at the timeout the whole group is killed.

Schedules: manual, once at a time, or five cron fields (minute, hour, day
of month, month, day of week) in UTC. A job whose time passed while the
server was down runs once when it is back, not once per missed occurrence.

Each run ends in one state:

| state | |
|---|---|
| `succeeded` / `failed` | the command exited zero / non-zero |
| `timed_out` | the host stopped it at the job's timeout |
| `missed` | the host did not collect it within 6 hours |
| `lost` | the host started it and never reported how it ended |

The last 64 KB of output (stdout and stderr together) and the newest 200
runs of each job are kept.

## Coding agents

`tendhostd` looks for these agents in the home of the user it runs as, or
in every home when it runs as root, including the several-accounts layout
(`~/.claude-profile-2` and so on):

| agent | directory | account name | usage | limits |
|---|---|---|---|---|
| Claude Code | `~/.claude` | address and plan | tokens per day and model | not kept locally by the agent |
| Codex | `~/.codex` | directory name | tokens per day and model | the windows the service last reported, with the plan |
| Gemini CLI | `~/.gemini` | address, when signed in with Google | not read yet | not read yet |
| Kimi Code | `~/.kimi-code` | directory name | not read yet | not read yet |

Until you choose to track an agent on a host, the server learns only that
it is installed and with how many accounts. Tracking is chosen per host and
agent in the dashboard (**Agents**) or with `tendhost track`; ending it
deletes what was collected.

What is read: session logs (`projects/` for Claude Code, `sessions/` for
Codex) and the agent's own account metadata. What is never read: login
tokens, credential files, keychains. Nothing is sent anywhere but your
server.

When several accounts of one agent share a session store, usage is counted
per agent on the host, and a limit reading belongs to whichever account ran
the latest session. Usage is kept 90 days.

## Alerts

| condition | opens when | resolves when |
|---|---|---|
| host offline | no contact for 90 seconds | the host is heard from |
| disk usage | a disk reaches the level (default 90%) | it falls below |
| agent limit | a limit window reaches the level (default 90%) | a later reading is below |
| job failed | a run ends other than `succeeded` | the job next succeeds |

Each condition raises one alert, not one per reading. Levels and which
conditions are on are set under **Alerts**. Alerts go out over every
channel under **Channels**: a webhook (`POST` of `{"title", "body",
"source": "tendhost"}`), an [ntfy](https://ntfy.sh) topic, or email through
an SMTP server. Each channel has a test button, and every delivery is
recorded with its result.

## Command line

`tendhost-<platform>` from the releases, or `tendhost` inside the image.
Create an api token in the dashboard, then:

```sh
tendhost login --server https://<server> --token <api token>

tendhost hosts
tendhost enroll --manage                 # prints the command for a new host
tendhost host show <name>
tendhost job add nightly --host <name> --command 'backup.sh' --cron '0 3 * * *'
tendhost job run nightly
tendhost job output nightly
tendhost agents
tendhost track <host> claude codex
tendhost alerts
tendhost audit
```

`--json` prints the server's json instead of a table. Hosts and jobs are
named by name or id.

## API

Under `/api/v1`. Sign in with the session cookie or `Authorization: Bearer
<api token>`. Errors are `{"error": {"code", "message"}}`. "admin" below
means admin or owner; "signed in" means any role.

| method | path | needs | |
|---|---|---|---|
| GET | `/setup` | | `{"needed": bool}` |
| POST | `/setup` | setup token | create the owner; signs in |
| POST | `/session` | | sign in with `name` and `password`; five wrong passwords for a name within 15 minutes lock it for 15 |
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
| DELETE | `/hosts/{id}` | admin | remove, with its credential, measurements, jobs, agent data and alerts |
| GET | `/presence` | signed in | last contact and online state per host |
| GET | `/hosts/{id}/samples` | signed in | measurements, oldest first; `?since=<time>` (default the last hour), `?points=<n>` (default 300: averaged over equal stretches; 0 for all) |
| GET, POST | `/jobs` | signed in / admin | list (`?host=<id>`) / create (`name`, `host_id`, `command`, `timeout_secs`, `schedule`) |
| GET, DELETE | `/jobs/{id}` | signed in / admin | |
| POST | `/jobs/{id}/enable`, `/disable` | admin | |
| GET, POST | `/jobs/{id}/runs` | signed in / admin | runs, newest first (`?limit=`) / run now |
| GET | `/agents` | signed in | every agent on every host |
| PUT | `/hosts/{id}/agents` | admin | `{"tracked": ["claude", …]}` |
| GET | `/hosts/{id}/agents/{agent}/usage` | signed in | per day and model; `?days=<n>` (default 30) |
| GET | `/alerts` | signed in | open alerts; `?state=all` for history |
| GET, PUT | `/alerts/settings` | signed in / admin | |
| GET, POST | `/channels` | admin | list / create (`name`, `target`); secrets are never returned |
| DELETE | `/channels/{id}` | admin | |
| POST | `/channels/{id}/test` | admin | send a test message; returns the delivery |
| GET | `/deliveries` | admin | the last 100 deliveries |
| GET | `/audit` | admin | newest first; `?before=<seq>&limit=<n>` |

A schedule is `{"kind": "manual"}`, `{"kind": "once", "at": "<time>"}` or
`{"kind": "cron", "expr": "0 3 * * *"}`. A channel target is `{"kind":
"webhook", "url"}`, `{"kind": "ntfy", "url", "token"}` or `{"kind":
"email", "host", "port", "security": "tls" | "starttls" | "none",
"username", "password", "from", "to"}`.

Outside the api: `GET /healthz` answers `ok`; `GET /install.sh` is the host
install script; `/hostd/v1/*` is the protocol `tendhostd` speaks.

## Security

- The server keeps only SHA-256 fingerprints of setup tokens, sessions,
  api tokens, enrollment tokens and host credentials, and argon2id hashes
  of passwords
- Channel secrets (an ntfy token, an SMTP password) are stored in the data
  directory as given and are never returned by the api. Protect the data
  directory
- The session cookie is `HttpOnly` and `SameSite=Strict`
- A host's credential opens the host protocol only, never the api
- Whoever is an admin can run commands as the daemon's user on every host
  enrolled with `--manage`. Enroll hosts to report only unless you need
  jobs on them
- The server speaks plain http. Put it behind a TLS reverse proxy before
  exposing it beyond a trusted network, and point hosts at the `https://`
  address: sessions, api tokens and host credentials travel in request
  headers
- The install script checks the downloaded daemon against the checksum
  published with the release

## Build from source

Requires a stable Rust toolchain and [bun](https://bun.sh).

```sh
(cd web && bun install && bun run build)
cargo build --release -p tendhost-server --features tendhost-server/web -p tendhost-hostd -p tendhost-cli
```

Without `--features tendhost-server/web` the server is the api alone.
`docker build -t tendhost .` builds the image, and `docker build --target
artifacts --output type=local,dest=out .` the static Linux binaries.

## License

MIT or Apache-2.0, at your option.
