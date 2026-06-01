use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub platforms: HashMap<String, PlatformConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub tracking_params: Vec<String>,
    #[serde(default)]
    pub tracking_prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformConfig {
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub tracking_params: Vec<String>,
    #[serde(default)]
    pub tracking_prefixes: Vec<String>,
    #[serde(default)]
    pub normalize_host: Option<String>,
    #[serde(default)]
    pub cleaner: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        if let Some(path) = config_file_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path)
                    && let Ok(config) = toml::from_str(&content)
                {
                    return config;
                }
            } else {
                init_config_file(&path);
            }
        }
        Config::default()
    }
}

fn init_config_file(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, DEFAULT_CONFIG_TOML);
}

const DEFAULT_CONFIG_TOML: &str = "# link-zapper configuration
# Auto-generated with default values.
# Edit this file to customize tracking parameters per platform.

[general]
# Tracking parameters removed from ALL URLs
tracking_params = [
    \"utm_source\",
    \"utm_medium\",
    \"utm_campaign\",
    \"utm_term\",
    \"utm_content\",
    \"utm_id\",
    \"fbclid\",
    \"gclid\",
    \"dclid\",
    \"msclkid\",
]
# Query parameters starting with these prefixes are also removed globally
tracking_prefixes = [\"utm_\"]

[platforms.youtube]
domains = [
    \"youtube.com\",
    \"www.youtube.com\",
    \"m.youtube.com\",
    \"youtu.be\",
    \"www.youtu.be\",
    \"music.youtube.com\",
    \"www.music.youtube.com\",
    \"youtube-nocookie.com\",
    \"www.youtube-nocookie.com\",
]
# Removed only on YouTube URLs
tracking_params = [\"si\"]
# Built-in cleaners: /redirect unwrapping and video URL reconstruction
# (youtube.com/watch?v=ID → youtu.be/ID)
cleaner = \"youtube\"

[platforms.x]
domains = [
    \"x.com\",
    \"www.x.com\",
    \"m.x.com\",
    \"twitter.com\",
    \"www.twitter.com\",
    \"m.twitter.com\",
]
tracking_params = [\"s\"]
normalize_host = \"x.com\"
# Built-in: t.co short URLs are automatically resolved via HTTP redirect
# and the destination URL is then cleaned through the full pipeline

[platforms.instagram]
domains = [
    \"instagram.com\",
    \"www.instagram.com\",
    \"m.instagram.com\",
]
tracking_params = [\"igshid\", \"igsh\"]

[platforms.facebook]
domains = [
    \"facebook.com\",
    \"www.facebook.com\",
    \"m.facebook.com\",
    \"fb.com\",
    \"www.fb.com\",
]
tracking_params = [\"mibextid\", \"__tn__\"]
";

impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig {
                tracking_params: vec![
                    "utm_source".into(),
                    "utm_medium".into(),
                    "utm_campaign".into(),
                    "utm_term".into(),
                    "utm_content".into(),
                    "utm_id".into(),
                    "fbclid".into(),
                    "gclid".into(),
                    "dclid".into(),
                    "msclkid".into(),
                ],
                tracking_prefixes: vec!["utm_".into()],
            },
            platforms: {
                let mut m = HashMap::new();

                m.insert(
                    "youtube".into(),
                    PlatformConfig {
                        domains: vec![
                            "youtube.com".into(),
                            "www.youtube.com".into(),
                            "m.youtube.com".into(),
                            "youtu.be".into(),
                            "www.youtu.be".into(),
                            "music.youtube.com".into(),
                            "www.music.youtube.com".into(),
                            "youtube-nocookie.com".into(),
                            "www.youtube-nocookie.com".into(),
                        ],
                        tracking_params: vec!["si".into()],
                        tracking_prefixes: vec![],
                        normalize_host: None,
                        cleaner: Some("youtube".into()),
                    },
                );

                m.insert(
                    "x".into(),
                    PlatformConfig {
                        domains: vec![
                            "x.com".into(),
                            "www.x.com".into(),
                            "m.x.com".into(),
                            "twitter.com".into(),
                            "www.twitter.com".into(),
                            "m.twitter.com".into(),
                        ],
                        tracking_params: vec!["s".into()],
                        tracking_prefixes: vec![],
                        normalize_host: Some("x.com".into()),
                        cleaner: None,
                    },
                );

                m.insert(
                    "instagram".into(),
                    PlatformConfig {
                        domains: vec![
                            "instagram.com".into(),
                            "www.instagram.com".into(),
                            "m.instagram.com".into(),
                        ],
                        tracking_params: vec!["igshid".into(), "igsh".into()],
                        tracking_prefixes: vec![],
                        normalize_host: None,
                        cleaner: None,
                    },
                );

                m.insert(
                    "facebook".into(),
                    PlatformConfig {
                        domains: vec![
                            "facebook.com".into(),
                            "www.facebook.com".into(),
                            "m.facebook.com".into(),
                            "fb.com".into(),
                            "www.fb.com".into(),
                        ],
                        tracking_params: vec!["mibextid".into(), "__tn__".into()],
                        tracking_prefixes: vec![],
                        normalize_host: None,
                        cleaner: None,
                    },
                );

                m
            },
        }
    }
}

fn config_file_path() -> Option<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        return None;
    };
    Some(base.join("link-zapper").join("config.toml"))
}

pub fn find_platform<'a>(host: &str, config: &'a Config) -> Option<(&'a str, &'a PlatformConfig)> {
    let host_lower = host.to_lowercase();
    for (name, platform) in &config.platforms {
        for domain in &platform.domains {
            if host_lower == *domain {
                return Some((name, platform));
            }
        }
    }
    None
}


