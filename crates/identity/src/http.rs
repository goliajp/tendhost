use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tendhost_http::{ApiResult, Credential, SESSION_COOKIE, Signed, credential};
use tendhost_kernel::{Error, Result, Role};

use crate::model::{NewToken, SESSION_LIFETIME, Token, User};
use crate::service::Identity;

/// Routes, relative to the api root: `/setup`, `/session`, `/users`,
/// `/tokens`.
pub fn router(identity: Arc<Identity>) -> Router {
    Router::new()
        .route("/setup", get(setup_state).post(complete_setup))
        .route("/session", get(current_user).post(sign_in).delete(sign_out))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", delete(remove_user))
        .route("/tokens", get(list_tokens).post(create_token))
        .route("/tokens/{id}", delete(revoke_token))
        .with_state(identity)
}

#[derive(Serialize)]
struct SetupState {
    needed: bool,
}

#[derive(Deserialize)]
struct SetupForm {
    setup_token: String,
    name: String,
    password: String,
}

#[derive(Deserialize)]
struct SignInForm {
    name: String,
    password: String,
}

#[derive(Deserialize)]
struct UserForm {
    name: String,
    password: String,
    role: Role,
}

#[derive(Deserialize)]
struct TokenForm {
    name: String,
    role: Role,
}

async fn setup_state(State(identity): State<Arc<Identity>>) -> ApiResult<Json<SetupState>> {
    Ok(Json(SetupState {
        needed: identity.needs_setup()?,
    }))
}

async fn complete_setup(
    State(identity): State<Arc<Identity>>,
    Json(form): Json<SetupForm>,
) -> ApiResult<Response> {
    let secure = identity.settings.secure_cookies;
    let (user, session) =
        hashing(move || identity.complete_setup(&form.setup_token, &form.name, &form.password))
            .await?;
    Ok((
        StatusCode::CREATED,
        [(SET_COOKIE, session_cookie(&session, secure))],
        Json(user),
    )
        .into_response())
}

async fn sign_in(
    State(identity): State<Arc<Identity>>,
    Json(form): Json<SignInForm>,
) -> ApiResult<Response> {
    let secure = identity.settings.secure_cookies;
    let (user, session) = hashing(move || identity.sign_in(&form.name, &form.password)).await?;
    Ok(([(SET_COOKIE, session_cookie(&session, secure))], Json(user)).into_response())
}

async fn sign_out(
    State(identity): State<Arc<Identity>>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    if let Some(Credential::Session(session)) = credential(&headers) {
        identity.sign_out(&session)?;
    }
    let expired = cookie("", 0, identity.settings.secure_cookies);
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, expired)]).into_response())
}

async fn current_user(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
) -> ApiResult<Json<User>> {
    Ok(Json(identity.user_of(&principal)?))
}

async fn list_users(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<User>>> {
    Ok(Json(identity.users(&principal)?))
}

async fn create_user(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
    Json(form): Json<UserForm>,
) -> ApiResult<(StatusCode, Json<User>)> {
    let user =
        hashing(move || identity.create_user(&principal, &form.name, &form.password, form.role))
            .await?;
    Ok((StatusCode::CREATED, Json(user)))
}

async fn remove_user(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    identity.remove_user(&principal, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_tokens(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<Token>>> {
    Ok(Json(identity.tokens(&principal)?))
}

async fn create_token(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
    Json(form): Json<TokenForm>,
) -> ApiResult<(StatusCode, Json<NewToken>)> {
    Ok((
        StatusCode::CREATED,
        Json(identity.create_token(&principal, &form.name, form.role)?),
    ))
}

async fn revoke_token(
    State(identity): State<Arc<Identity>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    identity.revoke_token(&principal, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Password hashing takes tens of milliseconds of cpu; keep it off the
/// threads that serve other requests.
async fn hashing<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(Error::internal)?
}

fn session_cookie(session: &str, secure: bool) -> String {
    cookie(session, SESSION_LIFETIME.num_seconds(), secure)
}

fn cookie(value: &str, max_age: i64, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "{SESSION_COOKIE}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Strict{secure}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_cookie_is_closed_to_scripts_and_other_sites() {
        let c = session_cookie("ths_x", false);
        assert!(c.starts_with("tendhost_session=ths_x; "));
        assert!(
            c.contains("HttpOnly")
                && c.contains("SameSite=Strict")
                && c.contains("Max-Age=2592000")
        );
        assert!(!c.contains("Secure"));
        assert!(session_cookie("ths_x", true).ends_with("; Secure"));
    }
}
