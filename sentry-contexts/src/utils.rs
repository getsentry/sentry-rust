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

            let mut buf = vec![0u8; size];
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
        assert!(f.chars().all(|c| !c.is_ascii_digit()));
    }

    #[test]
    fn test_macos_version_and_build() {
        let v = get_macos_version().unwrap();
        assert!(v.chars().all(|c| c.is_ascii_digit() || c == '.'));
        let dot_count = v.split('.').count() - 1;
        assert_eq!(dot_count, 2);
        let b = get_macos_build().unwrap();
        assert!(b
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit()));
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
#[cfg(not(windows))]
pub fn os_context() -> Option<Context> {
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

/// Returns the OS context
#[cfg(windows)]
pub fn os_context() -> Option<Context> {
    use os_info::Version;
    let version = match os_info::get().version() {
        Version::Unknown => None,
        version => Some(version.to_string()),
    };

    Some(
        OsContext {
            name: Some(PLATFORM.into()),
            version,
            ..Default::default()
        }
        .into(),
    )
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
            if let Some(uptime) = process_uptime_secs() {
                map.insert("process_uptime".to_string(), uptime.into());
            }
            map
        },
    }
    .into()
}

#[cfg(target_os = "linux")]
fn process_uptime_secs() -> Option<f64> {
    use std::fs;

    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let parts: Vec<&str> = stat.split_whitespace().collect();
    let starttime_ticks: u64 = parts.get(21)?.parse().ok()?;

    let uptime_str = fs::read_to_string("/proc/uptime").ok()?;
    let system_uptime: f64 = uptime_str.split_whitespace().next()?.parse().ok()?;

    let ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    let process_start_secs = starttime_ticks as f64 / ticks_per_sec;

    Some((system_uptime - process_start_secs).max(0.0))
}

#[cfg(target_os = "macos")]
fn process_uptime_secs() -> Option<f64> {
    use std::mem::MaybeUninit;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    unsafe {
        let pid = libc::getpid();
        let mut info = MaybeUninit::<libc::proc_bsdinfo>::uninit();
        let size = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of::<libc::proc_bsdinfo>() as i32,
        );
        if size <= 0 {
            return None;
        }
        let info = info.assume_init();
        let start_time = Duration::new(
            info.pbi_start_tvsec as u64,
            (info.pbi_start_tvusec * 1000) as u32,
        );
        let uptime = SystemTime::now()
            .duration_since(UNIX_EPOCH + start_time)
            .ok()?;
        Some(uptime.as_secs_f64())
    }
}

#[cfg(windows)]
fn process_uptime_secs() -> Option<f64> {
    use std::mem::MaybeUninit;

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    extern "system" {
        fn GetProcessTimes(
            process: *mut std::ffi::c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetSystemTimeAsFileTime(time: *mut FileTime);
    }

    unsafe {
        let mut creation = MaybeUninit::<FileTime>::uninit();
        let mut exit = MaybeUninit::<FileTime>::uninit();
        let mut kernel = MaybeUninit::<FileTime>::uninit();
        let mut user = MaybeUninit::<FileTime>::uninit();

        if GetProcessTimes(
            GetCurrentProcess(),
            creation.as_mut_ptr(),
            exit.as_mut_ptr(),
            kernel.as_mut_ptr(),
            user.as_mut_ptr(),
        ) == 0
        {
            return None;
        }

        let creation = creation.assume_init();
        let creation_time = ((creation.high as u64) << 32) | (creation.low as u64);

        let mut now = MaybeUninit::<FileTime>::uninit();
        GetSystemTimeAsFileTime(now.as_mut_ptr());
        let now = now.assume_init();
        let now_time = ((now.high as u64) << 32) | (now.low as u64);

        Some(now_time.saturating_sub(creation_time) as f64 / 10_000_000.0)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn process_uptime_secs() -> Option<f64> {
    None
}

/// Returns the device context.
pub fn device_context() -> Context {
    let mut ctx = DeviceContext {
        model: model_support::get_model(),
        family: model_support::get_family(),
        arch: Some(ARCH.into()),
        ..Default::default()
    };

    if let Some(mem) = memory_info() {
        ctx.memory_size = Some(mem.total);
        ctx.free_memory = Some(mem.free);
        ctx.usable_memory = Some(mem.available);
    }

    ctx.into()
}

struct MemoryInfo {
    total: u64,
    free: u64,
    available: u64,
}

#[cfg(target_os = "linux")]
fn memory_info() -> Option<MemoryInfo> {
    use std::fs;

    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut free = None;
    let mut available = None;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let value: u64 = parts[1].parse().ok()?;
        let bytes = value * 1024;
        match parts[0] {
            "MemTotal:" => total = Some(bytes),
            "MemFree:" => free = Some(bytes),
            "MemAvailable:" => available = Some(bytes),
            _ => {}
        }
    }

    Some(MemoryInfo {
        total: total?,
        free: free?,
        available: available.unwrap_or(free?),
    })
}

#[cfg(target_os = "macos")]
fn memory_info() -> Option<MemoryInfo> {
    use std::mem::MaybeUninit;

    unsafe {
        let mut mib = [libc::CTL_HW, libc::HW_MEMSIZE];
        let mut total: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        if libc::sysctl(
            mib.as_mut_ptr(),
            2,
            &mut total as *mut u64 as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }

        let mut vm_stats = MaybeUninit::<libc::vm_statistics64>::uninit();
        let mut count = std::mem::size_of::<libc::vm_statistics64>() as u32
            / std::mem::size_of::<libc::natural_t>() as u32;
        let host = libc::mach_host_self();
        if libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            vm_stats.as_mut_ptr() as *mut _,
            &mut count,
        ) != libc::KERN_SUCCESS
        {
            return None;
        }
        let vm_stats = vm_stats.assume_init();

        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as u64;
        let free = vm_stats.free_count as u64 * page_size;
        let available = (vm_stats.free_count as u64 + vm_stats.inactive_count as u64) * page_size;

        Some(MemoryInfo {
            total,
            free,
            available,
        })
    }
}

#[cfg(windows)]
fn memory_info() -> Option<MemoryInfo> {
    use std::mem::{size_of, MaybeUninit};

    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    unsafe {
        let mut status = MaybeUninit::<MemoryStatusEx>::uninit();
        (*status.as_mut_ptr()).length = size_of::<MemoryStatusEx>() as u32;
        if GlobalMemoryStatusEx(status.as_mut_ptr()) == 0 {
            return None;
        }
        let status = status.assume_init();
        Some(MemoryInfo {
            total: status.total_phys,
            free: status.avail_phys,
            available: status.avail_phys,
        })
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn memory_info() -> Option<MemoryInfo> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn windows_os_version_not_empty() {
        use super::*;
        let context = os_context();
        match context {
            Some(Context::Os(os_context)) => {
                // verify the version is a non-empty string
                let version = os_context.version.expect("OS version to be some");
                assert!(!version.is_empty());

                // verify the version is not equal to the unknown OS version
                let unknown_version = os_info::Version::Unknown.to_string();
                assert_ne!(version, unknown_version);
            }
            _ => unreachable!("os_context() should return a Context::Os"),
        }
    }
}
