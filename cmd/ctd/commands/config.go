package commands

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/pelletier/go-toml/v2"
)

// ProjectConfig represents the centered.toml configuration file
type ProjectConfig struct {
	App     AppConfig     `toml:"app"`
	IOS     IOSConfig     `toml:"ios"`
	Android AndroidConfig `toml:"android"`
	Build   BuildConfig   `toml:"build"`
}

type AppConfig struct {
	Name       string `toml:"name"`
	Identifier string `toml:"identifier"`
	Version    string `toml:"version"`
}

type IOSConfig struct {
	DeploymentTarget string `toml:"deployment_target"`
	DevelopmentTeam  string `toml:"development_team"`
	BundleIdentifier string `toml:"bundle_identifier"`
}

type AndroidConfig struct {
	MinSDK      int    `toml:"min_sdk"`
	TargetSDK   int    `toml:"target_sdk"`
	PackageName string `toml:"package_name"`
}

type BuildConfig struct {
	// Rust engine configuration
	EngineDir string `toml:"engine_dir"`
	// Output directory for builds
	OutputDir string `toml:"output_dir"`
}

// DefaultConfig returns a sensible default configuration
func DefaultConfig() ProjectConfig {
	return ProjectConfig{
		App: AppConfig{
			Name:       "MyApp",
			Identifier: "com.example.myapp",
			Version:    "1.0.0",
		},
		IOS: IOSConfig{
			DeploymentTarget: "15.0",
			DevelopmentTeam:  "",
			BundleIdentifier: "com.example.myapp",
		},
		Android: AndroidConfig{
			MinSDK:      26,
			TargetSDK:   34,
			PackageName: "com.example.myapp",
		},
		Build: BuildConfig{
			EngineDir: "engine",
			OutputDir: "build",
		},
	}
}

// LoadConfig loads the project configuration from centered.toml
// If the file doesn't exist, returns default config
func LoadConfig() (ProjectConfig, error) {
	config := DefaultConfig()

	// Look for centered.toml in current directory
	configPath := "centered.toml"
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		return config, nil
	}

	data, err := os.ReadFile(configPath)
	if err != nil {
		return config, fmt.Errorf("failed to read centered.toml: %w", err)
	}

	if err := toml.Unmarshal(data, &config); err != nil {
		return config, fmt.Errorf("failed to parse centered.toml: %w", err)
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

// SaveConfig saves the configuration to centered.toml
func SaveConfig(config ProjectConfig) error {
	data, err := toml.Marshal(config)
	if err != nil {
		return fmt.Errorf("failed to marshal config: %w", err)
	}

	if err := os.WriteFile("centered.toml", data, 0644); err != nil {
		return fmt.Errorf("failed to write centered.toml: %w", err)
	}

	return nil
}

// FindProjectRoot finds the project root by looking for centered.toml or go.mod
func FindProjectRoot() (string, error) {
	dir, err := os.Getwd()
	if err != nil {
		return "", err
	}

	for {
		// Check for centered.toml
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
			return "", fmt.Errorf("not in a Centered project (no centered.toml or go.mod found)")
		}
		dir = parent
	}
}
