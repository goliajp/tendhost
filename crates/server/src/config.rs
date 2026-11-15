use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `TENDHOST_LISTEN`, default `127.0.0.1:7080`
    pub listen: SocketAddr,
    /// `TENDHOST_DATA`, default `./data`
    pub data_dir: PathBuf,
    /// `TENDHOST_PUBLIC_URL`: the address people and hosts reach the server
    /// at, when it differs from the listen address (behind a proxy)
    pub public_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(var: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let listen = var("TENDHOST_LISTEN").unwrap_or_else(|| "127.0.0.1:7080".into());
        let listen = listen
            .parse()
            .with_context(|| format!("TENDHOST_LISTEN={listen} is not host:port"))?;
        let public_url =
            var("TENDHOST_PUBLIC_URL").map(|url| url.trim_end_matches('/').to_string());
        if let Some(url) = &public_url
            && !(url.starts_with("https://") || url.starts_with("http://"))
        {
            anyhow::bail!("TENDHOST_PUBLIC_URL={url} must start with http:// or https://");
        }
        Ok(Self {
            listen,
            data_dir: var("TENDHOST_DATA").unwrap_or_else(|| "data".into()).into(),
            public_url,
        })
    }

    /// Where a browser on this machine should be pointed. An address
    /// that listens on every interface is opened through loopback.
    pub fn url(&self) -> String {
        self.public_url
            .clone()
            .unwrap_or_else(|| match self.listen.ip().is_unspecified() {
                true => format!("http://127.0.0.1:{}", self.listen.port()),
                false => format!("http://{}", self.listen),
            })
    }

    pub fn served_over_https(&self) -> bool {
        self.public_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(pairs: &[(&str, &str)]) -> Result<Config> {
        Config::from_lookup(|name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn defaults_listen_on_loopback_only() {
        let c = config(&[]).unwrap();
        assert_eq!(c.listen.to_string(), "127.0.0.1:7080");
        assert_eq!(c.url(), "http://127.0.0.1:7080");
        assert!(!c.served_over_https());
    }

    #[test]
    fn listening_everywhere_is_opened_through_loopback() {
        let c = config(&[("TENDHOST_LISTEN", "0.0.0.0:7080")]).unwrap();
        assert_eq!(c.url(), "http://127.0.0.1:7080");
    }

    #[test]
    fn the_public_url_decides_https_and_loses_its_trailing_slash() {
        let c = config(&[("TENDHOST_PUBLIC_URL", "https://tend.example.com/")]).unwrap();
        assert_eq!(c.url(), "https://tend.example.com");
        assert!(c.served_over_https());
    }

    #[test]
    fn malformed_values_are_refused_with_the_variable_named() {
        let err = config(&[("TENDHOST_LISTEN", "7080")])
            .unwrap_err()
            .to_string();
        assert!(err.contains("TENDHOST_LISTEN"));
        assert!(config(&[("TENDHOST_PUBLIC_URL", "tend.example.com")]).is_err());
    }
}
