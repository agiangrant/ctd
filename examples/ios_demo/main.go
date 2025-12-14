// Package ios_demo demonstrates running Go + Rust FFI on iOS.
//
// Build with: gomobile bind -target iossimulator -o CenteredDemo.xcframework .
//
// Then integrate CenteredDemo.xcframework into an Xcode project.
//
// Note: Full wgpu rendering requires winit to own the app lifecycle.
// This demo tests FFI integration with text measurement and other non-windowing APIs.
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

// StartDemo initializes and starts the demo app.
// This should be called from the iOS app's initialization.
func StartDemo() {
	log.Printf("Starting Centered iOS Demo...")
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

	root := retained.VStack("bg-gray-900 w-full h-full gap-2 p-4",
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
		footer,
	)

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
