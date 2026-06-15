fn main() {
    // The `screencapturekit` crate links a Swift bridge that pulls in the Swift
    // runtime (`@rpath/libswiftCore.dylib`, etc). It emits the required rpath via
    // `cargo:rustc-link-arg`, but link-args from a *dependency* build script do
    // not propagate to the final binary link, so the runtime path is dropped and
    // the app fails to launch with "Library not loaded: @rpath/libswiftCore.dylib".
    // Add the rpath here, in the crate that actually produces the binary. macOS
    // 10.14.4+ ships the Swift runtime in the OS (dyld shared cache) at
    // /usr/lib/swift, which is the correct target for a distributable app.
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,/usr/lib/swift");

    tauri_build::build()
}
