// Example demonstrating the new event system with interactive widgets.
// Shows hover effects, click handling, and focus management.
package main

import (
	"fmt"
	"log"
	"runtime"
	"time"

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

func init() {
	runtime.LockOSThread()
}

// Colors for visual feedback
const (
	ColorNormal  = 0x3B82F6FF // blue-500
	ColorHover   = 0x2563EBFF // blue-600
	ColorPressed = 0x1D4ED8FF // blue-700
	ColorFocused = 0x60A5FAFF // blue-400
)

func main() {
	config := ctd.DefaultLoopConfig()
	loop := ctd.NewLoop(config)
	tree := loop.Tree()
	anims := loop.Animations()

	// Build the UI with interactive widgets
	root, widgets := buildInteractiveUI()
	tree.SetRoot(root)

	// Set up event handlers on the widgets
	setupEventHandlers(widgets, anims)

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

	loop.OnFrame(func(frame *ctd.Frame) {
		// Draw hover/focus indicator
		if hovered := loop.Events().HoveredWidget(); hovered != nil {
			bounds := hovered.ComputedBounds()
			frame.DrawText(
				fmt.Sprintf("Hovering: %s", hovered.Kind()),
				10, 10, 14, ctd.ColorWhite,
			)
			frame.DrawText(
				fmt.Sprintf("Bounds: (%.0f, %.0f, %.0f, %.0f)",
					bounds.X, bounds.Y, bounds.Width, bounds.Height),
				10, 30, 12, ctd.ColorGray400,
			)
		}

		if focused := loop.Events().FocusedWidget(); focused != nil {
			frame.DrawText(
				fmt.Sprintf("Focused: %s", focused.Kind()),
				10, 50, 14, ctd.ColorGreen400,
			)
		}
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Interactive Event Demo"
	appConfig.Width = 800
	appConfig.Height = 600

	log.Println("Starting Interactive Event Demo...")
	log.Println("  - Hover over widgets to see hover effects")
	log.Println("  - Click buttons to see click feedback")
	log.Println("  - Tab to change focus (coming soon)")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

// WidgetRefs holds references to widgets we want to add handlers to
type WidgetRefs struct {
	Button1          *ctd.Widget
	Button2          *ctd.Widget
	Button3          *ctd.Widget
	FullscreenButton *ctd.Widget
	HoverBox         *ctd.Widget
	ClickCounter     *ctd.Widget
	StatusText       *ctd.Widget
}

func buildInteractiveUI() (*ctd.Widget, *WidgetRefs) {
	refs := &WidgetRefs{}

	root := ctd.VStack("bg-gray-900 w-full")
	// Title
	title := ctd.Text("Interactive Event System Demo", "text-white text-2xl")
	// Instructions panel
	instructions := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		ctd.Text("Instructions:", "text-yellow-400 text-lg"),
		ctd.Text("- Hover over buttons to see color change", "text-gray-300 text-sm"),
		ctd.Text("- Click buttons to increment counter", "text-gray-300 text-sm"),
		ctd.Text("- Watch the status text update", "text-gray-300 text-sm"),
		ctd.Text("- Press ESC to exit", "text-gray-300 text-sm"),
	)

	// Button row - NOW USING TAILWIND hover: CLASSES!
	// Hover styles are applied automatically via Tailwind variants
	refs.Button1 = ctd.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-lg").
		WithChildren(
			ctd.Text("Button 1", "text-white text-base").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button2 = ctd.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-lg").
		WithFrame(160, 300, 120, 50).
		WithChildren(
			ctd.Text("Button 2", "text-white text-base").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button3 = ctd.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-lg").
		WithChildren(
			ctd.Text("Button 3", "text-white text-base").
				WithPositionMode(ctd.PositionRelative),
		)

	// Fullscreen toggle button
	refs.FullscreenButton = ctd.Container("bg-yellow-500 hover:bg-yellow-600 active:bg-yellow-700 rounded-lg").
		WithChildren(
			ctd.Text("Fullscreen", "text-white text-base").
				WithPositionMode(ctd.PositionRelative),
		)

	// Hover demonstration box - using Tailwind hover variant
	refs.HoverBox = ctd.Container("bg-gray-700 hover:bg-gray-600 rounded-xl").
		WithChildren(
			ctd.Text("Hover over me!", "text-white text-lg").
				WithPositionMode(ctd.PositionRelative),
		)

	// Counter display
	counterPanel := ctd.Container("bg-gray-800 rounded-lg")
	counterLabel := ctd.Text("Click Count:", "text-gray-400 text-sm")
	refs.ClickCounter = ctd.Text("0", "text-white text-4xl")

	// Status text
	refs.StatusText = ctd.Text("Waiting for interaction...", "text-gray-500 text-base")

	// Event info panel
	eventPanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		ctd.Text("Event System Features:", "text-white text-sm"),
		ctd.Text("- Cached bounds for O(1) hit testing", "text-gray-400 text-xs"),
		ctd.Text("- Capture/Bubble event phases", "text-gray-400 text-xs"),
		ctd.Text("- Click & double-click detection", "text-gray-400 text-xs"),
		ctd.Text("- Hover enter/leave tracking", "text-gray-400 text-xs"),
		ctd.Text("- Focus management", "text-gray-400 text-xs"),
		ctd.Text("- Object pooling for performance", "text-gray-400 text-xs"),
	)

	root.WithChildren(
		title,
		instructions,
		refs.Button1,
		refs.Button2,
		refs.Button3,
		refs.FullscreenButton,
		refs.HoverBox,
		counterPanel, counterLabel, refs.ClickCounter,
		refs.StatusText,
		eventPanel,
	)

	return root, refs
}

func setupEventHandlers(refs *WidgetRefs, anims *ctd.AnimationRegistry) {
	clickCount := 0

	// Helper to update click counter
	updateCounter := func() {
		clickCount++
		refs.ClickCounter.SetText(fmt.Sprintf("%d", clickCount))
	}

	// Button 1 handlers - hover/active colors now handled by Tailwind classes!
	refs.Button1.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering Button 1 (Tailwind hover:bg-blue-600)")
		refs.StatusText.SetTextColor(ctd.ColorBlue400)
	})
	refs.Button1.OnMouseLeave(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Mouse left Button 1")
		refs.StatusText.SetTextColor(ctd.ColorGray500)
	})
	refs.Button1.OnClick(func(e *ctd.MouseEvent) {
		updateCounter()
		refs.StatusText.SetText("Clicked Button 1!")
		refs.StatusText.SetTextColor(ctd.ColorGreen400)
		// Animate the button
		refs.Button1.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack).
			OnComplete(func() {
				refs.Button1.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	// Button 2 handlers - hover colors via Tailwind
	refs.Button2.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering Button 2 (Tailwind hover:bg-green-600)")
		refs.StatusText.SetTextColor(ctd.ColorGreen400)
	})
	refs.Button2.OnMouseLeave(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Mouse left Button 2")
		refs.StatusText.SetTextColor(ctd.ColorGray500)
	})
	refs.Button2.OnClick(func(e *ctd.MouseEvent) {
		updateCounter()
		refs.StatusText.SetText("Clicked Button 2!")
		refs.StatusText.SetTextColor(ctd.ColorGreen400)
	})

	// Button 3 handlers - hover colors via Tailwind
	refs.Button3.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering Button 3 (Tailwind hover:bg-purple-600)")
		refs.StatusText.SetTextColor(ctd.ColorPurple400)
	})
	refs.Button3.OnMouseLeave(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Mouse left Button 3")
		refs.StatusText.SetTextColor(ctd.ColorGray500)
	})
	refs.Button3.OnDoubleClick(func(e *ctd.MouseEvent) {
		clickCount += 5
		refs.ClickCounter.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Double-clicked Button 3! (+5 bonus)")
		refs.StatusText.SetTextColor(ctd.ColorYellow400)
	})
	refs.Button3.OnClick(func(e *ctd.MouseEvent) {
		updateCounter()
		refs.StatusText.SetText("Clicked Button 3!")
		refs.StatusText.SetTextColor(ctd.ColorPurple400)
	})

	// Fullscreen button handler
	refs.FullscreenButton.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering Fullscreen button (Tailwind hover:bg-yellow-600)")
		refs.StatusText.SetTextColor(ctd.ColorYellow400)
	})
	refs.FullscreenButton.OnMouseLeave(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Mouse left Fullscreen button")
		refs.StatusText.SetTextColor(ctd.ColorGray500)
	})
	refs.FullscreenButton.OnClick(func(e *ctd.MouseEvent) {
		ctd.WindowToggleFullscreen()
		refs.StatusText.SetText("Toggled fullscreen!")
		refs.StatusText.SetTextColor(ctd.ColorYellow400)
	})

	// Hover box handlers - hover color via Tailwind
	refs.HoverBox.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText(fmt.Sprintf("Entered hover box at (%.0f, %.0f) - Tailwind hover:bg-gray-600", e.LocalX, e.LocalY))
		refs.StatusText.SetTextColor(ctd.ColorWhite)
	})
	refs.HoverBox.OnMouseLeave(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Left hover box")
		refs.StatusText.SetTextColor(ctd.ColorGray500)
	})
	refs.HoverBox.OnMouseMove(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText(fmt.Sprintf("Mouse at (%.0f, %.0f) in hover box", e.LocalX, e.LocalY))
	})
}
