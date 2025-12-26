package main

import (
	"fmt"
	"os"

	"github.com/agiangrant/ctd/cmd/ctd/commands"
)

const version = "0.1.0"

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	cmd := os.Args[1]
	args := os.Args[2:]

	var err error
	switch cmd {
	case "init":
		err = commands.Init(args)
	case "dev":
		err = commands.Dev(args)
	case "generate":
		err = commands.Generate(args)
	case "build":
		// Default build for current platform
		err = commands.BuildMacOS(args) // TODO: detect platform
	case "build-macos":
		err = commands.BuildMacOS(args)
	case "build-linux":
		err = commands.BuildLinux(args)
	case "build-windows":
		err = commands.BuildWindows(args)
	case "build-ios":
		err = commands.BuildIOS(args)
	case "build-android":
		err = commands.BuildAndroid(args)
	case "create-ios":
		err = commands.CreateIOS(args)
	case "create-android":
		err = commands.CreateAndroid(args)
	case "run-ios":
		err = commands.RunIOS(args)
	case "run-android":
		err = commands.RunAndroid(args)
	case "version", "-v", "--version":
		fmt.Printf("ctd version %s\n", version)
	case "help", "-h", "--help":
		printUsage()
	default:
		fmt.Fprintf(os.Stderr, "Unknown command: %s\n\n", cmd)
		printUsage()
		os.Exit(1)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Println(`ctd - CTD Framework CLI

Usage: ctd <command> [options]

Project Setup:
  init            Initialize a new CTD project with ctd.toml and theme.toml
  generate        Generate tw/generated.go from theme.toml

Development:
  dev             Run with hot reload (watches for file changes)

Desktop Builds:
  build           Build for current platform
  build-macos     Build for macOS (--universal for universal binary)
  build-linux     Build for Linux (--arch amd64|arm64)
  build-windows   Build for Windows

Mobile Setup:
  create-ios      Create iOS Xcode project from ctd.toml config
  create-android  Create Android Studio project from ctd.toml config

Mobile Builds:
  build-ios       Build for iOS (--simulator or --device)
  build-android   Build for Android
  run-ios         Build and run on iOS simulator
  run-android     Build and run on Android emulator

Other:
  version         Print version information
  help            Show this help message

Examples:
  ctd init                        Create a new project
  ctd dev                         Start development with hot reload
  ctd generate                    Generate Tailwind styles from theme.toml
  ctd build --release             Build optimized release for current platform
  ctd build-ios --device          Build for physical iOS device
  ctd run-ios                     Build and run on iOS simulator

Configuration:
  Projects are configured via ctd.toml in the project root.
  This includes app info, permissions, and platform-specific settings.
  Run 'ctd init' to create a new project with default configuration.`)
}
