use serde::Deserialize;

/// Single listener entry - each port maps to one target
#[derive(Debug, Clone, Deserialize)]
pub struct Listener {
    /// Port to listen on
    pub port: u16,
    /// Target upstream URL (e.g., "http://app1:8080")
    pub target: String,
}

/// Listeners configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub listeners: Vec<Listener>,
}

impl Config {
    /// Load config from YAML file
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config() {
        let yaml = r#"
listeners:
  - port: 440
    target: http://api:3000
  - port: 441
    target: http://app:3001
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config = Config::load(file.path().to_str().unwrap()).unwrap();
        assert_eq!(config.listeners.len(), 2);
        assert_eq!(config.listeners[0].port, 440);
        assert_eq!(config.listeners[0].target, "http://api:3000");
        assert_eq!(config.listeners[1].port, 441);
    }

    #[test]
    fn test_load_empty_listeners() {
        let yaml = "listeners: []";
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config = Config::load(file.path().to_str().unwrap()).unwrap();
        assert!(config.listeners.is_empty());
    }

    #[test]
    fn test_load_invalid_yaml() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"invalid: yaml: content").unwrap();

        let result = Config::load(file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_missing_file() {
        let result = Config::load("/nonexistent/path.yaml");
        assert!(result.is_err());
    }
}
