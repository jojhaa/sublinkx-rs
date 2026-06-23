use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::{HeaderMap, HeaderValue, header};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, OnceLock},
};
use time::{Duration, OffsetDateTime};

use crate::{
    domain::user::User,
    dto::auth::{
        ChangeCredentialsRequest, LoginRequest, LoginResponse, LoginTokenData, MeData, MeResponse,
    },
    errors::AppError,
    repository::user_repo,
    state::AppState,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: i64,
    username: String,
    role: String,
    token_version: i64,
    csrf_token: String,
    exp: usize,
}

#[derive(Debug, Clone)]
struct LoginAttempt {
    failures: u32,
    blocked_until: Option<OffsetDateTime>,
    last_failed_at: OffsetDateTime,
}

static LOGIN_ATTEMPTS: OnceLock<Mutex<HashMap<String, LoginAttempt>>> = OnceLock::new();
const LOGIN_FAILURE_LIMIT: u32 = 5;
const LOGIN_WINDOW_MINUTES: i64 = 10;
const LOGIN_BLOCK_MINUTES: i64 = 15;
const AUTH_COOKIE_NAME: &str = "sublinkx_auth";
const CSRF_HEADER_NAME: &str = "x-csrf-token";

pub struct LoginOutcome {
    pub response: LoginResponse,
    pub session: AuthSession,
}

pub struct MeOutcome {
    pub response: MeResponse,
    pub session: AuthSession,
}

pub struct AuthSession {
    pub access_token: String,
    pub csrf_token: String,
}

pub async fn login(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    payload: LoginRequest,
) -> Result<LoginOutcome, AppError> {
    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(AppError::BadRequest(
            "username and password are required".to_string(),
        ));
    }

    let username = payload.username.trim();
    let rate_limit_key = login_rate_limit_key(
        headers,
        peer_ip,
        username,
        state.config.security.trust_proxy_headers,
    );
    ensure_login_not_limited(&rate_limit_key)?;

    let user = user_repo::find_by_username(&state.db, payload.username.trim())
        .await?
        .ok_or_else(|| {
            record_login_failure(&rate_limit_key);
            AppError::Unauthorized
        })?;

    if user.status != "active" {
        record_login_failure(&rate_limit_key);
        return Err(AppError::Unauthorized);
    }

    if let Err(error) = verify_password(&payload.password, &user.password_hash) {
        record_login_failure(&rate_limit_key);
        return Err(error);
    }

    clear_login_failures(&rate_limit_key);

    let session = issue_session(state, &user)?;

    let response = LoginResponse {
        code: "00000",
        data: LoginTokenData {
            csrf_token: session.csrf_token.clone(),
            expires_in_hours: state.config.security.jwt_exp_hours,
            user: user_to_me_data(user),
        },
    };

    Ok(LoginOutcome { response, session })
}

fn ensure_login_not_limited(key: &str) -> Result<(), AppError> {
    let now = OffsetDateTime::now_utc();
    let attempts = login_attempts();
    let mut attempts = attempts.lock().map_err(|_| AppError::Internal)?;
    prune_login_attempts(&mut attempts, now);

    if let Some(attempt) = attempts.get(key)
        && let Some(blocked_until) = attempt.blocked_until
        && blocked_until > now
    {
        return Err(AppError::TooManyRequests(
            "too many failed login attempts; try again later".to_string(),
        ));
    }

    Ok(())
}

fn record_login_failure(key: &str) {
    let now = OffsetDateTime::now_utc();
    let attempts = login_attempts();
    let Ok(mut attempts) = attempts.lock() else {
        return;
    };
    prune_login_attempts(&mut attempts, now);

    let attempt = attempts.entry(key.to_string()).or_insert(LoginAttempt {
        failures: 0,
        blocked_until: None,
        last_failed_at: now,
    });

    if now - attempt.last_failed_at > Duration::minutes(LOGIN_WINDOW_MINUTES) {
        attempt.failures = 0;
        attempt.blocked_until = None;
    }

    attempt.failures += 1;
    attempt.last_failed_at = now;
    if attempt.failures >= LOGIN_FAILURE_LIMIT {
        attempt.blocked_until = Some(now + Duration::minutes(LOGIN_BLOCK_MINUTES));
    }
}

fn clear_login_failures(key: &str) {
    let attempts = login_attempts();
    if let Ok(mut attempts) = attempts.lock() {
        attempts.remove(key);
    }
}

fn prune_login_attempts(attempts: &mut HashMap<String, LoginAttempt>, now: OffsetDateTime) {
    attempts.retain(|_, attempt| {
        attempt
            .blocked_until
            .is_some_and(|blocked_until| blocked_until > now)
            || now - attempt.last_failed_at <= Duration::minutes(LOGIN_WINDOW_MINUTES)
    });
}

fn login_attempts() -> &'static Mutex<HashMap<String, LoginAttempt>> {
    LOGIN_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn login_rate_limit_key(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    username: &str,
    trust_proxy_headers: bool,
) -> String {
    let ip = request_rate_limit_ip(headers, peer_ip, trust_proxy_headers);

    format!("{}:{}", ip, username.to_ascii_lowercase())
}

pub fn request_rate_limit_ip(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    trust_proxy_headers: bool,
) -> String {
    if trust_proxy_headers {
        return headers
            .get("x-real-ip")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                headers
                    .get("x-forwarded-for")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.rsplit(',').next())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or("unknown")
            .to_string();
    }

    peer_ip
        .map(|value| value.to_string())
        .unwrap_or_else(|| "direct".to_string())
}

pub async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<MeResponse, AppError> {
    let user = require_user_for_credentials_change(state, headers).await?;

    Ok(MeResponse {
        code: "00000",
        data: user_to_me_data(user),
    })
}

pub async fn change_credentials(
    state: &AppState,
    headers: &HeaderMap,
    payload: ChangeCredentialsRequest,
) -> Result<MeOutcome, AppError> {
    let user = require_user_for_credentials_change(state, headers).await?;
    let username = payload.username.trim();

    if username.is_empty() || payload.current_password.is_empty() || payload.new_password.is_empty()
    {
        return Err(AppError::BadRequest(
            "username, current password and new password are required".to_string(),
        ));
    }
    if payload.new_password != payload.confirm_password {
        return Err(AppError::BadRequest(
            "new password confirmation does not match".to_string(),
        ));
    }
    if payload.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "new password must be at least 8 characters".to_string(),
        ));
    }
    if payload.new_password == payload.current_password {
        return Err(AppError::BadRequest(
            "new password must be different from the current password".to_string(),
        ));
    }
    if username == "admin" || payload.new_password == "admin123456" {
        return Err(AppError::BadRequest(
            "default username and password must be changed".to_string(),
        ));
    }

    verify_password(&payload.current_password, &user.password_hash)?;

    if user_repo::username_exists_for_other_user(&state.db, username, user.id).await? {
        return Err(AppError::BadRequest("username already exists".to_string()));
    }

    let password_hash = user_repo::hash_password(&payload.new_password)?;
    user_repo::update_credentials(&state.db, user.id, username, &password_hash).await?;

    let user = user_repo::find_by_id(&state.db, user.id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let session = issue_session(state, &user)?;
    let response = MeResponse {
        code: "00000",
        data: user_to_me_data(user),
    };

    Ok(MeOutcome { response, session })
}

pub async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<User, AppError> {
    let user = require_user_for_credentials_change(state, headers).await?;
    if user.must_change_credentials != 0 {
        return Err(AppError::Forbidden(
            "credentials change required before continuing".to_string(),
        ));
    }

    Ok(user)
}

pub async fn require_user_for_credentials_change(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<User, AppError> {
    let claims = decode_bearer_token(headers, &state.config.security.jwt_secret)?;

    let user = user_repo::find_by_id(&state.db, claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if user.status != "active" {
        return Err(AppError::Unauthorized);
    }
    ensure_current_token_version(claims.token_version, user.token_version)?;

    Ok(user)
}

fn user_to_me_data(user: User) -> MeData {
    MeData {
        user_id: user.id,
        username: user.username,
        nickname: user.nickname,
        role: user.role,
        status: user.status,
        must_change_credentials: user.must_change_credentials != 0,
    }
}

fn verify_password(password: &str, password_hash: &str) -> Result<(), AppError> {
    let parsed_hash = PasswordHash::new(password_hash)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized)
}

fn issue_session(state: &AppState, user: &User) -> Result<AuthSession, AppError> {
    let csrf_token = generate_csrf_token();
    let access_token = issue_token(state, user, &csrf_token)?;

    Ok(AuthSession {
        access_token,
        csrf_token,
    })
}

fn issue_token(state: &AppState, user: &User, csrf_token: &str) -> Result<String, AppError> {
    let expiry = OffsetDateTime::now_utc() + Duration::hours(state.config.security.jwt_exp_hours);
    let exp = usize::try_from(expiry.unix_timestamp()).map_err(|_| AppError::Internal)?;
    let claims = Claims {
        sub: user.id,
        username: user.username.to_string(),
        role: user.role.to_string(),
        token_version: user.token_version,
        csrf_token: csrf_token.to_string(),
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.security.jwt_secret.as_bytes()),
    )
    .map_err(AppError::from)
}

fn decode_bearer_token(headers: &HeaderMap, jwt_secret: &str) -> Result<Claims, AppError> {
    let (token, requires_csrf) = if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        (value, false)
    } else {
        (
            auth_cookie_token(headers).ok_or(AppError::Unauthorized)?,
            true,
        )
    };

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )?;

    if requires_csrf {
        validate_csrf(headers, &data.claims)?;
    }

    Ok(data.claims)
}

pub fn append_session_headers(
    headers: &mut HeaderMap,
    session: &AuthSession,
    secure_cookie: bool,
) -> Result<(), AppError> {
    let secure = if secure_cookie { "; Secure" } else { "" };
    let cookie = format!(
        "{AUTH_COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Lax{secure}",
        session.access_token
    );
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AppError::Internal)?,
    );
    headers.insert(
        CSRF_HEADER_NAME,
        HeaderValue::from_str(&session.csrf_token).map_err(|_| AppError::Internal)?,
    );
    Ok(())
}

pub fn append_logout_headers(headers: &mut HeaderMap, secure_cookie: bool) -> Result<(), AppError> {
    let secure = if secure_cookie { "; Secure" } else { "" };
    let cookie = format!("{AUTH_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax{secure}; Max-Age=0");
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AppError::Internal)?,
    );
    Ok(())
}

fn auth_cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(name, value)| (name == AUTH_COOKIE_NAME).then_some(value))
}

fn validate_csrf(headers: &HeaderMap, claims: &Claims) -> Result<(), AppError> {
    let header_value = headers
        .get(CSRF_HEADER_NAME)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    if header_value != claims.csrf_token {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

fn generate_csrf_token() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn ensure_current_token_version(
    claims_token_version: i64,
    user_token_version: i64,
) -> Result<(), AppError> {
    if claims_token_version != user_token_version {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::{
        AuthSession, Claims, append_logout_headers, append_session_headers, auth_cookie_token,
        ensure_current_token_version, login_rate_limit_key, validate_csrf,
    };

    #[test]
    fn rejects_stale_token_version() {
        assert!(ensure_current_token_version(0, 1).is_err());
    }

    #[test]
    fn accepts_current_token_version() {
        assert!(ensure_current_token_version(2, 2).is_ok());
    }

    #[test]
    fn ignores_forwarded_headers_when_proxy_headers_are_not_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.10".parse().unwrap());

        assert_eq!(
            login_rate_limit_key(
                &headers,
                Some("192.0.2.55".parse().unwrap()),
                "Admin",
                false
            ),
            "192.0.2.55:admin"
        );
    }

    #[test]
    fn uses_trusted_real_ip_before_forwarded_chain() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.20".parse().unwrap());
        headers.insert("x-forwarded-for", "203.0.113.10, 10.0.0.1".parse().unwrap());

        assert_eq!(
            login_rate_limit_key(&headers, Some("192.0.2.55".parse().unwrap()), "Admin", true),
            "198.51.100.20:admin"
        );
    }

    #[test]
    fn uses_rightmost_forwarded_header_when_real_ip_is_absent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.10, 203.0.113.10".parse().unwrap(),
        );

        assert_eq!(
            login_rate_limit_key(&headers, Some("192.0.2.55".parse().unwrap()), "Admin", true),
            "203.0.113.10:admin"
        );
    }

    #[test]
    fn session_headers_set_secure_http_only_cookie_and_csrf_header() {
        let session = AuthSession {
            access_token: "jwt-value".to_string(),
            csrf_token: "csrf-value".to_string(),
        };
        let mut headers = HeaderMap::new();

        append_session_headers(&mut headers, &session, true).unwrap();

        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(cookie.contains("sublinkx_auth=jwt-value"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(!cookie.contains("Max-Age"));
        assert_eq!(
            headers
                .get("x-csrf-token")
                .and_then(|value| value.to_str().ok()),
            Some("csrf-value")
        );
    }

    #[test]
    fn session_headers_can_omit_secure_cookie_for_local_http() {
        let session = AuthSession {
            access_token: "jwt-value".to_string(),
            csrf_token: "csrf-value".to_string(),
        };
        let mut headers = HeaderMap::new();

        append_session_headers(&mut headers, &session, false).unwrap();

        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(cookie.contains("HttpOnly"));
        assert!(!cookie.contains("Secure"));
    }

    #[test]
    fn logout_headers_clear_auth_cookie() {
        let mut headers = HeaderMap::new();

        append_logout_headers(&mut headers, true).unwrap();

        let cookie = headers
            .get(axum::http::header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(cookie.contains("sublinkx_auth="));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn cookie_auth_requires_matching_csrf_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "sublinkx_auth=jwt-value".parse().unwrap(),
        );
        headers.insert("x-csrf-token", "csrf-value".parse().unwrap());
        let claims = Claims {
            sub: 1,
            username: "admin".to_string(),
            role: "admin".to_string(),
            token_version: 0,
            csrf_token: "csrf-value".to_string(),
            exp: 0,
        };

        assert_eq!(auth_cookie_token(&headers), Some("jwt-value"));
        assert!(validate_csrf(&headers, &claims).is_ok());
    }
}
