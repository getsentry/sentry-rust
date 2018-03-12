use std::fmt;
use std::str::FromStr;

/// Represents an auth header parsing error.
#[derive(Debug, Fail)]
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
    /// Raised if the version is missing entirely
    #[fail(display = "no valid version defined")]
    MissingVersion,
    /// Raised if the public key is missing entirely
    #[fail(display = "missing public key in auth header")]
    MissingPublicKey,
}

/// Represents an auth header.
#[derive(Default, Debug)]
pub struct Auth {
    timestamp: Option<f64>,
    client: Option<String>,
    version: u16,
    key: String,
    secret: Option<String>,
}

impl Auth {
    /// Returns the unix timestamp the client defined
    pub fn timestamp(&self) -> Option<f64> {
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

    /// Returns the client's relay
    pub fn client_relay(&self) -> Option<&str> {
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
            write!(f, ", sentry_timestamp={}", ts)?;
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
        let mut rv = Auth::default();
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
                    rv.timestamp = Some(ts.parse().map_err(|_| AuthParseError::InvalidTimestamp)?);
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
        if rv.version == 0 {
            return Err(AuthParseError::MissingVersion);
        }

        Ok(rv)
    }
}

#[test]
fn test_auth_parsing() {
    let auth: Auth = "Sentry sentry_timestamp=1328055286.51, \
                      sentry_client=raven-python/42, \
                      sentry_version=6, \
                      sentry_key=public, \
                      sentry_secret=secret"
        .parse()
        .unwrap();
    assert_eq!(auth.timestamp(), Some(1328055286.51));
    assert_eq!(auth.client_relay(), Some("raven-python/42"));
    assert_eq!(auth.version(), 6);
    assert_eq!(auth.public_key(), "public");
    assert_eq!(auth.secret_key(), Some("secret"));

    assert_eq!(
        auth.to_string(),
        "Sentry sentry_key=public, \
         sentry_version=6, \
         sentry_timestamp=1328055286.51, \
         sentry_client=raven-python/42, \
         sentry_secret=secret"
    );
}
