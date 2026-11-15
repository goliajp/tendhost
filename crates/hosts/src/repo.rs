use tendhost_kernel::Result;
use tendhost_store::Keyspace;

use crate::model::{Enrollment, Host};

const HOSTS: &str = "hosts";

/// Keys: `host:<id>`, `host-name:<name>` → id, `hosts` (by enrollment),
/// `cred:<fingerprint>` → host id, `host-cred:<id>` → fingerprint,
/// `enroll:<fingerprint>`.
pub(crate) struct Repo {
    ks: Keyspace,
}

impl Repo {
    pub fn new(ks: Keyspace) -> Self {
        Self { ks }
    }

    /// `false` when another host holds the name.
    pub fn claim_name(&self, name: &str, host_id: &str) -> Result<bool> {
        self.ks.put_new(&format!("host-name:{name}"), &host_id)
    }

    pub fn release_name(&self, name: &str) -> Result<()> {
        self.ks.delete(&format!("host-name:{name}")).map(|_| ())
    }

    pub fn put_host(&self, host: &Host) -> Result<()> {
        self.ks.put(&format!("host:{}", host.id), host)?;
        self.ks
            .index_put(HOSTS, host.enrolled_at.timestamp_millis(), &host.id)
    }

    pub fn host(&self, id: &str) -> Result<Option<Host>> {
        self.ks.get(&format!("host:{id}"))
    }

    pub fn hosts(&self) -> Result<Vec<Host>> {
        let mut hosts = Vec::new();
        for id in self.ks.index_all(HOSTS)? {
            hosts.extend(self.host(&id)?);
        }
        Ok(hosts)
    }

    pub fn remove_host(&self, host: &Host) -> Result<()> {
        if let Some(fingerprint) = self.ks.get::<String>(&format!("host-cred:{}", host.id))? {
            self.ks.delete(&format!("cred:{fingerprint}"))?;
        }
        self.ks.delete(&format!("host-cred:{}", host.id))?;
        self.ks.index_remove(HOSTS, &host.id)?;
        self.ks.delete(&format!("host:{}", host.id))?;
        self.release_name(&host.name)
    }

    pub fn put_credential(&self, fingerprint: &str, host_id: &str) -> Result<()> {
        self.ks.put(&format!("cred:{fingerprint}"), &host_id)?;
        self.ks.put(&format!("host-cred:{host_id}"), &fingerprint)
    }

    pub fn host_id_of_credential(&self, fingerprint: &str) -> Result<Option<String>> {
        self.ks.get(&format!("cred:{fingerprint}"))
    }

    pub fn put_enrollment(&self, fingerprint: &str, enrollment: &Enrollment) -> Result<()> {
        self.ks.put(&format!("enroll:{fingerprint}"), enrollment)
    }

    pub fn enrollment(&self, fingerprint: &str) -> Result<Option<Enrollment>> {
        self.ks.get(&format!("enroll:{fingerprint}"))
    }

    /// Removes the enrollment; `true` for the one caller that removed it.
    pub fn claim_enrollment(&self, fingerprint: &str) -> Result<bool> {
        self.ks.delete(&format!("enroll:{fingerprint}"))
    }
}
