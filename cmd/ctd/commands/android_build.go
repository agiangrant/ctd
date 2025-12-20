package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/pelletier/go-toml/v2"
)

// BuildAndroid implements the 'ctd build-android' command
func BuildAndroid(args []string) error {
	fs := flag.NewFlagSet("build-android", flag.ExitOnError)
	release := fs.Bool("release", false, "Build in release mode")
	engineDir := fs.String("engine", "engine", "Path to Rust engine directory")
	androidDir := fs.String("android", "android", "Path to Android project directory")
	arm64Only := fs.Bool("arm64-only", false, "Build only for arm64-v8a (faster builds)")
	fs.Parse(args)

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
	targets := []string{"aarch64-linux-android"}
	if !*arm64Only {
		targets = append(targets, "armv7-linux-androideabi")
		targets = append(targets, "x86_64-linux-android")
	}

	// Find NDK
	ndkHome, err := findAndroidNDK()
	if err != nil {
		return err
	}
	fmt.Printf("Using NDK: %s\n", ndkHome)

	// Determine host tag
	hostTag := getHostTag()

	// Build for each target
	buildType := "debug"
	if *release {
		buildType = "release"
	}

	for _, target := range targets {
		fmt.Printf("Building for %s...\n", target)

		// Ensure target is installed
		if err := ensureRustTarget(target); err != nil {
			return fmt.Errorf("failed to add target %s: %w", target, err)
		}

		// Set up toolchain environment
		toolchainDir := filepath.Join(ndkHome, "toolchains", "llvm", "prebuilt", hostTag)
		env := getAndroidBuildEnv(target, toolchainDir)

		// Build command
		buildArgs := []string{"build", "--target", target, "--lib"}
		if *release {
			buildArgs = append(buildArgs, "--release")
		}

		cmd := exec.Command("cargo", buildArgs...)
		cmd.Dir = *engineDir
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Env = append(os.Environ(), env...)

		if err := cmd.Run(); err != nil {
			return fmt.Errorf("build failed for %s: %w", target, err)
		}

		fmt.Printf("✓ Built for %s\n", target)
	}

	// Copy .so files to Android project
	fmt.Println("")
	fmt.Println("Copying libraries to Android project...")

	targetToABI := map[string]string{
		"aarch64-linux-android":   "arm64-v8a",
		"armv7-linux-androideabi": "armeabi-v7a",
		"x86_64-linux-android":    "x86_64",
	}

	for _, target := range targets {
		abi := targetToABI[target]
		srcPath := filepath.Join(*engineDir, "target", target, buildType, "libcentered_engine.so")
		dstDir := filepath.Join(*androidDir, "app", "src", "main", "jniLibs", abi)
		dstPath := filepath.Join(dstDir, "libcentered_engine.so")

		// Check source exists
		if _, err := os.Stat(srcPath); os.IsNotExist(err) {
			fmt.Printf("  ⚠ %s not found\n", srcPath)
			continue
		}

		// Create destination directory
		if err := os.MkdirAll(dstDir, 0755); err != nil {
			return fmt.Errorf("failed to create directory %s: %w", dstDir, err)
		}

		// Copy file
		if err := copyFile(srcPath, dstPath); err != nil {
			return fmt.Errorf("failed to copy %s: %w", srcPath, err)
		}

		fmt.Printf("  ✓ Copied to %s\n", dstPath)
	}

	// Copy bundled fonts to Android assets
	if err := copyBundledFontsToAssets(*androidDir); err != nil {
		fmt.Printf("Warning: %v\n", err)
	}

	fmt.Println("")
	fmt.Println("Build complete! Next steps:")
	fmt.Println("  cd android && ./gradlew assembleDebug")
	fmt.Println("  # or: ctd run-android")

	return nil
}

// RunAndroid implements the 'ctd run-android' command
func RunAndroid(args []string) error {
	fs := flag.NewFlagSet("run-android", flag.ExitOnError)
	emulatorName := fs.String("emulator", "", "Emulator AVD name (uses running emulator if not specified)")
	release := fs.Bool("release", false, "Build in release mode")
	engineDir := fs.String("engine", "engine", "Path to Rust engine directory")
	androidDir := fs.String("android", "android", "Path to Android project directory")
	fs.Parse(args)

	// Load config
	config, err := LoadConfig()
	if err != nil {
		fmt.Printf("Warning: %v, using defaults\n", err)
		config = DefaultConfig()
	}

	// Build first (arm64 only for speed)
	fmt.Println("Building for Android...")
	buildArgs := []string{"--arm64-only", "--engine", *engineDir, "--android", *androidDir}
	if *release {
		buildArgs = append(buildArgs, "--release")
	}
	if err := BuildAndroid(buildArgs); err != nil {
		return err
	}

	// Check for running emulator
	hasEmulator, err := checkRunningEmulator()
	if err != nil {
		fmt.Printf("Warning: %v\n", err)
	}

	if !hasEmulator && *emulatorName != "" {
		// Start emulator
		fmt.Printf("Starting emulator: %s\n", *emulatorName)
		if err := startEmulator(*emulatorName); err != nil {
			return err
		}
	} else if !hasEmulator {
		return fmt.Errorf("no running emulator found - specify one with --emulator or start one manually")
	}

	// Build APK with Gradle
	fmt.Println("Building APK...")
	gradleTask := "installDebug"
	if *release {
		gradleTask = "installRelease"
	}

	gradlew := "./gradlew"
	if runtime.GOOS == "windows" {
		gradlew = "gradlew.bat"
	}

	gradleCmd := exec.Command(gradlew, gradleTask)
	gradleCmd.Dir = *androidDir
	gradleCmd.Stdout = os.Stdout
	gradleCmd.Stderr = os.Stderr
	if err := gradleCmd.Run(); err != nil {
		return fmt.Errorf("gradle build failed: %w", err)
	}

	// Launch app
	fmt.Println("Launching app...")
	launchCmd := exec.Command("adb", "shell", "am", "start", "-n",
		config.Android.PackageName+"/.MainActivity")
	launchCmd.Stdout = os.Stdout
	launchCmd.Stderr = os.Stderr
	if err := launchCmd.Run(); err != nil {
		return fmt.Errorf("failed to launch app: %w", err)
	}

	fmt.Println("")
	fmt.Println("✓ App launched!")
	fmt.Println("  View logs: adb logcat | grep -i centered")

	return nil
}

func findAndroidNDK() (string, error) {
	// Check ANDROID_NDK_HOME first
	if ndkHome := os.Getenv("ANDROID_NDK_HOME"); ndkHome != "" {
		if _, err := os.Stat(ndkHome); err == nil {
			return ndkHome, nil
		}
	}

	// Check ANDROID_HOME/ndk
	androidHome := os.Getenv("ANDROID_HOME")
	if androidHome == "" {
		// Try common locations
		homeDir, _ := os.UserHomeDir()
		possiblePaths := []string{
			filepath.Join(homeDir, "Library", "Android", "sdk"),          // macOS
			filepath.Join(homeDir, "Android", "Sdk"),                     // Linux
			filepath.Join(os.Getenv("LOCALAPPDATA"), "Android", "Sdk"),   // Windows
		}
		for _, p := range possiblePaths {
			if _, err := os.Stat(p); err == nil {
				androidHome = p
				break
			}
		}
	}

	if androidHome == "" {
		return "", fmt.Errorf("ANDROID_HOME not set and SDK not found in common locations")
	}

	// Find latest NDK version
	ndkDir := filepath.Join(androidHome, "ndk")
	entries, err := os.ReadDir(ndkDir)
	if err != nil {
		return "", fmt.Errorf("NDK not found at %s - install via Android Studio SDK Manager", ndkDir)
	}

	// Get highest version
	var latestNDK string
	for _, entry := range entries {
		if entry.IsDir() {
			latestNDK = entry.Name()
		}
	}

	if latestNDK == "" {
		return "", fmt.Errorf("no NDK versions found in %s", ndkDir)
	}

	return filepath.Join(ndkDir, latestNDK), nil
}

func getHostTag() string {
	switch runtime.GOOS {
	case "darwin":
		return "darwin-x86_64"
	case "linux":
		return "linux-x86_64"
	case "windows":
		return "windows-x86_64"
	default:
		return "linux-x86_64"
	}
}

func getAndroidBuildEnv(target, toolchainDir string) []string {
	var env []string

	// API level for clang
	apiLevel := "34"

	switch target {
	case "aarch64-linux-android":
		cc := filepath.Join(toolchainDir, "bin", "aarch64-linux-android"+apiLevel+"-clang")
		ar := filepath.Join(toolchainDir, "bin", "llvm-ar")
		env = append(env,
			"CC_aarch64_linux_android="+cc,
			"AR_aarch64_linux_android="+ar,
			"CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="+cc,
		)
	case "armv7-linux-androideabi":
		cc := filepath.Join(toolchainDir, "bin", "armv7a-linux-androideabi"+apiLevel+"-clang")
		ar := filepath.Join(toolchainDir, "bin", "llvm-ar")
		env = append(env,
			"CC_armv7_linux_androideabi="+cc,
			"AR_armv7_linux_androideabi="+ar,
			"CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="+cc,
		)
	case "x86_64-linux-android":
		cc := filepath.Join(toolchainDir, "bin", "x86_64-linux-android"+apiLevel+"-clang")
		ar := filepath.Join(toolchainDir, "bin", "llvm-ar")
		env = append(env,
			"CC_x86_64_linux_android="+cc,
			"AR_x86_64_linux_android="+ar,
			"CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="+cc,
		)
	}

	return env
}

func checkRunningEmulator() (bool, error) {
	cmd := exec.Command("adb", "devices")
	output, err := cmd.Output()
	if err != nil {
		return false, err
	}
	return contains(string(output), "emulator"), nil
}

func startEmulator(name string) error {
	androidHome := os.Getenv("ANDROID_HOME")
	if androidHome == "" {
		return fmt.Errorf("ANDROID_HOME not set")
	}

	emulatorPath := filepath.Join(androidHome, "emulator", "emulator")
	cmd := exec.Command(emulatorPath, "-avd", name)
	cmd.Stdout = nil // Run in background
	cmd.Stderr = nil

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start emulator: %w", err)
	}

	fmt.Println("Waiting for emulator to boot...")

	// Wait for device to be ready
	for i := 0; i < 60; i++ {
		checkCmd := exec.Command("adb", "shell", "getprop", "sys.boot_completed")
		output, err := checkCmd.Output()
		if err == nil && contains(string(output), "1") {
			fmt.Println("✓ Emulator ready")
			return nil
		}
		exec.Command("sleep", "2").Run()
	}

	return fmt.Errorf("emulator took too long to boot")
}

// ThemeConfig represents the theme.toml structure for font extraction
type ThemeConfig struct {
	Fonts map[string]string `toml:"fonts"`
}

// copyBundledFontsToAssets reads theme.toml and copies bundled fonts to Android assets
func copyBundledFontsToAssets(androidDir string) error {
	// Read theme.toml
	themeData, err := os.ReadFile("theme.toml")
	if err != nil {
		if os.IsNotExist(err) {
			return nil // No theme.toml, nothing to copy
		}
		return fmt.Errorf("failed to read theme.toml: %w", err)
	}

	var theme ThemeConfig
	if err := toml.Unmarshal(themeData, &theme); err != nil {
		return fmt.Errorf("failed to parse theme.toml: %w", err)
	}

	if len(theme.Fonts) == 0 {
		return nil // No fonts configured
	}

	// Find and copy bundled fonts
	var copied int
	for name, value := range theme.Fonts {
		if !isBundledFontPath(value) {
			continue // System font, skip
		}

		// Check if source file exists
		if _, err := os.Stat(value); os.IsNotExist(err) {
			fmt.Printf("  ⚠ Bundled font not found: %s (%s)\n", name, value)
			continue
		}

		// Copy to Android assets directory
		assetsDir := filepath.Join(androidDir, "app", "src", "main", "assets")
		dstPath := filepath.Join(assetsDir, value)
		dstDir := filepath.Dir(dstPath)

		if err := os.MkdirAll(dstDir, 0755); err != nil {
			return fmt.Errorf("failed to create assets directory %s: %w", dstDir, err)
		}

		if err := copyFile(value, dstPath); err != nil {
			return fmt.Errorf("failed to copy font %s: %w", value, err)
		}

		copied++
		fmt.Printf("  ✓ Copied font to %s\n", dstPath)
	}

	if copied > 0 {
		fmt.Printf("Copied %d bundled font(s) to Android assets\n", copied)
	}

	return nil
}

// isBundledFontPath checks if a font value is a file path (bundled) vs system font name
func isBundledFontPath(value string) bool {
	lower := strings.ToLower(value)
	if strings.HasSuffix(lower, ".ttf") || strings.HasSuffix(lower, ".otf") ||
		strings.HasSuffix(lower, ".woff") || strings.HasSuffix(lower, ".woff2") {
		return true
	}
	// Check for path separators (indicating a file path)
	if strings.Contains(value, "/") || strings.Contains(value, "\\") {
		return true
	}
	return false
}
