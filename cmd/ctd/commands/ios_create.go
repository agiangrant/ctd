package commands

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"text/template"
)

// CreateIOS implements the 'ctd create-ios' command
func CreateIOS(args []string) error {
	fs := flag.NewFlagSet("create-ios", flag.ExitOnError)
	name := fs.String("name", "", "App name (defaults to project name from centered.toml)")
	outputDir := fs.String("output", "ios", "Output directory for iOS project")
	force := fs.Bool("force", false, "Overwrite existing files")
	fs.Parse(args)

	// Load project config
	config, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	// Use provided name or fall back to config
	appName := *name
	if appName == "" {
		appName = config.App.Name
	}

	// Sanitize app name for use in identifiers
	safeName := sanitizeName(appName)

	// Check if output directory exists
	if _, err := os.Stat(*outputDir); err == nil && !*force {
		return fmt.Errorf("directory %s already exists (use --force to overwrite)", *outputDir)
	}

	fmt.Printf("Creating iOS project for %s...\n", appName)

	// Template data
	data := IOSTemplateData{
		AppName:          appName,
		SafeName:         safeName,
		BundleIdentifier: config.IOS.BundleIdentifier,
		DeploymentTarget: config.IOS.DeploymentTarget,
		DevelopmentTeam:  config.IOS.DevelopmentTeam,
		Version:          config.App.Version,
	}

	// Create directory structure
	dirs := []string{
		*outputDir,
		filepath.Join(*outputDir, safeName+".xcodeproj"),
		filepath.Join(*outputDir, safeName),
	}
	for _, dir := range dirs {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
	}

	// Generate files
	files := map[string]string{
		filepath.Join(*outputDir, safeName+".xcodeproj", "project.pbxproj"): iosProjectTemplate,
		filepath.Join(*outputDir, safeName, "Info.plist"):                   iosInfoPlistTemplate,
		filepath.Join(*outputDir, safeName, "main.m"):                       iosMainMTemplate,
		filepath.Join(*outputDir, safeName, "Bridging-Header.h"):            iosBridgingHeaderTemplate,
		filepath.Join(*outputDir, "README.md"):                              iosReadmeTemplate,
	}

	for path, tmplStr := range files {
		if err := writeTemplate(path, tmplStr, data); err != nil {
			return fmt.Errorf("failed to write %s: %w", path, err)
		}
		fmt.Printf("  ✓ Created %s\n", path)
	}

	fmt.Println("")
	fmt.Printf("✓ iOS project created in %s/\n", *outputDir)
	fmt.Println("")
	fmt.Println("Next steps:")
	fmt.Println("  1. Build the Rust engine for iOS:")
	fmt.Println("     ctd build-ios --simulator")
	fmt.Println("  2. Open the Xcode project:")
	fmt.Printf("     open %s/%s.xcodeproj\n", *outputDir, safeName)
	fmt.Println("  3. Or run directly on simulator:")
	fmt.Println("     ctd run-ios")

	return nil
}

type IOSTemplateData struct {
	AppName          string
	SafeName         string
	BundleIdentifier string
	DeploymentTarget string
	DevelopmentTeam  string
	Version          string
}

func writeTemplate(path, tmplStr string, data interface{}) error {
	tmpl, err := template.New("").Parse(tmplStr)
	if err != nil {
		return err
	}

	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()

	return tmpl.Execute(f, data)
}

// iOS project templates
const iosInfoPlistTemplate = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>{{.SafeName}}</string>
	<key>CFBundleIdentifier</key>
	<string>{{.BundleIdentifier}}</string>
	<key>CFBundleName</key>
	<string>{{.AppName}}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{{.Version}}</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>LSRequiresIPhoneOS</key>
	<true/>
	<key>UIRequiredDeviceCapabilities</key>
	<array>
		<string>arm64</string>
	</array>
	<key>UISupportedInterfaceOrientations</key>
	<array>
		<string>UIInterfaceOrientationPortrait</string>
		<string>UIInterfaceOrientationLandscapeLeft</string>
		<string>UIInterfaceOrientationLandscapeRight</string>
	</array>
	<key>UIStatusBarHidden</key>
	<false/>
	<key>UIViewControllerBasedStatusBarAppearance</key>
	<false/>
	<key>UIStatusBarStyle</key>
	<string>UIStatusBarStyleDefault</string>
	<key>UILaunchStoryboardName</key>
	<string></string>
	<key>NSCameraUsageDescription</key>
	<string>This app needs camera access for video capture</string>
	<key>NSMicrophoneUsageDescription</key>
	<string>This app needs microphone access for audio recording</string>
	<key>NSPhotoLibraryUsageDescription</key>
	<string>This app needs photo library access to save media</string>
</dict>
</plist>
`

const iosProjectTemplate = `// !$*UTF8*$!
{
	archiveVersion = 1;
	classes = {
	};
	objectVersion = 56;
	objects = {
		/* This is a minimal Xcode project structure */
		/* The actual project.pbxproj is complex - this is a placeholder */
		/* Users should open Xcode and let it regenerate as needed */
	};
	rootObject = "{{.SafeName}}";
}
`

const iosMainMTemplate = `// main.m - Entry point for iOS app
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

// TODO: Replace with your actual Go framework import
// After running: gomobile bind -target ios -o YourApp.xcframework ./path/to/your/go/package
// Import it here:
// #import <YourApp/YourApp.h>

// Placeholder - replace YourPackage with your actual Go package name
// (gomobile exports functions with the package name prefix)
extern void YourPackageIOSMain(void);

int main(int argc, char * _Nullable argv[]) {
    @autoreleasepool {
        // Call Go's entry point which handles the full lifecycle
        // This function never returns (UIApplicationMain runs forever)
        //
        // TODO: Replace with your actual Go function:
        // YourPackageIOSMain();
        //
        // Your Go code should have an IOSMain() function that calls ffi.Run():
        //
        //   func IOSMain() {
        //       config := ffi.DefaultAppConfig()
        //       ffi.Run(config, func(event ffi.Event) ffi.FrameResponse {
        //           // Handle events and render
        //           return ffi.FrameResponse{}
        //       })
        //   }

        NSLog(@"ERROR: main.m needs to be updated to call your Go IOSMain function");
        NSLog(@"See comments in main.m for instructions");
        return 1;
    }
}
`

const iosBridgingHeaderTemplate = `// {{.SafeName}}-Bridging-Header.h
// This bridging header allows Swift code to access Rust FFI functions

#ifndef {{.SafeName}}_Bridging_Header_h
#define {{.SafeName}}_Bridging_Header_h

// Rust engine entry point
extern int centered_ios_main(int argc, char * _Nullable argv[]);

#endif /* {{.SafeName}}_Bridging_Header_h */
`

const iosReadmeTemplate = `# {{.AppName}} - iOS

This iOS project was generated by ` + "`ctd create-ios`" + `.

## Architecture

This app uses a Rust-owned iOS lifecycle:
1. ` + "`main.m`" + ` calls ` + "`centered_ios_main()`" + ` which starts UIApplicationMain with Rust's app delegate
2. Rust creates the UIWindow and UIView with a CAMetalLayer for GPU rendering
3. When the app is ready, Rust calls Go's ready callback
4. Go registers its event handler for rendering frames

This architecture provides direct control over rotation, touch events, and safe areas.

## Building

### Prerequisites

1. Xcode 15+ installed
2. Rust toolchain with iOS targets:
   ` + "```bash" + `
   rustup target add aarch64-apple-ios        # Physical device
   rustup target add aarch64-apple-ios-sim    # Simulator (Apple Silicon)
   rustup target add x86_64-apple-ios         # Simulator (Intel)
   ` + "```" + `

### Build for Simulator

` + "```bash" + `
ctd build-ios --simulator
` + "```" + `

### Build for Device

` + "```bash" + `
ctd build-ios --device
` + "```" + `

### Run on Simulator

` + "```bash" + `
ctd run-ios
` + "```" + `

## Project Structure

` + "```" + `
ios/
├── {{.SafeName}}.xcodeproj/   # Xcode project
├── {{.SafeName}}/
│   ├── main.m                 # Entry point (calls Rust)
│   ├── Bridging-Header.h      # Swift/ObjC bridge
│   └── Info.plist             # App configuration
└── README.md                  # This file
` + "```" + `

## Configuration

Edit ` + "`centered.toml`" + ` in your project root to configure:

` + "```toml" + `
[ios]
deployment_target = "{{.DeploymentTarget}}"
development_team = "{{.DevelopmentTeam}}"
bundle_identifier = "{{.BundleIdentifier}}"
` + "```" + `
`
