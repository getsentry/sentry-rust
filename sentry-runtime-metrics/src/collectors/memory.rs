//! Memory metrics collector.

use crate::collector::MetricCollector;
use crate::protocol::RuntimeMetric;

/// Collects memory-related metrics.
///
/// Metrics collected:
/// - `runtime.memory.rss` - Resident Set Size
/// - `runtime.memory.heap_allocated` - Heap allocation (with jemalloc feature)
pub struct MemoryCollector {
    _private: (),
}

impl MemoryCollector {
    /// Creates a new memory collector.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for MemoryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector for MemoryCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        let mut metrics = Vec::new();

        // Collect RSS (Resident Set Size)
        if let Some(rss) = get_rss_bytes() {
            metrics.push(
                RuntimeMetric::gauge("runtime.memory.rss", rss)
                    .with_unit("bytes"),
            );
        }

        // Collect jemalloc stats if available (Unix only)
        #[cfg(all(unix, feature = "jemalloc"))]
        {
            if let Some((allocated, resident)) = get_jemalloc_stats() {
                metrics.push(
                    RuntimeMetric::gauge("runtime.memory.heap_allocated", allocated)
                        .with_unit("bytes")
                        .with_tag("allocator", "jemalloc"),
                );
                metrics.push(
                    RuntimeMetric::gauge("runtime.memory.heap_resident", resident)
                        .with_unit("bytes")
                        .with_tag("allocator", "jemalloc"),
                );
            }
        }

        metrics
    }

    fn name(&self) -> &'static str {
        "memory"
    }
}

/// Gets the RSS (Resident Set Size) in bytes.
#[cfg(target_os = "linux")]
fn get_rss_bytes() -> Option<i64> {
    use std::fs;

    // Read from /proc/self/statm
    // Format: size resident shared text lib data dt
    // Values are in pages
    let statm = fs::read_to_string("/proc/self/statm").ok()?;
    let parts: Vec<&str> = statm.split_whitespace().collect();

    if parts.len() >= 2 {
        let resident_pages: i64 = parts[1].parse().ok()?;
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as i64;
        Some(resident_pages * page_size)
    } else {
        None
    }
}

/// Gets the RSS (Resident Set Size) in bytes.
/// Uses Mach task_info to get current resident memory, not peak.
#[cfg(target_os = "macos")]
fn get_rss_bytes() -> Option<i64> {
    use std::mem;

    // Mach kernel types for task_info
    type MachPort = libc::c_uint;
    type TaskFlavorT = libc::c_int;
    type MachMsgTypeNumberT = libc::c_uint;
    type KernReturnT = libc::c_int;

    const MACH_TASK_BASIC_INFO: TaskFlavorT = 20;
    const KERN_SUCCESS: KernReturnT = 0;

    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: libc::time_value_t,
        system_time: libc::time_value_t,
        policy: libc::c_int,
        suspend_count: libc::c_int,
    }

    const MACH_TASK_BASIC_INFO_COUNT: MachMsgTypeNumberT =
        (mem::size_of::<MachTaskBasicInfo>() / mem::size_of::<libc::c_int>()) as MachMsgTypeNumberT;

    extern "C" {
        fn mach_task_self() -> MachPort;
        fn task_info(
            target_task: MachPort,
            flavor: TaskFlavorT,
            task_info_out: *mut MachTaskBasicInfo,
            task_info_out_cnt: *mut MachMsgTypeNumberT,
        ) -> KernReturnT;
    }

    unsafe {
        let mut info: MachTaskBasicInfo = mem::zeroed();
        let mut count = MACH_TASK_BASIC_INFO_COUNT;

        let result = task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            &mut info,
            &mut count,
        );

        if result == KERN_SUCCESS {
            Some(info.resident_size as i64)
        } else {
            None
        }
    }
}

/// Gets the RSS (Resident Set Size) in bytes.
#[cfg(target_os = "windows")]
fn get_rss_bytes() -> Option<i64> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let process = GetCurrentProcess();
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

        if GetProcessMemoryInfo(
            process,
            &mut pmc,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
        {
            Some(pmc.WorkingSetSize as i64)
        } else {
            None
        }
    }
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_rss_bytes() -> Option<i64> {
    None
}

/// Gets jemalloc memory statistics (Unix only).
#[cfg(all(unix, feature = "jemalloc"))]
fn get_jemalloc_stats() -> Option<(i64, i64)> {
    use tikv_jemalloc_ctl::{epoch, stats};

    // Advance the epoch to get fresh stats
    epoch::advance().ok()?;

    let allocated = stats::allocated::read().ok()? as i64;
    let resident = stats::resident::read().ok()? as i64;

    Some((allocated, resident))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_collector() {
        let collector = MemoryCollector::new();
        let metrics = collector.collect();

        // Should collect at least RSS on supported platforms
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            assert!(!metrics.is_empty());
            assert!(metrics.iter().any(|m| m.name == "runtime.memory.rss"));
        }
    }
}
