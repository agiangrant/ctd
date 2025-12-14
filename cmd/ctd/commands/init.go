package commands

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
)

// Init implements the 'ctd init' command
func Init(args []string) error {
	fs := flag.NewFlagSet("init", flag.ExitOnError)
	name := fs.String("name", "", "Project name")
	identifier := fs.String("id", "", "App identifier (e.g., com.example.myapp)")
	force := fs.Bool("force", false, "Overwrite existing files")
	fs.Parse(args)

	// Get project name from args or current directory
	projectName := *name
	if projectName == "" {
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}
		projectName = filepath.Base(cwd)
	}

	// Generate identifier if not provided
	appIdentifier := *identifier
	if appIdentifier == "" {
		appIdentifier = "com.example." + sanitizeName(projectName)
	}

	// Check if centered.toml already exists
	if _, err := os.Stat("centered.toml"); err == nil && !*force {
		return fmt.Errorf("centered.toml already exists (use --force to overwrite)")
	}

	fmt.Printf("Initializing Centered project: %s\n", projectName)

	// Create centered.toml
	config := ProjectConfig{
		App: AppConfig{
			Name:       projectName,
			Identifier: appIdentifier,
			Version:    "1.0.0",
		},
		IOS: IOSConfig{
			DeploymentTarget: "15.0",
			DevelopmentTeam:  "",
			BundleIdentifier: appIdentifier,
		},
		Android: AndroidConfig{
			MinSDK:      26,
			TargetSDK:   34,
			PackageName: appIdentifier,
		},
		Build: BuildConfig{
			EngineDir: "engine",
			OutputDir: "build",
		},
	}

	if err := SaveConfig(config); err != nil {
		return err
	}
	fmt.Println("  ✓ Created centered.toml")

	// Create theme.toml if it doesn't exist
	if _, err := os.Stat("theme.toml"); os.IsNotExist(err) {
		if err := os.WriteFile("theme.toml", []byte(defaultThemeToml), 0644); err != nil {
			return fmt.Errorf("failed to create theme.toml: %w", err)
		}
		fmt.Println("  ✓ Created theme.toml")
	}

	// Create tw directory
	if err := os.MkdirAll("tw", 0755); err != nil {
		return fmt.Errorf("failed to create tw directory: %w", err)
	}

	// Create basic main.go if it doesn't exist
	if _, err := os.Stat("main.go"); os.IsNotExist(err) {
		mainGo := fmt.Sprintf(mainGoTemplate, projectName, appIdentifier)
		if err := os.WriteFile("main.go", []byte(mainGo), 0644); err != nil {
			return fmt.Errorf("failed to create main.go: %w", err)
		}
		fmt.Println("  ✓ Created main.go")
	}

	// Check for go.mod
	if _, err := os.Stat("go.mod"); os.IsNotExist(err) {
		fmt.Println("")
		fmt.Println("No go.mod found. Create one with:")
		fmt.Printf("  go mod init %s\n", appIdentifier)
		fmt.Println("  go get github.com/anthropics/centered")
	}

	fmt.Println("")
	fmt.Println("✓ Project initialized!")
	fmt.Println("")
	fmt.Println("Next steps:")
	fmt.Println("  1. Generate Tailwind styles:")
	fmt.Println("     ctd generate")
	fmt.Println("")
	fmt.Println("  2. For mobile development:")
	fmt.Println("     ctd create-ios      # Create iOS project")
	fmt.Println("     ctd create-android  # Create Android project")
	fmt.Println("")
	fmt.Println("  3. Build and run:")
	fmt.Println("     ctd build-macos     # Build for macOS")
	fmt.Println("     ctd run-ios         # Run on iOS simulator")
	fmt.Println("     ctd run-android     # Run on Android emulator")

	return nil
}

const defaultThemeToml = `# Centered Theme Configuration
# Customize colors, spacing, fonts, and breakpoints

# Custom font families
# Use system font names or paths to bundled fonts
[fonts]
# sans = "system"
# serif = "Times New Roman"
# mono = "Menlo"
# custom = "./fonts/MyFont.ttf"

# Theme customization
[theme]

# Responsive breakpoints (in pixels)
[theme.breakpoints]
# sm = 640
# md = 768
# lg = 1024
# xl = 1280
# 2xl = 1536

# Custom spacing scale
[theme.spacing]
# 0 = "0"
# 1 = "0.25rem"
# 2 = "0.5rem"
# custom = "100px"

# Custom colors
[theme.colors]
# primary = "#3B82F6"
# secondary = "#10B981"
# accent = "#F59E0B"

# Custom utility classes
[utilities]
# btn-primary = ["bg-blue-500", "text-white", "px-4", "py-2", "rounded-lg"]
# card = ["bg-white", "rounded-xl", "shadow-lg", "p-6"]
`

const mainGoTemplate = `package main

import (
	"github.com/anthropics/centered/retained"
)

func main() {
	retained.Run(retained.AppConfig{
		Title:  "%s",
		Width:  800,
		Height: 600,
	}, buildUI)
}

func buildUI() *retained.Widget {
	return retained.VStack(
		retained.Text("Welcome to %s").
			Class("text-2xl font-bold text-white"),

		retained.Spacer(),

		retained.Text("Built with Centered").
			Class("text-gray-400"),
	).Class("flex-1 items-center justify-center bg-gray-900")
}
`
