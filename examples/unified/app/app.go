// Package app contains the shared application logic for all platforms.
//
// This is the recommended pattern for CTD apps:
// - app/app.go: Shared UI and business logic (this file)
// - main.go: Desktop entry point (macOS/Linux/Windows)
// - For mobile: separate ios/ and android/ directories with entry points
//
// The same Go code powers desktop and mobile with platform-adaptive behavior.
package app

import (
	"fmt"
	"log"
	"time"

	"github.com/agiangrant/ctd"
	"github.com/agiangrant/ctd/internal/ffi"
)

// App holds the application state
type App struct {
	loop   *ctd.Loop
	root   *ctd.Widget
	refs   *WidgetRefs
	clicks int
}

// WidgetRefs holds references to interactive widgets
type WidgetRefs struct {
	Title       *ctd.Widget
	Counter     *ctd.Widget
	Status      *ctd.Widget
	ActionBtn   *ctd.Widget
	PlatformLbl *ctd.Widget
	TextField   *ctd.Widget
}

// New creates a new application instance
func New() *App {
	return &App{}
}

// Run starts the application
func (a *App) Run() error {
	log.Printf("Starting CTD Unified App on %s", ctd.CurrentPlatform())

	config := ctd.DefaultLoopConfig()
	a.loop = ctd.NewLoop(config)

	// Build platform-adaptive UI
	a.root, a.refs = a.buildUI()
	a.loop.Tree().SetRoot(a.root)

	// Set up event handlers
	a.setupHandlers()

	// Configure window based on platform
	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "CTD Unified App"

	if ctd.IsMobile() {
		// Mobile: Full screen, system handles dimensions
		appConfig.Width = 390
		appConfig.Height = 844
	} else {
		// Desktop: Windowed
		appConfig.Width = 800
		appConfig.Height = 600
	}

	return a.loop.Run(appConfig)
}

// buildUI creates the widget tree with platform-adaptive styling
func (a *App) buildUI() (*ctd.Widget, *WidgetRefs) {
	refs := &WidgetRefs{}

	// Platform-adaptive padding
	padding := "p-6"
	if ctd.IsMobile() {
		padding = "p-4 pt-12" // Extra top padding for safe area on mobile
	}

	// Title
	refs.Title = ctd.Text("CTD Unified App", "text-2xl font-bold text-white")

	// Platform indicator
	platformText := fmt.Sprintf("Running on: %s", ctd.CurrentPlatform())
	if ctd.IsMobile() {
		platformText += " (Mobile)"
	} else {
		platformText += " (Desktop)"
	}
	refs.PlatformLbl = ctd.Text(platformText, "text-sm text-gray-400")

	// Counter
	refs.Counter = ctd.Text("0", "text-6xl font-bold text-white")
	refs.Status = ctd.Text("Tap the button to increment", "text-sm text-gray-500")

	counterCard := ctd.VStack("bg-gray-800 rounded-2xl p-6 gap-2 items-center w-full",
		ctd.Text("Counter", "text-gray-400 text-sm"),
		refs.Counter,
		refs.Status,
	)

	// Action button with platform-adaptive behavior
	buttonLabel := "Click Me"
	if ctd.IsMobile() {
		buttonLabel = "Tap Me"
	}
	refs.ActionBtn = ctd.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-4").
		WithChildren(
			ctd.Text(buttonLabel, "text-white text-lg font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Text input
	placeholder := "Type here..."
	if ctd.HasPhysicalKeyboard() {
		placeholder = "Type here (Press Enter to submit)"
	}
	refs.TextField = ctd.TextField(placeholder, "w-full px-4 py-3 bg-gray-700 rounded-lg text-white")

	inputCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Text Input", "text-gray-400 text-sm"),
		refs.TextField,
	)

	// Platform capabilities card
	capabilitiesCard := a.buildCapabilitiesCard()

	// Feature list
	featureCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Cross-Platform Features", "text-white text-lg font-semibold"),
		ctd.Text("✓ Same Go code on all platforms", "text-green-400 text-sm"),
		ctd.Text("✓ Tailwind-style CSS classes", "text-green-400 text-sm"),
		ctd.Text("✓ Platform-adaptive UI", "text-green-400 text-sm"),
		ctd.Text("✓ Shared business logic", "text-green-400 text-sm"),
		ctd.Text("✓ Native performance (Rust + wgpu)", "text-green-400 text-sm"),
	)

	// Scroll content
	scrollContent := ctd.VStack("gap-4 "+padding+" w-full",
		refs.Title,
		refs.PlatformLbl,
		counterCard,
		refs.ActionBtn,
		inputCard,
		capabilitiesCard,
		featureCard,
		ctd.Container("h-5"), // Spacer
		ctd.Text("Built with CTD Framework", "text-gray-600 text-xs text-center"),
	)

	// Root with scrolling
	root := ctd.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

// buildCapabilitiesCard shows platform-specific capabilities
func (a *App) buildCapabilitiesCard() *ctd.Widget {
	items := []struct {
		label     string
		supported bool
	}{
		{"Haptic Feedback", ctd.SupportsHaptics()},
		{"System Tray", ctd.SupportsSystemTray()},
		{"Multiple Windows", ctd.SupportsMultiWindow()},
		{"File Dialogs", ctd.SupportsFileDialog()},
		{"Physical Keyboard", ctd.HasPhysicalKeyboard()},
	}

	children := make([]*ctd.Widget, 0, len(items)+1)
	children = append(children, ctd.Text("Platform Capabilities", "text-white text-lg font-semibold"))

	for _, item := range items {
		icon := "✓"
		color := "text-green-400"
		if !item.supported {
			icon = "✗"
			color = "text-gray-500"
		}
		children = append(children, ctd.Text(fmt.Sprintf("%s %s", icon, item.label), color+" text-sm"))
	}

	return ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full", children...)
}

// setupHandlers configures event handlers
func (a *App) setupHandlers() {
	anims := a.loop.Animations()

	// Handle resize
	a.loop.OnResize(func(width, height float32) {
		a.root.SetSize(width, height)
	})

	// Handle events
	a.loop.OnEvent(func(event ffi.Event) bool {
		// Desktop: ESC to quit
		if ctd.IsDesktop() && event.Type == ffi.EventKeyPressed {
			if event.Keycode() == uint32(ffi.KeyEscape) {
				ffi.RequestExit()
				return true
			}
		}
		return false
	})

	// Button click
	a.refs.ActionBtn.OnClick(func(e *ctd.MouseEvent) {
		a.clicks++
		a.refs.Counter.SetText(fmt.Sprintf("%d", a.clicks))

		// Platform-adaptive feedback
		if ctd.IsMobile() {
			a.refs.Status.SetText("Tapped!")
			// Haptic feedback on mobile
			if ctd.SupportsHaptics() {
				ffi.HapticFeedback(ffi.HapticImpactLight)
			}
		} else {
			a.refs.Status.SetText("Clicked!")
		}
		a.refs.Status.SetTextColor(ctd.ColorBlue400)

		// Animate button
		a.refs.ActionBtn.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)

		// Reset status after delay
		go func() {
			time.Sleep(1 * time.Second)
			if ctd.IsMobile() {
				a.refs.Status.SetText("Tap the button to increment")
			} else {
				a.refs.Status.SetText("Click the button to increment")
			}
			a.refs.Status.SetTextColor(ctd.ColorGray500)
		}()
	})
}
