package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

// BuildIOS implements the 'ctd build-ios' command
func BuildIOS(args []string) error {
	fs := flag.NewFlagSet("build-ios", flag.ExitOnError)
	simulator := fs.Bool("simulator", false, "Build for iOS simulator")
	device := fs.Bool("device", false, "Build for physical iOS device")
	release := fs.Bool("release", false, "Build in release mode")
	engineDir := fs.String("engine", "engine", "Path to Rust engine directory")
	fs.Parse(args)

	// Default to simulator if neither specified
	if !*simulator && !*device {
		*simulator = true
	}

	// Check we're on macOS
	if runtime.GOOS != "darwin" {
		return fmt.Errorf("iOS builds require macOS")
	}

	// Load config
	config, err := LoadConfig()
	if err != nil {
		fmt.Printf("Warning: %v, using defaults\n", err)
		config = DefaultConfig()
	}

	// Use config engine dir if not overridden
	if *engineDir == "engine" && config.Build.EngineDir != "" {
		*engineDir = config.Build.EngineDir
	}

	// Check engine directory exists
	if _, err := os.Stat(*engineDir); os.IsNotExist(err) {
		return fmt.Errorf("engine directory not found: %s", *engineDir)
	}

	// Determine targets
	var targets []string
	if *simulator {
		// Apple Silicon simulator
		targets = append(targets, "aarch64-apple-ios-sim")
		// Optionally add x86_64 for Intel Macs
		// targets = append(targets, "x86_64-apple-ios")
	}
	if *device {
		targets = append(targets, "aarch64-apple-ios")
	}

	// Build for each target
	for _, target := range targets {
		fmt.Printf("Building for %s...\n", target)

		// Ensure target is installed
		if err := ensureRustTarget(target); err != nil {
			return fmt.Errorf("failed to add target %s: %w", target, err)
		}

		// Build command
		buildArgs := []string{"build", "--target", target}
		if *release {
			buildArgs = append(buildArgs, "--release")
		}

		cmd := exec.Command("cargo", buildArgs...)
		cmd.Dir = *engineDir
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Env = append(os.Environ(), getIOSBuildEnv(target)...)

		if err := cmd.Run(); err != nil {
			return fmt.Errorf("build failed for %s: %w", target, err)
		}

		fmt.Printf("✓ Built for %s\n", target)
	}

	// Determine output paths
	buildType := "debug"
	if *release {
		buildType = "release"
	}

	fmt.Println("")
	fmt.Println("Build outputs:")
	for _, target := range targets {
		libPath := filepath.Join(*engineDir, "target", target, buildType, "libcentered_engine.a")
		if _, err := os.Stat(libPath); err == nil {
			fmt.Printf("  %s\n", libPath)
		}
		dylibPath := filepath.Join(*engineDir, "target", target, buildType, "libcentered_engine.dylib")
		if _, err := os.Stat(dylibPath); err == nil {
			fmt.Printf("  %s\n", dylibPath)
		}
	}

	return nil
}

// RunIOS implements the 'ctd run-ios' command
func RunIOS(args []string) error {
	fs := flag.NewFlagSet("run-ios", flag.ExitOnError)
	simulatorName := fs.String("simulator", "iPhone 15 Pro", "Simulator device name")
	release := fs.Bool("release", false, "Build in release mode")
	engineDir := fs.String("engine", "engine", "Path to Rust engine directory")
	iosDir := fs.String("ios", "ios", "Path to iOS project directory")
	fs.Parse(args)

	// Check we're on macOS
	if runtime.GOOS != "darwin" {
		return fmt.Errorf("iOS simulator requires macOS")
	}

	// Load config
	config, err := LoadConfig()
	if err != nil {
		fmt.Printf("Warning: %v, using defaults\n", err)
		config = DefaultConfig()
	}

	safeName := sanitizeName(config.App.Name)

	// Build first
	fmt.Println("Building for iOS simulator...")
	buildArgs := []string{"--simulator"}
	if *release {
		buildArgs = append(buildArgs, "--release")
	}
	buildArgs = append(buildArgs, "--engine", *engineDir)
	if err := BuildIOS(buildArgs); err != nil {
		return err
	}

	// Find simulator
	fmt.Printf("Finding simulator: %s\n", *simulatorName)
	simulatorID, err := findSimulator(*simulatorName)
	if err != nil {
		return err
	}

	// Create app bundle
	fmt.Println("Creating app bundle...")
	buildType := "debug"
	if *release {
		buildType = "release"
	}

	appBundleDir := filepath.Join(*engineDir, "target", "ios-app-bundle", safeName+".app")
	if err := os.MkdirAll(appBundleDir, 0755); err != nil {
		return fmt.Errorf("failed to create app bundle: %w", err)
	}

	// Copy binary
	binaryPath := filepath.Join(*engineDir, "target", "aarch64-apple-ios-sim", buildType, "centered_engine")
	if _, err := os.Stat(binaryPath); os.IsNotExist(err) {
		// Try the library instead
		binaryPath = filepath.Join(*engineDir, "target", "aarch64-apple-ios-sim", buildType, "libcentered_engine.dylib")
	}

	// Copy Info.plist
	infoPlistSrc := filepath.Join(*iosDir, safeName, "Info.plist")
	infoPlistDst := filepath.Join(appBundleDir, "Info.plist")
	if _, err := os.Stat(infoPlistSrc); err == nil {
		if err := copyFile(infoPlistSrc, infoPlistDst); err != nil {
			return fmt.Errorf("failed to copy Info.plist: %w", err)
		}
	}

	// Code sign
	fmt.Println("Code signing...")
	if err := exec.Command("codesign", "--force", "--sign", "-", appBundleDir).Run(); err != nil {
		fmt.Printf("Warning: code signing failed: %v\n", err)
	}

	// Boot simulator
	fmt.Println("Booting simulator...")
	exec.Command("xcrun", "simctl", "boot", simulatorID).Run() // Ignore error if already booted

	// Open Simulator app if not running
	exec.Command("open", "-a", "Simulator").Run()

	// Install app
	fmt.Println("Installing app...")
	installCmd := exec.Command("xcrun", "simctl", "install", "booted", appBundleDir)
	installCmd.Stdout = os.Stdout
	installCmd.Stderr = os.Stderr
	if err := installCmd.Run(); err != nil {
		return fmt.Errorf("failed to install app: %w", err)
	}

	// Launch app
	fmt.Println("Launching app...")
	launchCmd := exec.Command("xcrun", "simctl", "launch", "--console", "booted", config.IOS.BundleIdentifier)
	launchCmd.Stdout = os.Stdout
	launchCmd.Stderr = os.Stderr
	if err := launchCmd.Run(); err != nil {
		return fmt.Errorf("failed to launch app: %w", err)
	}

	return nil
}

func ensureRustTarget(target string) error {
	// Check if target is already installed
	cmd := exec.Command("rustup", "target", "list", "--installed")
	output, err := cmd.Output()
	if err != nil {
		return err
	}

	if contains(string(output), target) {
		return nil
	}

	// Install target
	fmt.Printf("Installing Rust target: %s\n", target)
	installCmd := exec.Command("rustup", "target", "add", target)
	installCmd.Stdout = os.Stdout
	installCmd.Stderr = os.Stderr
	return installCmd.Run()
}

func getIOSBuildEnv(target string) []string {
	// iOS builds may need specific environment variables
	// These vary based on Xcode version and SDK paths
	return []string{
		// Add any needed env vars here
	}
}

func findSimulator(name string) (string, error) {
	cmd := exec.Command("xcrun", "simctl", "list", "devices", "available", "-j")
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to list simulators: %w", err)
	}

	// Simple string search for the simulator name and UUID
	// In a real implementation, parse the JSON properly
	_ = output // Will be used when JSON parsing is added

	// Use grep-like approach to find the simulator
	grepCmd := exec.Command("xcrun", "simctl", "list", "devices", "available")
	grepOutput, _ := grepCmd.Output()

	lines := string(grepOutput)
	for _, line := range splitLines(lines) {
		if contains(line, name) {
			// Extract UUID from line like "iPhone 15 Pro (UUID) (Booted)"
			start := indexOf(line, "(")
			end := indexOf(line, ")")
			if start != -1 && end != -1 && end > start {
				uuid := line[start+1 : end]
				// Check if this looks like a UUID (36 chars with hyphens)
				if len(uuid) == 36 {
					return uuid, nil
				}
			}
		}
	}

	return "", fmt.Errorf("simulator '%s' not found", name)
}

// Helper functions are in utils.go
