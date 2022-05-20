use sentry_core::protocol::{Context, DeviceContext, Map, OsContext, RuntimeContext};

include!(concat!(env!("OUT_DIR"), "/constants.gen.rs"));

#[cfg(target_os = "macos")]
mod model_support {
    use libc::c_void;
    use std::ffi::CString;
    use std::ptr;

    fn sysctlbyname_call(name: &str) -> Option<String> {
        unsafe {
            let c_name = match CString::new(name) {
                Ok(name) => name.into_bytes_with_nul(),
                Err(_e) => return None,
            };
            let mut size = 0;
            let res = libc::sysctlbyname(
                c_name.as_ptr() as _,
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
                c_name.as_ptr() as _,
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

    pub fn get_model() -> Option<String> {
        sysctlbyname_call("hw.model")
    }

    pub fn get_macos_version() -> Option<String> {
        let version = sysctlbyname_call("kern.osproductversion")?;
        let dot_count = version.split('.').count() - 1;
        if dot_count < 2 {
            return Some(version + ".0");
        }
        Some(version)
    }

    pub fn get_macos_build() -> Option<String> {
        sysctlbyname_call("kern.osversion")
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

    #[test]
    fn test_macos_version_and_build() {
        let v = get_macos_version().unwrap();
        assert!(v.chars().all(|c| c.is_digit(10) || c == '.'));
        let dot_count = v.split('.').count() - 1;
        assert_eq!(dot_count, 2);
        let b = get_macos_build().unwrap();
        assert!(b.chars().all(|c| c.is_ascii_alphabetic() || c.is_digit(10)));
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
            #[cfg(target_os = "macos")]
            {
                Some(
                    OsContext {
                        name: Some("macOS".into()),
                        kernel_version: Some(info.version),
                        version: model_support::get_macos_version(),
                        build: model_support::get_macos_build(),
                        ..Default::default()
                    }
                    .into(),
                )
            }
            #[cfg(not(target_os = "macos"))]
            {
                Some(
                    OsContext {
                        name: Some(info.sysname),
                        kernel_version: Some(info.version),
                        version: Some(info.release),
                        ..Default::default()
                    }
                    .into(),
                )
            }
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
