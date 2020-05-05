use crate::protocol::{ClientSdkInfo, ClientSdkPackage};

/// The version of the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

include!(concat!(env!("OUT_DIR"), "/constants.gen.rs"));

lazy_static::lazy_static! {
    pub static ref USER_AGENT: String = format!("sentry.rust/{}", VERSION);
    pub static ref SDK_INFO: ClientSdkInfo = ClientSdkInfo {
        name: "sentry.rust".into(),
        version: VERSION.into(),
        packages: vec![ClientSdkPackage {
            name: "cargo:sentry".into(),
            version: VERSION.into(),
        }],
        integrations: {
            #[allow(unused_mut)]
            let mut rv = vec![];
            #[cfg(feature = "with_failure")]
            {
                rv.push("failure".to_string());
            }
            #[cfg(feature = "with_panic")]
            {
                rv.push("panic".to_string());
            }
            #[cfg(feature = "with_log")]
            {
                rv.push("log".to_string());
            }
            rv
        },
    };
}
