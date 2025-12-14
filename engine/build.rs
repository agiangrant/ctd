// Build script for centered-engine
// Handles platform-specific linking for iOS

use std::path::Path;
use std::process::Command;

fn main() {
    // Only apply iOS-specific linking when targeting iOS
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_os == "ios" {
        // Link against the Go framework (CenteredDemo.framework)
        // gomobile generates a static archive inside the .framework bundle
        // The framework path should be set via environment variable
        if let Ok(framework_path) = std::env::var("GO_FRAMEWORK_PATH") {
            // The static library is inside the framework bundle
            // Path: CenteredDemo.xcframework/ios-*/CenteredDemo.framework/CenteredDemo
            let framework_dir = format!("{}/CenteredDemo.framework", framework_path);
            let src_lib = format!("{}/CenteredDemo", framework_dir);
            let dst_lib = format!("{}/libCenteredDemo.a", framework_dir);

            // gomobile creates a fat binary (universal) archive
            // We need to extract the single-architecture slice for linking
            // The archive format for fat binaries isn't directly usable by the Rust linker
            // Use lipo -thin to get a proper ar archive (not a fat wrapper)
            if Path::new(&src_lib).exists() && !Path::new(&dst_lib).exists() {
                let arch = match target_arch.as_str() {
                    "aarch64" => "arm64",
                    "x86_64" => "x86_64",
                    _ => "arm64",
                };

                // -thin creates a proper ar archive, -extract creates a fat wrapper
                let _ = Command::new("lipo")
                    .args(["-thin", arch, &src_lib, "-output", &dst_lib])
                    .status();
            }

            println!("cargo:rustc-link-search=native={}", framework_dir);
            println!("cargo:rustc-link-lib=static=CenteredDemo");
        }

        // Link iOS system frameworks required by Go runtime
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=UIKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");

        // Link C++ runtime (required by Go on iOS)
        println!("cargo:rustc-link-lib=c++");

        // Rerun if the environment variable changes
        println!("cargo:rerun-if-env-changed=GO_FRAMEWORK_PATH");
    }
}
