package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"syscall"
	"time"
)

// Dev implements the 'ctd dev' command for development with hot reload
func Dev(args []string) error {
	fs := flag.NewFlagSet("dev", flag.ExitOnError)
	engineDir := fs.String("engine", "", "Path to Rust engine directory (default: from ctd.toml)")
	themeFile := fs.String("theme", "", "Path to theme.toml (default: from ctd.toml)")
	entryPoint := fs.String("entry", "", "Go entry point (default: from ctd.toml)")
	noRust := fs.Bool("no-rust", false, "Skip Rust engine rebuild (use existing)")
	verbose := fs.Bool("verbose", false, "Show verbose output")
	fs.Parse(args)

	// Load config
	config, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	// Apply config defaults
	if *engineDir == "" {
		*engineDir = config.Build.EngineDir
	}
	if *themeFile == "" {
		*themeFile = config.Build.ThemeFile
		if *themeFile == "" {
			*themeFile = "theme.toml"
		}
	}
	if *entryPoint == "" {
		*entryPoint = config.Build.EntryPoint
		if *entryPoint == "" {
			*entryPoint = "."
		}
	}

	fmt.Println("🚀 Starting CTD development server...")
	fmt.Printf("   App: %s\n", config.App.Name)
	fmt.Printf("   Engine: %s\n", *engineDir)
	fmt.Printf("   Theme: %s\n", *themeFile)
	fmt.Printf("   Entry: %s\n", *entryPoint)
	fmt.Println()

	// Build Rust engine first (unless skipped)
	if !*noRust {
		if _, err := os.Stat(*engineDir); err == nil {
			fmt.Println("📦 Building Rust engine...")
			if err := buildRustEngine(*engineDir, *verbose); err != nil {
				return fmt.Errorf("rust build failed: %w", err)
			}
			fmt.Println("   ✓ Rust engine built")
		}
	}

	// Generate Tailwind styles
	fmt.Println("🎨 Generating Tailwind styles...")
	if err := generateOnce(*themeFile, "tw"); err != nil {
		fmt.Printf("   ⚠ Theme generation warning: %v\n", err)
	} else {
		fmt.Println("   ✓ Styles generated")
	}

	// Create dev runner
	runner := &devRunner{
		config:     config,
		engineDir:  *engineDir,
		themeFile:  *themeFile,
		entryPoint: *entryPoint,
		verbose:    *verbose,
	}

	// Handle signals for graceful shutdown
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		fmt.Println("\n🛑 Shutting down...")
		runner.stop()
		os.Exit(0)
	}()

	// Start the app and watch for changes
	return runner.run()
}

type devRunner struct {
	config     ProjectConfig
	engineDir  string
	themeFile  string
	entryPoint string
	verbose    bool

	mu         sync.Mutex
	cmd        *exec.Cmd
	restarting bool
}

func (r *devRunner) run() error {
	// Start the app
	if err := r.start(); err != nil {
		return err
	}

	// Watch for file changes
	fmt.Println()
	fmt.Println("👀 Watching for changes...")
	fmt.Println("   Press Ctrl+C to stop")
	fmt.Println()

	return r.watch()
}

func (r *devRunner) start() error {
	r.mu.Lock()
	defer r.mu.Unlock()

	// Build the Go app
	fmt.Println("🔨 Building Go application...")
	buildCmd := exec.Command("go", "build", "-o", ".ctd-dev-bin", r.entryPoint)
	buildCmd.Stdout = os.Stdout
	buildCmd.Stderr = os.Stderr
	if err := buildCmd.Run(); err != nil {
		return fmt.Errorf("go build failed: %w", err)
	}

	// Run the app
	fmt.Println("▶️  Running application...")
	fmt.Println()

	r.cmd = exec.Command("./.ctd-dev-bin")
	r.cmd.Stdout = os.Stdout
	r.cmd.Stderr = os.Stderr

	// Set library path for the Rust dylib
	env := os.Environ()
	libPath := filepath.Join(r.engineDir, "target", "debug")
	switch runtime.GOOS {
	case "darwin":
		env = append(env, fmt.Sprintf("DYLD_LIBRARY_PATH=%s", libPath))
	case "linux":
		env = append(env, fmt.Sprintf("LD_LIBRARY_PATH=%s", libPath))
	}
	r.cmd.Env = env

	if err := r.cmd.Start(); err != nil {
		return fmt.Errorf("failed to start app: %w", err)
	}

	// Monitor the process in background
	go func() {
		r.cmd.Wait()
		r.mu.Lock()
		if !r.restarting {
			fmt.Println("\n⚠️  Application exited")
		}
		r.mu.Unlock()
	}()

	return nil
}

func (r *devRunner) stop() {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.cmd != nil && r.cmd.Process != nil {
		r.cmd.Process.Signal(syscall.SIGTERM)
		// Give it a moment to clean up
		time.Sleep(100 * time.Millisecond)
		r.cmd.Process.Kill()
	}

	// Clean up the binary
	os.Remove(".ctd-dev-bin")
}

func (r *devRunner) restart() {
	r.mu.Lock()
	r.restarting = true
	r.mu.Unlock()

	// Stop current instance
	if r.cmd != nil && r.cmd.Process != nil {
		r.cmd.Process.Signal(syscall.SIGTERM)
		time.Sleep(100 * time.Millisecond)
		r.cmd.Process.Kill()
		r.cmd.Wait()
	}

	r.mu.Lock()
	r.restarting = false
	r.mu.Unlock()

	// Start new instance
	if err := r.start(); err != nil {
		fmt.Printf("❌ Restart failed: %v\n", err)
	}
}

func (r *devRunner) watch() error {
	// Track file modification times
	lastMod := make(map[string]time.Time)
	debounce := time.NewTimer(0)
	<-debounce.C // Drain initial timer

	pendingRestart := false

	for {
		// Check for Go file changes
		changed := false
		goChanged := false
		themeChanged := false

		// Check Go files
		err := filepath.Walk(".", func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return nil // Skip errors
			}

			// Skip hidden dirs and vendor
			if info.IsDir() {
				name := info.Name()
				if strings.HasPrefix(name, ".") || name == "vendor" || name == "node_modules" {
					return filepath.SkipDir
				}
				return nil
			}

			// Watch .go files
			if strings.HasSuffix(path, ".go") {
				if prev, ok := lastMod[path]; !ok || info.ModTime().After(prev) {
					lastMod[path] = info.ModTime()
					if ok { // Only trigger on actual changes, not initial scan
						goChanged = true
					}
				}
			}

			return nil
		})
		if err != nil {
			return err
		}

		// Check theme.toml
		if info, err := os.Stat(r.themeFile); err == nil {
			if prev, ok := lastMod[r.themeFile]; !ok || info.ModTime().After(prev) {
				lastMod[r.themeFile] = info.ModTime()
				if ok {
					themeChanged = true
				}
			}
		}

		changed = goChanged || themeChanged

		if changed {
			if !pendingRestart {
				pendingRestart = true
				debounce.Reset(300 * time.Millisecond)
			}
		}

		select {
		case <-debounce.C:
			if pendingRestart {
				if themeChanged {
					fmt.Println()
					fmt.Println("🎨 Theme changed, regenerating styles...")
					if err := generateOnce(r.themeFile, "tw"); err != nil {
						fmt.Printf("   ⚠ %v\n", err)
					}
				}

				fmt.Println()
				fmt.Println("🔄 Files changed, restarting...")
				r.restart()
				pendingRestart = false
			}
		default:
		}

		time.Sleep(500 * time.Millisecond)
	}
}

func buildRustEngine(engineDir string, verbose bool) error {
	// Determine target based on platform
	var target string
	switch runtime.GOOS {
	case "darwin":
		if runtime.GOARCH == "arm64" {
			target = "aarch64-apple-darwin"
		} else {
			target = "x86_64-apple-darwin"
		}
	case "linux":
		if runtime.GOARCH == "arm64" {
			target = "aarch64-unknown-linux-gnu"
		} else {
			target = "x86_64-unknown-linux-gnu"
		}
	case "windows":
		target = "x86_64-pc-windows-msvc"
	default:
		return fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}

	args := []string{"build", "--target", target}
	cmd := exec.Command("cargo", args...)
	cmd.Dir = engineDir

	if verbose {
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	} else {
		// Capture output but don't show unless error
		cmd.Stdout = nil
		cmd.Stderr = nil
	}

	return cmd.Run()
}
