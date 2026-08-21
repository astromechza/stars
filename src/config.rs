#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub dev_user: Option<String>,
}

impl Config {
    pub fn from_env() -> Config {
        Config::from_map(|k| std::env::var(k).ok())
    }

    // Injectable for tests.
    pub fn from_map(get: impl Fn(&str) -> Option<String>) -> Config {
        Config {
            database_url: get("DATABASE_URL").unwrap_or_else(|| "sqlite://stars.db".to_string()),
            bind_addr: get("BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8080".to_string()),
            dev_user: get("DEV_USER").filter(|s| !s.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_from_map() {
        let cfg = Config::from_map(|_| None);
        assert_eq!(cfg.database_url, "sqlite://stars.db");
        assert_eq!(cfg.bind_addr, "0.0.0.0:8080");
        assert!(cfg.dev_user.is_none());
    }

    #[test]
    fn overrides_from_map() {
        let cfg = Config::from_map(|k| match k {
            "DATABASE_URL" => Some("sqlite:///data/x.db".into()),
            "DEV_USER" => Some("alice".into()),
            _ => None,
        });
        assert_eq!(cfg.database_url, "sqlite:///data/x.db");
        assert_eq!(cfg.dev_user.as_deref(), Some("alice"));
    }
}
