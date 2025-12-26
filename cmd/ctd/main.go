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
	case "generate":
		err = commands.Generate(args)
	case "create-ios":
		err = commands.CreateIOS(args)
	case "create-android":
		err = commands.CreateAndroid(args)
	case "build-ios":
		err = commands.BuildIOS(args)
	case "build-android":
		err = commands.BuildAndroid(args)
	case "build-macos":
		err = commands.BuildMacOS(args)
	case "run-ios":
		err = commands.RunIOS(args)
	case "run-android":
		err = commands.RunAndroid(args)
	case "init":
		err = commands.Init(args)
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
	fmt.Println(`ctd - Centered Framework CLI

Usage: ctd <command> [options]

Commands:
  generate        Generate tw/generated.go from theme.toml
  create-ios      Create iOS project scaffolding
  create-android  Create Android project scaffolding
  build-ios       Build for iOS (simulator or device)
  build-android   Build for Android
  build-macos     Build for macOS
  run-ios         Build and run on iOS simulator
  run-android     Build and run on Android emulator
  init            Initialize a new Centered project
  version         Print version information
  help            Show this help message

Examples:
  ctd generate                    Generate Tailwind styles from theme.toml
  ctd create-ios --name MyApp     Create iOS project for MyApp
  ctd build-ios --simulator       Build for iOS simulator
  ctd build-ios --device          Build for physical iOS device
  ctd run-ios                     Build and run on iOS simulator

Configuration:
  Projects can be configured via centered.toml in the project root.
  Run 'ctd init' to create a new project with default configuration.`)
}
