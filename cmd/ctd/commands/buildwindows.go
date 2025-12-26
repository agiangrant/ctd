package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// BuildWindows implements the 'ctd build-windows' command
func BuildWindows(args []string) error {
	fs := flag.NewFlagSet("build-windows", flag.ExitOnError)
	release := fs.Bool("release", false, "Build in release mode")
	engineDir := fs.String("engine", "", "Path to Rust engine directory")
	outputDir := fs.String("output", "", "Output directory for built artifacts")
	fs.Parse(args)

	// Load config
	config, err := LoadConfig()
	if err != nil {
		fmt.Printf("Warning: %v, using defaults\n", err)
		config = DefaultConfig()
	}

	// Apply config defaults
	if *engineDir == "" {
		*engineDir = config.Build.EngineDir
	}
	if *outputDir == "" {
		*outputDir = config.Build.OutputDir
	}

	// Check engine directory exists
	if _, err := os.Stat(*engineDir); os.IsNotExist(err) {
		return fmt.Errorf("engine directory not found: %s", *engineDir)
	}

	rustTarget := "x86_64-pc-windows-msvc"
	buildType := "debug"
	if *release {
		buildType = "release"
	}

	// Build Rust engine
	fmt.Printf("Building Rust engine for %s...\n", rustTarget)

	if err := ensureRustTarget(rustTarget); err != nil {
		return fmt.Errorf("failed to add target %s: %w", rustTarget, err)
	}

	buildArgs := []string{"build", "--target", rustTarget}
	if *release {
		buildArgs = append(buildArgs, "--release")
	}

	cmd := exec.Command("cargo", buildArgs...)
	cmd.Dir = *engineDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("cargo build failed: %w", err)
	}

	dllPath := filepath.Join(*engineDir, "target", rustTarget, buildType, "centered_engine.dll")
	fmt.Printf("✓ Built %s\n", dllPath)

	// Create output directory
	if err := os.MkdirAll(*outputDir, 0755); err != nil {
		return fmt.Errorf("failed to create output directory: %w", err)
	}

	// Build Go application
	fmt.Println("")
	fmt.Println("Building Go application...")

	goOutputPath := filepath.Join(*outputDir, config.App.Name+".exe")
	goBuildArgs := []string{"build", "-o", goOutputPath}
	if *release {
		goBuildArgs = append(goBuildArgs, "-ldflags=-s -w -H=windowsgui")
	}
	goBuildArgs = append(goBuildArgs, ".")

	goCmd := exec.Command("go", goBuildArgs...)
	goCmd.Stdout = os.Stdout
	goCmd.Stderr = os.Stderr

	// Set cross-compile env
	env := os.Environ()
	env = append(env, "GOOS=windows", "GOARCH=amd64")
	goCmd.Env = env

	if err := goCmd.Run(); err != nil {
		return fmt.Errorf("go build failed: %w", err)
	}

	fmt.Printf("✓ Built %s\n", goOutputPath)

	// Copy DLL to output directory
	dllDst := filepath.Join(*outputDir, "centered_engine.dll")
	if err := copyFile(dllPath, dllDst); err != nil {
		return fmt.Errorf("failed to copy DLL: %w", err)
	}
	fmt.Printf("✓ Copied DLL to %s\n", dllDst)

	fmt.Println("")
	fmt.Println("Build complete!")
	fmt.Printf("  Executable: %s\n", goOutputPath)
	fmt.Printf("  DLL: %s\n", dllDst)
	fmt.Println("")
	fmt.Println("To run on Windows:")
	fmt.Printf("  cd %s && %s.exe\n", *outputDir, config.App.Name)

	return nil
}
