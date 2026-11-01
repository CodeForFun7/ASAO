//! Email and password accounts.
//!
//! ASAO never talks to the database. Accounts live in YugabyteDB behind the ASAO accounts
//! API (the `accounts-api` folder, deployed on Vercel), which hashes passwords and issues
//! session tokens. This module is a small HTTPS client for it.
//!
//! The API's address is public, so it is written here. `ASAO_ACCOUNTS_URL` (in the
//! terminal, or in `.env` at build time, see build.rs) overrides it, for example to test
//! against a local copy of the API.

use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_ACCOUNTS_URL: &str = "https://asao-accounts-api-two.vercel.app";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub user: AuthUser,
    pub token: String,
}

#[derive(Debug)]
pub enum AuthError {
    Unreachable(String),
    /// A message from the accounts API, meant for the user.
    Server(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(e) => write!(f, "Couldn't reach the account server. Check your connection and try again. ({e})"),
            Self::Server(msg) => write!(f, "{msg}"),
        }
    }
}

/// Runtime environment first, then a value baked in at build time, then the deployed API.
pub fn accounts_url() -> String {
    std::env::var("ASAO_ACCOUNTS_URL")
        .ok()
        .or_else(|| option_env!("ASAO_ACCOUNTS_URL").map(str::to_string))
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_ACCOUNTS_URL.to_string())
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("ASAO/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default()
    })
}

#[derive(Serialize)]
struct Credentials<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

#[derive(Deserialize)]
struct SessionBody {
    user: AuthUser,
}

async fn send(req: reqwest::RequestBuilder) -> Result<reqwest::Response, AuthError> {
    req.send().await.map_err(|e| AuthError::Unreachable(e.without_url().to_string()))
}

/// The API's own message for a failed request, or a generic one.
async fn server_error(res: reqwest::Response) -> AuthError {
    let status = res.status();
    match res.json::<ErrorBody>().await {
        Ok(b) => AuthError::Server(b.error),
        Err(_) => AuthError::Server(format!("The account server answered with an error ({status}). Try again shortly.")),
    }
}

fn unexpected(e: reqwest::Error) -> AuthError {
    AuthError::Server(format!("The account server sent an unexpected reply. ({})", e.without_url()))
}

async fn credentials_call(action: &str, email: &str, password: &str) -> Result<AuthSession, AuthError> {
    let url = format!("{}/api/{action}", accounts_url());
    let res = send(http().post(url).json(&Credentials { email, password })).await?;
    if !res.status().is_success() {
        return Err(server_error(res).await);
    }
    res.json::<AuthSession>().await.map_err(unexpected)
}

pub async fn sign_up(email: &str, password: String) -> Result<AuthSession, AuthError> {
    credentials_call("sign-up", email, &password).await
}

pub async fn sign_in(email: &str, password: String) -> Result<AuthSession, AuthError> {
    credentials_call("sign-in", email, &password).await
}

/// The account a saved token belongs to, or `None` when it has expired or was signed out.
pub async fn restore(token: &str) -> Result<Option<AuthUser>, AuthError> {
    let url = format!("{}/api/session", accounts_url());
    let res = send(http().get(url).bearer_auth(token)).await?;
    match res.status() {
        s if s.is_success() => res.json::<SessionBody>().await.map(|b| Some(b.user)).map_err(unexpected),
        reqwest::StatusCode::UNAUTHORIZED => Ok(None),
        _ => Err(server_error(res).await),
    }
}

pub async fn sign_out(token: &str) -> Result<(), AuthError> {
    let url = format!("{}/api/sign-out", accounts_url());
    let res = send(http().post(url).bearer_auth(token)).await?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(server_error(res).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the deployed accounts API (or ASAO_ACCOUNTS_URL):
    /// `cargo test --lib auth::tests::live_round_trip -- --ignored`.
    /// Leaves a throwaway asao-test-…@example.com account behind.
    #[test]
    #[ignore]
    fn live_round_trip() {
        tauri::async_runtime::block_on(async {
            let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let email = format!("asao-test-{nonce}@example.com");
            let s = sign_up(&email, "test password 1".into()).await.unwrap();
            assert!(matches!(sign_up(&email, "test password 1".into()).await, Err(AuthError::Server(_))));
            assert!(matches!(sign_in(&email, "wrong password".into()).await, Err(AuthError::Server(_))));
            let s2 = sign_in(&email, "test password 1".into()).await.unwrap();
            assert_eq!(restore(&s.token).await.unwrap().unwrap().email, email);
            sign_out(&s2.token).await.unwrap();
            assert!(restore(&s2.token).await.unwrap().is_none());
            sign_out(&s.token).await.unwrap();
        });
    }
}
