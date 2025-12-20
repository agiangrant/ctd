//go:build ignore

// NOTE: This example uses a legacy API that was never fully implemented.
// See examples/ios_demo for the modern retained mode API.
// See examples/web_demo for the modern immediate mode API.

package main

import (
	"fmt"
	"log"

	"github.com/agiangrant/centered"
)

func main() {
	fmt.Println("Centered Engine Example")
	fmt.Println("Version:", centered.Version())

	// Create engine with default config
	config := centered.DefaultEngineConfig()
	config.Width = 1024
	config.Height = 768

	engine, err := centered.NewEngine(config)
	if err != nil {
		log.Fatalf("Failed to create engine: %v", err)
	}
	defer engine.Shutdown()

	// Build a simple UI
	// This demonstrates the Tailwind-style API
	ui := buildUI()

	// Render the frame (single FFI call per frame)
	events, err := engine.RenderFrame(ui)
	if err != nil {
		log.Fatalf("Failed to render frame: %v", err)
	}

	fmt.Printf("Rendered frame, received %d events\n", len(events))

	// In a real app, you would:
	// 1. Run this in a loop (game loop or event loop)
	// 2. Update state based on events
	// 3. Rebuild UI with new state
	// 4. Call RenderFrame again
}

// buildUI builds the widget tree for the current frame
func buildUI() centered.Widget {
	return centered.VStack("bg-gray-100 p-8 gap-4 w-full h-full",
		// Header
		centered.Heading(
			"Welcome to Centered",
			"text-4xl font-bold text-gray-900 mb-4",
		),

		// Card with button
		centered.Container("bg-white rounded-lg shadow-lg p-6 gap-4",
			centered.Text(
				"This is a demo of the Centered UI framework.",
				"text-lg text-gray-700 mb-4",
			),

			centered.Button(
				"Click Me",
				"bg-blue-500 hover:bg-blue-600 active:bg-blue-700 text-white font-semibold px-6 py-3 rounded-lg cursor-pointer transition duration-150",
			),

			// Flex row with multiple items
			centered.HStack("flex gap-4 mt-4",
				centered.Button(
					"Secondary",
					"bg-gray-200 hover:bg-gray-300 text-gray-800 px-4 py-2 rounded cursor-pointer",
				),
				centered.Button(
					"Danger",
					"bg-red-500 hover:bg-red-600 text-white px-4 py-2 rounded cursor-pointer",
				),
			),
		),

		// Form example
		centered.Container("bg-white rounded-lg shadow-md p-6 gap-4 mt-4",
			centered.Label("Email Address", "text-sm font-medium text-gray-700"),
			centered.TextField(
				"Enter your email",
				"border border-gray-300 rounded-md px-3 py-2 focus:border-blue-500 focus:ring-1 focus:ring-blue-500",
			),

			centered.Label("Message", "text-sm font-medium text-gray-700 mt-4"),
			centered.TextArea(
				"Enter your message",
				"border border-gray-300 rounded-md px-3 py-2 h-32 focus:border-blue-500 focus:ring-1 focus:ring-blue-500",
			),

			centered.Checkbox(
				"Subscribe to newsletter",
				"mt-4",
			),
		),

		// Grid example
		centered.Container("grid grid-cols-3 gap-4 mt-4",
			centered.Container("bg-blue-500 text-white p-4 rounded-lg text-center",
				centered.Text("Item 1", "font-semibold"),
			),
			centered.Container("bg-green-500 text-white p-4 rounded-lg text-center",
				centered.Text("Item 2", "font-semibold"),
			),
			centered.Container("bg-purple-500 text-white p-4 rounded-lg text-center",
				centered.Text("Item 3", "font-semibold"),
			),
		),

		// Arbitrary values example
		centered.Container("bg-white rounded-[20px] shadow-lg p-6 mt-4",
			centered.Text(
				"This container uses arbitrary values: rounded-[20px], w-[80%]",
				"text-gray-600 w-[80%]",
			),
		),
	)
}
