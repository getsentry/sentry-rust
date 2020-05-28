use std::env;

use sentry_core::protocol::debugid::DebugId;
use sentry_core::protocol::{DebugImage, SymbolicDebugImage};
use sentry_core::types::Uuid;

use findshlibs::{Segment, SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED};

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

        images.push(
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

    images
}
