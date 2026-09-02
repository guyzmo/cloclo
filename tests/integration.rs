use cloclo::auth::{read_token_file, resolve_secret};
use cloclo::chanson::{startup_banner, switching_quote, DEFAULT_PORT};
use cloclo::config::{ClocloConfig, GeneralConfig, ProfileConfig, SecretSource};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

// ============================================================================
// CONFIG TESTS
// ============================================================================

#[test]
fn test_parse_full_config() {
    let config_toml = r#"
[general]
port = 9393
bind = "127.0.0.1"
default_profile = "personal"

[profiles.personal]
type = "oauth"
display_name = "Personal Pro (Comme d'habitude)"
token_file = "~/.claude/personal-pro-token"

[profiles.enterprise]
type = "enterprise_sso"
display_name = "Enterprise SSO (Alexandrie Alexandra)"
token_file = "~/.claude/enterprise-token"

[profiles.copilot]
type = "proxy"
display_name = "GitHub Copilot (Le Telephone Pleure)"
upstream_url = "http://localhost:4141"
auth_token = "sk-dummy"
"#;

    let config: ClocloConfig = toml::from_str(config_toml).expect("Failed to parse config");

    // Verify general config
    assert_eq!(config.general.port, 9393);
    assert_eq!(config.general.bind, "127.0.0.1");
    assert_eq!(config.general.default_profile, "personal");

    // Verify all 3 profiles are present
    assert_eq!(config.profiles.len(), 3);

    // Verify personal profile (OAuth)
    assert!(config.profiles.contains_key("personal"));
    if let ProfileConfig::OAuth { display_name, .. } = &config.profiles["personal"] {
        assert_eq!(display_name, "Personal Pro (Comme d'habitude)");
    } else {
        panic!("Expected OAuth profile");
    }

    // Verify enterprise profile (Enterprise SSO)
    assert!(config.profiles.contains_key("enterprise"));
    if let ProfileConfig::EnterpriseSso { display_name, .. } = &config.profiles["enterprise"] {
        assert_eq!(display_name, "Enterprise SSO (Alexandrie Alexandra)");
    } else {
        panic!("Expected EnterpriseSso profile");
    }

    // Verify copilot profile (Proxy)
    assert!(config.profiles.contains_key("copilot"));
    if let ProfileConfig::Proxy { display_name, .. } = &config.profiles["copilot"] {
        assert_eq!(display_name, "GitHub Copilot (Le Telephone Pleure)");
    } else {
        panic!("Expected Proxy profile");
    }
}

#[test]
fn test_config_roundtrip() {
    // Create a config programmatically
    let mut profiles = HashMap::new();

    profiles.insert(
        "test_profile".to_string(),
        ProfileConfig::OAuth {
            display_name: "Test Profile".to_string(),
            token_file: PathBuf::from("~/test-token"),
            base_url: Some("https://test.example.com".to_string()),
            model: Some("claude-3-sonnet".to_string()),
        },
    );

    let original = ClocloConfig {
        general: GeneralConfig {
            port: 8080,
            bind: "0.0.0.0".to_string(),
            default_profile: "test_profile".to_string(),
            log_file: None,
            pid_file: None,
            claude_bin: None,
        },
        profiles,
    };

    // Serialize to TOML
    let toml_str = toml::to_string(&original).expect("Failed to serialize config");

    // Deserialize back
    let deserialized: ClocloConfig = toml::from_str(&toml_str).expect("Failed to deserialize config");

    // Verify equality of key fields
    assert_eq!(deserialized.general.port, 8080);
    assert_eq!(deserialized.general.bind, "0.0.0.0");
    assert_eq!(deserialized.general.default_profile, "test_profile");
    assert_eq!(deserialized.profiles.len(), 1);

    let profile = &deserialized.profiles["test_profile"];
    assert_eq!(profile.display_name(), "Test Profile");
    assert_eq!(profile.model(), Some("claude-3-sonnet"));
    assert_eq!(profile.base_url(), Some("https://test.example.com"));
}

#[test]
fn test_config_defaults() {
    // Parse an empty TOML string (or minimal)
    let config_toml = r#"
[general]

[profiles]
"#;

    let config: ClocloConfig = toml::from_str(config_toml).expect("Failed to parse config");

    // Verify GeneralConfig defaults are applied
    assert_eq!(config.general.port, DEFAULT_PORT);
    assert_eq!(config.general.bind, "127.0.0.1");
    assert_eq!(config.general.default_profile, "default");
    assert!(config.general.log_file.is_none());
    assert!(config.general.pid_file.is_none());
    assert!(config.general.claude_bin.is_none());
}

#[test]
fn test_profile_display_name() {
    // Test OAuth profile
    let oauth_profile = ProfileConfig::OAuth {
        display_name: "My OAuth Profile".to_string(),
        token_file: PathBuf::from("~/token"),
        base_url: None,
        model: None,
    };
    assert_eq!(oauth_profile.display_name(), "My OAuth Profile");

    // Test EnterpriseSso profile
    let sso_profile = ProfileConfig::EnterpriseSso {
        display_name: "My SSO Profile".to_string(),
        token_file: PathBuf::from("~/token"),
        base_url: None,
        model: None,
    };
    assert_eq!(sso_profile.display_name(), "My SSO Profile");

    // Test Proxy profile
    let proxy_profile = ProfileConfig::Proxy {
        display_name: "My Proxy Profile".to_string(),
        upstream_url: "http://localhost:8000".to_string(),
        auth_token: None,
        model: None,
        subprocess: None,
    };
    assert_eq!(proxy_profile.display_name(), "My Proxy Profile");
}

// ============================================================================
// AUTH TESTS
// ============================================================================

#[test]
fn test_resolve_secret_literal() {
    let secret = SecretSource::Literal("my-secret-value".to_string());
    let resolved = resolve_secret(&secret).expect("Failed to resolve secret");
    assert_eq!(resolved, "my-secret-value");
}

#[test]
fn test_resolve_secret_env() {
    // Use a unique env var name to avoid conflicts
    let env_var_name = "CLOCLO_TEST_SECRET_12345";
    let expected_value = "test-secret-from-env-12345";

    // Set the environment variable
    std::env::set_var(env_var_name, expected_value);

    let secret = SecretSource::Env {
        env: env_var_name.to_string(),
    };
    let resolved = resolve_secret(&secret).expect("Failed to resolve secret");
    assert_eq!(resolved, expected_value);

    // Clean up
    std::env::remove_var(env_var_name);
}

#[test]
fn test_resolve_secret_file() {
    // Create a temporary file with a token
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("cloclo_test_token_secret.txt");
    let token_value = "test-token-from-file-12345";

    let mut file = fs::File::create(&temp_file).expect("Failed to create temp file");
    write!(file, "{}", token_value).expect("Failed to write to temp file");

    let secret = SecretSource::File {
        file: temp_file.clone(),
    };
    let resolved = resolve_secret(&secret).expect("Failed to resolve secret");
    assert_eq!(resolved, token_value);

    // Clean up
    fs::remove_file(temp_file).expect("Failed to remove temp file");
}

#[test]
fn test_read_token_file_trims_whitespace() {
    // Create a temporary file with whitespace around the token
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("cloclo_test_token_whitespace.txt");
    let token_value = "my-secret-token";

    let mut file = fs::File::create(&temp_file).expect("Failed to create temp file");
    write!(file, "  \n{}\n  \t", token_value).expect("Failed to write to temp file");

    let result = read_token_file(&temp_file).expect("Failed to read token file");
    assert_eq!(result, token_value);

    // Clean up
    fs::remove_file(temp_file).expect("Failed to remove temp file");
}

// ============================================================================
// CHANSON TESTS
// ============================================================================

#[test]
fn test_startup_banner_contains_port_and_profile() {
    let port = 9393;
    let profile = "personal";

    let banner = startup_banner(port, profile);

    // Verify the banner contains the port number
    assert!(
        banner.contains("9393"),
        "Banner should contain port number: {}",
        banner
    );

    // Verify the banner contains the profile name
    assert!(
        banner.contains("personal"),
        "Banner should contain profile name: {}",
        banner
    );

    // Verify the banner contains the tool name
    assert!(
        banner.contains("cloclo"),
        "Banner should contain cloclo: {}",
        banner
    );
}

#[test]
fn test_switching_quote_not_empty() {
    let quote = switching_quote();
    assert!(!quote.is_empty(), "switching_quote() should return a non-empty string");
    assert!(
        quote.len() > 0,
        "Quote should have meaningful length: {}",
        quote
    );
}

#[test]
fn test_startup_banner_visual_structure() {
    let port = 8080;
    let profile = "work";

    let banner = startup_banner(port, profile);

    // Verify banner contains port and profile info
    assert!(banner.contains("8080"), "Banner should contain port");
    assert!(banner.contains("work"), "Banner should contain profile");
    assert!(banner.contains("cloclo"), "Banner should contain tool name");
}
