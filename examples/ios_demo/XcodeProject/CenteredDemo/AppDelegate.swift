import UIKit
import CenteredDemo

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        print("AppDelegate: didFinishLaunching called")
        NSLog("AppDelegate: About to call Go StartDemo()")

        // Call Go to start the Centered app
        // Winit will create its own window and manage the event loop
        DispatchQueue.main.async {
            NSLog("AppDelegate: Calling Ios_demoStartDemo() on main queue")
            Ios_demoStartDemo()
            NSLog("AppDelegate: Ios_demoStartDemo() returned")
        }
        return true
    }
}
