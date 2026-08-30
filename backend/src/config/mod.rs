use std::{env, num::ParseIntError};

use thiserror::Error;

pub const DEFAULT_JWT_SECRET: &str = "change-me-in-production";
pub const DEFAULT_BOOTSTRAP_ADMIN_USERNAME: &str = "admin";
pub const DEFAULT_BOOTSTRAP_ADMIN_PASSWORD: &str = "admin123456";
pub const MIN_JWT_EXP_HOURS: i64 = 1;
pub const MAX_JWT_EXP_HOURS: i64 = 720;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub ip_intelligence: IpIntelligenceConfig,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub environment: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub jwt_exp_hours: i64,
    pub bootstrap_admin_username: String,
    pub bootstrap_admin_password: String,
    pub trust_proxy_headers: bool,
    pub auth_cookie_secure: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct IpIntelligenceConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_token: String,
    pub source_key: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid port value: {0}")]
    InvalidPort(#[from] ParseIntError),
    #[error("{0}")]
    InvalidProductionConfig(String),
    #[error("{0}")]
    SecurityConfig(String),
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let port = env::var("APP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?;
        let environment = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/app.db".to_string());
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string());
        let jwt_exp_hours = env::var("JWT_EXP_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<i64>()?;
        validate_jwt_exp_hours(jwt_exp_hours)?;
        let bootstrap_admin_username = env::var("BOOTSTRAP_ADMIN_USERNAME")
            .unwrap_or_else(|_| DEFAULT_BOOTSTRAP_ADMIN_USERNAME.to_string());
        let bootstrap_admin_password = env::var("BOOTSTRAP_ADMIN_PASSWORD")
            .unwrap_or_else(|_| DEFAULT_BOOTSTRAP_ADMIN_PASSWORD.to_string());
        let trust_proxy_headers = env::var("TRUST_PROXY_HEADERS")
            .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
            .unwrap_or(false);
        let auth_cookie_secure = env::var("AUTH_COOKIE_SECURE")
            .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
            .unwrap_or_else(|_| is_production_env(&environment));
        let ip_intelligence_enabled = env_flag("IP_INTELLIGENCE_ENABLED", false);
        let ip_intelligence_base_url = env::var("IP_INTELLIGENCE_BASE_URL")
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string();
        let ip_intelligence_api_token = env::var("IP_INTELLIGENCE_API_TOKEN").unwrap_or_default();
        let ip_intelligence_source_key =
            env::var("IP_INTELLIGENCE_SOURCE_KEY").unwrap_or_else(|_| "sublinkx-rs".to_string());

        validate_ip_intelligence_config(
            ip_intelligence_enabled,
            &ip_intelligence_base_url,
            &ip_intelligence_api_token,
            &ip_intelligence_source_key,
        )?;

        if is_production_env(&environment) {
            validate_production_secret(&jwt_secret)?;
        }

        Ok(Self {
            server: ServerConfig { port, environment },
            database: DatabaseConfig { url: database_url },
            security: SecurityConfig {
                jwt_secret,
                jwt_exp_hours,
                bootstrap_admin_username,
                bootstrap_admin_password,
                trust_proxy_headers,
                auth_cookie_secure,
            },
            ip_intelligence: IpIntelligenceConfig {
                enabled: ip_intelligence_enabled,
                base_url: ip_intelligence_base_url,
                api_token: ip_intelligence_api_token,
                source_key: ip_intelligence_source_key,
            },
        })
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    env::var(name)
        .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
        .unwrap_or(default)
}

fn validate_ip_intelligence_config(
    enabled: bool,
    base_url: &str,
    api_token: &str,
    source_key: &str,
) -> Result<(), ConfigError> {
    if !enabled {
        return Ok(());
    }
    let url = url::Url::parse(base_url).map_err(|_| {
        ConfigError::SecurityConfig(
            "IP_INTELLIGENCE_BASE_URL must be a valid http or https URL".to_string(),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConfigError::SecurityConfig(
            "IP_INTELLIGENCE_BASE_URL must be an http or https origin without credentials, query, or fragment"
                .to_string(),
        ));
    }
    if !(32..=512).contains(&api_token.len())
        || api_token.trim() != api_token
        || api_token.chars().any(char::is_whitespace)
    {
        return Err(ConfigError::SecurityConfig(
            "IP_INTELLIGENCE_API_TOKEN must contain 32 to 512 non-whitespace bytes".to_string(),
        ));
    }
    if source_key.is_empty()
        || source_key.len() > 128
        || source_key.trim() != source_key
        || !source_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(ConfigError::SecurityConfig(
            "IP_INTELLIGENCE_SOURCE_KEY must contain 1 to 128 safe ASCII characters".to_string(),
        ));
    }
    Ok(())
}

pub fn is_production_env(environment: &str) -> bool {
    environment.eq_ignore_ascii_case("production")
}

fn validate_production_secret(jwt_secret: &str) -> Result<(), ConfigError> {
    if jwt_secret == DEFAULT_JWT_SECRET || jwt_secret.trim().len() < 32 {
        return Err(ConfigError::InvalidProductionConfig(
            "JWT_SECRET must be set to a non-default value with at least 32 characters in production"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_jwt_exp_hours(value: i64) -> Result<(), ConfigError> {
    if !(MIN_JWT_EXP_HOURS..=MAX_JWT_EXP_HOURS).contains(&value) {
        return Err(ConfigError::SecurityConfig(format!(
            "JWT_EXP_HOURS must be between {MIN_JWT_EXP_HOURS} and {MAX_JWT_EXP_HOURS}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_JWT_SECRET, validate_ip_intelligence_config, validate_jwt_exp_hours,
        validate_production_secret,
    };

    #[test]
    fn rejects_default_production_jwt_secret() {
        assert!(validate_production_secret(DEFAULT_JWT_SECRET).is_err());
    }

    #[test]
    fn rejects_short_production_jwt_secret() {
        assert!(validate_production_secret("short-secret").is_err());
    }

    #[test]
    fn accepts_strong_production_jwt_secret() {
        assert!(validate_production_secret("0123456789abcdef0123456789abcdef").is_ok());
    }

    #[test]
    fn rejects_non_positive_jwt_expiry() {
        assert!(validate_jwt_exp_hours(0).is_err());
        assert!(validate_jwt_exp_hours(-1).is_err());
    }

    #[test]
    fn rejects_excessive_jwt_expiry() {
        assert!(validate_jwt_exp_hours(721).is_err());
    }

    #[test]
    fn accepts_reasonable_jwt_expiry() {
        assert!(validate_jwt_exp_hours(24).is_ok());
    }

    #[test]
    fn validates_enabled_ip_intelligence_configuration() {
        assert!(
            validate_ip_intelligence_config(
                true,
                "http://127.0.0.1:8090/api/v1",
                "0123456789abcdef0123456789abcdef",
                "sublinkx-rs:test",
            )
            .is_ok()
        );
        assert!(validate_ip_intelligence_config(true, "file:///tmp/api", "x", "bad key").is_err());
        assert!(validate_ip_intelligence_config(false, "", "", "").is_ok());
    }
}
