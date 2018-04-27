use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};

use dsn::Dsn;
use protocol;
use utils::{datetime_to_timestamp, timestamp_to_datetime};

/// Represents an auth header parsing error.
#[derive(Debug, Fail, Copy, Clone, Eq, PartialEq)]
pub enum AuthParseError {
    /// Raised if the auth header is not indicating sentry auth
    #[fail(display = "non sentry auth")]
    NonSentryAuth,
    /// Raised if the timestamp value is invalid.
    #[fail(display = "invalid value for timestamp")]
    InvalidTimestamp,
    /// Raised if the version value is invalid
    #[fail(display = "invalid value for version")]
    InvalidVersion,
    /// Raised if the public key is missing entirely
    #[fail(display = "missing public key in auth header")]
    MissingPublicKey,
}

/// Represents an auth header.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Auth {
    #[serde(skip)]
    timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "sentry_client")]
    client: Option<String>,
    #[serde(rename = "sentry_version")]
    version: u16,
    #[serde(rename = "sentry_key")]
    key: String,
    #[serde(rename = "sentry_secret")]
    secret: Option<String>,
}

impl Auth {
    /// Creates an auth header from key value pairs.
    pub fn from_pairs<'a, 'b, I: Iterator<Item = (&'a str, &'b str)>>(
        pairs: I,
    ) -> Result<Auth, AuthParseError> {
        let mut rv = Auth {
            timestamp: None,
            client: None,
            version: protocol::LATEST,
            key: "".into(),
            secret: None,
        };

        for (mut key, value) in pairs {
            if key.starts_with("sentry_") {
                key = &key[7..];
            }
            match key {
                "timestamp" => {
                    rv.timestamp =
                        Some(value.parse().map_err(|_| AuthParseError::InvalidTimestamp)?);
                }
                "client" => {
                    rv.client = Some(value.into());
                }
                "version" => {
                    rv.version = value.parse().map_err(|_| AuthParseError::InvalidVersion)?;
                }
                "key" => {
                    rv.key = value.into();
                }
                "secret" => {
                    rv.secret = Some(value.into());
                }
                _ => {}
            }
        }

        if rv.key.is_empty() {
            return Err(AuthParseError::MissingPublicKey);
        }

        Ok(rv)
    }

    /// Returns the unix timestamp the client defined
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        self.timestamp
    }

    /// Returns the protocol version the client speaks
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Returns the public key
    pub fn public_key(&self) -> &str {
        &self.key
    }

    /// Returns the client's secret if it authenticated with a secret.
    pub fn secret_key(&self) -> Option<&str> {
        self.secret.as_ref().map(|x| x.as_str())
    }

    /// Returns true if the authentication implies public auth (no secret)
    pub fn is_public(&self) -> bool {
        self.secret.is_none()
    }

    /// Returns the client's agent
    pub fn client_agent(&self) -> Option<&str> {
        self.client.as_ref().map(|x| x.as_str())
    }
}

impl fmt::Display for Auth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Sentry sentry_key={}, sentry_version={}",
            self.key, self.version
        )?;
        if let Some(ts) = self.timestamp {
            write!(f, ", sentry_timestamp={}", datetime_to_timestamp(&ts))?;
        }
        if let Some(ref client) = self.client {
            write!(f, ", sentry_client={}", client)?;
        }
        if let Some(ref secret) = self.secret {
            write!(f, ", sentry_secret={}", secret)?;
        }
        Ok(())
    }
}

impl FromStr for Auth {
    type Err = AuthParseError;

    fn from_str(s: &str) -> Result<Auth, AuthParseError> {
        let mut rv = Auth {
            timestamp: None,
            client: None,
            version: protocol::LATEST,
            key: "".into(),
            secret: None,
        };
        let mut base_iter = s.splitn(2, ' ');
        if !base_iter
            .next()
            .unwrap_or("")
            .eq_ignore_ascii_case("sentry")
        {
            return Err(AuthParseError::NonSentryAuth);
        }
        let items = base_iter.next().unwrap_or("");
        for item in items.split(',') {
            let mut kviter = item.trim().split('=');
            match (kviter.next(), kviter.next()) {
                (Some("sentry_timestamp"), Some(ts)) => {
                    let f: f64 = ts.parse().map_err(|_| AuthParseError::InvalidTimestamp)?;
                    rv.timestamp = Some(timestamp_to_datetime(f));
                }
                (Some("sentry_client"), Some(client)) => {
                    rv.client = Some(client.into());
                }
                (Some("sentry_version"), Some(version)) => {
                    rv.version = version.parse().map_err(|_| AuthParseError::InvalidVersion)?;
                }
                (Some("sentry_key"), Some(key)) => {
                    rv.key = key.into();
                }
                (Some("sentry_secret"), Some(secret)) => {
                    rv.secret = Some(secret.into());
                }
                _ => {}
            }
        }

        if rv.key.is_empty() {
            return Err(AuthParseError::MissingPublicKey);
        }

        Ok(rv)
    }
}

pub(crate) fn auth_from_dsn_and_client(dsn: &Dsn, client: Option<&str>) -> Auth {
    Auth {
        timestamp: Some(Utc::now()),
        client: client.map(|x| x.to_string()),
        version: protocol::LATEST,
        key: dsn.public_key().to_string(),
        secret: dsn.secret_key().map(|x| x.to_string()),
    }
}
