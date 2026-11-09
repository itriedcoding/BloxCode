fn main() {
    println!("cargo:rerun-if-changed=assets/bloxcode.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    // Only embed Windows resources when building on Windows (MSVC `rc.exe` or
    // `llvm-rc` is present). Cross-compiling from Linux skips the resource step
    // so `cargo check` works without a toolchain.
    let host = std::env::var("HOST").unwrap_or_default();
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target == "windows" && host.contains("windows") {
        embed_resource::compile("assets/bloxcode.rc", embed_resource::NONE);
    }
}
