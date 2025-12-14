// Frameless Window Example
//
// This example demonstrates frameless window capabilities:
// - No window decorations (standard title bar removed)
// - Native window controls (traffic lights on macOS) with custom positioning
// - Rounded window corners via platform-native APIs
// - Title bar search input pattern (like Spotlight, Raycast, etc.)

package main

import (
	"log"
	"runtime"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

var searchInput *retained.Widget

func main() {
	log.Println("Starting Frameless Window Demo...")
	log.Println("  - Native window controls (traffic lights) in top-left")
	log.Println("  - Rounded window corners via Core Animation")
	log.Println("  - Type in the search box")
	log.Println("  - Press ESC to close")

	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)
	tree := loop.Tree()

	// Build the UI
	root := buildUI()
	tree.SetRoot(root)

	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		return false
	})

	// Create frameless window config with native controls and rounded corners
	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Frameless Demo"
	appConfig.Width = 600
	appConfig.Height = 400
	appConfig.Decorations = false // Frameless window (no standard title bar)
	appConfig.Resizable = true    // Allow resizing
	appConfig.AlwaysOnTop = false // Normal window stacking

	// Size constraints
	appConfig.MinWidth = 400
	appConfig.MinHeight = 200

	// Frameless window styling options
	appConfig.CornerRadius = 12.0       // Rounded corners (points)
	appConfig.ShowNativeControls = true // Show traffic lights
	appConfig.EnableMinimize = true     // Allow minimize
	appConfig.EnableMaximize = true     // Allow maximize (since it's resizable)

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI() *retained.Widget {
	// Main container - flexbox column layout
	root := retained.Container("bg-gray-800 flex flex-col w-full")

	// Title bar / drag area
	// We reserve 70px on the left for the native traffic light controls
	titleBar := retained.Container("bg-gray-700 w-full h-8 flex items-center").
		WithChildren(
			// Spacer for native window controls (traffic lights)
			retained.Container("w-20 h-8 bg-blue-400"), // ~80px to leave room for traffic lights
			// Title (centered in remaining space)
			retained.Container("flex-1 flex items-center justify-center").WithChildren(

				retained.Text("Frameless App", "text-gray-300 text-sm text-center font-medium"),
			),
			// Right padding to balance the layout
			retained.Container("w-20 h-8 bg-blue-500"),
		)

	// Search bar
	searchBar := retained.Container("px-4 py-3 flex items-center bg-gray-750 shrink-0").
		WithChildren(
			// Search icon
			retained.Text("🔍", "text-gray-500 text-lg mr-3"),
			// Search input
			createSearchInput(),
		)

	// Content area (fills remaining space)
	content := retained.Container("flex-1 p-4 overflow-auto").
		WithChildren(
			retained.Text("This is a resizable frameless window with native window controls.", "text-gray-400 text-sm"),
		)

	// Status bar
	statusBar := retained.Container("h-6 bg-gray-700 px-4 flex items-center shrink-0").
		WithChildren(
			retained.Text("Ready", "text-gray-500 text-xs"),
		)

	root.WithChildren(titleBar, searchBar, content, statusBar)

	return root
}

func createSearchInput() *retained.Widget {
	searchInput = retained.TextField("Search anything...", "flex-1 bg-transparent text-white text-lg")

	return searchInput
}
