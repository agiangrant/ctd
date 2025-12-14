// main.m - Entry point for iOS app
// This file delegates to Go which then calls into Rust to start the app lifecycle.
//
// Flow:
// 1. main.m calls Go's IOSMain()
// 2. Go's IOSMain() calls ffi.Run() which:
//    a. Registers Go's ready callback with Rust
//    b. Calls centered_ios_main() to start UIApplicationMain
// 3. UIApplicationMain runs with Rust's CenteredAppDelegate
// 4. When didFinishLaunching fires, Rust calls Go's ready callback
// 5. Go registers its event handler for rendering

#import <UIKit/UIKit.h>
#import <CenteredDemo/CenteredDemo.h>

int main(int argc, char * _Nullable argv[]) {
    @autoreleasepool {
        // Call Go's entry point which handles the full lifecycle
        // This function never returns (UIApplicationMain runs forever)
        Ios_demoIOSMain();
        return 0; // Never reached
    }
}
