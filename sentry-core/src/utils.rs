//! Useful utilities for working with events.
use crate::protocol::{Context, DebugImage};

/// Split the str into a typename and optional module prefix.
///
/// # Examples
///
/// ```
/// use sentry_core::utils::parse_type_name;
///
/// let parsed = parse_type_name(std::any::type_name::<Vec<Option<usize>>>());
/// assert_eq!(parsed, (Some("alloc::vec".into()), "Vec<core::option::Option<usize>>".into()));
/// ```
pub fn parse_type_name(mut type_name: &str) -> (Option<String>, String) {
    let is_dyn = type_name.starts_with("dyn ");
    if is_dyn {
        type_name = &type_name[4..];
    }
    let name = |ty| {
        let mut name = if is_dyn {
            String::from("dyn ")
        } else {
            String::new()
        };
        name.push_str(ty);
        name
    };

    // The nesting level of `</>` brackets for type parameters.
    let mut param_level = 0usize;
    // If we have just seen a `:`.
    let mut in_colon = false;
    // We iterate back to front, looking for the first `::` module separator
    // that is not inside a type parameter.
    for (i, c) in type_name.chars().rev().enumerate() {
        match c {
            '>' => {
                param_level += 1;
                in_colon = false;
            }
            '<' => {
                param_level = param_level.saturating_sub(1);
                in_colon = false;
            }
            ':' if in_colon => {
                let (module, ty) = type_name.split_at(type_name.len() - i - 1);
                return (Some(module.into()), name(&ty[2..]));
            }
            ':' if param_level == 0 => in_colon = true,
            _ => in_colon = false,
        }
    }

    (None, name(type_name))
}

#[test]
fn test_parse_type_name() {
    assert_eq!(parse_type_name("JustName"), (None, "JustName".into()));
    assert_eq!(
        parse_type_name("With<Generics>"),
        (None, "With<Generics>".into()),
    );
    assert_eq!(
        parse_type_name("with::module::Path"),
        (Some("with::module".into()), "Path".into()),
    );
    assert_eq!(
        parse_type_name("with::module::Path<and::Generics>"),
        (Some("with::module".into()), "Path<and::Generics>".into()),
    );

    assert_eq!(
        parse_type_name("dyn std::error::Error"),
        (Some("std::error".into()), "dyn Error".into()),
    );
}

#[cfg(all(feature = "with_device_info", target_os = "macos"))]
mod model_support {
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

        use crate::internals::Uuid;
        use crate::protocol::debugid::DebugId;
        use crate::protocol::SymbolicDebugImage;
        use findshlibs::{
            Segment, SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED,
        };
        use std::env;

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
        hostname::get().ok().and_then(|s| s.into_string().ok())
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
    #[cfg(feature = "with_rust_info")]
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
    #[cfg(not(feature = "with_rust_info"))]
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
