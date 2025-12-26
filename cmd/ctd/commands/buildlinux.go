package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

// BuildLinux implements the 'ctd build-linux' command
func BuildLinux(args []string) error {
	fs := flag.NewFlagSet("build-linux", flag.ExitOnError)
	release := fs.Bool("release", false, "Build in release mode")
	engineDir := fs.String("engine", "", "Path to Rust engine directory")
	outputDir := fs.String("output", "", "Output directory for built artifacts")
	targetArch := fs.String("arch", "", "Target architecture (amd64, arm64)")
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
	if *targetArch == "" {
		if runtime.GOARCH == "arm64" {
			*targetArch = "arm64"
		} else {
			*targetArch = "amd64"
		}
	}

	// Check engine directory exists
	if _, err := os.Stat(*engineDir); os.IsNotExist(err) {
		return fmt.Errorf("engine directory not found: %s", *engineDir)
	}

	// Determine Rust target
	var rustTarget string
	switch *targetArch {
	case "amd64", "x86_64":
		rustTarget = "x86_64-unknown-linux-gnu"
	case "arm64", "aarch64":
		rustTarget = "aarch64-unknown-linux-gnu"
	default:
		return fmt.Errorf("unsupported architecture: %s", *targetArch)
	}

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

	libPath := filepath.Join(*engineDir, "target", rustTarget, buildType, "libcentered_engine.so")
	fmt.Printf("✓ Built %s\n", libPath)

	// Create output directory
	if err := os.MkdirAll(*outputDir, 0755); err != nil {
		return fmt.Errorf("failed to create output directory: %w", err)
	}

	// Build Go application
	fmt.Println("")
	fmt.Println("Building Go application...")

	goOutputPath := filepath.Join(*outputDir, config.App.Name)
	goBuildArgs := []string{"build", "-o", goOutputPath}
	if *release {
		goBuildArgs = append(goBuildArgs, "-ldflags=-s -w")
	}
	goBuildArgs = append(goBuildArgs, ".")

	goCmd := exec.Command("go", goBuildArgs...)
	goCmd.Stdout = os.Stdout
	goCmd.Stderr = os.Stderr

	// Set cross-compile env if needed
	env := os.Environ()
	env = append(env, "GOOS=linux")
	if *targetArch == "arm64" || *targetArch == "aarch64" {
		env = append(env, "GOARCH=arm64")
	} else {
		env = append(env, "GOARCH=amd64")
	}
	goCmd.Env = env

	if err := goCmd.Run(); err != nil {
		return fmt.Errorf("go build failed: %w", err)
	}

	fmt.Printf("✓ Built %s\n", goOutputPath)

	// Copy library to output directory
	libDst := filepath.Join(*outputDir, "libcentered_engine.so")
	if err := copyFile(libPath, libDst); err != nil {
		return fmt.Errorf("failed to copy library: %w", err)
	}
	fmt.Printf("✓ Copied library to %s\n", libDst)

	fmt.Println("")
	fmt.Println("Build complete!")
	fmt.Printf("  Executable: %s\n", goOutputPath)
	fmt.Printf("  Library: %s\n", libDst)
	fmt.Println("")
	fmt.Println("To run:")
	fmt.Printf("  cd %s && LD_LIBRARY_PATH=. ./%s\n", *outputDir, config.App.Name)

	return nil
}
