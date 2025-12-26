// Example demonstrating bundled fonts from theme.toml
//
// This example shows how to use the font-serif class which is configured
// in theme.toml to use a bundled TTF font (examples/fontbundle/example.ttf).
//
// The theme.toml configuration:
//
//	[fonts]
//	serif = "examples/fontbundle/example.ttf"
//
// This maps font-serif to a bundled font file, while font-sans and font-mono
// use sensible system font defaults.
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

	// Build the UI demonstrating different font families
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
	fmt.Println("Font Bundle Example - Press ESC to exit")
	fmt.Println("Demonstrating theme-level font configuration:")
	fmt.Println("  font-sans  -> system (system default)")
	fmt.Println("  font-serif -> examples/fontbundle/example.ttf (bundled)")
	fmt.Println("  font-mono  -> Menlo (system)")

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Font Bundle Example"
	appConfig.Width = 1024
	appConfig.Height = 768

	if err := loop.Run(appConfig); err != nil {
		fmt.Printf("Error: %v\n", err)
	}
}

func buildUI() *ctd.Widget {
	return ctd.VStack("flex-1 bg-gray-900",
		// Header
		ctd.Text("Font Bundle Example", "text-4xl font-bold text-white p-8"),

		// Container with font examples
		ctd.VStack("p-8 gap-4",
			// System sans-serif (default)
			ctd.VStack("bg-gray-800 w-full rounded-lg p-6 mb-4",
				ctd.Text("font-sans (System Default)", "text-sm text-gray-400 mb-2"),
				ctd.Text("The quick brown fox jumps over the lazy dog.", "text-2xl text-white font-sans"),
			),

			// Bundled serif font
			ctd.VStack("bg-gray-800 w-full rounded-lg p-6 mb-4",
				ctd.Text("font-serif (Bundled: example.ttf)", "text-sm text-gray-400 mb-2"),
				ctd.Text("The quick brown fox jumps over the lazy dog.", "text-2xl text-white font-serif"),
			),

			// System monospace
			ctd.VStack("bg-gray-800 w-full rounded-lg p-6 mb-4",
				ctd.Text("font-mono (System: Menlo)", "text-sm text-gray-400 mb-2"),
				ctd.Text("The quick brown fox jumps over the lazy dog.", "text-2xl text-white font-mono"),
			),

			// Comparison side by side
			ctd.HStack("bg-gray-800 rounded-lg p-6 gap-4 w-full",
				ctd.VStack("flex-1 items-center",
					ctd.Text("Sans", "text-sm text-gray-500"),
					ctd.Text("ABC abc 123", "text-3xl text-blue-400 font-sans"),
				),
				ctd.VStack("flex-1 items-center",
					ctd.Text("Serif (Bundled)", "text-sm text-gray-500"),
					ctd.Text("ABC abc 123", "text-3xl text-green-400 font-serif"),
				),
				ctd.VStack("flex-1 items-center",
					ctd.Text("Mono", "text-sm text-gray-500"),
					ctd.Text("ABC abc 123", "text-3xl text-purple-400 font-mono"),
				),
			),
		),
	)
}
