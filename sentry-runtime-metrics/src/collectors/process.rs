//! Process metrics collector.

use crate::collector::MetricCollector;
use crate::protocol::RuntimeMetric;

/// Collects process-level metrics.
///
/// Metrics collected:
/// - `process.threads.count` - Number of threads
/// - `process.cpu.user_time` - User CPU time in milliseconds
/// - `process.cpu.system_time` - System CPU time in milliseconds
/// - `process.open_fds` - Open file descriptors (Unix only)
/// - `process.uptime` - Process uptime in seconds
pub struct ProcessCollector {
    start_time: std::time::Instant,
}

impl ProcessCollector {
    /// Creates a new process collector.
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
        }
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

        // Process uptime
        let uptime = self.start_time.elapsed().as_secs();
        metrics.push(
            RuntimeMetric::gauge("process.uptime", uptime as i64)
                .with_unit("seconds"),
        );

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

        // Should always have uptime
        assert!(!metrics.is_empty());
        assert!(metrics.iter().any(|m| m.name == "process.uptime"));
    }

    #[test]
    fn test_cpu_times() {
        #[cfg(unix)]
        {
            let times = get_cpu_times();
            assert!(times.is_some());
        }
    }
}
