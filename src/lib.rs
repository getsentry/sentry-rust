extern crate log;

#[macro_use]
extern crate error_chain;

extern crate backtrace;
extern crate time;
extern crate url;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate hyper;
extern crate hyper_tls;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::sync::Arc;
use std::default::Default;
use std::collections::HashMap;

use futures::*;
use tokio_core::reactor::{Handle, Remote};
use hyper::header::{ContentType, ContentLength};

mod errors {
    error_chain! {
        foreign_links {
            HyperError(::hyper::Error);
            HyperUri(::hyper::error::UriError);
            Json(::serde_json::Error);
            UrlParse(::url::ParseError);
        }

        errors {
            CredentialParseError {
                description("Invalid Sentry DSN syntax. Expected the form `(http|https)://{public key}:{private key}@{host}:{port}/{project id}`")
            }
        }
    }
}
pub use errors::*;

#[derive(Debug, Clone, Serialize)]
pub struct StackFrame {
    filename: String,
    function: String,
    lineno: u32,
}

// see https://docs.getsentry.com/hosted/clientdev/attributes/
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    // required
    event_id: String, // uuid4 exactly 32 characters (no dashes!)
    message: String, // Maximum length is 1000 characters.
    timestamp: String, // ISO 8601 format, without a timezone ex: "2011-05-02T17:41:36"
    level: String, // fatal, error, warning, info, debug
    logger: String, // ex "my.logger.name"
    platform: String, // Acceptable values ..., other
    sdk: SDK,
    device: Device,
    // optional
    culprit: Option<String>, // the primary perpetrator of this event ex: "my.module.function_name"
    server_name: Option<String>, // host client from which the event was recorded
    stack_trace: Option<Vec<StackFrame>>, // stack trace
    release: Option<String>, // generally be something along the lines of the git SHA for the given project
    tags: HashMap<String, String>, // WARNING! should be serialized as json object k->v
    environment: Option<String>, // ex: "production"
    modules: HashMap<String, String>, // WARNING! should be serialized as json object k->v
    extra: HashMap<String, String>, // WARNING! should be serialized as json object k->v
    fingerprint: Vec<String>, // An array of strings used to dictate the deduplicating for this event.
}

impl Event {
    pub fn new(
        logger: &str,
        level: &str,
        message: &str,
        device: &Device,
        culprit: Option<&str>,
        fingerprint: Option<Vec<String>>,
        server_name: Option<&str>,
        stack_trace: Option<Vec<StackFrame>>,
        release: Option<&str>,
        environment: Option<&str>,
        tags: Option<HashMap<String, String>>,
        extra: Option<HashMap<String, String>>,
    ) -> Event {
        Event {
            event_id: "".to_owned(),
            message: message.to_owned(),
            timestamp: time::strftime("%FT%T", &time::now().to_utc()).unwrap_or("".to_owned()),
            level: level.to_owned(),
            logger: logger.to_owned(),
            platform: "other".to_owned(),
            sdk: SDK {
                name: "rust-sentry".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            device: device.to_owned(),
            culprit: culprit.map(|c| c.to_owned()),
            server_name: server_name.map(|c| c.to_owned()),
            stack_trace: stack_trace,
            release: release.map(|c| c.to_owned()),
            tags: tags.unwrap_or(Default::default()),
            environment: environment.map(|c| c.to_owned()),
            modules: Default::default(),
            extra: extra.unwrap_or_else(|| Default::default()),
            fingerprint: fingerprint.unwrap_or(vec![]),
        }
    }

    pub fn push_tag(&mut self, key: String, value: String) {
        self.tags.insert(key, value);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SDK {
    name: String,
    version: String,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Device {
    name: String,
    version: String,
    build: String,
}

impl Device {
    pub fn new(name: String, version: String, build: String) -> Device {
        Device {
            name: name,
            version: version,
            build: build,
        }
    }
}

impl Default for Device {
    fn default() -> Device {
        Device {
            name: std::env::var_os("OSTYPE")
                .and_then(|cs| cs.into_string().ok())
                .unwrap_or("".to_owned()),
            version: "".to_owned(),
            build: "".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentryCredential {
    scheme: String,
    key: String,
    secret: String,
    host: String,
    port: u16,
    project_id: String,

    uri: hyper::Uri,
}

impl SentryCredential {
    /// {SCHEME}://{PUBLIC_KEY}:{SECRET_KEY}@{HOST}/{PATH}{PROJECT_ID}/store/
    fn uri<'a>(&'a self) -> &'a hyper::Uri {
        &self.uri
    }
}

impl std::str::FromStr for SentryCredential {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<SentryCredential, Error> {
        let url = url::Url::parse(s).map_err(Error::from)?;

        let scheme = url.scheme();
        if scheme != "http" && scheme != "https" {
            bail!(ErrorKind::CredentialParseError);
        }

        let host = url.host_str().ok_or(ErrorKind::CredentialParseError)?;
        let port = url.port().unwrap_or_else(
            || if scheme == "http" { 80 } else { 443 },
        );

        let key = url.username();
        let secret = url.password().ok_or(ErrorKind::CredentialParseError)?;

        let project_id = url.path_segments().and_then(|paths| paths.last()).ok_or(
            ErrorKind::CredentialParseError,
        )?;

        if key.is_empty() || project_id.is_empty() {
            bail!(ErrorKind::CredentialParseError);
        }

        let uri_str = format!(
            "{}://{}:{}@{}:{}/api/{}/store/",
            scheme,
            key,
            secret,
            host,
            port,
            project_id
        );
        let uri = uri_str.parse().map_err(Error::from)?;

        Ok(SentryCredential {
            scheme: scheme.to_owned(),
            key: key.to_owned(),
            secret: secret.to_owned(),
            host: host.to_owned(),
            port,
            project_id: project_id.to_owned(),

            uri,
        })
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Settings {
    pub server_name: String,
    pub release: String,
    pub environment: String,
    pub device: Device,
}

impl Settings {
    pub fn new(
        server_name: String,
        release: String,
        environment: String,
        device: Device,
    ) -> Settings {
        Settings {
            server_name: server_name,
            release: release,
            environment: environment,
            device: device,
        }
    }
}

header! { (XSentryAuth, "X-Sentry-Auth") => [String] }

#[derive(Clone)]
pub struct Sentry {
    remote: Remote,
    credential: Arc<SentryCredential>,
    settings: Arc<Settings>,
}

impl Sentry {
    pub fn new(
        handle: Handle,
        server_name: String,
        release: String,
        environment: String,
        credential: SentryCredential,
    ) -> Sentry {
        let settings = Settings {
            server_name: server_name,
            release: release,
            environment: environment,
            ..Settings::default()
        };

        Sentry::from_settings(handle, settings, credential)
    }

    pub fn from_settings(
        handle: Handle,
        settings: Settings,
        credential: SentryCredential,
    ) -> Sentry {
        Sentry {
            remote: handle.remote().clone(),
            credential: Arc::new(credential),
            settings: Arc::new(settings),
        }
    }

    pub fn log_event(&self, e: Event) {
        let cred = self.credential.clone();
        self.remote.spawn(move |handle| {
            post(handle, &cred, e).map_err(|_e| ())
        });
    }

    pub fn register_panic_handler<F>(&self, maybe_f: Option<F>)
    where
        F: Fn(&std::panic::PanicInfo) + 'static + Sync + Send,
    {
        let cred = self.credential.clone();
        let settings = self.settings.clone();
        let remote = self.remote.clone();
        std::panic::set_hook(Box::new(move |info: &std::panic::PanicInfo| {
            let location = info.location()
                .map(|l| format!("{}: {}", l.file(), l.line()))
                .unwrap_or("NA".to_owned());
            let msg = match info.payload().downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => {
                    match info.payload().downcast_ref::<String>() {
                        Some(s) => &s[..],
                        None => "Box<Any>",
                    }
                }
            };

            let mut frames = vec![];
            backtrace::trace(|frame: &backtrace::Frame| {
                backtrace::resolve(frame.ip(), |symbol| {
                    let name = symbol.name().map_or(
                        "unresolved symbol".to_owned(),
                        |name| name.to_string(),
                    );
                    let filename = symbol.filename().map_or("".to_owned(), |sym| {
                        sym.to_string_lossy().into_owned()
                    });
                    let lineno = symbol.lineno().unwrap_or(0);
                    frames.push(StackFrame {
                        filename: filename,
                        function: name,
                        lineno: lineno,
                    });
                });

                true // keep going to the next frame
            });

            let e = Event::new(
                "panic",
                "fatal",
                msg,
                &settings.device,
                Some(&location),
                None,
                Some(&settings.server_name),
                Some(frames),
                Some(&settings.release),
                Some(&settings.environment),
                None,
                None,
            );
            if let Some(ref f) = maybe_f {
                f(info);
            }
            let cred = cred.clone();
            remote.spawn(move |handle| post(handle, &cred, e).map_err(|_e| {}));
        }));
    }

    pub fn unregister_panic_handler(&self) {
        let _ = std::panic::take_hook();
    }

    // fatal, error, warning, info, debug
    pub fn fatal(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "fatal", message, culprit, None, None, None);
    }
    pub fn error(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "error", message, culprit, None, None, None);
    }
    pub fn warning(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "warning", message, culprit, None, None, None);
    }
    pub fn info(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "info", message, culprit, None, None, None);
    }
    pub fn debug(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "debug", message, culprit, None, None, None);
    }

    pub fn log(
        &self,
        logger: &str,
        level: &str,
        message: &str,
        culprit: Option<&str>,
        fingerprint: Option<Vec<String>>,
        tags: Option<HashMap<String, String>>,
        extra: Option<HashMap<String, String>>,
    ) {
        let fpr = match fingerprint {
            Some(f) => f,
            None => {
                vec![
                    logger.to_owned(),
                    level.to_owned(),
                    culprit.unwrap_or("").to_owned(),
                ]
            }
        };
        let settings = self.settings.clone();
        let e = Event::new(
            logger,
            level,
            message,
            &settings.device,
            culprit,
            Some(fpr),
            Some(&settings.server_name),
            None,
            Some(&settings.release),
            Some(&settings.environment),
            tags,
            extra,
        );
        self.log_event(e)
    }
}

// POST /api/1/store/ HTTP/1.1
// Content-Type: application/json
//
fn post(handle: &Handle, cred: &SentryCredential, e: Event) -> Result<()> {
    if cred.scheme == "https" {
        // https
        let client = hyper::Client::configure()
            .connector(hyper_tls::HttpsConnector::new(4, handle).unwrap())
            .build(handle);
        post_client(client, handle.clone(), cred, e)
    } else {
        // http
        post_client(hyper::Client::new(handle), handle.clone(), cred, e)
    }
}

fn post_client<C>(
    client: hyper::client::Client<C>,
    handle: Handle,
    cred: &SentryCredential,
    e: Event,
) -> Result<()>
where
    C: hyper::client::Connect,
{
    let mut req = hyper::Request::new(hyper::Method::Post, cred.uri().clone());
    let body = serde_json::to_string(&e).map_err(Error::from)?;
    {
        let headers = req.headers_mut();

        // X-Sentry-Auth: Sentry sentry_version=7,
        // sentry_client=<client version, arbitrary>,
        // sentry_timestamp=<current timestamp>,
        // sentry_key=<public api key>,
        // sentry_secret=<secret api key>
        //
        let timestamp = time::get_time().sec.to_string();
        let xsentryauth = format!(
            "Sentry sentry_version=7,sentry_client=rust-sentry/{},sentry_timestamp={},sentry_key={},sentry_secret={}",
            env!("CARGO_PKG_VERSION"),
            timestamp,
            cred.key,
            cred.secret
        );
        headers.set(XSentryAuth(xsentryauth));
        headers.set(ContentType::json());
        headers.set(ContentLength(body.len() as u64));
    }
    req.set_body(hyper::Body::from(body));

    let f = client
        .request(req)
        .map_err(|_| ())
        .and_then(|_resp| {
            let body = _resp.body();
            body.collect()
                .map(|chunks| {
                    let mut buf = Vec::new();
                    for e in chunks.into_iter() {
                        buf.extend_from_slice(&e);
                    }
                    ()
                })
                .map_err(|_e| ())
        })
        .map_err(|_e| ());
    handle.spawn(f);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::*;
    use std::thread;
    use std::sync::Mutex;
    use tokio_core::reactor::Core;

    #[test]
    fn it_registrer_panic_handler() {
        let core = Core::new().unwrap();
        let handle = core.handle();

        let dsn = "https://xx:xx@app.getsentry.com/xx";
        let cred = dsn.parse().unwrap();

        let sentry = Sentry::new(
            handle,
            "Server Name".to_string(),
            "release".to_string(),
            "test_env".to_string(),
            cred,
        );

        let (sender, receiver) = std::sync::mpsc::channel();
        let s = Mutex::new(sender);

        sentry.register_panic_handler(Some(move |_: &PanicInfo| -> () {
            let lock = match s.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let _ = lock.send(true);
        }));

        let t1 = thread::spawn(|| {
            panic!("Panic Handler Testing");
        });
        let _ = t1.join();

        assert_eq!(receiver.recv().unwrap(), true);
        sentry.unregister_panic_handler();

    }

    #[test]
    fn it_share_sentry_accross_threads() {
        let core = Core::new().unwrap();
        let handle = core.handle();

        let dsn = "https://xx:xx@app.getsentry.com/xx";
        let cred = dsn.parse().unwrap();

        let sentry = Arc::new(Sentry::new(
            handle,
            "Server Name".to_string(),
            "release".to_string(),
            "test_env".to_string(),
            cred,
        ));

        let sentry1 = sentry.clone();
        let t1 = thread::spawn(move || sentry1.settings.server_name.clone());
        let sentry2 = sentry.clone();
        let t2 = thread::spawn(move || sentry2.settings.server_name.clone());

        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();

        assert!(r1 == sentry.settings.server_name);
        assert!(r2 == sentry.settings.server_name);
    }

    #[test]
    fn test_parsing_dsn_when_valid() {
        let cred: SentryCredential = "https://mypublickey:myprivatekey@myhost/myprojectid"
            .parse()
            .unwrap();
        assert_eq!("mypublickey", cred.key);
        assert_eq!("myprivatekey", cred.secret);
        assert_eq!("myhost", cred.host);
        assert_eq!("myprojectid", cred.project_id);
    }

    #[test]
    fn test_parsing_dsn_with_nested_project_id() {
        let cred: SentryCredential = "https://mypublickey:myprivatekey@myhost/foo/bar/myprojectid"
            .parse()
            .unwrap();
        assert_eq!("mypublickey", cred.key);
        assert_eq!("myprivatekey", cred.secret);
        assert_eq!("myhost", cred.host);
        assert_eq!("myprojectid", cred.project_id);
    }

    #[test]
    fn test_parsing_dsn_when_lacking_project_id() {
        let parsed_creds = "https://mypublickey:myprivatekey@myhost/".parse::<SentryCredential>();
        assert!(parsed_creds.is_err());
    }

    #[test]
    fn test_parsing_dsn_when_lacking_private_key() {
        let parsed_creds = "https://mypublickey@myhost/myprojectid".parse::<SentryCredential>();
        assert!(parsed_creds.is_err());
    }

    #[test]
    fn test_parsing_dsn_when_lacking_protocol() {
        let parsed_creds = "mypublickey:myprivatekey@myhost/myprojectid"
            .parse::<SentryCredential>();
        assert!(parsed_creds.is_err());
    }

    #[test]
    fn test_empty_settings_constructor_matches_empty_new_constructor() {
        let core = Core::new().unwrap();
        let handle = core.handle();

        let creds = "https://mypublickey:myprivatekey@myhost/myprojectid"
            .parse::<SentryCredential>()
            .unwrap();
        let from_settings =
            Sentry::from_settings(handle.clone(), Default::default(), creds.clone());
        let from_new = Sentry::new(
            handle,
            "".to_string(),
            "".to_string(),
            "".to_string(),
            creds,
        );
        assert_eq!(from_settings.settings, from_new.settings);
    }

    #[test]
    fn test_full_settings_constructor_overrides_all_settings() {
        let core = Core::new().unwrap();
        let handle = core.handle();

        let creds = "https://mypublickey:myprivatekey@myhost/myprojectid"
            .parse::<SentryCredential>()
            .unwrap();
        let server_name = "server_name".to_string();
        let release = "release".to_string();
        let environment = "environment".to_string();
        let device = Device::new(
            "device_name".to_string(),
            "version".to_string(),
            "build".to_string(),
        );
        let settings = Settings {
            server_name: server_name.clone(),
            release: release.clone(),
            environment: environment.clone(),
            device: device.clone(),
        };
        let from_settings = Sentry::from_settings(handle, settings, creds);
        assert_eq!(from_settings.settings.server_name, server_name);
        assert_eq!(from_settings.settings.release, release);
        assert_eq!(from_settings.settings.environment, environment);
        assert_eq!(from_settings.settings.device, device);
    }
}
