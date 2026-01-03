package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

// BuildMacOS implements the 'ctd build-macos' command
func BuildMacOS(args []string) error {
	fs := flag.NewFlagSet("build-macos", flag.ExitOnError)
	release := fs.Bool("release", false, "Build in release mode")
	outputDir := fs.String("output", "build", "Output directory for built artifacts")
	universal := fs.Bool("universal", false, "Build universal binary (arm64 + x86_64)")
	fs.Parse(args)

	// Check we're on macOS
	if runtime.GOOS != "darwin" {
		return fmt.Errorf("macOS builds require macOS")
	}

	// Load config
	config, err := LoadConfig()
	if err != nil {
		fmt.Printf("Warning: %v, using defaults\n", err)
		config = DefaultConfig()
	}

	// Use config output dir if not overridden
	if *outputDir == "build" && config.Build.OutputDir != "" {
		*outputDir = config.Build.OutputDir
	}

	// Determine targets
	var targets []string
	if *universal {
		targets = []string{"aarch64-apple-darwin", "x86_64-apple-darwin"}
	} else {
		// Build for current architecture
		if runtime.GOARCH == "arm64" {
			targets = []string{"aarch64-apple-darwin"}
		} else {
			targets = []string{"x86_64-apple-darwin"}
		}
	}

	// Ensure engine is built for each target
	var builtLibs []string
	for _, target := range targets {
		libPath, err := EnsureEngineBuilt(target, *release)
		if err != nil {
			return fmt.Errorf("failed to build engine for %s: %w", target, err)
		}
		builtLibs = append(builtLibs, libPath)
	}

	// Create universal binary if requested
	if *universal && len(builtLibs) == 2 {
		fmt.Println("Creating universal binary...")

		if err := os.MkdirAll(*outputDir, 0755); err != nil {
			return fmt.Errorf("failed to create output directory: %w", err)
		}

		universalPath := filepath.Join(*outputDir, "libcentered_engine.dylib")
		lipoCmd := exec.Command("lipo", "-create", "-output", universalPath, builtLibs[0], builtLibs[1])
		lipoCmd.Stdout = os.Stdout
		lipoCmd.Stderr = os.Stderr

		if err := lipoCmd.Run(); err != nil {
			return fmt.Errorf("lipo failed: %w", err)
		}

		fmt.Printf("✓ Universal binary: %s\n", universalPath)
	}

	// Build Go application
	fmt.Println("")
	fmt.Println("Building Go application...")

	goOutputPath := filepath.Join(*outputDir, config.App.Name)
	goBuildArgs := []string{"build", "-o", goOutputPath}
	if *release {
		goBuildArgs = append(goBuildArgs, "-ldflags=-s -w") // Strip debug info
	}
	goBuildArgs = append(goBuildArgs, ".")

	goCmd := exec.Command("go", goBuildArgs...)
	goCmd.Stdout = os.Stdout
	goCmd.Stderr = os.Stderr

	if err := goCmd.Run(); err != nil {
		return fmt.Errorf("go build failed: %w", err)
	}

	fmt.Printf("✓ Built %s\n", goOutputPath)

	// Copy dylib to output directory
	if !*universal {
		libSrc := builtLibs[0]
		libDst := filepath.Join(*outputDir, "libcentered_engine.dylib")
		if err := copyFile(libSrc, libDst); err != nil {
			return fmt.Errorf("failed to copy library: %w", err)
		}
		fmt.Printf("✓ Copied library to %s\n", libDst)
	}

	fmt.Println("")
	fmt.Println("Build complete!")
	fmt.Printf("  Executable: %s\n", goOutputPath)
	fmt.Printf("  Library: %s/libcentered_engine.dylib\n", *outputDir)
	fmt.Println("")
	fmt.Println("To run:")
	fmt.Printf("  cd %s && ./%s\n", *outputDir, config.App.Name)

	return nil
}
