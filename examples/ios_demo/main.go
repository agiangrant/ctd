// Package ios_demo demonstrates running Go + Rust FFI on iOS.
//
// Build with: gomobile bind -target iossimulator -o CenteredDemo.xcframework .
//
// Then integrate CenteredDemo.xcframework into an Xcode project.
//
// Architecture:
// - main.m calls IOSMain() which is the entry point
// - IOSMain() calls ffi.Run() which handles the iOS-specific flow:
//   1. Registers Go's ready callback with Rust
//   2. Calls centered_ios_main() which starts UIApplicationMain (never returns)
//   3. When iOS app is ready, Rust calls Go's ready callback
//   4. Go's callback registers the event handler for rendering
package ios_demo

import (
	"fmt"
	"log"
	"runtime"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

// Dummy export required by gomobile
func Dummy() {}

// Global state for the demo
var (
	demoLoop   *retained.Loop
	clickCount int
)

// IOSMain is the entry point for the iOS app.
// This should be called from main.m (which is the actual iOS entry point).
// It handles the full app lifecycle via ffi.Run().
func IOSMain() {
	StartDemo()
}

// StartDemo initializes and starts the demo app.
// On iOS, this is called via IOSMain().
// On desktop, this can be called directly for testing.
func StartDemo() {
	log.Printf("=== Starting Centered iOS Demo (UPDATED BUILD) ===")
	log.Printf("runtime.GOOS = %s", runtime.GOOS)
	log.Printf("runtime.GOARCH = %s", runtime.GOARCH)

	runtime.LockOSThread()

	config := retained.DefaultLoopConfig()
	demoLoop = retained.NewLoop(config)
	tree := demoLoop.Tree()
	anims := demoLoop.Animations()

	// Build the UI
	root, refs := buildDemoUI()
	tree.SetRoot(root)

	// Set up event handlers
	setupDemoHandlers(refs, anims)

	demoLoop.OnResize(func(width, height float32) {
		log.Printf("OnResize called: %fx%f", width, height)
		root.SetSize(width, height)
	})

	demoLoop.OnEvent(func(event ffi.Event) bool {
		// iOS doesn't have escape key, but handle other events
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Centered iOS Demo"
	appConfig.Width = 390  // iPhone logical width
	appConfig.Height = 844 // iPhone logical height

	log.Println("Starting Centered iOS Demo...")

	if err := demoLoop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

// DemoWidgetRefs holds references to widgets we want to interact with
type DemoWidgetRefs struct {
	CounterText *retained.Widget
	StatusText  *retained.Widget
	Button1     *retained.Widget
	Button2     *retained.Widget
	Button3     *retained.Widget
}

func buildDemoUI() (*retained.Widget, *DemoWidgetRefs) {
	refs := &DemoWidgetRefs{}

	// Title
	title := retained.Text("Centered iOS Demo", "text-white text-2xl font-bold")

	// Subtitle
	subtitle := retained.Text("Tailwind-style UI on iOS with Go", "text-gray-400 text-base")

	// Counter card
	counterCard := retained.Container("bg-gray-800 rounded-2xl p-4")

	counterLabel := retained.Text("Tap Counter", "text-gray-400 text-sm")

	refs.CounterText = retained.Text("0", "text-white text-6xl font-bold")

	// Button row
	refs.Button1 = retained.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Blue", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.Button2 = retained.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Green", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.Button3 = retained.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Purple", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Status text
	refs.StatusText = retained.Text("Tap a button to interact", "text-gray-500 text-sm")

	// Feature list
	featureCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		retained.Text("Features", "text-white text-lg font-semibold"),
		retained.Text("✓ Tailwind CSS classes", "text-green-400 text-sm"),
		retained.Text("✓ Touch interactions", "text-green-400 text-sm"),
		retained.Text("✓ Hover/Active states", "text-green-400 text-sm"),
		retained.Text("✓ Animations", "text-green-400 text-sm"),
		retained.Text("✓ Go + Rust engine", "text-green-400 text-sm"),
		retained.Text("✓ No CGO required", "text-green-400 text-sm"),
	)

	// Footer
	footer := retained.Text("Built with Centered Framework", "text-gray-600 text-xs")

	// Scrollable content container
	scrollContent := retained.VStack("gap-2 p-4 w-full",
		title,
		subtitle,
		counterCard,
		counterLabel,
		refs.CounterText,
		refs.Button1,
		refs.Button2,
		refs.Button3,
		refs.StatusText,
		featureCard,
		// Add extra items to ensure scrollable content
		retained.Text("Scroll down for more →", "text-gray-500 text-sm mt-4"),
		retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
			retained.Text("More Features", "text-white text-lg font-semibold"),
			retained.Text("✓ Touch scrolling", "text-green-400 text-sm"),
			retained.Text("✓ Momentum scrolling", "text-green-400 text-sm"),
			retained.Text("✓ Gesture recognition", "text-green-400 text-sm"),
		),
		retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
			retained.Text("Platform Support", "text-white text-lg font-semibold"),
			retained.Text("✓ macOS", "text-blue-400 text-sm"),
			retained.Text("✓ iOS", "text-blue-400 text-sm"),
			retained.Text("○ Android (planned)", "text-gray-500 text-sm"),
			retained.Text("○ Windows (planned)", "text-gray-500 text-sm"),
			retained.Text("○ Linux (planned)", "text-gray-500 text-sm"),
		),
		footer,
	)

	// Root container with vertical scrolling enabled
	// Note: flex-col is needed so the container properly calculates content height for scrolling
	root := retained.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

func setupDemoHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Button 1 - Blue
	refs.Button1.OnClick(func(e *retained.MouseEvent) {
		clickCount++
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Blue button tapped!")
		refs.StatusText.SetTextColor(retained.ColorBlue400)

		// Animate
		refs.Button1.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack).
			OnComplete(func() {
				refs.Button1.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	refs.Button1.OnMouseEnter(func(e *retained.MouseEvent) {
		refs.StatusText.SetText("Hovering blue button")
		refs.StatusText.SetTextColor(retained.ColorBlue400)
	})

	// Button 2 - Green
	refs.Button2.OnClick(func(e *retained.MouseEvent) {
		clickCount += 2
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Green button tapped! (+2)")
		refs.StatusText.SetTextColor(retained.ColorGreen400)

		refs.Button2.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack).
			OnComplete(func() {
				refs.Button2.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	refs.Button2.OnMouseEnter(func(e *retained.MouseEvent) {
		refs.StatusText.SetText("Hovering green button")
		refs.StatusText.SetTextColor(retained.ColorGreen400)
	})

	// Button 3 - Purple
	refs.Button3.OnClick(func(e *retained.MouseEvent) {
		clickCount += 5
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Purple button tapped! (+5)")
		refs.StatusText.SetTextColor(retained.ColorPurple400)

		refs.Button3.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack).
			OnComplete(func() {
				refs.Button3.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	refs.Button3.OnMouseEnter(func(e *retained.MouseEvent) {
		refs.StatusText.SetText("Hovering purple button")
		refs.StatusText.SetTextColor(retained.ColorPurple400)
	})

	// Double tap on any button for bonus
	refs.Button3.OnDoubleClick(func(e *retained.MouseEvent) {
		clickCount += 20
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("DOUBLE TAP BONUS! (+20)")
		refs.StatusText.SetTextColor(retained.ColorYellow400)
	})
}

// Main is exported for testing on desktop
func Main() {
	StartDemo()
}
