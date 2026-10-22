//! Test and example binaries link against screencapturekit's Swift bridge;
//! a dependency's link args do not reach them, so add the Swift runtime rpath here.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}
