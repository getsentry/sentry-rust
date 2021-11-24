use sentry_core::protocol::{Context, DeviceContext, Map, OsContext, RuntimeContext};

include!(concat!(env!("OUT_DIR"), "/constants.gen.rs"));

#[cfg(target_os = "macos")]
mod model_support {
    use libc::c_void;
    use std::ptr;

    pub fn get_model() -> Option<String> {
        unsafe {
            let mut size = 0;
            let res = libc::sysctlbyname(
                "hw.model\x00".as_ptr() as _,
                ptr::null_mut(),
                &mut size,
                ptr::null_mut(),
                0,
            );
            if res != 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let res = libc::sysctlbyname(
                "hw.model\x00".as_ptr() as _,
                buf.as_mut_ptr() as *mut c_void,
                &mut size,
                ptr::null_mut(),
                0,
            );
            if res != 0 {
                return None;
            }
            Some(
                buf.into_iter()
                    .take(size)
                    .take_while(|&c| c != b'\0')
                    .map(|c| c as char)
                    .collect(),
            )
        }
    }

    pub fn get_family() -> Option<String> {
        get_model().map(|mut s| {
            let len = s
                .as_bytes()
                .iter()
                .take_while(|c| c.is_ascii_alphabetic())
                .count();
            s.truncate(len);
            s
        })
    }

    #[test]
    fn test_macos_hw_model() {
        let m = get_model().unwrap();
        assert!(m.chars().all(|c| c != '\0'));
        let f = get_family().unwrap();
        assert!(f.chars().all(|c| !c.is_digit(10)));
    }
}

#[cfg(not(target_os = "macos"))]
mod model_support {
    pub fn get_model() -> Option<String> {
        None
    }

    pub fn get_family() -> Option<String> {
        None
    }
}

/// Returns the server name (hostname) if available.
pub fn server_name() -> Option<String> {
    hostname::get().ok().and_then(|s| s.into_string().ok())
}

/// Returns the OS context
pub fn os_context() -> Option<Context> {
    #[cfg(not(windows))]
    {
        use uname::uname;
        if let Ok(info) = uname() {
            Some(
                OsContext {
                    name: Some(info.sysname),
                    kernel_version: Some(info.version),
                    version: Some(info.release),
                    ..Default::default()
                }
                .into(),
            )
        } else {
            None
        }
    }
    #[cfg(windows)]
    {
        Some(
            OsContext {
                name: Some(PLATFORM.into()),
                ..Default::default()
            }
            .into(),
        )
    }
}

/// Returns the rust info.
pub fn rust_context() -> Context {
    RuntimeContext {
        name: Some("rustc".into()),
        version: RUSTC_VERSION.map(|x| x.into()),
        other: {
            let mut map = Map::default();
            if let Some(channel) = RUSTC_CHANNEL {
                map.insert("channel".to_string(), channel.into());
            }
            map
        },
    }
    .into()
}

/// Returns the device context.
pub fn device_context() -> Context {
    DeviceContext {
        model: model_support::get_model(),
        family: model_support::get_family(),
        arch: Some(ARCH.into()),
        ..Default::default()
    }
    .into()
}
