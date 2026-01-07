//! Tests for configuration loading and required fields.

use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::{NamedTempFile, tempdir};

use crate::network::IpVersion;

use super::*;

mod required_fields {
    use super::*;

    #[test]
    fn missing_url_returns_error() {
        let cli = cli(&["--ip-version", "ipv4"]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(
            result,
            Err(ConfigError::MissingRequired { field: "url", .. })
        ));
    }

    #[test]
    fn missing_ip_version_returns_error() {
        let cli = cli(&["--url", "https://example.com"]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(
            result,
            Err(ConfigError::MissingRequired {
                field: "ip_version",
                ..
            })
        ));
    }

    #[test]
    fn both_required_fields_from_cli() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/");
        assert_eq!(config.ip_version, IpVersion::V4);
    }

    #[test]
    fn required_fields_from_toml() {
        let cli = cli(&[]);
        let toml = toml(
            r#"
            [webhook]
            url = "https://example.com/webhook"
            ip_version = "both"
        "#,
        );

        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/webhook");
        assert_eq!(config.ip_version, IpVersion::Both);
    }
}

mod config_load {
    use super::*;

    #[test]
    fn load_from_config_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
            [webhook]
            url = "https://example.com/webhook"
            ip_version = "ipv4"
        "#
        )
        .unwrap();

        let cli = cli(&["--config", file.path().to_str().unwrap()]);
        let config = ValidatedConfig::load(&cli).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/webhook");
        assert_eq!(config.ip_version, IpVersion::V4);
    }

    #[test]
    fn load_without_config_file() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv6"]);
        let config = ValidatedConfig::load(&cli).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/");
        assert_eq!(config.ip_version, IpVersion::V6);
    }

    #[test]
    fn load_nonexistent_config_file_returns_error() {
        let cli = cli(&["--config", "nonexistent_file_12345.toml"]);
        let result = ValidatedConfig::load(&cli);

        assert!(matches!(result, Err(ConfigError::FileRead { .. })));
    }
}

mod write_config {
    use super::super::super::validated::write_default_config;
    use super::*;

    #[test]
    fn write_default_config_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test-config.toml");

        write_default_config(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("[webhook]"));
        assert!(content.contains("[filter]"));
        assert!(content.contains("[monitor]"));
        assert!(content.contains("[retry]"));
    }

    #[test]
    fn write_default_config_to_invalid_path_returns_error() {
        // Try writing to an invalid path (directory that doesn't exist)
        let path = Path::new("/nonexistent_dir_12345/config.toml");
        let result = write_default_config(path);

        assert!(matches!(result, Err(ConfigError::FileWrite { .. })));
    }
}
