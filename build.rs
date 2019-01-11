use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.gen.rs");
    let mut f = File::create(&dest_path).unwrap();

    let target = env::var("TARGET").unwrap();
    let mut target_bits = target.split('-');
    let arch = target_bits.next().unwrap();
    target_bits.next();
    let platform = target_bits.next().unwrap();

    #[cfg(feature = "with_rust_info")]
    {
        use rustc_version::{version, version_meta, Channel};
        write!(
            f,
            "/// The rustc version that was used to compile this crate\n"
        )
        .ok();
        if let Ok(version) = version() {
            write!(f, "#[allow(dead_code)] pub const RUSTC_VERSION: Option<&'static str> = Some(\"{}\");\n", version).ok();
        } else {
            write!(
                f,
                "#[allow(dead_code)] pub const RUSTC_VERSION: Option<&'static str> = None;\n"
            )
            .ok();
        }
        if let Ok(version_meta) = version_meta() {
            let chan = match version_meta.channel {
                Channel::Dev => "dev",
                Channel::Nightly => "nightly",
                Channel::Beta => "beta",
                Channel::Stable => "stable",
            };
            write!(f, "#[allow(dead_code)] pub const RUSTC_CHANNEL: Option<&'static str> = Some(\"{}\");\n", chan).ok();
        } else {
            write!(
                f,
                "#[allow(dead_code)] pub const RUSTC_CHANNEL: Option<&'static str> = None;\n"
            )
            .ok();
        }
    }

    write!(f, "/// The platform identifier\n").ok();
    write!(
        f,
        "#[allow(dead_code)] pub const PLATFORM: &str = \"{}\";\n",
        platform
    )
    .ok();
    write!(f, "/// The CPU architecture identifier\n").ok();
    write!(
        f,
        "#[allow(dead_code)] pub const ARCH: &str = \"{}\";\n",
        arch
    )
    .ok();
    println!("cargo:rerun-if-changed=build.rs\n");
    println!("cargo:rerun-if-changed=Cargo.toml\n");
}
