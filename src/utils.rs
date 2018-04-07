//! Useful utilities for working with events.
use backtrace;

use api::protocol::{Context, DeviceContext, OsContext, RuntimeContext, Stacktrace};
use backtrace_support::backtrace_to_stacktrace;

/// Returns the current backtrace as sentry stacktrace.
pub fn current_stacktrace() -> Option<Stacktrace> {
    backtrace_to_stacktrace(&backtrace::Backtrace::new())
}

#[cfg(all(feature = "with_device_info", target_os = "macos"))]
mod model_support {
    use std::ptr;
    use libc;
    use libc::c_void;
    use regex::Regex;

    lazy_static! {
        static ref FAMILY_RE: Regex = Regex::new(r#"([a-zA-Z]+)\d"#).unwrap();
    }

    pub fn get_model() -> Option<String> {
        unsafe {
            let mut size = 0;
            libc::sysctlbyname(
                "hw.model\x00".as_ptr() as *const i8,
                ptr::null_mut(),
                &mut size,
                ptr::null_mut(),
                0,
            );
            let mut buf = vec![0u8; size as usize];
            libc::sysctlbyname(
                "hw.model\x00".as_ptr() as *const i8,
                buf.as_mut_ptr() as *mut c_void,
                &mut size,
                ptr::null_mut(),
                0,
            );
            Some(String::from_utf8_lossy(&buf).to_string())
        }
    }

    pub fn get_family() -> Option<String> {
        get_model()
            .as_ref()
            .and_then(|model| FAMILY_RE.captures(model))
            .and_then(|m| m.get(1))
            .map(|group| group.as_str().to_string())
    }
}

#[cfg(any(not(target_os = "macos"), not(feature = "with_device_info")))]
mod model_support {
    pub fn get_model() -> Option<String> {
        None
    }

    pub fn get_family() -> Option<String> {
        None
    }
}

/// Returns the model identifier.
pub fn device_model() -> Option<String> {
    model_support::get_model()
}

/// Returns the model family identifier.
pub fn device_family() -> Option<String> {
    model_support::get_family()
}

/// Returns the CPU architecture.
pub fn cpu_arch() -> Option<String> {
    use constants::ARCH;
    Some(ARCH.into())
}

/// Returns the server name (hostname) if available.
pub fn server_name() -> Option<String> {
    #[cfg(feature = "with_device_info")]
    {
        use hostname::get_hostname;
        get_hostname()
    }
    #[cfg(not(feature = "with_device_info"))]
    {
        None
    }
}

/// Returns the OS context
pub fn os_context() -> Option<Context> {
    #[cfg(all(feature = "with_device_info", not(windows)))]
    {
        use uname::uname;
        if let Ok(info) = uname() {
            Some(
                OsContext {
                    name: Some(info.sysname.into()),
                    kernel_version: Some(info.version.into()),
                    version: Some(info.release.into()),
                    ..Default::default()
                }.into(),
            )
        } else {
            None
        }
    }
    #[cfg(all(feature = "with_device_info", windows))]
    {
        use constants::PLATFORM;
        Some(
            OsContext {
                name: Some(PLATFORM.into()),
                ..Default::default()
            }.into(),
        )
    }
    #[cfg(not(feature = "with_device_info"))]
    {
        None
    }
}

/// Returns the rust info.
pub fn rust_context() -> Option<Context> {
    #[cfg(feature = "with_device_info")]
    {
        use constants::{RUSTC_CHANNEL, RUSTC_VERSION};
        let mut ctx: Context = RuntimeContext {
            name: Some("rustc".into()),
            version: RUSTC_VERSION.map(|x| x.into()),
        }.into();
        if let Some(channel) = RUSTC_CHANNEL {
            ctx.extra.insert("channel".into(), channel.into());
        }
        Some(ctx)
    }
    #[cfg(not(feature = "with_device_info"))]
    {
        None
    }
}

/// Returns the device context.
pub fn device_context() -> Option<Context> {
    #[cfg(feature = "with_device_info")]
    {
        let model = device_model();
        let family = device_family();
        let arch = cpu_arch();
        Some(
            DeviceContext {
                model: model,
                family: family,
                arch: arch,
                ..Default::default()
            }.into(),
        )
    }
    #[cfg(not(feature = "with_device_info"))]
    {
        None
    }
}
