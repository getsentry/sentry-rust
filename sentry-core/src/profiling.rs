use indexmap::set::IndexSet;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::DateTime;
use findshlibs::{SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED};

use sentry_types::protocol::v7::Profile;
use sentry_types::protocol::v7::{
    DebugImage, DebugMeta, DeviceMetadata, OSMetadata, RuntimeMetadata, RustFrame, Sample,
    SampleProfile, SymbolicDebugImage, ThreadMetadata, TraceId, Transaction, TransactionMetadata,
    Version,
};
use sentry_types::{CodeId, DebugId, Uuid};

#[cfg(feature = "client")]
use crate::Client;

static PROFILER_RUNNING: AtomicBool = AtomicBool::new(false);

pub(crate) struct ProfilerGuard(pprof::ProfilerGuard<'static>);

impl fmt::Debug for ProfilerGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ProfilerGuard]")
    }
}

pub(crate) fn start_profiling(client: &Client) -> Option<ProfilerGuard> {
    // if profiling is not enabled or the profile was not sampled
    // return None immediately
    if !client.options().enable_profiling
        || !client.sample_should_send(client.options().profiles_sample_rate)
    {
        return None;
    }

    // if no other profile is being collected, then
    // start the profiler
    if let Ok(false) =
        PROFILER_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
    {
        let profile_guard_builder = pprof::ProfilerGuardBuilder::default()
            .frequency(100)
            .blocklist(&["libc", "libgcc", "pthread", "vdso"])
            .build();

        match profile_guard_builder {
            Ok(guard_builder) => return Some(ProfilerGuard(guard_builder)),
            Err(err) => {
                sentry_debug!(
                    "could not start the profiler due to the following error: {:?}",
                    err
                );
                PROFILER_RUNNING.store(false, Ordering::SeqCst);
            }
        }
    }
    None
}

pub(crate) fn finish_profiling(
    transaction: &Transaction,
    profiler_guard: ProfilerGuard,
    trace_id: TraceId,
) -> Option<SampleProfile> {
    let sample_profile = match profiler_guard.0.report().build_unresolved() {
        Ok(report) => Some(get_profile_from_report(&report, trace_id, transaction)),
        Err(err) => {
            sentry_debug!(
                "could not build the profile result due to the error: {}",
                err
            );
            None
        }
    };

    PROFILER_RUNNING.store(false, Ordering::SeqCst);
    sample_profile
}

/// Converts an ELF object identifier into a `DebugId`.
///
/// The identifier data is first truncated or extended to match 16 byte size of
/// Uuids. If the data is declared in little endian, the first three Uuid fields
/// are flipped to match the big endian expected by the breakpad processor.
///
/// The `DebugId::appendix` field is always `0` for ELF.
fn debug_id_from_build_id(build_id: &[u8]) -> Option<DebugId> {
    const UUID_SIZE: usize = 16;
    let mut data = [0u8; UUID_SIZE];
    let len = build_id.len().min(UUID_SIZE);
    data[0..len].copy_from_slice(&build_id[0..len]);

    #[cfg(target_endian = "little")]
    {
        // The ELF file targets a little endian architecture. Convert to
        // network byte order (big endian) to match the Breakpad processor's
        // expectations. For big endian object files, this is not needed.
        data[0..4].reverse(); // uuid field 1
        data[4..6].reverse(); // uuid field 2
        data[6..8].reverse(); // uuid field 3
    }

    Uuid::from_slice(&data).map(DebugId::from_uuid).ok()
}

pub fn debug_images() -> Vec<DebugImage> {
    let mut images = vec![];
    if !TARGET_SUPPORTED {
        return images;
    }

    //crate:: ::{CodeId, DebugId, Uuid};
    TargetSharedLibrary::each(|shlib| {
        let maybe_debug_id = shlib.debug_id().and_then(|id| match id {
            SharedLibraryId::Uuid(bytes) => Some(DebugId::from_uuid(Uuid::from_bytes(bytes))),
            SharedLibraryId::GnuBuildId(ref id) => debug_id_from_build_id(id),
            SharedLibraryId::PdbSignature(guid, age) => DebugId::from_guid_age(&guid, age).ok(),
            _ => None,
        });

        let debug_id = match maybe_debug_id {
            Some(debug_id) => debug_id,
            None => return,
        };

        let mut name = shlib.name().to_string_lossy().to_string();
        if name.is_empty() {
            name = std::env::current_exe()
                .map(|x| x.display().to_string())
                .unwrap_or_else(|_| "<main>".to_string());
        }

        let code_id = shlib.id().map(|id| CodeId::new(format!("{}", id)));
        let debug_name = shlib.debug_name().map(|n| n.to_string_lossy().to_string());

        // For windows, the `virtual_memory_bias` actually returns the real
        // `module_base`, which is the address that sentry uses for symbolication.
        // Going via the segments means that the `image_addr` would be offset in
        // a way that symbolication yields wrong results.
        let (image_addr, image_vmaddr) = if cfg!(windows) {
            (shlib.virtual_memory_bias().0.into(), 0.into())
        } else {
            (
                shlib.actual_load_addr().0.into(),
                shlib.stated_load_addr().0.into(),
            )
        };

        images.push(
            SymbolicDebugImage {
                id: debug_id,
                name,
                arch: None,
                image_addr,
                image_size: shlib.len() as u64,
                image_vmaddr,
                code_id,
                debug_file: debug_name,
            }
            .into(),
        );
    });

    images
}

fn get_profile_from_report(
    rep: &pprof::UnresolvedReport,
    trace_id: TraceId,
    transaction: &Transaction,
) -> SampleProfile {
    use std::time::SystemTime;

    let mut samples: Vec<Sample> = Vec::with_capacity(rep.data.len());
    let mut stacks: Vec<Vec<u32>> = Vec::with_capacity(rep.data.len());
    let mut address_to_frame_idx: IndexSet<RustFrame> = IndexSet::new();
    let mut thread_metadata: HashMap<String, ThreadMetadata> = HashMap::new();

    for sample in rep.data.keys() {
        let stack = sample
            .frames
            .iter()
            .map(|frame| {
                #[cfg(feature = "frame-pointer")]
                let instruction_addr = format!("{:p}", frame.ip as *mut core::ffi::c_void);
                #[cfg(not(feature = "frame-pointer"))]
                let instruction_addr = format!("{:p}", frame.ip());
                let rust_frame = RustFrame { instruction_addr };

                address_to_frame_idx
                    .get_index_of(&rust_frame)
                    .unwrap_or_else(|| -> usize {
                        address_to_frame_idx.insert(rust_frame);

                        address_to_frame_idx.len() - 1
                    }) as u32
            })
            .collect();

        stacks.push(stack);
        samples.push(Sample {
            stack_id: (stacks.len() - 1) as u32,
            thread_id: sample.thread_id,
            relative_timestamp_ns: sample
                .sample_timestamp
                .duration_since(rep.timing.start_time)
                .unwrap()
                .as_nanos() as u64,
        });

        thread_metadata
            .entry(sample.thread_id.to_string())
            .or_insert(ThreadMetadata {
                name: Some(
                    String::from_utf8_lossy(&sample.thread_name[0..sample.thread_name_length])
                        .into_owned(),
                ),
            });
    }

    SampleProfile {
        version: Version::V1,
        debug_meta: Some(DebugMeta {
            sdk_info: None,
            images: debug_images(),
        }),
        device: DeviceMetadata {
            architecture: Some(std::env::consts::ARCH.to_string()),
        },
        os: OSMetadata {
            name: sys_info::os_type().unwrap(),
            version: sys_info::os_release().unwrap(),
            build_number: None,
        },
        runtime: Some(RuntimeMetadata {
            name: "rustc".to_string(),
            version: rustc_version_runtime::version().to_string(),
        }),
        environment: match &transaction.environment {
            Some(env) => env.to_string(),
            _ => "".to_string(),
        },

        event_id: uuid::Uuid::new_v4(),
        release: transaction
            .release
            .as_ref()
            .map_or("".to_string(), |r| -> String { r.to_string() }),
        timestamp: DateTime::from(rep.timing.start_time),
        transactions: vec![TransactionMetadata {
            id: transaction.event_id,
            name: transaction.name.clone().unwrap_or_else(|| "".to_string()),
            trace_id,
            relative_start_ns: 0,
            relative_end_ns: transaction
                .timestamp
                .unwrap_or_else(SystemTime::now)
                .duration_since(rep.timing.start_time)
                .unwrap()
                .as_nanos() as u64,
            active_thread_id: transaction.active_thread_id.unwrap_or(0),
        }],
        platform: "rust".to_string(),
        profile: Profile {
            samples,
            stacks,
            frames: address_to_frame_idx.into_iter().collect(),
            thread_metadata,
        },
    }
}
