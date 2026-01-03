package commands

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

const (
	ctdCacheDir    = ".ctd"
	ctdSourceDir   = "src"
	ctdLibDir      = "lib"
	ctdRepoURL     = "https://github.com/agiangrant/ctd.git"
	engineCrateName = "centered_engine"
)

// GetCurrentTarget returns the Rust target triple for the current platform
func GetCurrentTarget() string {
	switch runtime.GOOS {
	case "darwin":
		if runtime.GOARCH == "arm64" {
			return "aarch64-apple-darwin"
		}
		return "x86_64-apple-darwin"
	case "linux":
		if runtime.GOARCH == "arm64" {
			return "aarch64-unknown-linux-gnu"
		}
		return "x86_64-unknown-linux-gnu"
	case "windows":
		if runtime.GOARCH == "arm64" {
			return "aarch64-pc-windows-msvc"
		}
		return "x86_64-pc-windows-msvc"
	default:
		return ""
	}
}

// GetLibraryName returns the library filename for a given target
func GetLibraryName(target string) string {
	switch {
	case strings.Contains(target, "darwin"):
		return "libcentered_engine.dylib"
	case strings.Contains(target, "windows"):
		return "centered_engine.dll"
	case strings.Contains(target, "ios"):
		return "libcentered_engine.a"
	case strings.Contains(target, "android"):
		return "libcentered_engine.so"
	case strings.Contains(target, "linux"):
		return "libcentered_engine.so"
	default:
		return "libcentered_engine.so"
	}
}

// GetEngineSourcePath returns the path to the cached engine source
func GetEngineSourcePath() string {
	return filepath.Join(ctdCacheDir, ctdSourceDir, "engine")
}

// GetEngineLibraryPath returns the path to the cached library for a target
func GetEngineLibraryPath(target string) string {
	return filepath.Join(ctdCacheDir, ctdLibDir, target, GetLibraryName(target))
}

// EngineSourceExists checks if the engine source has been fetched
func EngineSourceExists() bool {
	cargoPath := filepath.Join(GetEngineSourcePath(), "Cargo.toml")
	_, err := os.Stat(cargoPath)
	return err == nil
}

// EngineLibraryExists checks if the library has been built for a target
func EngineLibraryExists(target string) bool {
	_, err := os.Stat(GetEngineLibraryPath(target))
	return err == nil
}

// FetchEngineSource fetches the engine source from the CTD repository
func FetchEngineSource(force bool) error {
	srcDir := filepath.Join(ctdCacheDir, ctdSourceDir)
	engineDir := GetEngineSourcePath()

	// Check if already exists
	if !force && EngineSourceExists() {
		return nil
	}

	// Clean up existing source if force
	if force {
		os.RemoveAll(srcDir)
	}

	fmt.Println("Fetching CTD engine source...")

	// Create .ctd directory
	if err := os.MkdirAll(ctdCacheDir, 0755); err != nil {
		return fmt.Errorf("failed to create cache directory: %w", err)
	}

	// Clone with sparse checkout (only engine/ directory)
	// Using filter and sparse for minimal download
	cloneCmd := exec.Command("git", "clone",
		"--filter=blob:none",
		"--no-checkout",
		"--depth=1",
		ctdRepoURL,
		srcDir)
	cloneCmd.Stdout = os.Stdout
	cloneCmd.Stderr = os.Stderr

	if err := cloneCmd.Run(); err != nil {
		os.RemoveAll(srcDir)
		return fmt.Errorf("failed to clone repository: %w", err)
	}

	// Set up sparse checkout
	sparseCmd := exec.Command("git", "sparse-checkout", "init", "--cone")
	sparseCmd.Dir = srcDir
	if err := sparseCmd.Run(); err != nil {
		os.RemoveAll(srcDir)
		return fmt.Errorf("failed to init sparse checkout: %w", err)
	}

	// Configure sparse checkout to only include engine/
	sparseSetCmd := exec.Command("git", "sparse-checkout", "set", "engine")
	sparseSetCmd.Dir = srcDir
	if err := sparseSetCmd.Run(); err != nil {
		os.RemoveAll(srcDir)
		return fmt.Errorf("failed to set sparse checkout: %w", err)
	}

	// Checkout the files
	checkoutCmd := exec.Command("git", "checkout")
	checkoutCmd.Dir = srcDir
	checkoutCmd.Stdout = os.Stdout
	checkoutCmd.Stderr = os.Stderr
	if err := checkoutCmd.Run(); err != nil {
		os.RemoveAll(srcDir)
		return fmt.Errorf("failed to checkout: %w", err)
	}

	// Verify engine directory exists
	if _, err := os.Stat(engineDir); os.IsNotExist(err) {
		os.RemoveAll(srcDir)
		return fmt.Errorf("engine directory not found after checkout")
	}

	fmt.Printf("✓ Engine source cached in %s\n", engineDir)
	return nil
}

// EnsureEngineSource ensures the engine source is available
func EnsureEngineSource() error {
	if EngineSourceExists() {
		return nil
	}
	return FetchEngineSource(false)
}

// BuildEngine builds the engine for a specific target
func BuildEngine(target string, release bool) error {
	engineDir := GetEngineSourcePath()

	// Check if source exists
	if !EngineSourceExists() {
		return fmt.Errorf("engine source not found - run 'ctd init' first or check .ctd/src/")
	}

	buildType := "debug"
	if release {
		buildType = "release"
	}

	fmt.Printf("Building engine for %s (%s)...\n", target, buildType)

	// Build with cargo
	buildArgs := []string{"build", "--target", target}
	if release {
		buildArgs = append(buildArgs, "--release")
	}

	cmd := exec.Command("cargo", buildArgs...)
	cmd.Dir = engineDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("cargo build failed: %w", err)
	}

	// Copy built library to cache
	libName := GetLibraryName(target)
	srcLib := filepath.Join(engineDir, "target", target, buildType, libName)
	dstDir := filepath.Join(ctdCacheDir, ctdLibDir, target)
	dstLib := filepath.Join(dstDir, libName)

	if err := os.MkdirAll(dstDir, 0755); err != nil {
		return fmt.Errorf("failed to create library cache directory: %w", err)
	}

	if err := copyFile(srcLib, dstLib); err != nil {
		return fmt.Errorf("failed to cache library: %w", err)
	}

	fmt.Printf("✓ Cached library: %s\n", dstLib)
	return nil
}

// EnsureEngineBuilt ensures the engine is built for a target, building if necessary
// Returns the path to the library
func EnsureEngineBuilt(target string, release bool) (string, error) {
	libPath := GetEngineLibraryPath(target)

	// Check if library already exists
	if EngineLibraryExists(target) {
		return libPath, nil
	}

	// Ensure source is available
	if err := EnsureEngineSource(); err != nil {
		return "", fmt.Errorf("failed to fetch engine source: %w", err)
	}

	// Build the engine
	if err := BuildEngine(target, release); err != nil {
		return "", err
	}

	return libPath, nil
}

// UpdateEngineSource pulls the latest engine source
func UpdateEngineSource() error {
	srcDir := filepath.Join(ctdCacheDir, ctdSourceDir)

	if !EngineSourceExists() {
		return FetchEngineSource(false)
	}

	fmt.Println("Updating engine source...")

	// Pull latest
	pullCmd := exec.Command("git", "pull", "--ff-only")
	pullCmd.Dir = srcDir
	pullCmd.Stdout = os.Stdout
	pullCmd.Stderr = os.Stderr

	if err := pullCmd.Run(); err != nil {
		return fmt.Errorf("failed to update: %w", err)
	}

	fmt.Println("✓ Engine source updated")
	fmt.Println("Run 'ctd build' to rebuild with the new engine")
	return nil
}

// Clean removes the .ctd cache directory
func Clean(args []string) error {
	if _, err := os.Stat(ctdCacheDir); os.IsNotExist(err) {
		fmt.Println("Nothing to clean")
		return nil
	}

	fmt.Printf("Removing %s/...\n", ctdCacheDir)
	if err := os.RemoveAll(ctdCacheDir); err != nil {
		return fmt.Errorf("failed to remove cache: %w", err)
	}

	fmt.Println("✓ Cache cleared")
	return nil
}
