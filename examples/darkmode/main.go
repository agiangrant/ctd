// Example demonstrating dark mode support with the dark: Tailwind variant
package main

import (
	"fmt"
	"log"
	"runtime"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := retained.DefaultLoopConfig()
	// Start with system preference (default)
	config.ColorScheme = retained.ColorSchemeSystem

	loop := retained.NewLoop(config)
	tree := loop.Tree()

	// Status label to show current mode
	var statusLabel *retained.Widget
	var modeLabel *retained.Widget

	root := buildUI(loop, &statusLabel, &modeLabel)
	tree.SetRoot(root)

	// Update mode label with initial state
	updateModeLabel(modeLabel, loop)

	// Handle resize
	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	// Handle events
	loop.OnEvent(func(event ffi.Event) bool {
		switch event.Type {
		case ffi.EventKeyPressed:
			keycode := event.Keycode()

			// Escape to quit
			if keycode == uint32(ffi.KeyEscape) {
				ffi.RequestExit()
				return true
			}

			// 1 = System, 2 = Light, 3 = Dark
			switch keycode {
			case uint32(ffi.Key1):
				loop.SetColorScheme(retained.ColorSchemeSystem)
				updateModeLabel(modeLabel, loop)
				statusLabel.SetText("Switched to System preference")
				return true
			case uint32(ffi.Key2):
				loop.SetColorScheme(retained.ColorSchemeLight)
				updateModeLabel(modeLabel, loop)
				statusLabel.SetText("Switched to Light mode")
				return true
			case uint32(ffi.Key3):
				loop.SetColorScheme(retained.ColorSchemeDark)
				updateModeLabel(modeLabel, loop)
				statusLabel.SetText("Switched to Dark mode")
				return true
			case uint32(ffi.KeyR):
				// Refresh system dark mode (useful if OS setting changed)
				loop.RefreshSystemDarkMode()
				updateModeLabel(modeLabel, loop)
				statusLabel.SetText("Refreshed system dark mode preference")
				return true
			}
		}
		return false
	})

	// Frame callback
	loop.OnFrame(func(frame *retained.Frame) {
		fps := 1.0 / frame.DeltaTime
		frame.DrawText(
			fmt.Sprintf("FPS: %.1f", fps),
			float32(550), 10, 14, retained.ColorWhite,
		)
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Dark Mode Demo"
	appConfig.Width = 700
	appConfig.Height = 500

	log.Println("Starting dark mode demo...")
	log.Printf("  - OS Dark Mode: %v", ffi.SystemDarkMode())
	log.Println("  - Press 1 for System, 2 for Light, 3 for Dark")
	log.Println("  - Press R to refresh system preference")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func updateModeLabel(label *retained.Widget, loop *retained.Loop) {
	schemeStr := "System"
	switch loop.ColorScheme() {
	case retained.ColorSchemeLight:
		schemeStr = "Light"
	case retained.ColorSchemeDark:
		schemeStr = "Dark"
	}

	darkStr := "OFF"
	if loop.DarkMode() {
		darkStr = "ON"
	}

	label.SetText(fmt.Sprintf("Scheme: %s | Dark Mode: %s", schemeStr, darkStr))
}

func buildUI(loop *retained.Loop, statusLabel, modeLabel **retained.Widget) *retained.Widget {
	// Root container - uses dark: variant for automatic switching
	// bg-gray-100 in light mode, bg-gray-900 in dark mode
	root := retained.Container("bg-gray-100 dark:bg-gray-900").
		WithSize(700, 500)

	// Header panel - demonstrates dark: variant colors
	headerText := retained.Text("Dark Mode Demo", "text-2xl text-gray-900 dark:text-white")

	header := retained.HStack("p-4 bg-gray-200 dark:bg-gray-800",
		headerText,
	)

	// Mode display - text color changes with dark mode
	mode := retained.Text("", "text-sm text-gray-600 dark:text-gray-300")
	*modeLabel = mode

	// Status display
	status := retained.Text("Press 1/2/3 to change color scheme", "text-sm text-gray-500 dark:text-gray-400")
	*statusLabel = status

	// Demo cards showing light/dark styling
	cardsPanel := retained.HStack("w-full gap-4 p-4")

	// Card 1 - Adaptive card (changes with dark mode)
	card1 := retained.VStack("flex-1 gap-2 p-4 rounded-lg bg-white dark:bg-gray-700")
	card1Title := retained.Text("Adaptive Card", "text-base text-gray-900 dark:text-white")
	card1Body := retained.Text("This card adapts to light/dark mode automatically.", "text-xs text-gray-600 dark:text-gray-300")
	card1.WithChildren(card1Title, card1Body)

	// Card 2 - Always dark card (no dark: variants, just dark colors)
	card2 := retained.VStack("flex-1 gap-2 p-4 rounded-lg bg-gray-800")
	card2Title := retained.Text("Dark Card", "text-base text-white")
	card2Body := retained.Text("This card is always dark themed.", "text-xs text-gray-400")
	card2.WithChildren(card2Title, card2Body)

	// Card 3 - Accent card (blue that shifts shade in dark mode)
	card3 := retained.VStack("flex-1 gap-2 p-4 rounded-lg bg-blue-500 dark:bg-blue-700")
	card3Title := retained.Text("Accent Card", "text-base text-white")
	card3Body := retained.Text("Blue accent shifts in dark mode.", "text-xs text-blue-100 dark:text-blue-200")
	card3.WithChildren(card3Title, card3Body)

	cardsPanel.WithChildren(card1, card2, card3)

	// Instructions panel
	instructionsTitle := retained.Text("Keyboard Controls:", "text-sm text-gray-700 dark:text-gray-300")
	inst1 := retained.Text("  1 - System (follow OS)", "text-xs text-gray-500 dark:text-gray-400")
	inst2 := retained.Text("  2 - Force Light mode", "text-xs text-gray-500 dark:text-gray-400")
	inst3 := retained.Text("  3 - Force Dark mode", "text-xs text-gray-500 dark:text-gray-400")
	inst4 := retained.Text("  R - Refresh system preference", "text-xs text-gray-500 dark:text-gray-400")
	inst5 := retained.Text("  ESC - Quit", "text-xs text-gray-500 dark:text-gray-400")

	instructions := retained.VStack("gap-1 p-4",
		instructionsTitle,
		inst1, inst2, inst3, inst4, inst5,
	)

	// Main layout
	mainLayout := retained.VStack("flex-1 gap-4")
	mainLayout.WithChildren(header, mode, status, cardsPanel, instructions)

	root.WithChildren(mainLayout)
	return root
}
