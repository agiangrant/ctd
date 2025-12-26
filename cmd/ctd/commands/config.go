package commands

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/pelletier/go-toml/v2"
)

// ProjectConfig represents the ctd.toml configuration file
type ProjectConfig struct {
	App         AppConfig         `toml:"app"`
	Permissions PermissionsConfig `toml:"permissions"`
	IOS         IOSConfig         `toml:"ios"`
	Android     AndroidConfig     `toml:"android"`
	Build       BuildConfig       `toml:"build"`
}

type AppConfig struct {
	Name        string `toml:"name"`
	Identifier  string `toml:"identifier"`
	Version     string `toml:"version"`
	Description string `toml:"description"`
	Author      string `toml:"author"`
	Website     string `toml:"website"`
}

// PermissionsConfig defines app permissions (shared across platforms)
type PermissionsConfig struct {
	// Camera access
	Camera bool `toml:"camera"`
	// Microphone access
	Microphone bool `toml:"microphone"`
	// Photo library access
	PhotoLibrary bool `toml:"photo_library"`
	// Location access
	Location bool `toml:"location"`
	// Bluetooth access
	Bluetooth bool `toml:"bluetooth"`
	// Network/Internet access
	Network bool `toml:"network"`
	// File system access
	FileAccess bool `toml:"file_access"`
	// Notifications
	Notifications bool `toml:"notifications"`
	// Background processing
	BackgroundProcessing bool `toml:"background_processing"`

	// Custom usage descriptions (for iOS)
	CameraUsage      string `toml:"camera_usage"`
	MicrophoneUsage  string `toml:"microphone_usage"`
	PhotoLibraryUsage string `toml:"photo_library_usage"`
	LocationUsage    string `toml:"location_usage"`
	BluetoothUsage   string `toml:"bluetooth_usage"`
}

type IOSConfig struct {
	DeploymentTarget string `toml:"deployment_target"`
	DevelopmentTeam  string `toml:"development_team"`
	BundleIdentifier string `toml:"bundle_identifier"`
	// App Store category
	Category string `toml:"category"`
	// Device families (iphone, ipad, or both)
	DeviceFamily string `toml:"device_family"`
	// App icon path
	IconPath string `toml:"icon_path"`
	// Launch screen storyboard or color
	LaunchScreen string `toml:"launch_screen"`
}

type AndroidConfig struct {
	MinSDK      int    `toml:"min_sdk"`
	TargetSDK   int    `toml:"target_sdk"`
	PackageName string `toml:"package_name"`
	// App icon path
	IconPath string `toml:"icon_path"`
	// Adaptive icon paths
	IconForeground string `toml:"icon_foreground"`
	IconBackground string `toml:"icon_background"`
}

type BuildConfig struct {
	// Rust engine configuration
	EngineDir string `toml:"engine_dir"`
	// Output directory for builds
	OutputDir string `toml:"output_dir"`
	// Theme file path (for Tailwind generation)
	ThemeFile string `toml:"theme_file"`
	// Entry point for the application (main.go location)
	EntryPoint string `toml:"entry_point"`
}

// DefaultConfig returns a sensible default configuration
func DefaultConfig() ProjectConfig {
	return ProjectConfig{
		App: AppConfig{
			Name:        "MyApp",
			Identifier:  "com.example.myapp",
			Version:     "1.0.0",
			Description: "",
			Author:      "",
			Website:     "",
		},
		Permissions: PermissionsConfig{
			Network: true, // Most apps need network
		},
		IOS: IOSConfig{
			DeploymentTarget: "15.0",
			DevelopmentTeam:  "",
			BundleIdentifier: "",
			Category:         "public.app-category.utilities",
			DeviceFamily:     "both",
		},
		Android: AndroidConfig{
			MinSDK:      26,
			TargetSDK:   34,
			PackageName: "",
		},
		Build: BuildConfig{
			EngineDir:  "engine",
			OutputDir:  "build",
			ThemeFile:  "theme.toml",
			EntryPoint: ".",
		},
	}
}

// LoadConfig loads the project configuration from ctd.toml
// If the file doesn't exist, returns default config
func LoadConfig() (ProjectConfig, error) {
	config := DefaultConfig()

	// Look for ctd.toml in current directory (also check legacy centered.toml)
	configPath := "ctd.toml"
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		// Check legacy name
		if _, err := os.Stat("centered.toml"); err == nil {
			configPath = "centered.toml"
		} else {
			return config, nil
		}
	}

	data, err := os.ReadFile(configPath)
	if err != nil {
		return config, fmt.Errorf("failed to read %s: %w", configPath, err)
	}

	if err := toml.Unmarshal(data, &config); err != nil {
		return config, fmt.Errorf("failed to parse %s: %w", configPath, err)
	}

	// Apply defaults for empty values
	if config.IOS.BundleIdentifier == "" {
		config.IOS.BundleIdentifier = config.App.Identifier
	}
	if config.Android.PackageName == "" {
		config.Android.PackageName = config.App.Identifier
	}

	return config, nil
}

// SaveConfig saves the configuration to ctd.toml
func SaveConfig(config ProjectConfig) error {
	data, err := toml.Marshal(config)
	if err != nil {
		return fmt.Errorf("failed to marshal config: %w", err)
	}

	if err := os.WriteFile("ctd.toml", data, 0644); err != nil {
		return fmt.Errorf("failed to write ctd.toml: %w", err)
	}

	return nil
}

// FindProjectRoot finds the project root by looking for ctd.toml or go.mod
func FindProjectRoot() (string, error) {
	dir, err := os.Getwd()
	if err != nil {
		return "", err
	}

	for {
		// Check for ctd.toml
		if _, err := os.Stat(filepath.Join(dir, "ctd.toml")); err == nil {
			return dir, nil
		}
		// Check for legacy centered.toml
		if _, err := os.Stat(filepath.Join(dir, "centered.toml")); err == nil {
			return dir, nil
		}
		// Check for go.mod as fallback
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir, nil
		}

		parent := filepath.Dir(dir)
		if parent == dir {
			// Reached filesystem root
			return "", fmt.Errorf("not in a CTD project (no ctd.toml or go.mod found)")
		}
		dir = parent
	}
}
