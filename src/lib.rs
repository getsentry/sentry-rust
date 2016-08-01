extern crate time;

use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fmt::Debug;
use std::time::Duration;
use std::io::Read;
use std::env;
use std::io::Write;

#[macro_use]
extern crate hyper;
use hyper::Client;
use hyper::header::{Headers, ContentType};

extern crate chrono;
use chrono::offset::utc::UTC;




struct ThreadState<'a> {
    alive: &'a mut Arc<AtomicBool>,
}
impl<'a> ThreadState<'a> {
    fn set_alive(&self) {
        self.alive.store(true, Ordering::Relaxed);
    }
}
impl<'a> Drop for ThreadState<'a> {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
    }
}

pub trait WorkerClosure<T, P>: Fn(&P, T) -> () + Send + Sync {}
impl<T, F, P> WorkerClosure<T, P> for F where F: Fn(&P, T) -> () + Send + Sync {}


pub struct SingleWorker<T: 'static + Send, P: Clone + Send> {
    parameters: P,
    f: Arc<Box<WorkerClosure<T, P, Output = ()>>>,
    receiver: Arc<Mutex<Receiver<T>>>,
    sender: Mutex<Sender<T>>,
    alive: Arc<AtomicBool>,
}

impl<T: 'static + Debug + Send, P: 'static + Clone + Send> SingleWorker<T, P> {
    pub fn new(parameters: P, f: Box<WorkerClosure<T, P, Output = ()>>) -> SingleWorker<T, P> {
        let (sender, receiver) = channel::<T>();

        let worker = SingleWorker {
            parameters: parameters,
            f: Arc::new(f),
            receiver: Arc::new(Mutex::new(receiver)),
            sender: Mutex::new(sender), /* too bad sender is not sync -- suboptimal.... see https://github.com/rust-lang/rfcs/pull/1299/files */
            alive: Arc::new(AtomicBool::new(true)),
        };
        SingleWorker::spawn_thread(&worker);
        worker
    }

    fn is_alive(&self) -> bool {
        self.alive.clone().load(Ordering::Relaxed)
    }

    fn spawn_thread(worker: &SingleWorker<T, P>) {
        let mut alive = worker.alive.clone();
        let f = worker.f.clone();
        let receiver = worker.receiver.clone();
        let parameters = worker.parameters.clone();
        thread::spawn(move || {
            let state = ThreadState { alive: &mut alive };
            state.set_alive();

            let lock = match receiver.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            loop {
                match lock.recv() {
                    Ok(value) => f(&parameters, value),
                    Err(_) => {
                        thread::yield_now();
                    }
                };
            }

        });
        while !worker.is_alive() {
            thread::yield_now();
        }
    }

    pub fn work_with(&self, msg: T) {
        let alive = self.is_alive();
        if !alive {
            SingleWorker::spawn_thread(self);
        }

        let lock = match self.sender.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let _ = lock.send(msg);
    }
}


// see https://docs.getsentry.com/hosted/clientdev/attributes/
#[derive(Debug,Clone)]
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
    release: Option<String>, // generally be something along the lines of the git SHA for the given project
    tags: Vec<(String, String)>, // WARNING! should be serialized as json object k->v
    environment: Option<String>, // ex: "production"
    modules: Vec<(String, String)>, // WARNING! should be serialized as json object k->v
    extra: Vec<(String, String)>, // WARNING! should be serialized as json object k->v
    fingerprint: Vec<String>, // An array of strings used to dictate the deduplicating for this event.
}
impl Event {
    pub fn to_json_string(&self) -> String {
        let mut s = String::new();
        s.push_str("{");
        s.push_str(&format!("\"event_id\":\"{}\",", self.event_id));
        s.push_str(&format!("\"message\":\"{}\",", self.message));
        s.push_str(&format!("\"timestamp\":\"{}\",", self.timestamp));
        s.push_str(&format!("\"level\":\"{}\",", self.level));
        s.push_str(&format!("\"logger\":\"{}\",", self.logger));
        s.push_str(&format!("\"platform\":\"{}\",", self.platform));
        s.push_str(&format!("\"sdk\": {},", self.sdk.to_json_string()));
        s.push_str(&format!("\"device\": {}", self.device.to_json_string()));

        if let Some(ref culprit) = self.culprit {
            s.push_str(&format!(",\"culprit\":\"{}\"", culprit));
        }
        if let Some(ref server_name) = self.server_name {
            s.push_str(&format!(",\"server_name\":\"{}\"", server_name));
        }
        if let Some(ref release) = self.release {
            s.push_str(&format!(",\"release\":\"{}\"", release));
        }
        if self.tags.len() > 0 {
            s.push_str(",\"tags\":\"{");
            for tag in self.tags.iter() {
                s.push_str(&format!("\"{}\":\"{}\"", tag.0, tag.1));
            }
            s.push_str("}");
        }
        if let Some(ref environment) = self.environment {
            s.push_str(&format!(",\"environment\":\"{}\"", environment));
        }
        if self.modules.len() > 0 {
            s.push_str(",\"modules\":\"{");
            for module in self.modules.iter() {
                s.push_str(&format!("\"{}\":\"{}\"", module.0, module.1));
            }
            s.push_str("}");
        }
        if self.extra.len() > 0 {
            s.push_str(",\"extra\":\"{");
            for extra in self.extra.iter() {
                s.push_str(&format!("\"{}\":\"{}\"", extra.0, extra.1));
            }
            s.push_str("}");
        }
        if self.fingerprint.len() > 0 {
            s.push_str(",\"fingerprint\":\"[");
            for fingerprint in self.fingerprint.iter() {
                s.push_str(&format!("\"{}\"", fingerprint));
            }
            s.push_str("]");
        }

        s.push_str("}");
        s
    }

    pub fn new(logger: &str,
               level: &str,
               message: &str,
               culprit: Option<&str>,
               server_name: Option<&str>,
               release: Option<&str>,
               environment: Option<&str>)
               -> Event {


        Event {
            event_id: "".to_string(),
            message: message.to_owned(),
            timestamp: UTC::now().format("%Y-%m-%dT%H:%M:%S").to_string(), /* ISO 8601 format, without a timezone ex: "2011-05-02T17:41:36" */
            level: level.to_owned(),
            logger: logger.to_owned(),
            platform: "other".to_string(),
            sdk: SDK {
                name: "rust-sentry".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            device: Device {
                name: env::var_os("OSTYPE")
                    .and_then(|cs| cs.into_string().ok())
                    .unwrap_or("".to_string()),
                version: "".to_string(),
                build: "".to_string(),
            },
            culprit: culprit.map(|c| c.to_owned()),
            server_name: server_name.map(|c| c.to_owned()),
            release: release.map(|c| c.to_owned()),
            tags: vec![],
            environment: environment.map(|c| c.to_owned()),
            modules: vec![],
            extra: vec![],
            fingerprint: vec![],
        }
    }
}

#[derive(Debug,Clone)]
pub struct SDK {
    name: String,
    version: String,
}
impl SDK {
    pub fn to_json_string(&self) -> String {
        format!("{{\"name\":\"{}\",\"version\":\"{}\"}}",
                self.name,
                self.version)
    }
}
#[derive(Debug,Clone)]
pub struct Device {
    name: String,
    version: String,
    build: String,
}
impl Device {
    pub fn to_json_string(&self) -> String {
        format!("{{\"name\":\"{}\",\"version\":\"{}\",\"build\":\"{}\"}}",
                self.name,
                self.version,
                self.build)
    }
}


#[derive(Debug,Clone)]
pub struct SentryCrediential {
    pub key: String,
    pub secret: String,
    pub host: String,
    pub project_id: String,
}
pub struct Sentry {
    server_name: String,
    release: String,
    environment: String,
    worker: Arc<SingleWorker<Event, SentryCrediential>>,
}

header! { (XSentryAuth, "X-Sentry-Auth") => [String] }

impl Sentry {
    pub fn new(server_name: String,
               release: String,
               environment: String,
               credential: SentryCrediential)
               -> Sentry {
        let worker = SingleWorker::new(credential,
                                       Box::new(move |credential, e| -> () {
                                           Sentry::post(credential, &e);
                                       }));
        Sentry {
            server_name: server_name,
            release: release,
            environment: environment,
            worker: Arc::new(worker),
        }
    }



    // POST /api/1/store/ HTTP/1.1
    // Content-Type: application/json
    //
    fn post(credential: &SentryCrediential, e: &Event) {
        writeln!(&mut ::std::io::stderr(), "SENTRY - {:?}", e);

        let mut headers = Headers::new();

        // X-Sentry-Auth: Sentry sentry_version=7,
        // sentry_client=<client version, arbitrary>,
        // sentry_timestamp=<current timestamp>,
        // sentry_key=<public api key>,
        // sentry_secret=<secret api key>
        //
        let timestamp = time::get_time().sec.to_string();
        let xsentryauth = format!("Sentry sentry_version=7,sentry_client=rust-sentry/0.1.0,\
                                   sentry_timestamp={},sentry_key={},sentry_secret={}",
                                  timestamp,
                                  credential.key,
                                  credential.secret);
        headers.set(XSentryAuth(xsentryauth));


        headers.set(ContentType::json());

        let body = e.to_json_string();
        println!("Sentry body {}", body);

        let mut client = Client::new();
        client.set_read_timeout(Some(Duration::new(5, 0)));
        client.set_write_timeout(Some(Duration::new(5, 0)));

        // {PROTOCOL}://{PUBLIC_KEY}:{SECRET_KEY}@{HOST}/{PATH}{PROJECT_ID}/store/
        let url = format!("https://{}:{}@{}/api/{}/store/",
                          credential.key,
                          credential.secret,
                          credential.host,
                          credential.project_id);

        let mut res = client.post(&url)
            .headers(headers)
            .body(&body)
            .send()
            .unwrap();

        // Read the Response.
        let mut body = String::new();
        res.read_to_string(&mut body).unwrap();
        println!("Sentry Response {}", body);
    }

    pub fn register_panic_handler(&self) {

        let server_name = self.server_name.clone();
        let release = self.release.clone();
        let environment = self.environment.clone();

        let worker = self.worker.clone();

        std::panic::set_hook(Box::new(move |info: &std::panic::PanicInfo| {

            let location = info.location()
                .map(|l| format!("{}: {}", l.file(), l.line()))
                .unwrap_or("NA".to_string());
            let msg = match info.payload().downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => {
                    match info.payload().downcast_ref::<String>() {
                        Some(s) => &s[..],
                        None => "Box<Any>",
                    }
                }
            };

            let e = Event::new(&location,
                               "fatal",
                               msg,
                               None,
                               Some(&server_name),
                               Some(&release),
                               Some(&environment));
            let _ = worker.work_with(e.clone());
        }));
    }
    pub fn unregister_panic_handler(&self) {
        let _ = std::panic::take_hook();
    }

    // fatal, error, warning, info, debug
    pub fn fatal(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "fatal", message, culprit);
    }
    pub fn error(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "error", message, culprit);
    }
    pub fn warning(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "warning", message, culprit);
    }
    pub fn info(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "info", message, culprit);
    }
    pub fn debug(&self, logger: &str, message: &str, culprit: Option<&str>) {
        self.log(logger, "debug", message, culprit);
    }

    fn log(&self, logger: &str, level: &str, message: &str, culprit: Option<&str>) {
        self.worker.work_with(Event::new(logger,
                                         level,
                                         message,
                                         culprit,
                                         Some(&self.server_name),
                                         Some(&self.release),
                                         Some(&self.environment)));
    }
}

#[cfg(test)]
mod tests {
    use super::SingleWorker;
    use super::Sentry;
    use super::SentryCrediential;

    use std::sync::{Arc, Mutex};
    use std::sync::mpsc::channel;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    // use std::time::Duration;

    #[test]
    fn it_should_pass_value_to_worker_thread() {

        let (sender, receiver) = channel();
        let s = Mutex::new(sender);
        let worker = SingleWorker::new("",
                                       Box::new(move |_, v| {
                                           let _ = s.lock().unwrap().send(v);
                                       }));
        let v = "Value";
        worker.work_with(v);

        let recv_v = receiver.recv().ok();
        assert!(recv_v == Some(v));
    }

    #[test]
    fn it_should_pass_value_event_after_thread_panic() {
        let (sender, receiver) = channel();
        let s = Mutex::new(sender);
        let i = AtomicUsize::new(0);
        let worker = SingleWorker::new("",
                                       Box::new(move |_, v| {
            let lock = match s.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let _ = lock.send(v);

            i.fetch_add(1, Ordering::SeqCst);
            if i.load(Ordering::Relaxed) == 2 {
                panic!("PanicTesting");
            }

        }));
        let v0 = "Value0";
        let v1 = "Value1";
        let v2 = "Value2";
        let v3 = "Value3";
        worker.work_with(v0);
        worker.work_with(v1);
        let recv_v0 = receiver.recv().ok();
        let recv_v1 = receiver.recv().ok();

        while worker.is_alive() {
            thread::yield_now();
        }

        worker.work_with(v2);
        worker.work_with(v3);
        let recv_v2 = receiver.recv().ok();
        let recv_v3 = receiver.recv().ok();

        assert!(recv_v0 == Some(v0));
        assert!(recv_v1 == Some(v1));
        assert!(recv_v2 == Some(v2));
        assert!(recv_v3 == Some(v3));

    }

    #[test]
    fn it_post_sentry_event() {
        let sentry = Sentry::new("Server Name".to_string(),
                                 "release".to_string(),
                                 "test_env".to_string(),
                                 SentryCrediential {
                                     key: "xx".to_string(),
                                     secret: "xx".to_string(),
                                     host: "app.getsentry.com".to_string(),
                                     project_id: "xx".to_string(),
                                 });
        sentry.register_panic_handler();
        sentry.unregister_panic_handler();

    }

    #[test]
    fn it_share_sentry_accross_threads() {
        let sentry = Arc::new(Sentry::new("Server Name".to_string(),
                                          "release".to_string(),
                                          "test_env".to_string(),
                                          SentryCrediential {
                                              key: "xx".to_string(),
                                              secret: "xx".to_string(),
                                              host: "app.getsentry.com".to_string(),
                                              project_id: "xx".to_string(),
                                          }));

        let sentry1 = sentry.clone();
        let t1 = thread::spawn(move || sentry1.server_name.clone());
        let sentry2 = sentry.clone();
        let t2 = thread::spawn(move || sentry2.server_name.clone());

        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();

        assert!(r1 == sentry.server_name);
        assert!(r2 == sentry.server_name);
    }


    // #[test]
    // fn it_post_sentry_event() {
    //     let sentry = Sentry::new("Server Name".to_string(),
    //                              "release".to_string(),
    //                              "test_env".to_string(),
    //                              SentryCrediential {
    //                                  key: "xx".to_string(),
    //                                  secret: "xx".to_string(),
    //                                  host: "app.getsentry.com".to_string(),
    //                                  project_id: "xx".to_string(),
    //                              });
    //
    //     sentry.info("test.logger", "Test Message", None);
    //
    //     thread::sleep(Duration::new(5, 0));
    //
    // }
}
