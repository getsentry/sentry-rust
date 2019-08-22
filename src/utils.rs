//! Useful utilities for working with events.
use std::thread;

use crate::protocol::{Context, DebugImage, Stacktrace, Thread};

#[cfg(all(feature = "with_device_info", target_os = "macos"))]
mod model_support {
    use libc;
    use libc::c_void;
    use regex::Regex;
    use std::ptr;

    lazy_static::lazy_static! {
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
            Some(
                String::from_utf8_lossy(if buf.ends_with(b"\x00") {
                    &buf[..size - 1]
                } else {
                    &buf
                })
                .to_string(),
            )
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

#[cfg(feature = "with_debug_meta")]
mod findshlibs_support {
    use super::*;

    #[cfg(unix)]
    pub fn find_shlibs() -> Option<Vec<DebugImage>> {
        if !TARGET_SUPPORTED {
            return None;
        }

        use std::env;
        use findshlibs::{
            Segment, SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED,
        };
        use crate::internals::Uuid;
        use crate::protocol::debugid::DebugId;
        use crate::protocol::SymbolicDebugImage;

        let mut rv = vec![];
        TargetSharedLibrary::each(|shlib| {
            let maybe_debug_id = shlib.id().and_then(|id| match id {
                SharedLibraryId::Uuid(bytes) => Some(DebugId::from_uuid(Uuid::from_bytes(bytes))),
                SharedLibraryId::GnuBuildId(ref id) => DebugId::from_guid_age(&id[..16], 0).ok(),
            });

            let debug_id = match maybe_debug_id {
                Some(debug_id) => debug_id,
                None => return,
            };

            let mut lowest_addr = !0;
            let mut lowest_vmaddr = !0;
            let mut highest_addr = 0;

            for seg in shlib.segments() {
                if !seg.is_code() {
                    continue;
                }
                let svma: u64 = seg.stated_virtual_memory_address().0 as u64;
                let avma: u64 = seg.actual_virtual_memory_address(shlib).0 as u64;
                if lowest_addr > avma {
                    lowest_addr = avma;
                }
                if highest_addr < avma {
                    highest_addr = avma;
                }
                if lowest_vmaddr > svma {
                    lowest_vmaddr = svma;
                }
            }

            let mut name = shlib.name().to_string_lossy().to_string();
            if name == "" {
                name = env::current_exe()
                    .map(|x| x.display().to_string())
                    .unwrap_or_else(|_| "<main>".to_string());
            }

            rv.push(
                SymbolicDebugImage {
                    name,
                    arch: None,
                    image_addr: lowest_addr.into(),
                    image_size: highest_addr - lowest_addr,
                    image_vmaddr: lowest_vmaddr.into(),
                    id: debug_id,
                }
                .into(),
            );
        });

        Some(rv)
    }

    #[cfg(not(unix))]
    pub fn find_shlibs() -> Option<Vec<DebugImage>> {
        None
    }
}

#[cfg(not(feature = "with_debug_meta"))]
mod findshlibs_support {
    use super::*;
    pub fn find_shlibs() -> Option<Vec<DebugImage>> {
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
    use crate::constants::ARCH;
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
                crate::protocol::OsContext {
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
    #[cfg(all(feature = "with_device_info", windows))]
    {
        use crate::constants::PLATFORM;
        Some(
            crate::protocol::OsContext {
                name: Some(PLATFORM.into()),
                ..Default::default()
            }
            .into(),
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
        use crate::constants::{RUSTC_CHANNEL, RUSTC_VERSION};
        let ctx = crate::protocol::RuntimeContext {
            name: Some("rustc".into()),
            version: RUSTC_VERSION.map(|x| x.into()),
            other: {
                let mut map = crate::protocol::Map::default();
                if let Some(channel) = RUSTC_CHANNEL {
                    map.insert("channel".to_string(), channel.into());
                }
                map
            },
        }
        .into();
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
            crate::protocol::DeviceContext {
                model,
                family,
                arch,
                ..Default::default()
            }
            .into(),
        )
    }
    #[cfg(not(feature = "with_device_info"))]
    {
        None
    }
}

/// Returns the loaded debug images.
pub fn debug_images() -> Vec<DebugImage> {
    findshlibs_support::find_shlibs().unwrap_or_else(Vec::new)
}

/// Captures information about the current thread.
///
/// If `with_stack` is set to `true` the current stacktrace is
/// attached.
pub fn current_thread(with_stack: bool) -> Thread {
    let thread_id: u64 = unsafe { std::mem::transmute(thread::current().id()) };
    Thread {
        id: Some(thread_id.to_string().into()),
        name: thread::current().name().map(str::to_owned),
        current: true,
        stacktrace: if with_stack {
            current_stacktrace()
        } else {
            None
        },
        ..Default::default()
    }
}

/// Returns the current backtrace as sentry stacktrace.
pub fn current_stacktrace() -> Option<Stacktrace> {
    #[cfg(feature = "with_backtrace")]
    {
        use crate::backtrace_support::current_stacktrace;
        current_stacktrace()
    }
    #[cfg(not(feature = "with_backtrace"))]
    {
        None
    }
}
