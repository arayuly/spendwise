use std::{env, net::IpAddr, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub app_origin: String,
    pub host: IpAddr,
    pub port: u16,
    pub cookie_secure: bool,
    pub production: bool,
    pub frontend_dir: PathBuf,
    pub run_migrations: bool,
    pub access_token_minutes: i64,
    pub refresh_token_days: i64,
    pub database_max_connections: u32,
    pub auth_rate_limit_per_minute: u32,
}

fn setting<T: std::str::FromStr>(name: &str, default: T) -> Result<T, String> {
    match env::var(name) {
        Ok(v) => v.parse().map_err(|_| format!("Invalid {name}")),
        Err(_) => Ok(default),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".into());
        if !["development", "production", "test"].contains(&app_env.as_str()) {
            return Err("APP_ENV must be development, production or test".into());
        }
        let config = Self {
            database_url: env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required")?,
            jwt_secret: env::var("JWT_SECRET")
                .map_err(|_| "JWT_SECRET is required (at least 48 random characters)")?,
            app_origin: env::var("APP_ORIGIN").unwrap_or_else(|_| "http://localhost:8080".into()),
            host: setting(
                "HOST",
                "127.0.0.1"
                    .parse::<IpAddr>()
                    .map_err(|_| "Invalid default host")?,
            )?,
            port: setting("PORT", 8080)?,
            cookie_secure: setting("COOKIE_SECURE", app_env == "production")?,
            production: app_env == "production",
            frontend_dir: env::var_os("FRONTEND_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("frontend/dist")),
            run_migrations: setting("RUN_MIGRATIONS", true)?,
            access_token_minutes: setting("ACCESS_TOKEN_MINUTES", 10)?,
            refresh_token_days: setting("REFRESH_TOKEN_DAYS", 30)?,
            database_max_connections: setting("DATABASE_MAX_CONNECTIONS", 10)?,
            auth_rate_limit_per_minute: setting("AUTH_RATE_LIMIT_PER_MINUTE", 20)?,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.jwt_secret.len() < 48
            || self.jwt_secret.contains("CHANGE_ME")
            || self
                .jwt_secret
                .chars()
                .collect::<std::collections::HashSet<_>>()
                .len()
                < 12
        {
            return Err(
                "JWT_SECRET must contain at least 48 random characters, not an example value"
                    .into(),
            );
        }
        let origin = url::Url::parse(&self.app_origin).map_err(|_| "Invalid APP_ORIGIN URL")?;
        if origin.origin().ascii_serialization() != self.app_origin
            || !["http", "https"].contains(&origin.scheme())
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err(
                "APP_ORIGIN must be an exact http(s) origin without a trailing slash".into(),
            );
        }
        if self.production && (origin.scheme() != "https" || !self.cookie_secure) {
            return Err("Production requires HTTPS APP_ORIGIN and COOKIE_SECURE=true".into());
        }
        if !(1..=15).contains(&self.access_token_minutes)
            || !(1..=90).contains(&self.refresh_token_days)
            || !(1..=100).contains(&self.database_max_connections)
            || !(1..=300).contains(&self.auth_rate_limit_per_minute)
        {
            return Err("Invalid token lifetime, pool size or authentication rate limit".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn production() -> Config {
        Config {
            database_url: "postgresql://localhost/test".into(),
            jwt_secret: "test-config-secret-0123456789abcdefghijklmnopqrstuvwxyz".into(),
            app_origin: "https://money.example.com".into(),
            host: "127.0.0.1".parse().unwrap(),
            port: 8080,
            cookie_secure: true,
            production: true,
            frontend_dir: "frontend/dist".into(),
            run_migrations: true,
            access_token_minutes: 10,
            refresh_token_days: 30,
            database_max_connections: 10,
            auth_rate_limit_per_minute: 20,
        }
    }
    #[test]
    fn rejects_unsafe_production_configuration() {
        assert!(production().validate().is_ok());
        let mut config = production();
        config.cookie_secure = false;
        assert!(config.validate().is_err());
        let mut config = production();
        config.app_origin = "http://money.example.com".into();
        assert!(config.validate().is_err());
        let mut config = production();
        config.jwt_secret = "x".repeat(64);
        assert!(config.validate().is_err());
        let mut config = production();
        config.app_origin.push('/');
        assert!(config.validate().is_err());
        let mut config = production();
        config.access_token_minutes = 60;
        assert!(config.validate().is_err());
    }
}
