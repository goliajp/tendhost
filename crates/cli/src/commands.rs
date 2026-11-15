use std::fmt::Write;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use reqwest::Method;
use serde_json::{Value, json};

use crate::api::Api;
use crate::table::{bytes, render};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List hosts with their state
    Hosts,
    /// Work with one host
    #[command(subcommand)]
    Host(HostCommand),
    /// Open an enrollment and print the commands to run on the new host
    Enroll {
        /// Also let the server run commands on the host
        #[arg(long)]
        manage: bool,
    },
    /// List jobs
    Jobs,
    /// Work with one job
    #[command(subcommand)]
    Job(JobCommand),
    /// List the coding agents found on hosts
    Agents,
    /// Choose which agents to track on a host; none ends all tracking there
    Track { host: String, agents: Vec<String> },
    /// List open alerts
    Alerts {
        /// Include resolved alerts
        #[arg(long)]
        all: bool,
    },
    /// Show the audit log, newest first
    Audit {
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
pub enum HostCommand {
    /// Show one host
    Show { host: String },
    /// Rename a host
    Rename { host: String, name: String },
    /// Replace a host's labels
    Label { host: String, labels: Vec<String> },
    /// Remove a host and everything kept about it
    Rm { host: String },
}

#[derive(Subcommand, Debug)]
pub enum JobCommand {
    /// Create a job
    Add {
        /// A name for the job
        name: String,
        /// The host to run on, by name or id
        #[arg(long)]
        host: String,
        /// The shell command
        #[arg(long)]
        command: String,
        /// Run on a schedule: five cron fields, UTC
        #[arg(long, conflicts_with = "at")]
        cron: Option<String>,
        /// Run once at this time (RFC 3339)
        #[arg(long)]
        at: Option<String>,
        /// Stop the command after this many seconds
        #[arg(long, default_value_t = 3600)]
        timeout: u32,
    },
    /// Run a job now
    Run {
        job: String,
    },
    /// Show a job's runs, newest first
    Runs {
        job: String,
    },
    /// Show what a job's latest run printed
    Output {
        job: String,
    },
    Enable {
        job: String,
    },
    Disable {
        job: String,
    },
    Rm {
        job: String,
    },
}

/// Runs one command and writes what it has to say to `out`. With `raw`,
/// that is the server's json.
pub async fn run(api: &Api, command: Command, raw: bool, out: &mut String) -> Result<()> {
    let shown = match command {
        Command::Hosts => {
            let (hosts, presence) = (api.get("/hosts").await?, api.get("/presence").await?);
            Shown::table(
                hosts_table(&hosts, &presence),
                json!({ "hosts": hosts, "presence": presence }),
            )
        }
        Command::Host(HostCommand::Show { host }) => {
            let host = find(api, "/hosts", &host, "host").await?;
            Shown::table(host_detail(&host), host)
        }
        Command::Host(HostCommand::Rename { host, name }) => {
            let id = id_of(api, "/hosts", &host, "host").await?;
            let host = api
                .send(
                    Method::PATCH,
                    &format!("/hosts/{id}"),
                    json!({ "name": name }),
                )
                .await?;
            Shown::line(format!("Renamed to {}.", text(&host["name"])), host)
        }
        Command::Host(HostCommand::Label { host, labels }) => {
            let id = id_of(api, "/hosts", &host, "host").await?;
            let host = api
                .send(
                    Method::PATCH,
                    &format!("/hosts/{id}"),
                    json!({ "labels": labels }),
                )
                .await?;
            Shown::line(format!("Labels: {}.", list(&host["labels"])), host)
        }
        Command::Host(HostCommand::Rm { host }) => {
            let found = find(api, "/hosts", &host, "host").await?;
            api.delete(&format!("/hosts/{}", text(&found["id"])))
                .await?;
            Shown::line(format!("Removed {}.", text(&found["name"])), Value::Null)
        }
        Command::Enroll { manage } => {
            let opened = api.send(Method::POST, "/enrollments", json!({})).await?;
            Shown::line(
                enroll_help(
                    api.server(),
                    text(&opened["token"]),
                    text(&opened["expires_at"]),
                    manage,
                ),
                opened,
            )
        }
        Command::Jobs => {
            let (jobs, hosts) = (api.get("/jobs").await?, api.get("/hosts").await?);
            Shown::table(jobs_table(&jobs, &hosts), jobs)
        }
        Command::Job(JobCommand::Add {
            name,
            host,
            command,
            cron,
            at,
            timeout,
        }) => {
            let host_id = id_of(api, "/hosts", &host, "host").await?;
            let schedule = match (cron, at) {
                (Some(expr), _) => json!({ "kind": "cron", "expr": expr }),
                (None, Some(at)) => json!({ "kind": "once", "at": at }),
                (None, None) => json!({ "kind": "manual" }),
            };
            let body = json!({ "name": name, "host_id": host_id, "command": command, "timeout_secs": timeout, "schedule": schedule });
            let job = api.send(Method::POST, "/jobs", body).await?;
            Shown::line(
                format!("Created job {} ({}).", text(&job["name"]), text(&job["id"])),
                job,
            )
        }
        Command::Job(JobCommand::Run { job }) => {
            let id = id_of(api, "/jobs", &job, "job").await?;
            let run = api
                .send(Method::POST, &format!("/jobs/{id}/runs"), json!({}))
                .await?;
            Shown::line("Queued. The host starts it at its next sync.".into(), run)
        }
        Command::Job(JobCommand::Runs { job }) => {
            let id = id_of(api, "/jobs", &job, "job").await?;
            let runs = api.get(&format!("/jobs/{id}/runs")).await?;
            Shown::table(runs_table(&runs), runs)
        }
        Command::Job(JobCommand::Output { job }) => {
            let id = id_of(api, "/jobs", &job, "job").await?;
            let runs = api.get(&format!("/jobs/{id}/runs?limit=1")).await?;
            let latest = runs.get(0).cloned().context("this job has not run yet")?;
            Shown::line(text(&latest["output"]).trim_end().to_string(), latest)
        }
        Command::Job(JobCommand::Enable { job }) => toggle(api, &job, "enable").await?,
        Command::Job(JobCommand::Disable { job }) => toggle(api, &job, "disable").await?,
        Command::Job(JobCommand::Rm { job }) => {
            let found = find(api, "/jobs", &job, "job").await?;
            api.delete(&format!("/jobs/{}", text(&found["id"]))).await?;
            Shown::line(
                format!("Removed job {}.", text(&found["name"])),
                Value::Null,
            )
        }
        Command::Agents => {
            let (agents, hosts) = (api.get("/agents").await?, api.get("/hosts").await?);
            Shown::table(agents_table(&agents, &hosts), agents)
        }
        Command::Track { host, agents } => {
            let id = id_of(api, "/hosts", &host, "host").await?;
            let saved = api
                .send(
                    Method::PUT,
                    &format!("/hosts/{id}/agents"),
                    json!({ "tracked": agents }),
                )
                .await?;
            let sentence = match saved["tracked"].as_array().is_some_and(|t| !t.is_empty()) {
                true => format!(
                    "Tracking {} on {host}. Detail arrives with the host's next sync.",
                    list(&saved["tracked"])
                ),
                false => format!("No agent is tracked on {host}; what was collected is deleted."),
            };
            Shown::line(sentence, saved)
        }
        Command::Alerts { all } => {
            let alerts = api
                .get(if all { "/alerts?state=all" } else { "/alerts" })
                .await?;
            let rows: Vec<_> = items(&alerts)
                .map(|a| {
                    vec![
                        opt(&a["opened_at"]).map(short_time),
                        opt(&a["resolved_at"]).map(short_time),
                        opt(&a["summary"]),
                    ]
                })
                .collect();
            let table = if rows.is_empty() {
                "No open alerts.\n".into()
            } else {
                render(&["OPENED", "RESOLVED", "ALERT"], &rows)
            };
            Shown::table(table, alerts)
        }
        Command::Audit { limit } => {
            let entries = api.get(&format!("/audit?limit={limit}")).await?;
            let rows: Vec<_> = items(&entries)
                .map(|e| {
                    vec![
                        opt(&e["at"]).map(short_time),
                        Some(actor(&e["actor"])),
                        opt(&e["topic"]),
                        Some(e["payload"].to_string()),
                    ]
                })
                .collect();
            Shown::table(render(&["WHEN", "WHO", "WHAT", "DETAIL"], &rows), entries)
        }
    };
    match raw {
        true => writeln!(out, "{}", serde_json::to_string_pretty(&shown.json)?)?,
        false => {
            out.push_str(&shown.text);
            if !shown.text.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    Ok(())
}

struct Shown {
    text: String,
    json: Value,
}

impl Shown {
    fn table(text: String, json: Value) -> Self {
        Self { text, json }
    }

    fn line(text: String, json: Value) -> Self {
        Self { text, json }
    }
}

async fn toggle(api: &Api, job: &str, action: &str) -> Result<Shown> {
    let id = id_of(api, "/jobs", job, "job").await?;
    let job = api
        .send(Method::POST, &format!("/jobs/{id}/{action}"), json!({}))
        .await?;
    Ok(Shown::line(
        format!("Job {} is {action}d.", text(&job["name"])),
        job,
    ))
}

/// The item of a list whose id or name is `wanted`. A name shared by
/// several items is refused: the id says which.
async fn find(api: &Api, list_path: &str, wanted: &str, kind: &str) -> Result<Value> {
    let all = api.get(list_path).await?;
    pick(&all, wanted, kind)
}

async fn id_of(api: &Api, list_path: &str, wanted: &str, kind: &str) -> Result<String> {
    Ok(text(&find(api, list_path, wanted, kind).await?["id"]).to_string())
}

fn pick(all: &Value, wanted: &str, kind: &str) -> Result<Value> {
    if let Some(by_id) = items(all).find(|item| item["id"] == wanted) {
        return Ok(by_id.clone());
    }
    let named: Vec<&Value> = items(all).filter(|item| item["name"] == wanted).collect();
    match named.as_slice() {
        [one] => Ok((*one).clone()),
        [] => bail!("there is no {kind} called {wanted}"),
        many => {
            let ids: Vec<&str> = many.iter().map(|item| text(&item["id"])).collect();
            bail!(
                "{} {kind}s are called {wanted}; use the id: {}",
                many.len(),
                ids.join(", ")
            )
        }
    }
}

fn hosts_table(hosts: &Value, presence: &Value) -> String {
    let rows: Vec<_> = items(hosts)
        .map(|host| {
            let seen = items(presence).find(|p| p["host_id"] == host["id"]);
            let state = seen.map(|p| {
                if p["online"] == true {
                    "online"
                } else {
                    "offline"
                }
                .to_string()
            });
            vec![
                opt(&host["name"]),
                state,
                opt(&host["mode"]),
                opt(&host["facts"]["os"]),
                opt(&host["facts"]["arch"]),
                host["facts"]["cpu_count"].as_u64().map(|n| n.to_string()),
                host["facts"]["memory_bytes"].as_u64().map(bytes),
                Some(list(&host["labels"])),
            ]
        })
        .collect();
    if rows.is_empty() {
        return "No hosts yet. Add one with: tendhost enroll\n".into();
    }
    render(
        &[
            "NAME", "STATE", "MODE", "OS", "ARCH", "CPUS", "MEMORY", "LABELS",
        ],
        &rows,
    )
}

fn host_detail(host: &Value) -> String {
    let facts = &host["facts"];
    let rows = vec![
        vec![Some("id".into()), opt(&host["id"])],
        vec![Some("name".into()), opt(&host["name"])],
        vec![Some("mode".into()), opt(&host["mode"])],
        vec![Some("labels".into()), Some(list(&host["labels"]))],
        vec![
            Some("enrolled".into()),
            opt(&host["enrolled_at"]).map(short_time),
        ],
        vec![Some("hostname".into()), opt(&facts["hostname"])],
        vec![
            Some("system".into()),
            Some(format!(
                "{} {} ({})",
                text(&facts["os"]),
                text(&facts["os_version"]),
                text(&facts["arch"])
            )),
        ],
        vec![
            Some("cpus".into()),
            facts["cpu_count"].as_u64().map(|n| n.to_string()),
        ],
        vec![
            Some("memory".into()),
            facts["memory_bytes"].as_u64().map(bytes),
        ],
        vec![Some("daemon".into()), opt(&facts["daemon_version"])],
    ];
    render(&["FIELD", "VALUE"], &rows)
}

fn jobs_table(jobs: &Value, hosts: &Value) -> String {
    let rows: Vec<_> = items(jobs)
        .map(|job| {
            let schedule = match text(&job["schedule"]["kind"]) {
                "cron" => text(&job["schedule"]["expr"]).to_string(),
                "once" => format!(
                    "once {}",
                    short_time(text(&job["schedule"]["at"]).to_string())
                ),
                _ => "manual".to_string(),
            };
            vec![
                opt(&job["name"]),
                Some(host_name(hosts, &job["host_id"])),
                Some(schedule),
                Some(if job["enabled"] == true { "yes" } else { "no" }.to_string()),
                opt(&job["next_run_at"]).map(short_time),
                opt(&job["id"]),
            ]
        })
        .collect();
    if rows.is_empty() {
        return "No jobs yet. Create one with: tendhost job add\n".into();
    }
    render(
        &["NAME", "HOST", "SCHEDULE", "ENABLED", "NEXT RUN", "ID"],
        &rows,
    )
}

fn runs_table(runs: &Value) -> String {
    let rows: Vec<_> = items(runs)
        .map(|run| {
            vec![
                opt(&run["due_at"]).map(short_time),
                opt(&run["state"]),
                run["exit_code"].as_i64().map(|code| code.to_string()),
                opt(&run["finished_at"]).map(short_time),
            ]
        })
        .collect();
    if rows.is_empty() {
        return "This job has not run yet.\n".into();
    }
    render(&["ASKED", "STATE", "EXIT", "FINISHED"], &rows)
}

fn agents_table(agents: &Value, hosts: &Value) -> String {
    let rows: Vec<_> = items(agents)
        .map(|agent| {
            let labels: Vec<&str> = items(&agent["accounts"])
                .map(|a| text(&a["label"]))
                .collect();
            let windows: Vec<String> = items(&agent["limits"]["windows"])
                .map(|w| {
                    format!(
                        "{} {:.0}%",
                        text(&w["name"]),
                        w["used_ratio"].as_f64().unwrap_or(0.0) * 100.0
                    )
                })
                .collect();
            let limits = match (agent["tracked"] == true, agent["supports_limits"] == true) {
                (false, _) => None,
                (true, false) => Some("not kept by this agent".to_string()),
                (true, true) => Some(windows.join(", ")).filter(|w| !w.is_empty()),
            };
            vec![
                Some(host_name(hosts, &agent["host_id"])),
                opt(&agent["agent"]),
                agent["account_count"].as_u64().map(|n| n.to_string()),
                Some(
                    if agent["tracked"] == true {
                        "yes"
                    } else {
                        "no"
                    }
                    .to_string(),
                ),
                Some(labels.join(", ")),
                limits,
            ]
        })
        .collect();
    if rows.is_empty() {
        return "No coding agents were found on any host.\n".into();
    }
    render(
        &[
            "HOST",
            "AGENT",
            "ACCOUNTS",
            "TRACKED",
            "SIGNED IN AS",
            "LIMITS",
        ],
        &rows,
    )
}

fn enroll_help(server: &str, token: &str, expires_at: &str, manage: bool) -> String {
    let flag = if manage { " --manage" } else { "" };
    format!(
        "On the machine to add, before {}:\n\n    curl -fsSL {server}/install.sh | sh -s -- --token {token}{flag}\n\n\
         That installs tendhostd, enrolls the machine and starts it as a service.\n\
         With the binary already there: tendhostd enroll --server {server} --token {token}{flag}",
        short_time(expires_at.to_string())
    )
}

fn items(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn opt(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

fn list(value: &Value) -> String {
    items(value).map(text).collect::<Vec<_>>().join(", ")
}

fn host_name(hosts: &Value, id: &Value) -> String {
    items(hosts)
        .find(|h| h["id"] == *id)
        .map_or_else(|| text(id).to_string(), |h| text(&h["name"]).to_string())
}

fn actor(value: &Value) -> String {
    match text(&value["kind"]) {
        "system" => "system".to_string(),
        kind => format!("{kind} {}", text(&value["id"])),
    }
}

/// `2026-01-01 12:00` from an RFC 3339 time, in UTC.
fn short_time(rfc3339: String) -> String {
    chrono::DateTime::parse_from_rfc3339(&rfc3339)
        .map(|t| {
            t.with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or(rfc3339)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_is_found_by_id_or_by_a_name_only_one_has() {
        let all = json!([
            { "id": "host_1", "name": "web" },
            { "id": "host_2", "name": "db" },
            { "id": "host_3", "name": "db" },
            { "id": "host_4", "name": "host_1" },
        ]);
        assert_eq!(pick(&all, "web", "host").unwrap()["id"], "host_1");
        assert_eq!(pick(&all, "host_1", "host").unwrap()["name"], "web");
        let twice = pick(&all, "db", "host").unwrap_err().to_string();
        assert!(twice.contains("host_2, host_3"), "{twice}");
        assert!(
            pick(&all, "cache", "host")
                .unwrap_err()
                .to_string()
                .contains("no host called cache")
        );
    }

    #[test]
    fn times_print_short_and_in_utc() {
        assert_eq!(
            short_time("2026-01-01T21:05:09+09:00".into()),
            "2026-01-01 12:05"
        );
        assert_eq!(short_time("soon".into()), "soon");
    }

    #[test]
    fn the_enroll_help_carries_the_mode() {
        let help = enroll_help(
            "https://t.example.com",
            "the_x",
            "2026-01-01T12:15:00Z",
            true,
        );
        assert!(help.contains(
            "curl -fsSL https://t.example.com/install.sh | sh -s -- --token the_x --manage"
        ));
        assert!(help.contains("before 2026-01-01 12:15"));
        assert!(!enroll_help("https://t", "the_x", "x", false).contains("--manage"));
    }
}
