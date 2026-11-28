//! Linker setup for the GUI showcase firmware (RP2040).
//!
//! The 256-byte second-stage bootloader lives at `0x10000000`, so flash starts
//! at `0x10000100`; `link-rp.x` (emitted by `embassy-rp`) places `.boot2`.

use std::env;
use std::fs;
use std::path::PathBuf;

const MEMORY: &str = r#"
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}
"#;

fn main() {
    assert!(
        env::var_os("CARGO_FEATURE_RP2040").is_some(),
        "enable the `rp2040` feature"
    );
    let target = env::var("TARGET").unwrap_or_default();
    assert_eq!(
        target, "thumbv6m-none-eabi",
        "this firmware must be built for thumbv6m-none-eabi (got {target})"
    );

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out.join("memory.x"), MEMORY).expect("write memory.x");

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rerun-if-changed=build.rs");
}
