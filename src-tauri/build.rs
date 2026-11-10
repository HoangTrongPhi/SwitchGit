use std::env;
use std::path::PathBuf;

fn main() {
    let windows_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let attributes = if windows_msvc {
        // tauri-build only embeds its manifest into the app binary; test binaries then fail to
        // start (STATUS_ENTRYPOINT_NOT_FOUND). Embedding it via the linker covers both.
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else {
        tauri_build::Attributes::new()
    };
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
