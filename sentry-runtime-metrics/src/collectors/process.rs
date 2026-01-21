//! Process metrics collector.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::time::SystemTime;

use crate::collector::MetricCollector;
use crate::protocol::RuntimeMetric;

/// Collects process-level metrics.
///
/// Metrics collected:
/// - `process.threads.count` - Number of threads
/// - `process.cpu.user_time` - User CPU time in milliseconds
/// - `process.cpu.system_time` - System CPU time in milliseconds
/// - `process.open_fds` - Open file descriptors (Unix only)
/// - `process.uptime` - Process uptime in seconds (actual process lifetime)
pub struct ProcessCollector;

impl ProcessCollector {
    /// Creates a new process collector.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for ProcessCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        let mut metrics = Vec::new();

        // Process uptime - actual process lifetime, not collector lifetime
        if let Some(uptime) = get_process_uptime() {
            metrics.push(
                RuntimeMetric::gauge("process.uptime", uptime)
                    .with_unit("seconds"),
            );
        }

        // Thread count
        if let Some(thread_count) = get_thread_count() {
            metrics.push(
                RuntimeMetric::gauge("process.threads.count", thread_count)
                    .with_unit("count"),
            );
        }

        // CPU times
        if let Some((user_time, system_time)) = get_cpu_times() {
            metrics.push(
                RuntimeMetric::counter("process.cpu.user_time", user_time)
                    .with_unit("milliseconds"),
            );
            metrics.push(
                RuntimeMetric::counter("process.cpu.system_time", system_time)
                    .with_unit("milliseconds"),
            );
        }

        // Open file descriptors (Unix only)
        #[cfg(unix)]
        if let Some(fd_count) = get_open_fds() {
            metrics.push(
                RuntimeMetric::gauge("process.open_fds", fd_count)
                    .with_unit("count"),
            );
        }

        metrics
    }

    fn name(&self) -> &'static str {
        "process"
    }
}

/// Gets the number of threads in the current process.
#[cfg(target_os = "linux")]
fn get_thread_count() -> Option<i64> {
    use std::fs;

    // Count entries in /proc/self/task/
    let entries = fs::read_dir("/proc/self/task").ok()?;
    Some(entries.count() as i64)
}

/// Gets the number of threads in the current process.
#[cfg(target_os = "macos")]
fn get_thread_count() -> Option<i64> {
    // macOS doesn't have an easy way to get thread count
    // Would require mach APIs
    None
}

/// Gets the number of threads in the current process.
#[cfg(target_os = "windows")]
fn get_thread_count() -> Option<i64> {
    // Getting thread count on Windows requires snapshot APIs (CreateToolhelp32Snapshot)
    // For simplicity, we return None here
    None
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_thread_count() -> Option<i64> {
    None
}

/// Gets the actual process uptime in seconds.
#[cfg(target_os = "linux")]
fn get_process_uptime() -> Option<i64> {
    use std::fs;

    // Read process start time from /proc/self/stat
    // Field 22 (0-indexed: 21) is starttime in clock ticks since boot
    let stat = fs::read_to_string("/proc/self/stat").ok()?;

    // Find the last ')' to skip the command name (which may contain spaces)
    let after_comm = stat.rfind(')')? + 2;
    let fields: Vec<&str> = stat[after_comm..].split_whitespace().collect();

    // starttime is field 20 after the command (field 22 overall, 0-indexed as 21)
    // After ')' we're at field 2, so starttime is at index 19
    let starttime_ticks: u64 = fields.get(19)?.parse().ok()?;

    // Get system boot time from /proc/stat
    let stat_content = fs::read_to_string("/proc/stat").ok()?;
    let btime_line = stat_content.lines().find(|l| l.starts_with("btime "))?;
    let boot_time: u64 = btime_line.split_whitespace().nth(1)?.parse().ok()?;

    // Get clock ticks per second (usually 100)
    let ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;

    // Process start time in seconds since epoch
    let process_start = boot_time + (starttime_ticks / ticks_per_sec);

    // Current time
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();

    Some((now - process_start) as i64)
}

/// Gets the actual process uptime in seconds.
#[cfg(target_os = "macos")]
fn get_process_uptime() -> Option<i64> {
    use std::mem;

    // Use sysctl to get process start time
    // CTL_KERN, KERN_PROC, KERN_PROC_PID, pid
    const CTL_KERN: libc::c_int = 1;
    const KERN_PROC: libc::c_int = 14;
    const KERN_PROC_PID: libc::c_int = 1;

    unsafe {
        let pid = libc::getpid();
        let mut mib = [CTL_KERN, KERN_PROC, KERN_PROC_PID, pid];

        let mut info: libc::kinfo_proc = mem::zeroed();
        let mut size = mem::size_of::<libc::kinfo_proc>();

        let result = libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            &mut info as *mut _ as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        );

        if result == 0 {
            // p_starttime is a timeval with process start time
            let start_secs = info.kp_proc.p_starttime.tv_sec as u64;

            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .ok()?
                .as_secs();

            Some((now - start_secs) as i64)
        } else {
            None
        }
    }
}

/// Gets the actual process uptime in seconds.
#[cfg(target_os = "windows")]
fn get_process_uptime() -> Option<i64> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    unsafe {
        let process = GetCurrentProcess();
        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();

        if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
            // FILETIME is 100-nanosecond intervals since January 1, 1601 (UTC)
            let creation_100ns =
                ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64);

            // Get current time as FILETIME
            let mut now: FILETIME = std::mem::zeroed();
            windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime(&mut now);
            let now_100ns = ((now.dwHighDateTime as u64) << 32) | (now.dwLowDateTime as u64);

            // Difference in 100-nanosecond intervals, convert to seconds
            let uptime_secs = (now_100ns - creation_100ns) / 10_000_000;
            Some(uptime_secs as i64)
        } else {
            None
        }
    }
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_process_uptime() -> Option<i64> {
    None
}

/// Gets CPU times (user, system) in milliseconds.
#[cfg(unix)]
fn get_cpu_times() -> Option<(i64, i64)> {
    use std::mem;

    unsafe {
        let mut usage: libc::rusage = mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            let user_ms = usage.ru_utime.tv_sec * 1000 + usage.ru_utime.tv_usec / 1000;
            let sys_ms = usage.ru_stime.tv_sec * 1000 + usage.ru_stime.tv_usec / 1000;
            Some((user_ms, sys_ms))
        } else {
            None
        }
    }
}

/// Gets CPU times (user, system) in milliseconds.
#[cfg(target_os = "windows")]
fn get_cpu_times() -> Option<(i64, i64)> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    unsafe {
        let process = GetCurrentProcess();
        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();

        if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
            // FILETIME is in 100-nanosecond intervals
            let user_100ns =
                ((user.dwHighDateTime as u64) << 32) | (user.dwLowDateTime as u64);
            let kernel_100ns =
                ((kernel.dwHighDateTime as u64) << 32) | (kernel.dwLowDateTime as u64);

            // Convert to milliseconds
            let user_ms = (user_100ns / 10_000) as i64;
            let kernel_ms = (kernel_100ns / 10_000) as i64;

            Some((user_ms, kernel_ms))
        } else {
            None
        }
    }
}

/// Fallback for unsupported platforms.
#[cfg(not(any(unix, target_os = "windows")))]
fn get_cpu_times() -> Option<(i64, i64)> {
    None
}

/// Gets the number of open file descriptors.
#[cfg(target_os = "linux")]
fn get_open_fds() -> Option<i64> {
    use std::fs;

    let entries = fs::read_dir("/proc/self/fd").ok()?;
    Some(entries.count() as i64)
}

/// Gets the number of open file descriptors.
#[cfg(target_os = "macos")]
fn get_open_fds() -> Option<i64> {
    // Would require PROC_PIDLISTFDS
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_collector() {
        let collector = ProcessCollector::new();
        let metrics = collector.collect();

        // Should have some metrics (uptime may not be available on all platforms)
        // On Linux/macOS/Windows, uptime should be present
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            assert!(!metrics.is_empty());
            assert!(
                metrics.iter().any(|m| m.name == "process.uptime"),
                "Expected process.uptime metric on supported platform"
            );
        }
    }

    #[test]
    fn test_cpu_times() {
        #[cfg(unix)]
        {
            let times = get_cpu_times();
            assert!(times.is_some());
        }
    }

    #[test]
    fn test_process_uptime() {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let uptime = get_process_uptime();
            assert!(uptime.is_some(), "Expected uptime on supported platform");
            // Uptime should be non-negative (process just started, so likely 0 or small)
            assert!(uptime.unwrap() >= 0);
        }
    }
}
