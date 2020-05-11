//! Useful utilities for working with events.

use crate::protocol::DebugImage;

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

/// Returns the loaded debug images.
pub fn debug_images() -> Vec<DebugImage> {
    findshlibs_support::find_shlibs().unwrap_or_else(Vec::new)
}
