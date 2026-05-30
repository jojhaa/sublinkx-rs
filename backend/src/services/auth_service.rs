use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::{HeaderMap, header};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
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
    exp: usize,
}

pub async fn login(state: &AppState, payload: LoginRequest) -> Result<LoginResponse, AppError> {
    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(AppError::BadRequest(
            "username and password are required".to_string(),
        ));
    }

    let user = user_repo::find_by_username(&state.db, payload.username.trim())
        .await?
        .ok_or(AppError::Unauthorized)?;

    if user.status != "active" {
        return Err(AppError::Unauthorized);
    }

    verify_password(&payload.password, &user.password_hash)?;

    let access_token = issue_token(state, user.id, &user.username, &user.role)?;

    Ok(LoginResponse {
        code: "00000",
        data: LoginTokenData {
            access_token,
            token_type: "Bearer",
            expires_in_hours: state.config.security.jwt_exp_hours,
            user: user_to_me_data(user),
        },
    })
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
) -> Result<MeResponse, AppError> {
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
    if username == user.username {
        return Err(AppError::BadRequest(
            "new username must be different from the current username".to_string(),
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

    Ok(MeResponse {
        code: "00000",
        data: user_to_me_data(user),
    })
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

fn issue_token(
    state: &AppState,
    user_id: i64,
    username: &str,
    role: &str,
) -> Result<String, AppError> {
    let expiry = OffsetDateTime::now_utc() + Duration::hours(state.config.security.jwt_exp_hours);
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        role: role.to_string(),
        exp: expiry.unix_timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.security.jwt_secret.as_bytes()),
    )
    .map_err(AppError::from)
}

fn decode_bearer_token(headers: &HeaderMap, jwt_secret: &str) -> Result<Claims, AppError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let token = value
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(data.claims)
}
