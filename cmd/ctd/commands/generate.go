package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// Generate implements the 'ctd generate' command
// It wraps the existing tools/generate functionality
func Generate(args []string) error {
	fs := flag.NewFlagSet("generate", flag.ExitOnError)
	themeFile := fs.String("theme", "theme.toml", "Path to theme.toml file")
	outputDir := fs.String("output", "tw", "Output directory for generated.go")
	watch := fs.Bool("watch", false, "Watch for changes and regenerate")
	fs.Parse(args)

	// Check if theme.toml exists
	if _, err := os.Stat(*themeFile); os.IsNotExist(err) {
		fmt.Printf("ℹ No %s found, using defaults\n", *themeFile)
	}

	// For now, we'll run go generate or directly invoke the generator
	// The actual generation logic lives in tools/generate/main.go
	// Users should have that as part of the framework

	if *watch {
		return generateWatch(*themeFile, *outputDir)
	}

	return generateOnce(*themeFile, *outputDir)
}

func generateOnce(themeFile, outputDir string) error {
	// Try to find the generate tool in the project
	// Option 1: Check if we're in a project that has tools/generate
	if _, err := os.Stat("tools/generate/main.go"); err == nil {
		cmd := exec.Command("go", "run", "tools/generate/main.go")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		return cmd.Run()
	}

	// Option 2: Use go generate if defined in go.mod
	// For projects using centered as a dependency, they'd have their own generator
	// or use the embedded one

	// For now, provide a helpful message
	fmt.Println("To generate Tailwind styles, ensure you have:")
	fmt.Println("  1. A theme.toml file in your project root (optional)")
	fmt.Println("  2. Run: go run github.com/agiangrant/ctd/tools/generate")
	fmt.Println("")
	fmt.Printf("Output directory: %s\n", outputDir)

	// Check if the user has the ctd module
	goModPath := "go.mod"
	if _, err := os.Stat(goModPath); err == nil {
		// They have a go.mod, try running the generator from the module
		cmd := exec.Command("go", "run", "github.com/agiangrant/ctd/tools/generate")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			return fmt.Errorf("failed to run generator: %w", err)
		}
		return nil
	}

	return fmt.Errorf("no go.mod found - are you in a Go project?")
}

func generateWatch(themeFile, outputDir string) error {
	fmt.Printf("Watching %s for changes...\n", themeFile)
	fmt.Println("Press Ctrl+C to stop")

	// Get initial mod time
	info, err := os.Stat(themeFile)
	if err != nil {
		// File doesn't exist yet, that's okay
		info = nil
	}
	var lastMod int64
	if info != nil {
		lastMod = info.ModTime().UnixNano()
	}

	// Initial generation
	if err := generateOnce(themeFile, outputDir); err != nil {
		fmt.Printf("Warning: %v\n", err)
	}

	// Watch loop
	for {
		select {
		case <-make(chan struct{}): // This would be a signal handler in real implementation
			return nil
		default:
			info, err := os.Stat(themeFile)
			if err == nil {
				currentMod := info.ModTime().UnixNano()
				if currentMod != lastMod {
					lastMod = currentMod
					fmt.Printf("\n%s changed, regenerating...\n", themeFile)
					if err := generateOnce(themeFile, outputDir); err != nil {
						fmt.Printf("Error: %v\n", err)
					}
				}
			}
			// Sleep for a bit before checking again
			// In a real implementation, use fsnotify
			cmd := exec.Command("sleep", "1")
			cmd.Run()
		}
	}
}

// EnsureOutputDir creates the output directory if it doesn't exist
func EnsureOutputDir(dir string) error {
	return os.MkdirAll(dir, 0755)
}

// FindThemeFile looks for theme.toml in common locations
func FindThemeFile() string {
	locations := []string{
		"theme.toml",
		"config/theme.toml",
		".centered/theme.toml",
	}
	for _, loc := range locations {
		if _, err := os.Stat(loc); err == nil {
			return loc
		}
	}
	return "theme.toml" // default
}

// GetGeneratorPath returns the path to the generator tool
func GetGeneratorPath() (string, error) {
	// Check local tools directory first
	localPath := filepath.Join("tools", "generate", "main.go")
	if _, err := os.Stat(localPath); err == nil {
		return localPath, nil
	}

	// Check if installed globally
	// This would be the case after `go install github.com/agiangrant/ctd/tools/generate@latest`
	return "", fmt.Errorf("generator not found - run: go install github.com/agiangrant/ctd/tools/generate@latest")
}
