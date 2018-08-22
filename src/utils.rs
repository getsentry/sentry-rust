//! Useful utilities for working with events.
use std::fmt::Debug;
use std::mem;
use std::thread;

use regex::{Captures, Regex};

use api::protocol::{
    Context, DebugImage, DeviceContext, Event, Level, LogEntry, OsContext, RuntimeContext,
    Stacktrace, Thread,
};

lazy_static! {
    static ref PARAM_RE: Regex = Regex::new(r#"\{\{|\}\}|\{\}"#).unwrap();
}

#[cfg(all(feature = "with_device_info", target_os = "macos"))]
mod model_support {
    use libc;
    use libc::c_void;
    use regex::Regex;
    use std::ptr;

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
            Some(
                String::from_utf8_lossy(if buf.ends_with(b"\x00") {
                    &buf[..size - 1]
                } else {
                    &buf
                }).to_string(),
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
    use api::protocol::debugid::DebugId;
    use api::protocol::SymbolicDebugImage;
    use findshlibs::{
        Segment, SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED,
    };
    use uuid::Uuid;

    pub fn find_shlibs() -> Option<Vec<DebugImage>> {
        if !TARGET_SUPPORTED {
            return None;
        }

        let mut rv = vec![];
        TargetSharedLibrary::each(|shlib| {
            let debug_id = match shlib.id() {
                Some(SharedLibraryId::Uuid(bytes)) => {
                    DebugId::from_uuid(Uuid::from_uuid_bytes(bytes))
                }
                None => return,
            };

            let mut lowest_addr = !0;
            let mut lowest_vmaddr = !0;
            let mut highest_addr = 0;

            for seg in shlib.segments() {
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

            rv.push(
                SymbolicDebugImage {
                    name: shlib.name().to_string_lossy().to_string(),
                    arch: None,
                    image_addr: lowest_addr.into(),
                    image_size: highest_addr - lowest_addr,
                    image_vmaddr: lowest_vmaddr.into(),
                    id: debug_id,
                }.into(),
            );
        });

        Some(rv)
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
                    name: Some(info.sysname),
                    kernel_version: Some(info.version),
                    version: Some(info.release),
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
                model,
                family,
                arch,
                ..Default::default()
            }.into(),
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
    let thread_id: u64 = unsafe { mem::transmute(thread::current().id()) };
    Thread {
        id: Some(thread_id.to_string().into()),
        name: thread::current().name().map(|x| x.to_string()),
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
        use backtrace_support::current_stacktrace;
        current_stacktrace()
    }
    #[cfg(not(feature = "with_backtrace"))]
    {
        None
    }
}

/// Creates an event from a message.
///
/// If no params are provided the message is stored in the `message` attribute, otherwise
/// they are formatted out into a log entry.
pub fn event_from_message(msg: &str, params: &[&Debug], level: Level) -> Event<'static> {
    let mut event = Event {
        level,
        ..Default::default()
    };

    if params.is_empty() {
        event.message = Some(msg.to_string());
    } else {
        event.logentry = Some(LogEntry {
            message: PARAM_RE
                .replace_all(&msg, |caps: &Captures| match &caps[0] {
                    "{{" => "{".to_string(),
                    "}}" => "}".to_string(),
                    "{}" => "%s".to_string(),
                    _ => unreachable!(),
                })
                .to_string(),
            params: params.iter().map(|x| format!("{:?}", x).into()).collect(),
        });
    }

    event
}

#[test]
fn test_event_from_message() {
    use serde_json::Value;

    let evt = event_from_message("Hello World!", &[], Level::Info);
    assert_eq!(evt.message.as_ref().unwrap(), "Hello World!");

    let evt = event_from_message("Hello World!", &[&42, &"test"], Level::Info);
    let entry = evt.logentry.unwrap();
    assert_eq!(&entry.message, "Hello World!");
    assert_eq!(&entry.params, &vec![Value::String("42".into()), Value::String("\"test\"".into())]);
}
