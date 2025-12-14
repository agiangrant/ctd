// AppDelegate.swift - DEPRECATED
// This file is no longer used. The app lifecycle is now owned by Rust via main.m.
// Rust's CenteredAppDelegate handles UIApplicationDelegate callbacks.
// Kept for reference only.

import UIKit

// NOTE: @main removed - entry point is now main.m which calls centered_ios_main()
// The Rust engine creates its own UIApplication and UIWindow with CenteredAppDelegate.

/*
// Old implementation - no longer used:
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // This is now handled by Rust's CenteredAppDelegate
        return true
    }
}
*/
