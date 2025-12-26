// Example demonstrating image widgets with various loading methods
//
// This example shows how to:
// 1. Load images from URLs (async)
// 2. Load images from bundled files (sync)
// 3. Load pre-loaded textures
// 4. Apply Tailwind-style classes to images
package main

import (
	"fmt"
	"runtime"

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

func init() {
	// Lock the main goroutine to the main OS thread.
	// Required on macOS for windowing.
	runtime.LockOSThread()
}

func main() {
	// Create the game loop
	config := ctd.DefaultLoopConfig()
	loop := ctd.NewLoop(config)

	// Build the UI demonstrating different image loading methods
	root := buildUI()
	loop.Tree().SetRoot(root)

	// Handle resize
	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	// Handle escape to exit
	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		return false
	})

	// Run the event loop
	fmt.Println("Image Widget Example - Press ESC to exit")
	fmt.Println("")
	fmt.Println("Demonstrating image loading methods:")
	fmt.Println("  1. From URL (async) - loads from the internet")
	fmt.Println("  2. From file (sync) - loads from local file")
	fmt.Println("  3. Pre-loaded texture - manual texture management")

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Image Widget Example"
	appConfig.Width = 1024
	appConfig.Height = 768

	if err := loop.Run(appConfig); err != nil {
		fmt.Printf("Error: %v\n", err)
	}
}

func buildUI() *ctd.Widget {
	return ctd.VStack("flex-1 bg-gray-900 p-8",
		// Header
		ctd.Text("Image Widget Example", "text-4xl font-bold text-white mb-8"),

		// Image examples in a grid
		ctd.HStack("gap-8 flex-wrap",
			// URL-based image loading (async)
			imageCard(
				"From URL (Async)",
				"Images loaded from URLs are fetched asynchronously.\nA placeholder is shown while loading.",
				ctd.ImageFromURL(
					"https://placecats.com/200/200",
					"w-48 h-48 rounded-lg",
				),
			),

			// Another URL example with different size
			imageCard(
				"URL with Aspect Ratio",
				"Different image sizes work with Tailwind classes.",
				ctd.ImageFromURL(
					"https://placecats.com/300/200",
					"w-64 h-40 rounded-xl",
				),
			),

			// Bundled file loading
			imageCard(
				"From File (Sync)",
				"Bundled images load synchronously.\nPath relative to working directory.",
				ctd.ImageFromFile(
					"examples/images/sample.png",
					"w-48 h-48 rounded-lg bg-gray-700",
				),
			),
		),

		// Object-fit examples (using non-square image in square container)
		ctd.Text("Object Fit Modes", "text-2xl font-semibold text-white mt-8 mb-4"),
		ctd.HStack("gap-4",
			// Each mode with a wide image (300x200) in a square container (96x96)
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/300/200", "w-24 h-24 bg-gray-700 object-fill"),
				ctd.Text("fill", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/301/201", "w-24 h-24 bg-gray-700 object-contain"),
				ctd.Text("contain", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/302/202", "w-24 h-24 bg-gray-700 object-cover"),
				ctd.Text("cover", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/303/203", "w-24 h-24 bg-gray-700 object-none"),
				ctd.Text("none", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/304/204", "w-24 h-24 bg-gray-700 object-scale-down"),
				ctd.Text("scale-down", "text-xs text-gray-400"),
			),
		),

		// Object-position examples (using cover mode to show positioning)
		ctd.Text("Object Position (with cover)", "text-2xl font-semibold text-white mt-8 mb-4"),
		ctd.HStack("gap-4",
			// Different positions with cover mode
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/400/300", "w-24 h-24 bg-gray-700 object-cover object-left-top"),
				ctd.Text("left-top", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/401/301", "w-24 h-24 bg-gray-700 object-cover object-top"),
				ctd.Text("top", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/402/302", "w-24 h-24 bg-gray-700 object-cover object-right-top"),
				ctd.Text("right-top", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/403/303", "w-24 h-24 bg-gray-700 object-cover object-left"),
				ctd.Text("left", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/404/304", "w-24 h-24 bg-gray-700 object-cover object-center"),
				ctd.Text("center", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/405/305", "w-24 h-24 bg-gray-700 object-cover object-right"),
				ctd.Text("right", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/406/306", "w-24 h-24 bg-gray-700 object-cover object-left-bottom"),
				ctd.Text("left-bottom", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/407/307", "w-24 h-24 bg-gray-700 object-cover object-bottom"),
				ctd.Text("bottom", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/408/308", "w-24 h-24 bg-gray-700 object-cover object-right-bottom"),
				ctd.Text("right-bottom", "text-xs text-gray-400"),
			),
		),

		// Arbitrary value syntax examples
		ctd.Text("Arbitrary Values", "text-2xl font-semibold text-white mt-8 mb-4"),
		ctd.HStack("gap-4",
			// Arbitrary fit values
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/500/400", "w-24 h-24 bg-gray-700 object-[cover]"),
				ctd.Text("object-[cover]", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/501/401", "w-24 h-24 bg-gray-700 object-[contain]"),
				ctd.Text("object-[contain]", "text-xs text-gray-400"),
			),
			// Arbitrary position values (with cover)
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/502/402", "w-24 h-24 bg-gray-700 object-cover object-[top]"),
				ctd.Text("object-[top]", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/503/403", "w-24 h-24 bg-gray-700 object-cover object-[60%_100%]"),
				ctd.Text("object-[left_bottom]", "text-xs text-gray-400"),
			),
		),

		// Rounded corners examples
		ctd.Text("Rounded Corners", "text-2xl font-semibold text-white mt-8 mb-4"),
		ctd.HStack("gap-4",
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/100/100", "w-24 h-24 rounded-none"),
				ctd.Text("none", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/101/101", "w-24 h-24 rounded-md"),
				ctd.Text("md", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/102/102", "w-24 h-24 rounded-lg"),
				ctd.Text("lg", "text-xs text-gray-400"),
			),
			ctd.VStack("items-center gap-2",
				ctd.ImageFromURL("https://placecats.com/103/103", "w-24 h-24 rounded-full"),
				ctd.Text("full", "text-xs text-gray-400"),
			),
		),

		// Spacer
		ctd.Container("flex-1"),
	)
}

// imageCard creates a card with an image and description
func imageCard(title, description string, image *ctd.Widget) *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-4",
		ctd.Text(title, "text-lg font-semibold text-white"),
		image,
		ctd.Text(description, "text-sm text-gray-400 max-w-xs"),
	)
}
