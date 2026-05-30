use std::{env, num::ParseIntError};

use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
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
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid port value: {0}")]
    InvalidPort(#[from] ParseIntError),
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let port = env::var("APP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?;
        let environment = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/app.db".to_string());
        let jwt_secret =
            env::var("JWT_SECRET").unwrap_or_else(|_| "change-me-in-production".to_string());
        let jwt_exp_hours = env::var("JWT_EXP_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<i64>()?;
        let bootstrap_admin_username =
            env::var("BOOTSTRAP_ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let bootstrap_admin_password =
            env::var("BOOTSTRAP_ADMIN_PASSWORD").unwrap_or_else(|_| "change-me-now".to_string());

        Ok(Self {
            server: ServerConfig { port, environment },
            database: DatabaseConfig { url: database_url },
            security: SecurityConfig {
                jwt_secret,
                jwt_exp_hours,
                bootstrap_admin_username,
                bootstrap_admin_password,
            },
        })
    }
}
