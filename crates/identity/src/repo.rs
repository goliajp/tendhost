use tendhost_kernel::Result;
use tendhost_store::Keyspace;

use crate::model::{Session, StoredToken, StoredUser};

const USERS: &str = "users";
const TOKENS: &str = "tokens";
const SETUP: &str = "setup-token";

/// Keys: `user:<id>`, `user-name:<name>` → id, `users` (by creation),
/// `session:<fingerprint>`, `token:<id>`, `token-fp:<fingerprint>` → id,
/// `tokens` (by creation), `setup-token` → fingerprint.
pub(crate) struct Repo {
    ks: Keyspace,
}

impl Repo {
    pub fn new(ks: Keyspace) -> Self {
        Self { ks }
    }

    /// `false` when the name is taken; nothing is written then.
    pub fn insert_user(&self, user: &StoredUser) -> Result<bool> {
        if !self
            .ks
            .put_new(&format!("user-name:{}", user.user.name), &user.user.id)?
        {
            return Ok(false);
        }
        self.ks.put(&format!("user:{}", user.user.id), user)?;
        self.ks.index_put(
            USERS,
            user.user.created_at.timestamp_millis(),
            &user.user.id,
        )?;
        Ok(true)
    }

    pub fn user(&self, id: &str) -> Result<Option<StoredUser>> {
        self.ks.get(&format!("user:{id}"))
    }

    pub fn user_by_name(&self, name: &str) -> Result<Option<StoredUser>> {
        match self.ks.get::<String>(&format!("user-name:{name}"))? {
            Some(id) => self.user(&id),
            None => Ok(None),
        }
    }

    pub fn users(&self) -> Result<Vec<StoredUser>> {
        let mut users = Vec::new();
        for id in self.ks.index_all(USERS)? {
            users.extend(self.user(&id)?);
        }
        Ok(users)
    }

    pub fn user_count(&self) -> Result<usize> {
        self.ks.index_len(USERS)
    }

    pub fn remove_user(&self, user: &StoredUser) -> Result<()> {
        self.ks.index_remove(USERS, &user.user.id)?;
        self.ks.delete(&format!("user:{}", user.user.id))?;
        self.ks.delete(&format!("user-name:{}", user.user.name))?;
        Ok(())
    }

    pub fn put_session(&self, fingerprint: &str, session: &Session) -> Result<()> {
        self.ks.put(&format!("session:{fingerprint}"), session)
    }

    pub fn session(&self, fingerprint: &str) -> Result<Option<Session>> {
        self.ks.get(&format!("session:{fingerprint}"))
    }

    pub fn remove_session(&self, fingerprint: &str) -> Result<()> {
        self.ks
            .delete(&format!("session:{fingerprint}"))
            .map(|_| ())
    }

    pub fn insert_token(&self, token: &StoredToken) -> Result<()> {
        self.ks.put(&format!("token:{}", token.token.id), token)?;
        self.ks
            .put(&format!("token-fp:{}", token.fingerprint), &token.token.id)?;
        self.ks.index_put(
            TOKENS,
            token.token.created_at.timestamp_millis(),
            &token.token.id,
        )
    }

    pub fn token(&self, id: &str) -> Result<Option<StoredToken>> {
        self.ks.get(&format!("token:{id}"))
    }

    pub fn token_by_fingerprint(&self, fingerprint: &str) -> Result<Option<StoredToken>> {
        match self.ks.get::<String>(&format!("token-fp:{fingerprint}"))? {
            Some(id) => self.token(&id),
            None => Ok(None),
        }
    }

    pub fn tokens(&self) -> Result<Vec<StoredToken>> {
        let mut tokens = Vec::new();
        for id in self.ks.index_all(TOKENS)? {
            tokens.extend(self.token(&id)?);
        }
        Ok(tokens)
    }

    pub fn remove_token(&self, token: &StoredToken) -> Result<()> {
        self.ks.index_remove(TOKENS, &token.token.id)?;
        self.ks.delete(&format!("token-fp:{}", token.fingerprint))?;
        self.ks
            .delete(&format!("token:{}", token.token.id))
            .map(|_| ())
    }

    pub fn put_setup_fingerprint(&self, fingerprint: &str) -> Result<()> {
        self.ks.put(SETUP, &fingerprint)
    }

    pub fn setup_fingerprint(&self) -> Result<Option<String>> {
        self.ks.get(SETUP)
    }

    /// Removes the setup token; `true` for the one caller that removed it.
    pub fn claim_setup(&self) -> Result<bool> {
        self.ks.delete(SETUP)
    }
}
