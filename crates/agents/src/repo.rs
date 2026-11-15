use chrono::{Datelike, NaiveDate};
use tendhost_kernel::Result;
use tendhost_store::Keyspace;

use crate::model::{Detail, Installed, UsageDay};

const HOSTS: &str = "hosts";

/// Keys: `installed:<host>`, `tracked:<host>`, `hosts` (hosts that
/// reported any agent), `detail:<host>:<agent>`,
/// `usage:<host>:<agent>:<day>` (that day's rows, one per model),
/// `days:<host>:<agent>` (days with usage, by day number).
pub(crate) struct Repo {
    ks: Keyspace,
}

impl Repo {
    pub fn new(ks: Keyspace) -> Self {
        Self { ks }
    }

    pub fn put_installed(&self, host_id: &str, installed: &[Installed]) -> Result<()> {
        self.ks.put(&format!("installed:{host_id}"), &installed)?;
        self.ks.index_put(HOSTS, 0, host_id)
    }

    pub fn installed(&self, host_id: &str) -> Result<Vec<Installed>> {
        Ok(self
            .ks
            .get(&format!("installed:{host_id}"))?
            .unwrap_or_default())
    }

    pub fn hosts(&self) -> Result<Vec<String>> {
        self.ks.index_all(HOSTS)
    }

    pub fn put_tracked(&self, host_id: &str, agents: &[String]) -> Result<()> {
        self.ks.put(&format!("tracked:{host_id}"), &agents)
    }

    pub fn tracked(&self, host_id: &str) -> Result<Vec<String>> {
        Ok(self
            .ks
            .get(&format!("tracked:{host_id}"))?
            .unwrap_or_default())
    }

    pub fn put_detail(&self, host_id: &str, agent: &str, detail: &Detail) -> Result<()> {
        self.ks.put(&format!("detail:{host_id}:{agent}"), detail)
    }

    pub fn detail(&self, host_id: &str, agent: &str) -> Result<Detail> {
        Ok(self
            .ks
            .get(&format!("detail:{host_id}:{agent}"))?
            .unwrap_or_default())
    }

    /// Replaces the rows of one day.
    pub fn put_usage_day(
        &self,
        host_id: &str,
        agent: &str,
        day: NaiveDate,
        rows: &[UsageDay],
    ) -> Result<()> {
        self.ks.put(&usage_key(host_id, agent, day), &rows)?;
        self.ks.index_put(
            &days_key(host_id, agent),
            i64::from(day.num_days_from_ce()),
            &day.to_string(),
        )
    }

    /// Rows from `since` on, oldest day first.
    pub fn usage_since(
        &self,
        host_id: &str,
        agent: &str,
        since: NaiveDate,
    ) -> Result<Vec<UsageDay>> {
        let mut rows = Vec::new();
        for day in self.ks.index_range(
            &days_key(host_id, agent),
            i64::from(since.num_days_from_ce()),
            i64::MAX,
        )? {
            rows.extend(
                self.ks
                    .get::<Vec<UsageDay>>(&format!("usage:{host_id}:{agent}:{day}"))?
                    .unwrap_or_default(),
            );
        }
        Ok(rows)
    }

    /// Deletes usage days before `before`.
    pub fn drop_usage_before(&self, host_id: &str, agent: &str, before: NaiveDate) -> Result<()> {
        let last = i64::from(before.num_days_from_ce()) - 1;
        for day in self
            .ks
            .index_range(&days_key(host_id, agent), i64::MIN, last)?
        {
            self.ks.delete(&format!("usage:{host_id}:{agent}:{day}"))?;
        }
        self.ks
            .index_trim(&days_key(host_id, agent), last)
            .map(|_| ())
    }

    /// Deletes everything collected about one agent on one host.
    pub fn forget_agent(&self, host_id: &str, agent: &str) -> Result<()> {
        self.drop_usage_before(host_id, agent, NaiveDate::MAX)?;
        self.ks
            .delete(&format!("detail:{host_id}:{agent}"))
            .map(|_| ())
    }

    pub fn forget_host(&self, host_id: &str, agents: &[&str]) -> Result<()> {
        for agent in agents {
            self.forget_agent(host_id, agent)?;
        }
        self.ks.delete(&format!("installed:{host_id}"))?;
        self.ks.delete(&format!("tracked:{host_id}"))?;
        self.ks.index_remove(HOSTS, host_id)
    }
}

fn usage_key(host_id: &str, agent: &str, day: NaiveDate) -> String {
    format!("usage:{host_id}:{agent}:{day}")
}

fn days_key(host_id: &str, agent: &str) -> String {
    format!("days:{host_id}:{agent}")
}
