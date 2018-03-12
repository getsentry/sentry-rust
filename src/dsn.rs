use std::fmt;
use std::str::FromStr;

use url::Url;

use project_id::{ProjectId, ProjectIdParseError};

/// Represents a dsn url parsing error.
#[derive(Debug, Fail)]
pub enum DsnParseError {
    /// raised on completely invalid urls
    #[fail(display = "no valid url provided")]
    InvalidUrl,
    /// raised the scheme is invalid / unsupported.
    #[fail(display = "no valid scheme")]
    InvalidScheme,
    /// raised if the username (public key) portion is missing.
    #[fail(display = "username is empty")]
    NoUsername,
    /// raised the project is is missing (first path component)
    #[fail(display = "empty path")]
    NoProjectId,
    /// raised the project id is invalid.
    #[fail(display = "invalid project id")]
    InvalidProjectId(#[cause] ProjectIdParseError),
}

/// Represents the scheme of an url http/https.
///
/// This holds schemes that are supported by sentry and relays.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Scheme {
    /// unencrypted HTTP scheme (should not be used)
    Http,
    /// encrypted HTTPS scheme
    Https,
}

impl Scheme {
    /// Returns the default port for this scheme.
    pub fn default_port(&self) -> u16 {
        match *self {
            Scheme::Http => 80,
            Scheme::Https => 443,
        }
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Scheme::Https => "https",
                Scheme::Http => "http",
            }
        )
    }
}

/// Represents a Sentry dsn.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Dsn {
    scheme: Scheme,
    public_key: String,
    secret_key: Option<String>,
    host: String,
    port: Option<u16>,
    project_id: ProjectId,
}

impl Dsn {
    /// Returns the scheme
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// Returns the public_key
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// Returns secret_key
    pub fn secret_key(&self) -> Option<&str> {
        self.secret_key.as_ref().map(|x| x.as_str())
    }

    /// Returns the host
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the port
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// Returns the project_id
    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }
}

impl fmt::Display for Dsn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}://{}", self.scheme, self.public_key)?;
        if let Some(ref secret_key) = self.secret_key {
            write!(f, ":{}", secret_key)?;
        }
        write!(f, "@{}", self.host)?;
        if let Some(ref port) = self.port {
            write!(f, ":{}", port)?;
        }
        write!(f, "/{}", self.project_id)?;
        Ok(())
    }
}

impl FromStr for Dsn {
    type Err = DsnParseError;

    fn from_str(s: &str) -> Result<Dsn, DsnParseError> {
        let url = Url::parse(s).map_err(|_| DsnParseError::InvalidUrl)?;

        if url.path() == "/" {
            return Err(DsnParseError::NoProjectId);
        }

        let path_segments = url.path_segments()
            .ok_or_else(|| DsnParseError::NoProjectId)?;
        if path_segments.count() > 1 {
            return Err(DsnParseError::InvalidUrl);
        }

        let public_key = match url.username() {
            "" => return Err(DsnParseError::NoUsername),
            username => username.to_string(),
        };

        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => return Err(DsnParseError::InvalidScheme),
        };

        let secret_key = url.password().map(|s| s.into());
        let port = url.port().map(|s| s.into());
        let host = match url.host_str() {
            Some(host) => host.into(),
            None => return Err(DsnParseError::InvalidUrl),
        };
        let project_id = url.path()
            .trim_matches('/')
            .parse()
            .map_err(DsnParseError::InvalidProjectId)?;

        Ok(Dsn {
            scheme,
            public_key,
            secret_key,
            port,
            host,
            project_id,
        })
    }
}

impl_str_serialization!(Dsn, "a sentry dsn");

#[cfg(test)]
mod test {

    use super::*;
    use serde_json;

    #[test]
    fn test_dsn_serialize_deserialize() {
        let dsn = Dsn::from_str("https://username@domain/42").unwrap();
        let serialized = serde_json::to_string(&dsn).unwrap();
        assert_eq!(serialized, "\"https://username@domain/42\"");
        let deserialized: Dsn = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.to_string(), "https://username@domain/42");
    }

    #[test]
    fn test_dsn_parsing() {
        let url = "https://username:password@domain:8888/23";
        let dsn = url.parse::<Dsn>().unwrap();
        assert_eq!(dsn.scheme(), Scheme::Https);
        assert_eq!(dsn.public_key(), "username");
        assert_eq!(dsn.secret_key(), Some("password"));
        assert_eq!(dsn.host(), "domain");
        assert_eq!(dsn.port(), Some(8888));
        assert_eq!(dsn.project_id(), ProjectId::from(23));
        assert_eq!(url, dsn.to_string());
    }

    #[test]
    fn test_dsn_no_port() {
        let url = "https://username@domain/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(url, dsn.to_string());
    }

    #[test]
    fn test_dsn_no_password() {
        let url = "https://username@domain:8888/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(url, dsn.to_string());
    }

    #[test]
    fn test_dsn_http_url() {
        let url = "http://username@domain:8888/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(url, dsn.to_string());
    }

    #[test]
    #[should_panic(expected = "InvalidUrl")]
    fn test_dsn_more_than_one_non_integer_path() {
        Dsn::from_str("http://username@domain:8888/path/path2").unwrap();
    }

    #[test]
    #[should_panic(expected = "NoUsername")]
    fn test_dsn_no_username() {
        Dsn::from_str("https://:password@domain:8888/23").unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidUrl")]
    fn test_dsn_invalid_url() {
        Dsn::from_str("random string").unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidUrl")]
    fn test_dsn_no_host() {
        Dsn::from_str("https://username:password@:8888/42").unwrap();
    }

    #[test]
    #[should_panic(expected = "NoProjectId")]
    fn test_dsn_no_project_id() {
        Dsn::from_str("https://username:password@domain:8888/").unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidScheme")]
    fn test_dsn_invalid_scheme() {
        Dsn::from_str("ftp://username:password@domain:8888/1").unwrap();
    }
}
