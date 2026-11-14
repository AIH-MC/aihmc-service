use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey},
};
use serde::{Deserialize, Deserializer};
use url::Url;

#[derive(Clone, Copy, Default)]
pub struct PositiveDuration(pub Duration);

impl<'de> Deserialize<'de> for PositiveDuration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse_duration(&text)
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom("expected a positive Go duration"))
    }
}

// Keep Go's compound duration units (including fractional and subsecond values).
fn parse_duration(text: &str) -> Option<Duration> {
    let mut remaining = text.strip_prefix('+').unwrap_or(text);
    let mut nanos = 0.0;
    while !remaining.is_empty() {
        let end = remaining.find(|c: char| !c.is_ascii_digit() && c != '.')?;
        let number = remaining[..end].parse::<f64>().ok()?;
        remaining = &remaining[end..];
        let (unit, multiplier) = [
            ("ns", 1.0),
            ("us", 1_000.0),
            ("µs", 1_000.0),
            ("μs", 1_000.0),
            ("ms", 1_000_000.0),
            ("s", 1_000_000_000.0),
            ("m", 60_000_000_000.0),
            ("h", 3_600_000_000_000.0),
        ]
        .into_iter()
        .find(|(unit, _)| remaining.starts_with(unit))?;
        nanos += (number * multiplier).floor();
        remaining = &remaining[unit.len()..];
    }
    if nanos <= 0.0 || nanos >= i64::MAX as f64 {
        return None;
    }
    Some(Duration::from_nanos(nanos as u64))
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: Server,
    pub site: Site,
    pub database: Database,
    pub ucenter: UCenter,
    pub auth: Auth,
    pub textures: Textures,
    pub admin: Admin,
    pub logging: Logging,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Server {
    pub listen: String,
    pub public_base_url: String,
    pub read_header_timeout: PositiveDuration,
    pub read_timeout: PositiveDuration,
    pub write_timeout: PositiveDuration,
    pub shutdown_timeout: PositiveDuration,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Site {
    pub name: String,
    pub implementation_name: String,
    pub implementation_version: String,
    pub homepage: String,
    pub register_url: String,
    pub allow_non_email_login: bool,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub password_file: PathBuf,
    pub charset: String,
    pub collation: String,
    pub max_open_connections: u32,
    pub max_idle_connections: u32,
    pub connect_timeout: PositiveDuration,
    pub query_timeout: PositiveDuration,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UCenter {
    pub endpoint: String,
    pub app_id: String,
    pub secret_file: PathBuf,
    pub nonce: String,
    pub request_timeout: PositiveDuration,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Auth {
    pub private_key_file: PathBuf,
    pub public_key_file: PathBuf,
    pub login_record_retention: PositiveDuration,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Textures {
    pub directory: PathBuf,
    pub public_base_url: String,
    pub request_timeout: PositiveDuration,
    pub mojang_base_url: String,
    pub crafatar_base_url: String,
    pub crafthead_base_url: String,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Admin {
    pub super_key_file: PathBuf,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Logging {
    pub level: String,
    pub format: String,
}

// Intentionally no Debug: this structure contains runtime secrets and private keys.
pub struct RuntimeConfig {
    pub config: Config,
    pub database_password: String,
    pub ucenter_secret: String,
    pub super_key: String,
    pub private_key: RsaPrivateKey,
    pub public_key_pem: String,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error: toml::de::Error| {
            // TOML diagnostics may embed secrets from the source. Report only location.
            match error.span() {
                Some(span) => format!(
                    "invalid configuration field or TOML value at byte {}",
                    span.start
                ),
                None => "invalid configuration field or TOML value".into(),
            }
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        let required = [
            ("site.name", self.site.name.as_str()),
            ("site.implementation_name", &self.site.implementation_name),
            (
                "site.implementation_version",
                &self.site.implementation_version,
            ),
            ("database.host", &self.database.host),
            ("database.name", &self.database.name),
            ("database.user", &self.database.user),
            ("database.charset", &self.database.charset),
            ("database.collation", &self.database.collation),
            ("ucenter.app_id", &self.ucenter.app_id),
            ("ucenter.nonce", &self.ucenter.nonce),
        ];
        for (name, value) in required {
            if value.is_empty() {
                return Err(format!("{name} is required"));
            }
        }
        for (name, path) in [
            ("database.password_file", &self.database.password_file),
            ("ucenter.secret_file", &self.ucenter.secret_file),
            ("admin.super_key_file", &self.admin.super_key_file),
            ("auth.private_key_file", &self.auth.private_key_file),
            ("auth.public_key_file", &self.auth.public_key_file),
            ("textures.directory", &self.textures.directory),
        ] {
            if path.as_os_str().is_empty() {
                return Err(format!("{name} is required"));
            }
        }
        // Hostnames, empty bind hosts and port zero are supported by the Go listener.
        let Some((host, port)) = self.server.listen.rsplit_once(':') else {
            return Err("server.listen must be a host:port address".into());
        };
        if port.parse::<u16>().is_err()
            || (host.contains(':') && !(host.starts_with('[') && host.ends_with(']')))
        {
            return Err("server.listen must be a host:port address".into());
        }
        for (name, value) in [
            ("server.public_base_url", &self.server.public_base_url),
            ("site.homepage", &self.site.homepage),
            ("site.register_url", &self.site.register_url),
            ("ucenter.endpoint", &self.ucenter.endpoint),
            ("textures.public_base_url", &self.textures.public_base_url),
            ("textures.mojang_base_url", &self.textures.mojang_base_url),
            (
                "textures.crafatar_base_url",
                &self.textures.crafatar_base_url,
            ),
            (
                "textures.crafthead_base_url",
                &self.textures.crafthead_base_url,
            ),
        ] {
            let valid = Url::parse(value).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            });
            if !valid {
                return Err(format!(
                    "{name} must be an HTTP or HTTPS URL without credentials"
                ));
            }
        }
        if self.database.port == 0 {
            return Err("database.port must be between 1 and 65535".into());
        }
        if self.database.max_open_connections == 0
            || self.database.max_idle_connections > self.database.max_open_connections
        {
            return Err("database connection pool sizes are invalid".into());
        }
        for (name, duration) in [
            (
                "server.read_header_timeout",
                self.server.read_header_timeout,
            ),
            ("server.read_timeout", self.server.read_timeout),
            ("server.write_timeout", self.server.write_timeout),
            ("server.shutdown_timeout", self.server.shutdown_timeout),
            ("database.connect_timeout", self.database.connect_timeout),
            ("database.query_timeout", self.database.query_timeout),
            ("ucenter.request_timeout", self.ucenter.request_timeout),
            (
                "auth.login_record_retention",
                self.auth.login_record_retention,
            ),
            ("textures.request_timeout", self.textures.request_timeout),
        ] {
            if duration.0.is_zero() {
                return Err(format!("{name} must be a positive duration"));
            }
        }
        if !matches!(
            self.logging.level.as_str(),
            "debug" | "info" | "warn" | "error"
        ) {
            return Err("logging.level must be debug, info, warn or error".into());
        }
        if !matches!(self.logging.format.as_str(), "json" | "text") {
            return Err("logging.format must be json or text".into());
        }
        Ok(())
    }
}

impl RuntimeConfig {
    pub fn load(path: &Path) -> Result<Self, String> {
        if path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case("config.example.toml")
        }) {
            return Err("config.example.toml is a template; provide config.toml".into());
        }
        let text =
            fs::read_to_string(path).map_err(|_| "cannot read runtime configuration file")?;
        let mut config = Config::parse(&text)?;
        let absolute =
            std::path::absolute(path).map_err(|_| "cannot resolve configuration directory")?;
        let base = absolute
            .parent()
            .ok_or("cannot resolve configuration directory")?;
        for path in [
            &mut config.database.password_file,
            &mut config.ucenter.secret_file,
            &mut config.admin.super_key_file,
            &mut config.auth.private_key_file,
            &mut config.auth.public_key_file,
            &mut config.textures.directory,
        ] {
            if path.is_relative() {
                *path = base.join(&path);
            }
        }
        let database_password =
            read_secret(&config.database.password_file, "database.password_file")?;
        let ucenter_secret = read_secret(&config.ucenter.secret_file, "ucenter.secret_file")?;
        let super_key = read_secret(&config.admin.super_key_file, "admin.super_key_file")?;
        let private_pem = fs::read_to_string(&config.auth.private_key_file)
            .map_err(|_| "cannot read auth.private_key_file")?;
        let private_key = RsaPrivateKey::from_pkcs1_pem(&private_pem)
            .or_else(|_| RsaPrivateKey::from_pkcs8_pem(&private_pem))
            .map_err(|_| "auth.private_key_file must contain a PKCS1 or PKCS8 RSA private key")?;
        private_key
            .validate()
            .map_err(|_| "auth.private_key_file contains an invalid RSA key")?;
        let public_key_pem = fs::read_to_string(&config.auth.public_key_file)
            .map_err(|_| "cannot read auth.public_key_file")?;
        let public_key = RsaPublicKey::from_pkcs1_pem(&public_key_pem)
            .or_else(|_| RsaPublicKey::from_public_key_pem(&public_key_pem))
            .map_err(|_| "auth.public_key_file must contain a PKCS1 or PKIX RSA public key")?;
        if public_key != private_key.to_public_key() {
            return Err("auth public and private keys do not match".into());
        }
        if !config.textures.directory.is_dir() {
            return Err("textures.directory must reference an existing directory".into());
        }
        // create_new prevents clobbering an existing file even during concurrent startup.
        let probe = config
            .textures
            .directory
            .join(format!(".write-check-{}", std::process::id()));
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .map_err(|_| "textures.directory must be writable")?;
        drop(file);
        fs::remove_file(probe).map_err(|_| "cannot complete textures.directory access check")?;
        Ok(Self {
            config,
            database_password,
            ucenter_secret,
            super_key,
            private_key,
            public_key_pem,
        })
    }
}

fn read_secret(path: &Path, field: &str) -> Result<String, String> {
    let value = fs::read_to_string(path)
        .map_err(|_| format!("{field} must reference a readable nonempty secret"))?;
    let value = value.trim_end_matches(['\r', '\n']);
    if value.is_empty() {
        return Err(format!("{field} must reference a readable nonempty secret"));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../config.example.toml");

    #[test]
    fn parses_existing_template_and_go_durations() {
        assert!(Config::parse(TEMPLATE).is_ok());
        for (text, nanos) in [
            ("1h30m", 5_400_000_000_000),
            ("+1.5ms", 1_500_000),
            (".5s", 500_000_000),
            ("1µs", 1_000),
        ] {
            assert_eq!(parse_duration(text), Some(Duration::from_nanos(nanos)));
        }
        for text in ["", "0s", "-1s", "1d", "1", "1..2s", "999999999999h"] {
            assert!(parse_duration(text).is_none(), "accepted {text}");
        }
    }

    #[test]
    fn rejects_invalid_configuration_without_secret_values() {
        let cases = [
            format!("{TEMPLATE}\nsecret_typo = \"private-marker\""),
            TEMPLATE.replace(
                "connect_timeout = \"5s\"",
                "connect_timeout = \"private-marker\"",
            ),
            TEMPLATE.replace("connect_timeout = \"5s\"", "connect_timeout = \"0s\""),
            TEMPLATE.replace("host = \"127.0.0.1\"", "host = \"\""),
            TEMPLATE.replace("port = 3306", "port = 0"),
            TEMPLATE.replace("max_idle_connections = 5", "max_idle_connections = 15"),
            TEMPLATE.replace("port = 3306", "port = \"private-marker\""),
            TEMPLATE.replace("https://ucenter.example.com/api", "file:///private-marker"),
            TEMPLATE.replace("[database]", "[database]\nport = 1234"),
        ];
        for text in cases {
            let error = Config::parse(&text)
                .err()
                .expect("invalid configuration accepted");
            assert!(!error.contains("private-marker"));
        }
    }

    #[test]
    fn does_not_use_template_when_runtime_config_missing() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("config.example.toml"), TEMPLATE).unwrap();
        assert!(RuntimeConfig::load(&directory.path().join("config.toml")).is_err());
        for name in ["config.example.toml", "CONFIG.EXAMPLE.TOML"] {
            assert!(
                RuntimeConfig::load(&directory.path().join(name))
                    .err()
                    .unwrap()
                    .contains("template")
            );
        }
    }
}
