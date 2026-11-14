use std::{
    sync::LazyLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use reqwest::{
    Client as HttpClient,
    header::{HeaderMap, HeaderValue},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::config::RuntimeConfig;

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9_.+\-]+@[a-zA-Z0-9\-]+\.[a-zA-Z0-9\-.]+$")
        .expect("valid legacy email expression")
});

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidCredentials,
    Upstream,
}

#[derive(Debug, PartialEq, Eq)]
pub struct User {
    pub uid: i64,
    pub name: String,
    pub email: String,
}

pub struct Client {
    endpoint: String,
    app_id: String,
    nonce: String,
    secret: String,
    timeout: Duration,
    http: HttpClient,
}

pub fn signature(nonce: &str, timestamp: i64, secret: &str) -> String {
    STANDARD.encode(format!(
        "{:x}",
        Sha256::digest(format!("{nonce}{timestamp}{secret}"))
    ))
}

pub fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_secs() as i64
}

impl Client {
    pub fn new(runtime: &RuntimeConfig) -> Result<Self, Error> {
        let config = &runtime.config.ucenter;
        Self::with_settings(
            &config.endpoint,
            &config.app_id,
            &config.nonce,
            &runtime.ucenter_secret,
            config.request_timeout.0,
        )
    }

    pub fn with_settings(
        endpoint: &str,
        app_id: &str,
        nonce: &str,
        secret: &str,
        timeout: Duration,
    ) -> Result<Self, Error> {
        // A provider may already have been installed by another client in this process.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = HttpClient::builder()
            .tls_backend_rustls()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|_| Error::Upstream)?;
        Ok(Self {
            endpoint: endpoint.into(),
            app_id: app_id.into(),
            nonce: nonce.into(),
            secret: secret.into(),
            timeout,
            http,
        })
    }

    async fn request(&self, path: &str, body: Value, headers: HeaderMap) -> Result<Value, Error> {
        // Existing endpoints may route through /api/?/token. Do not URL-join paths.
        let response = self
            .http
            .post(format!("{}{path}", self.endpoint))
            .headers(headers)
            .header("content-type", "application/json; charset=utf-8")
            .json(&body)
            .send()
            .await
            .map_err(|_| Error::Upstream)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(Error::Upstream);
        }
        response.json().await.map_err(|_| Error::Upstream)
    }

    async fn headers(&self) -> Result<HeaderMap, Error> {
        let timestamp = unix_timestamp();
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("appid", self.app_id.clone()),
            ("nonce", self.nonce.clone()),
            ("t", timestamp.to_string()),
            ("sign", signature(&self.nonce, timestamp, &self.secret)),
        ] {
            headers.insert(
                name,
                HeaderValue::from_str(&value).map_err(|_| Error::Upstream)?,
            );
        }
        let response = self
            .request("/token", json!({"token": ""}), headers.clone())
            .await?;
        let token = response["token"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or(Error::Upstream)?;
        headers.insert(
            "token",
            HeaderValue::from_str(token).map_err(|_| Error::Upstream)?,
        );
        Ok(headers)
    }

    pub async fn login(&self, account: &str, password: &str) -> Result<User, Error> {
        tokio::time::timeout(self.timeout, self.login_sequence(account, password))
            .await
            .map_err(|_| Error::Upstream)?
    }

    async fn login_sequence(&self, account: &str, password: &str) -> Result<User, Error> {
        let headers = self.headers().await?;
        let login = self.request("/user/login", json!({"username": account, "password": password, "isuid": if EMAIL.is_match(account) { 2 } else { 0 }}), headers).await?;
        let status = integer(&login["data"]["status"]).ok_or(Error::Upstream)?;
        if status <= 0 {
            return Err(Error::InvalidCredentials);
        }
        let name = match &login["data"]["username"] {
            Value::Null => "",
            Value::String(name) => name,
            _ => return Err(Error::Upstream),
        };
        let headers = self.headers().await?;
        let response = self
            .request(
                "/user/get_user",
                json!({"username": name, "isuid": 0}),
                headers,
            )
            .await?;
        let uid = integer(&response["data"]["uid"])
            .filter(|uid| *uid > 0)
            .ok_or(Error::Upstream)?;
        let name = response["data"]["username"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or(Error::Upstream)?;
        let email = match &response["data"]["email"] {
            Value::Null => "",
            Value::String(email) => email,
            _ => return Err(Error::Upstream),
        };
        Ok(User {
            uid,
            name: name.into(),
            email: email.into(),
        })
    }
}

fn integer(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
}
