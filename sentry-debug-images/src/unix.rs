use std::env;

use findshlibs::{Segment, SharedLibrary, SharedLibraryId, TargetSharedLibrary, TARGET_SUPPORTED};
use sentry_core::protocol::debugid::DebugId;
use sentry_core::protocol::{DebugImage, SymbolicDebugImage};
use sentry_core::types::Uuid;

pub fn debug_images() -> Vec<DebugImage> {
    let mut images = vec![];
    if !TARGET_SUPPORTED {
        return images;
    }

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
