use std::env;

use sentry_core::protocol::debugid::DebugId;
use sentry_core::protocol::{DebugImage, SymbolicDebugImage};
use sentry_core::types::Uuid;

use findshlibs::{SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED};

const UUID_SIZE: usize = 16;

/// Converts an ELF object identifier into a `DebugId`.
///
/// The identifier data is first truncated or extended to match 16 byte size of
/// Uuids. If the data is declared in little endian, the first three Uuid fields
/// are flipped to match the big endian expected by the breakpad processor.
///
/// The `DebugId::appendix` field is always `0` for ELF.
fn debug_id_from_build_id(build_id: &[u8]) -> Option<DebugId> {
    let mut data = [0 as u8; UUID_SIZE];
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

/// Filters for PT_LOAD segments.
#[cfg(target_os = "linux")]
fn filter_seg(lib: &findshlibs::linux::Segment) -> bool {
    lib.is_load()
}

/// Filters for __TEXT segments.
#[cfg(target_os = "macos")]
fn filter_seg(lib: &findshlibs::macos::Segment) -> bool {
    lib.is_code()
}

pub fn debug_images() -> Vec<DebugImage> {
    let mut images = vec![];
    if !TARGET_SUPPORTED {
        return images;
    }

    TargetSharedLibrary::each(|shlib| {
        let maybe_debug_id = shlib.id().and_then(|id| match id {
            SharedLibraryId::Uuid(bytes) => Some(DebugId::from_uuid(Uuid::from_bytes(bytes))),
            SharedLibraryId::GnuBuildId(ref id) => debug_id_from_build_id(id),
        });

        let debug_id = match maybe_debug_id {
            Some(debug_id) => debug_id,
            None => return,
        };

        let mut name = shlib.name().to_string_lossy().to_string();
        if name.is_empty() {
            name = env::current_exe()
                .map(|x| x.display().to_string())
                .unwrap_or_else(|_| "<main>".to_string());
        }

        images.push(
            SymbolicDebugImage {
                name,
                arch: None,
                image_addr: shlib.actual_load_addr().0.into(),
                image_size: shlib.len() as u64,
                image_vmaddr: shlib.stated_load_addr().0.into(),
                id: debug_id,
            }
            .into(),
        );
    });

    images
}
