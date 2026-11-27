//! Linker setup for the RP2040 / RP2350 firmware.
//!
//! * RP2040: a 256-byte second-stage bootloader lives at `0x10000000`, so flash
//!   starts at `0x10000100` and `link-rp.x` (emitted by `embassy-rp`) places
//!   `.boot2` there.
//! * RP2350: the boot ROM needs the `.start_block` / `.bi_entries` /
//!   `.end_block` sections from the upstream embassy example `memory.x`.

use std::env;
use std::fs;
use std::path::PathBuf;

const MEMORY_RP2040: &str = r#"
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}
"#;

const MEMORY_RP2350: &str = r#"
MEMORY {
    FLASH : ORIGIN = 0x10000000, LENGTH = 2048K
    RAM   : ORIGIN = 0x20000000, LENGTH = 512K
    SRAM8 : ORIGIN = 0x20080000, LENGTH = 4K
    SRAM9 : ORIGIN = 0x20081000, LENGTH = 4K
}
SECTIONS {
    .start_block : ALIGN(4)
    {
        __start_block_addr = .;
        KEEP(*(.start_block));
        KEEP(*(.boot_info));
    } > FLASH
} INSERT AFTER .vector_table;
_stext = ALIGN(ADDR(.start_block) + SIZEOF(.start_block), ALIGNOF(.text));
SECTIONS {
    .bi_entries : ALIGN(4)
    {
        __bi_entries_start = .;
        KEEP(*(.bi_entries));
        . = ALIGN(4);
        __bi_entries_end = .;
    } > FLASH
} INSERT AFTER .text;
SECTIONS {
    .end_block : ALIGN(4)
    {
        __end_block_addr = .;
        KEEP(*(.end_block));
    } > FLASH
} INSERT AFTER .uninit;
PROVIDE(start_to_end = __end_block_addr - __start_block_addr);
PROVIDE(end_to_start = __start_block_addr - __end_block_addr);
"#;

fn main() {
    let rp2040 = env::var_os("CARGO_FEATURE_RP2040").is_some();
    let rp235x = env::var_os("CARGO_FEATURE_RP235XA").is_some();
    let target = env::var("TARGET").unwrap_or_default();

    if rp2040 == rp235x {
        panic!("enable exactly one chip feature: `rp2040` (default) or `rp235xa`");
    }
    if rp2040 && target != "thumbv6m-none-eabi" {
        panic!("the rp2040 feature must be built for thumbv6m-none-eabi (got {target})");
    }
    if rp235x && target != "thumbv8m.main-none-eabihf" {
        panic!("the rp235xa feature must be built for thumbv8m.main-none-eabihf (got {target})");
    }

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(
        out.join("memory.x"),
        if rp2040 { MEMORY_RP2040 } else { MEMORY_RP2350 },
    )
    .expect("write memory.x");

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    if rp2040 {
        println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    }
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_RP2040");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_RP235XA");
}
