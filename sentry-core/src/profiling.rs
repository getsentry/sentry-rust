use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use findshlibs::{SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED};

use crate::TransactionInner;
use sentry_types::protocol::v7::Profile;
use sentry_types::protocol::v7::{
    DebugImage, DebugMeta, RustFrame, Sample, SampledProfile, SymbolicDebugImage, TraceId,
    Transaction,
};
use sentry_types::{CodeId, DebugId, Uuid};

#[cfg(feature = "client")]
use crate::Client;

pub(crate) struct ProfilerGuard(pprof::ProfilerGuard<'static>);

impl fmt::Debug for ProfilerGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ProfilerGuard]")
    }
}

pub(crate) fn start_profiling(
    profiler_running: &AtomicBool,
    client: &Arc<Client>,
) -> Option<ProfilerGuard> {
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
        profiler_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
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
            }
        }
    }
    None
}

pub(crate) fn finish_profiling(
    profiler_running: &AtomicBool,
    transaction: &Transaction,
    transaction_inner: &mut TransactionInner,
) -> Option<Profile> {
    // if the profiler is running for this transaction,
    // then stop it and return the profile
    if let Some(profiler_guard) = transaction_inner.profiler_guard.take() {
        let mut profile: Option<Profile> = None;

        match profiler_guard.0.report().build_unresolved() {
            Ok(report) => {
                profile = Some(get_profile_from_report(
                    &report,
                    transaction_inner.context.trace_id,
                    transaction.event_id,
                    transaction.name.as_ref().unwrap().clone(),
                ));
            }
            Err(err) => {
                sentry_debug!(
                    "could not build the profile result due to the error: {}",
                    err
                );
            }
        }
        profiler_running.store(false, Ordering::SeqCst);
        return profile;
    }
    None
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
    transaction_id: sentry_types::Uuid,
    transaction_name: String,
) -> Profile {
    use std::time::SystemTime;

    let mut samples: Vec<Sample> = Vec::new();

    for sample in rep.data.keys() {
        let frames = sample
            .frames
            .iter()
            .map(|frame| RustFrame {
                instruction_addr: format!("{:p}", frame.ip()),
            })
            .collect();

        samples.push(Sample {
            frames,
            thread_name: String::from_utf8_lossy(&sample.thread_name[0..sample.thread_name_length])
                .into_owned(),
            thread_id: sample.thread_id,
            nanos_relative_to_start: sample
                .sample_timestamp
                .duration_since(rep.timing.start_time)
                .unwrap()
                .as_nanos() as u64,
        });
    }
    let sampled_profile = SampledProfile {
        start_time_nanos: rep
            .timing
            .start_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
        start_time_secs: rep
            .timing
            .start_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        duration_nanos: rep.timing.duration.as_nanos() as u64,
        samples,
    };

    let profile: Profile = Profile {
        duration_ns: sampled_profile.duration_nanos,
        debug_meta: DebugMeta {
            sdk_info: None,
            images: debug_images(),
        },
        platform: "rust".to_string(),
        architecture: Some(std::env::consts::ARCH.to_string()),
        trace_id,
        transaction_name,
        transaction_id,
        profile_id: uuid::Uuid::new_v4(),
        sampled_profile,
        os_name: sys_info::os_type().unwrap(),
        os_version: sys_info::os_release().unwrap(),
        version_name: env!("CARGO_PKG_VERSION").to_string(),
        version_code: build_id::get().to_simple().to_string(),
    };

    profile
}
