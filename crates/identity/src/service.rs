use std::sync::{Arc, OnceLock};

use tendhost_http::{Authenticator, Credential};
use tendhost_kernel::{Actor, Bus, Clock, Error, Principal, Result, Role, id, secret};
use tendhost_store::Keyspace;

use crate::events::{OwnerCreated, SignedIn, TokenCreated, TokenRevoked, UserCreated, UserRemoved};
use crate::model::{
    NewToken, SESSION_LIFETIME, Session, StoredToken, StoredUser, Token, User, check_label,
    check_password, normalize_name,
};
use crate::password;
use crate::repo::Repo;

#[derive(Debug, Clone, Copy, Default)]
pub struct Settings {
    /// Mark the session cookie `Secure`. On when the server is reached
    /// over https.
    pub secure_cookies: bool,
}

pub struct Identity {
    repo: Repo,
    bus: Bus,
    clock: Arc<dyn Clock>,
    pub(crate) settings: Settings,
}

impl Identity {
    pub fn new(keyspace: Keyspace, bus: Bus, clock: Arc<dyn Clock>, settings: Settings) -> Self {
        Self {
            repo: Repo::new(keyspace),
            bus,
            clock,
            settings,
        }
    }

    /// True until the first user exists.
    pub fn needs_setup(&self) -> Result<bool> {
        Ok(self.repo.user_count()? == 0)
    }

    /// Called once at startup. While setup is still needed it issues a new
    /// setup token, replacing any earlier one, and returns it to be shown
    /// to whoever can read the server's output.
    pub fn begin_setup(&self) -> Result<Option<String>> {
        if !self.needs_setup()? {
            return Ok(None);
        }
        let token = secret::generate("thsetup");
        self.repo
            .put_setup_fingerprint(&secret::fingerprint(&token))?;
        Ok(Some(token))
    }

    /// Creates the owner and signs them in. Works once.
    pub fn complete_setup(
        &self,
        setup_token: &str,
        name: &str,
        password: &str,
    ) -> Result<(User, String)> {
        let name = normalize_name(name)?;
        check_password(password)?;
        if self.repo.setup_fingerprint()? != Some(secret::fingerprint(setup_token.trim())) {
            return Err(Error::invalid(
                "setup_token",
                "is not the one the server printed",
            ));
        }
        let password_hash = password::hash(password)?;
        if !self.repo.claim_setup()? {
            return Err(Error::Conflict("setup was already completed".into()));
        }
        let user = self.insert_user(name, password_hash, Role::Owner)?;
        self.bus.publish(
            Actor::User(user.id.clone()),
            &OwnerCreated {
                user_id: user.id.clone(),
                name: user.name.clone(),
            },
        );
        let session = self.open_session(&user)?;
        Ok((user, session))
    }

    /// A wrong name and a wrong password are the same failure to the caller.
    pub fn sign_in(&self, name: &str, password: &str) -> Result<(User, String)> {
        let stored = match normalize_name(name) {
            Ok(name) => self.repo.user_by_name(&name)?,
            Err(_) => None,
        };
        let Some(stored) = stored else {
            // spend the same time as a real check so the answer does not
            // reveal whether the name exists
            password::verify(password, dummy_hash());
            return Err(Error::Unauthenticated);
        };
        if !password::verify(password, &stored.password_hash) {
            return Err(Error::Unauthenticated);
        }
        let session = self.open_session(&stored.user)?;
        self.bus.publish(
            Actor::User(stored.user.id.clone()),
            &SignedIn {
                user_id: stored.user.id.clone(),
            },
        );
        Ok((stored.user, session))
    }

    pub fn sign_out(&self, session: &str) -> Result<()> {
        self.repo.remove_session(&secret::fingerprint(session))
    }

    /// The user a principal acts as.
    pub fn user_of(&self, principal: &Principal) -> Result<User> {
        let Actor::User(id) = &principal.actor else {
            return Err(Error::Forbidden);
        };
        Ok(self.repo.user(id)?.ok_or(Error::Unauthenticated)?.user)
    }

    pub fn users(&self, principal: &Principal) -> Result<Vec<User>> {
        principal.require(Role::Admin)?;
        Ok(self.repo.users()?.into_iter().map(|u| u.user).collect())
    }

    pub fn create_user(
        &self,
        principal: &Principal,
        name: &str,
        password: &str,
        role: Role,
    ) -> Result<User> {
        principal.require(Role::Admin.max(role))?;
        let name = normalize_name(name)?;
        check_password(password)?;
        let user = self.insert_user(name, password::hash(password)?, role)?;
        self.bus.publish(
            principal.actor.clone(),
            &UserCreated {
                user_id: user.id.clone(),
                name: user.name.clone(),
                role,
            },
        );
        Ok(user)
    }

    /// Nobody removes themselves, so the last owner always remains.
    pub fn remove_user(&self, principal: &Principal, user_id: &str) -> Result<()> {
        let stored = self
            .repo
            .user(user_id)?
            .ok_or(Error::NotFound { kind: "user" })?;
        principal.require(Role::Admin.max(stored.user.role))?;
        if principal.actor == Actor::User(user_id.to_string()) {
            return Err(Error::Conflict("you cannot remove yourself".into()));
        }
        for token in self
            .repo
            .tokens()?
            .into_iter()
            .filter(|t| t.token.user_id == user_id)
        {
            self.repo.remove_token(&token)?;
        }
        self.repo.remove_user(&stored)?;
        self.bus.publish(
            principal.actor.clone(),
            &UserRemoved {
                user_id: stored.user.id,
                name: stored.user.name,
            },
        );
        Ok(())
    }

    pub fn tokens(&self, principal: &Principal) -> Result<Vec<Token>> {
        principal.require(Role::Admin)?;
        Ok(self.repo.tokens()?.into_iter().map(|t| t.token).collect())
    }

    /// A token acts as the user who made it, with a role no higher than
    /// theirs.
    pub fn create_token(&self, principal: &Principal, name: &str, role: Role) -> Result<NewToken> {
        principal.require(Role::Admin.max(role))?;
        let user = self.user_of(principal)?;
        let name = check_label("name", name)?;
        let now = self.clock.now();
        let secret = secret::generate("tht");
        let token = Token {
            id: id::new("tok", now),
            name,
            role,
            user_id: user.id,
            created_at: now,
        };
        self.repo.insert_token(&StoredToken {
            token: token.clone(),
            fingerprint: secret::fingerprint(&secret),
        })?;
        self.bus.publish(
            principal.actor.clone(),
            &TokenCreated {
                token_id: token.id.clone(),
                name: token.name.clone(),
                role,
            },
        );
        Ok(NewToken { token, secret })
    }

    pub fn revoke_token(&self, principal: &Principal, token_id: &str) -> Result<()> {
        principal.require(Role::Admin)?;
        let stored = self
            .repo
            .token(token_id)?
            .ok_or(Error::NotFound { kind: "token" })?;
        self.repo.remove_token(&stored)?;
        self.bus.publish(
            principal.actor.clone(),
            &TokenRevoked {
                token_id: stored.token.id,
                name: stored.token.name,
            },
        );
        Ok(())
    }

    fn insert_user(&self, name: String, password_hash: String, role: Role) -> Result<User> {
        let now = self.clock.now();
        let user = User {
            id: id::new("usr", now),
            name,
            role,
            created_at: now,
        };
        if !self.repo.insert_user(&StoredUser {
            user: user.clone(),
            password_hash,
        })? {
            return Err(Error::Conflict(format!("the name {} is taken", user.name)));
        }
        Ok(user)
    }

    fn open_session(&self, user: &User) -> Result<String> {
        let session = secret::generate("ths");
        self.repo.put_session(
            &secret::fingerprint(&session),
            &Session {
                user_id: user.id.clone(),
                expires_at: self.clock.now() + SESSION_LIFETIME,
            },
        )?;
        Ok(session)
    }
}

impl Authenticator for Identity {
    fn authenticate(&self, credential: &Credential) -> Result<Principal> {
        let (user_id, role_cap) = match credential {
            Credential::Session(session) => {
                let fingerprint = secret::fingerprint(session);
                let found = self
                    .repo
                    .session(&fingerprint)?
                    .ok_or(Error::Unauthenticated)?;
                if found.expires_at <= self.clock.now() {
                    self.repo.remove_session(&fingerprint)?;
                    return Err(Error::Unauthenticated);
                }
                (found.user_id, Role::Owner)
            }
            Credential::Bearer(token) => {
                let found = self
                    .repo
                    .token_by_fingerprint(&secret::fingerprint(token))?
                    .ok_or(Error::Unauthenticated)?;
                (found.token.user_id, found.token.role)
            }
        };
        // the user is read on every request: removing a user or lowering
        // their role takes effect at once
        let user = self
            .repo
            .user(&user_id)?
            .ok_or(Error::Unauthenticated)?
            .user;
        Ok(Principal {
            actor: Actor::User(user.id),
            role: user.role.min(role_cap),
        })
    }
}

fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        password::hash("no user has this password").expect("argon2 hashes a constant")
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{Duration, TimeZone, Utc};
    use tendhost_kernel::{Envelope, ManualClock, Sink};
    use tendhost_store::Store;

    use super::*;

    #[derive(Default)]
    struct Topics(Mutex<Vec<String>>);

    impl Sink for Topics {
        fn accept(&self, envelope: &Envelope) {
            self.0.lock().unwrap().push(envelope.topic.clone());
        }
    }

    struct Fixture {
        identity: Identity,
        clock: Arc<ManualClock>,
        topics: Arc<Topics>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let topics = Arc::new(Topics::default());
        bus.attach(topics.clone());
        let keyspace = Store::temp().unwrap().keyspace("identity");
        Fixture {
            identity: Identity::new(keyspace, bus, clock.clone(), Settings::default()),
            clock,
            topics,
        }
    }

    const PASSWORD: &str = "a long password";

    fn with_owner() -> (Fixture, Principal) {
        let f = fixture();
        let token = f.identity.begin_setup().unwrap().unwrap();
        let (owner, _) = f
            .identity
            .complete_setup(&token, "owner", PASSWORD)
            .unwrap();
        (
            f,
            Principal {
                actor: Actor::User(owner.id),
                role: Role::Owner,
            },
        )
    }

    #[test]
    fn setup_creates_the_owner_once() {
        let f = fixture();
        assert!(f.identity.needs_setup().unwrap());
        let token = f.identity.begin_setup().unwrap().unwrap();
        let (owner, session) = f.identity.complete_setup(&token, "Ada", PASSWORD).unwrap();
        assert_eq!((owner.name.as_str(), owner.role), ("ada", Role::Owner));
        assert!(!f.identity.needs_setup().unwrap());
        assert_eq!(f.identity.begin_setup().unwrap(), None);
        assert!(matches!(
            f.identity.complete_setup(&token, "eve", PASSWORD),
            Err(Error::Invalid { .. })
        ));
        let principal = f
            .identity
            .authenticate(&Credential::Session(session))
            .unwrap();
        assert_eq!(
            principal,
            Principal {
                actor: Actor::User(owner.id),
                role: Role::Owner
            }
        );
        assert_eq!(*f.topics.0.lock().unwrap(), ["identity.owner_created"]);
    }

    #[test]
    fn setup_with_a_wrong_token_changes_nothing() {
        let f = fixture();
        let token = f.identity.begin_setup().unwrap().unwrap();
        assert!(
            f.identity
                .complete_setup("thsetup_guess", "eve", PASSWORD)
                .is_err()
        );
        assert!(f.identity.needs_setup().unwrap());
        assert!(f.identity.complete_setup(&token, "ada", PASSWORD).is_ok());
    }

    #[test]
    fn a_rejected_setup_form_does_not_use_up_the_token() {
        let f = fixture();
        let token = f.identity.begin_setup().unwrap().unwrap();
        assert!(f.identity.complete_setup(&token, "ada", "short").is_err());
        assert!(f.identity.complete_setup(&token, "ada", PASSWORD).is_ok());
    }

    #[test]
    fn restarting_before_setup_replaces_the_setup_token() {
        let f = fixture();
        let first = f.identity.begin_setup().unwrap().unwrap();
        let second = f.identity.begin_setup().unwrap().unwrap();
        assert!(f.identity.complete_setup(&first, "ada", PASSWORD).is_err());
        assert!(f.identity.complete_setup(&second, "ada", PASSWORD).is_ok());
    }

    #[test]
    fn sign_in_fails_alike_for_a_wrong_password_and_an_unknown_name() {
        let (f, _) = with_owner();
        assert!(matches!(
            f.identity.sign_in("owner", "wrong password"),
            Err(Error::Unauthenticated)
        ));
        assert!(matches!(
            f.identity.sign_in("nobody", PASSWORD),
            Err(Error::Unauthenticated)
        ));
        assert!(matches!(
            f.identity.sign_in("", PASSWORD),
            Err(Error::Unauthenticated)
        ));
        assert!(f.identity.sign_in(" Owner ", PASSWORD).is_ok());
    }

    #[test]
    fn a_session_ends_at_sign_out_and_at_expiry() {
        let (f, _) = with_owner();
        let (_, a) = f.identity.sign_in("owner", PASSWORD).unwrap();
        let (_, b) = f.identity.sign_in("owner", PASSWORD).unwrap();
        f.identity.sign_out(&a).unwrap();
        assert!(f.identity.authenticate(&Credential::Session(a)).is_err());
        f.clock.advance(SESSION_LIFETIME - Duration::seconds(1));
        assert!(
            f.identity
                .authenticate(&Credential::Session(b.clone()))
                .is_ok()
        );
        f.clock.advance(Duration::seconds(1));
        assert!(f.identity.authenticate(&Credential::Session(b)).is_err());
    }

    #[test]
    fn user_names_are_unique() {
        let (f, owner) = with_owner();
        f.identity
            .create_user(&owner, "bob", PASSWORD, Role::Viewer)
            .unwrap();
        assert!(matches!(
            f.identity
                .create_user(&owner, "BOB", PASSWORD, Role::Viewer),
            Err(Error::Conflict(_))
        ));
        assert_eq!(f.identity.users(&owner).unwrap().len(), 2);
    }

    #[test]
    fn roles_limit_who_manages_users() {
        let (f, owner) = with_owner();
        let admin = f
            .identity
            .create_user(&owner, "admin", PASSWORD, Role::Admin)
            .unwrap();
        let viewer = f
            .identity
            .create_user(&owner, "viewer", PASSWORD, Role::Viewer)
            .unwrap();
        let admin_p = Principal {
            actor: Actor::User(admin.id.clone()),
            role: Role::Admin,
        };
        let viewer_p = Principal {
            actor: Actor::User(viewer.id.clone()),
            role: Role::Viewer,
        };
        let Actor::User(owner_id) = &owner.actor else {
            unreachable!()
        };

        assert!(matches!(f.identity.users(&viewer_p), Err(Error::Forbidden)));
        assert!(matches!(
            f.identity
                .create_user(&admin_p, "o2", PASSWORD, Role::Owner),
            Err(Error::Forbidden)
        ));
        assert!(matches!(
            f.identity.remove_user(&admin_p, owner_id),
            Err(Error::Forbidden)
        ));
        assert!(matches!(
            f.identity.remove_user(&admin_p, &admin.id),
            Err(Error::Conflict(_))
        ));
        assert!(matches!(
            f.identity.remove_user(&owner, owner_id),
            Err(Error::Conflict(_))
        ));
        f.identity.remove_user(&admin_p, &viewer.id).unwrap();
        assert!(matches!(
            f.identity.remove_user(&admin_p, &viewer.id),
            Err(Error::NotFound { .. })
        ));
    }

    #[test]
    fn removing_a_user_ends_their_sessions_and_tokens_and_frees_the_name() {
        let (f, owner) = with_owner();
        let bob = f
            .identity
            .create_user(&owner, "bob", PASSWORD, Role::Admin)
            .unwrap();
        let (_, session) = f.identity.sign_in("bob", PASSWORD).unwrap();
        let bob_p = Principal {
            actor: Actor::User(bob.id.clone()),
            role: Role::Admin,
        };
        let token = f.identity.create_token(&bob_p, "ci", Role::Viewer).unwrap();

        f.identity.remove_user(&owner, &bob.id).unwrap();
        assert!(
            f.identity
                .authenticate(&Credential::Session(session))
                .is_err()
        );
        assert!(
            f.identity
                .authenticate(&Credential::Bearer(token.secret))
                .is_err()
        );
        assert!(f.identity.tokens(&owner).unwrap().is_empty());
        assert!(
            f.identity
                .create_user(&owner, "bob", PASSWORD, Role::Viewer)
                .is_ok()
        );
    }

    #[test]
    fn a_token_carries_its_own_role_and_stops_working_when_revoked() {
        let (f, owner) = with_owner();
        let token = f
            .identity
            .create_token(&owner, " deploy ", Role::Viewer)
            .unwrap();
        assert_eq!(token.token.name, "deploy");
        let principal = f
            .identity
            .authenticate(&Credential::Bearer(token.secret.clone()))
            .unwrap();
        assert_eq!(
            principal,
            Principal {
                actor: owner.actor.clone(),
                role: Role::Viewer
            }
        );
        assert_eq!(
            f.identity.tokens(&owner).unwrap(),
            std::slice::from_ref(&token.token)
        );

        f.identity.revoke_token(&owner, &token.token.id).unwrap();
        assert!(
            f.identity
                .authenticate(&Credential::Bearer(token.secret))
                .is_err()
        );
        assert!(matches!(
            f.identity.revoke_token(&owner, &token.token.id),
            Err(Error::NotFound { .. })
        ));
    }

    #[test]
    fn a_token_cannot_outrank_its_maker() {
        let (f, owner) = with_owner();
        let admin = f
            .identity
            .create_user(&owner, "admin", PASSWORD, Role::Admin)
            .unwrap();
        let admin_p = Principal {
            actor: Actor::User(admin.id),
            role: Role::Admin,
        };
        assert!(matches!(
            f.identity.create_token(&admin_p, "t", Role::Owner),
            Err(Error::Forbidden)
        ));
        assert!(f.identity.create_token(&admin_p, "t", Role::Admin).is_ok());
    }

    #[test]
    fn every_change_is_published() {
        let (f, owner) = with_owner();
        let bob = f
            .identity
            .create_user(&owner, "bob", PASSWORD, Role::Viewer)
            .unwrap();
        f.identity.sign_in("bob", PASSWORD).unwrap();
        let token = f.identity.create_token(&owner, "t", Role::Viewer).unwrap();
        f.identity.revoke_token(&owner, &token.token.id).unwrap();
        f.identity.remove_user(&owner, &bob.id).unwrap();
        assert_eq!(
            *f.topics.0.lock().unwrap(),
            [
                "identity.owner_created",
                "identity.user_created",
                "identity.signed_in",
                "identity.token_created",
                "identity.token_revoked",
                "identity.user_removed",
            ]
        );
    }
}
