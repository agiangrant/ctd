//! iOS app entry point
//!
//! This binary is the iOS app executable. It initializes Go and runs the app.
//! This architecture allows winit to call UIApplicationMain before any other code runs.
//!
//! There is no app logic here - all logic lives in Go. This is just the entry point
//! required for iOS because winit must own UIApplicationMain.

#[cfg(target_os = "ios")]
fn main() {
    // Call into Go to start the app
    // Go will call back into Rust's centered_app_run() which handles winit
    unsafe {
        // This function is exported by the gomobile-generated Go framework
        Ios_demoStartDemo();
    }
}

#[cfg(target_os = "ios")]
#[link(name = "CenteredDemo", kind = "static")]
#[link(name = "Foundation", kind = "framework")]
#[link(name = "UIKit", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "Security", kind = "framework")]
#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "c++")]
extern "C" {
    /// Calls Go's StartDemo function (provided by gomobile-generated CenteredDemo framework)
    /// The function name follows gomobile's naming convention: {Package}_{Function}
    fn Ios_demoStartDemo();
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("This binary is only for iOS. Use the Go entry point for other platforms.");
    std::process::exit(1);
}
