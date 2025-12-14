// Example demonstrating Tailwind CSS class integration with retained mode
// and the new animation system for automatic 60 FPS mode switching.
// Now includes class-based animations (animate-pulse, animate-bounce, etc.)!
package main

import (
	"fmt"
	"log"
	"runtime"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)
	tree := loop.Tree()
	anims := loop.Animations()

	root := buildUI()
	tree.SetRoot(root)

	// Initialize animations from animate-* classes in the widget tree
	// This automatically starts looping animations like animate-pulse, animate-bounce
	loop.InitAnimations()

	var clickCount int
	var counterText *retained.Widget
	var animatedBox *retained.Widget
	var animStatusText *retained.Widget

	findWidgets(root, &counterText, &animatedBox, &animStatusText)

	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	// No looping animation - the app starts in event-driven mode (battery efficient)
	// Click to trigger a one-shot animation which temporarily enables 60 FPS mode

	loop.OnEvent(func(event ffi.Event) bool {
		switch event.Type {
		case ffi.EventMousePressed:
			clickCount++
			if counterText != nil {
				counterText.SetText(fmt.Sprintf("Clicks: %d", clickCount))
			}

			// Demo: trigger a one-shot animation on click
			if animatedBox != nil {
				// Scale animation - grows then shrinks
				animatedBox.Animate(anims).
					Duration(200 * time.Millisecond).
					Easing(retained.EaseOutBack).
					OnComplete(func() {
						// Shrink back
						animatedBox.Animate(anims).
							Duration(150 * time.Millisecond).
							Easing(retained.EaseOutCubic).
							Size(100, 100)
					}).
					SizeFromTo(100, 100, 120, 120)
			}
			return true

		case ffi.EventKeyPressed:
			if event.Keycode() == uint32(ffi.KeyEscape) {
				ffi.RequestExit()
				return true
			}
		}
		return false
	})

	// OnFrame is called each frame. We only do immediate draws when animations
	// are active - otherwise we let the system go to event-driven mode.
	loop.OnFrame(func(frame *retained.Frame) {
		activeCount := anims.Count()

		// Only draw FPS overlay when animations are active
		// This allows the system to drop to event-driven mode when idle
		if activeCount > 0 {
			fps := 1.0 / frame.DeltaTime
			frame.DrawText(fmt.Sprintf("FPS: %.1f", fps), 900, 10, 14, retained.ColorWhite)
			frame.DrawText(fmt.Sprintf("Active anims: %d", activeCount), 900, 30, 12, retained.ColorGray400)
			frame.DrawText(fmt.Sprintf("Cache: %d styles", retained.StyleCacheSize()), 900, 50, 12, retained.ColorGray400)
		}

		// Update status text (this is a retained widget, not immediate draw)
		if animStatusText != nil {
			if activeCount > 0 {
				animStatusText.SetText(fmt.Sprintf("60 FPS mode (%d anims)", activeCount))
				animStatusText.SetTextColor(retained.ColorGreen400)
			} else {
				animStatusText.SetText("Event-driven mode (idle)")
				animStatusText.SetTextColor(retained.ColorGray500)
			}
		}
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Tailwind + Animation Demo"
	appConfig.Width = 1024
	appConfig.Height = 768

	log.Println("Starting Tailwind + Animation demo...")
	log.Println("  - Using Tailwind classes for styling")
	log.Println("  - Animation system for automatic 60 FPS mode")
	log.Println("  - Class-based animations: animate-pulse, animate-bounce, animate-spin, animate-ping")
	log.Println("  - Click anywhere to trigger bounce animation")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

// buildUI creates the widget tree using Tailwind classes
func buildUI() *retained.Widget {
	// Root container with Tailwind classes
	root := retained.Container("bg-gray-900").
		WithSize(1024, 768)

	// Header with Tailwind styling - spans full width
	header := retained.HStack("bg-gray-800 rounded-lg p-4",
		retained.Text("Tailwind + Animation Demo", "text-white text-2xl"),
	).WithFrame(20, 20, 984, 50)

	// ============ Column 1 (x=20, width=300) ============

	// Class-based animations demo panel - FEATURED at top!
	// Using absolute positioned boxes for reliable layout with animations
	animClassPanel := retained.Container("bg-indigo-900 rounded-xl").
		WithFrame(20, 90, 300, 220)

	// Title and subtitle
	animTitle := retained.Text("Class-Based Animations", "text-white text-lg").
		WithFrame(36, 106, 200, 24)
	animSubtitle := retained.Text("Just add animate-* classes!", "text-indigo-300 text-xs").
		WithFrame(36, 132, 200, 16)

	// Row of animated boxes - each positioned absolutely with spacing
	// animate-pulse - opacity pulses like a heartbeat
	pulseBox := retained.Container("bg-blue-500 rounded-lg animate-pulse").
		WithFrame(36, 156, 50, 50)
	pulseLabel := retained.Text("pulse", "text-gray-400 text-xs").
		WithFrame(36, 210, 50, 16)

	// animate-bounce - bounces up and down
	bounceBox := retained.Container("bg-green-500 rounded-lg animate-bounce").
		WithFrame(96, 156, 50, 50)
	bounceLabel := retained.Text("bounce", "text-gray-400 text-xs").
		WithFrame(92, 210, 50, 16)

	// animate-spin - rotates (color shift placeholder)
	spinBox := retained.Container("bg-purple-500 rounded-lg animate-spin").
		WithFrame(156, 156, 50, 50)
	spinLabel := retained.Text("spin", "text-gray-400 text-xs").
		WithFrame(160, 210, 50, 16)

	// animate-ping - scales up and fades
	pingBox := retained.Container("bg-red-500 rounded-lg animate-ping").
		WithFrame(216, 156, 50, 50)
	pingLabel := retained.Text("ping", "text-gray-400 text-xs").
		WithFrame(220, 210, 50, 16)

	// Custom animation row - fast pulse with elastic easing!
	fastBox := retained.Container("bg-yellow-500 rounded-lg animate-[pulse_500ms_elastic]").
		WithFrame(36, 236, 50, 50)
	fastLabel := retained.Text("fast", "text-gray-400 text-xs").
		WithFrame(36, 290, 50, 16)
	fastSyntax := retained.Text("animate-[pulse_500ms_elastic]", "text-indigo-200 text-xs").
		WithFrame(96, 254, 200, 16)

	// Counter panel
	counterPanel := retained.VStack("bg-blue-900 rounded-xl p-4 gap-2",
		retained.Text("Interactive Counter", "text-gray-300 text-sm"),
		retained.Text("Clicks: 0", "text-white text-2xl").
			WithData("counterText"),
		retained.Text("(click anywhere to bounce)", "text-gray-500 text-xs"),
	).WithFrame(20, 310, 300, 110)

	// Animated box panel - click to animate
	animPanel := retained.VStack("bg-blue-900 rounded-xl p-4 gap-2",
		retained.Text("Click-Triggered Animation", "text-gray-300 text-sm"),
		retained.Container("bg-blue-500 rounded-lg w-[80px] h-[80px]").
			WithData("animatedBox"),
		retained.Text("Bounces on click", "text-gray-500 text-xs"),
	).WithFrame(20, 440, 300, 150)

	// Animation status panel
	statusPanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		retained.Text("Render Mode:", "text-white text-sm"),
		retained.Text("Checking...", "text-gray-500 text-base").
			WithData("animStatusText"),
		retained.Text("When animations active: 60 FPS", "text-gray-600 text-xs"),
		retained.Text("When idle: event-driven", "text-gray-600 text-xs"),
	).WithFrame(20, 610, 300, 110)

	// ============ Column 2 (x=340, width=320) ============

	// Code example for class-based animations
	animClassCodePanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		retained.Text("Class-Based Usage:", "text-yellow-400 text-sm"),
		retained.Text("retained.Container(", "text-gray-300 text-xs"),
		retained.Text("  \"bg-blue-500 animate-pulse\"", "text-green-400 text-xs"),
		retained.Text(")", "text-gray-300 text-xs"),
		retained.Text("", "text-gray-300 text-xs"),
		retained.Text("loop.InitAnimations() // auto-start", "text-gray-400 text-xs"),
	).WithFrame(340, 90, 320, 140)

	// Programmatic animation example
	codePanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		retained.Text("Programmatic Animation:", "text-yellow-400 text-sm"),
		retained.Text("box.Animate(anims).", "text-gray-300 text-xs"),
		retained.Text("  Duration(200*time.Millisecond).", "text-gray-300 text-xs"),
		retained.Text("  Easing(EaseOutBack).", "text-gray-300 text-xs"),
		retained.Text("  Size(120, 120)", "text-gray-300 text-xs"),
	).WithFrame(340, 250, 320, 130)

	// Info panel showing animation features
	infoPanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		retained.Text("Animation Features:", "text-white text-sm"),
		retained.Text("• Automatic 60 FPS mode switching", "text-gray-400 text-xs"),
		retained.Text("• Easing: cubic, back, elastic, bounce", "text-gray-400 text-xs"),
		retained.Text("• Properties: color, size, position, opacity", "text-gray-400 text-xs"),
		retained.Text("• Looping + OnComplete callbacks", "text-gray-400 text-xs"),
		retained.Text("• Single FFI call per frame", "text-gray-400 text-xs"),
	).WithFrame(340, 400, 320, 140)

	// Color palette demo
	colorPanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		retained.Text("Color Palette (Tailwind)", "text-white text-sm"),
		retained.HStack("gap-2",
			retained.Container("bg-red-500 rounded w-[35px] h-[35px]"),
			retained.Container("bg-orange-500 rounded w-[35px] h-[35px]"),
			retained.Container("bg-yellow-500 rounded w-[35px] h-[35px]"),
			retained.Container("bg-green-500 rounded w-[35px] h-[35px]"),
			retained.Container("bg-blue-500 rounded w-[35px] h-[35px]"),
			retained.Container("bg-purple-500 rounded w-[35px] h-[35px]"),
		),
	).WithFrame(340, 560, 320, 90)

	// ============ Column 3 (x=680, width=320) ============

	// FLEXBOX DEMO - showcase new layout features!
	flexDemoPanel := retained.Container("bg-emerald-900 rounded-xl flex flex-col gap-2 p-4").
		WithFrame(680, 90, 320, 180)

	flexTitle := retained.Text("Flexbox Layout Demo", "text-white text-lg").
		WithSize(300, 24)

	// Row of boxes using flex-row with justify-between
	flexRow := retained.Container("flex flex-row justify-between items-center gap-2").
		WithSize(280, 50)
	for i := 0; i < 4; i++ {
		colors := []string{"bg-red-500", "bg-yellow-500", "bg-green-500", "bg-blue-500"}
		box := retained.Container(colors[i] + " rounded").
			WithSize(60, 40)
		flexRow.AddChild(box)
	}

	// Row with justify-center
	flexCenterRow := retained.Container("flex flex-row justify-center gap-4").
		WithSize(280, 40)
	for i := 0; i < 3; i++ {
		box := retained.Container("bg-purple-500 rounded").
			WithSize(40, 30)
		flexCenterRow.AddChild(box)
	}

	flexLabel := retained.Text("flex flex-row justify-between", "text-emerald-300 text-xs").
		WithSize(280, 16)

	flexDemoPanel.WithChildren(
		flexTitle,
		flexRow,
		flexLabel,
		flexCenterRow,
		retained.Text("flex flex-row justify-center", "text-emerald-300 text-xs").WithSize(280, 16),
	)

	// Available animations list
	availableAnimsPanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		retained.Text("Available Animations:", "text-white text-sm"),
		retained.Text("animate-pulse    - Opacity fade in/out", "text-gray-400 text-xs"),
		retained.Text("animate-bounce   - Vertical bounce", "text-gray-400 text-xs"),
		retained.Text("animate-spin     - Color rotation*", "text-gray-400 text-xs"),
		retained.Text("animate-ping     - Scale + fade out", "text-gray-400 text-xs"),
		retained.Text("animate-none     - Stop animation", "text-gray-400 text-xs"),
		retained.Text("", "text-gray-600 text-xs"),
		retained.Text("*spin uses color shift (no rotation yet)", "text-gray-600 text-xs"),
	).WithFrame(680, 290, 320, 160)

	// Easing functions list
	easingPanel := retained.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		retained.Text("Easing Functions:", "text-white text-sm"),
		retained.Text("EaseLinear       - Constant speed", "text-gray-400 text-xs"),
		retained.Text("EaseInOutCubic   - Smooth start/end", "text-gray-400 text-xs"),
		retained.Text("EaseOutBack      - Overshoot bounce", "text-gray-400 text-xs"),
		retained.Text("EaseOutElastic   - Elastic wobble", "text-gray-400 text-xs"),
		retained.Text("EaseOutBounce    - Bouncing stop", "text-gray-400 text-xs"),
	).WithFrame(680, 470, 320, 140)

	// Coming soon panel
	comingSoonPanel := retained.VStack("bg-gray-700 rounded-lg p-4 gap-1",
		retained.Text("Flexbox Classes:", "text-emerald-400 text-sm"),
		retained.Text("• flex flex-row flex-col", "text-gray-400 text-xs"),
		retained.Text("• justify-start/center/end/between/around", "text-gray-400 text-xs"),
		retained.Text("• items-start/center/end/stretch", "text-gray-400 text-xs"),
		retained.Text("• flex-grow flex-shrink", "text-gray-400 text-xs"),
	).WithFrame(680, 630, 320, 110)

	root.WithChildren(
		header,
		// Column 1 - Animation demo panel and its contents
		animClassPanel,
		animTitle, animSubtitle,
		pulseBox, pulseLabel,
		bounceBox, bounceLabel,
		spinBox, spinLabel,
		pingBox, pingLabel,
		fastBox, fastLabel, fastSyntax,
		// Column 1 - Other panels
		counterPanel, animPanel, statusPanel,
		// Column 2
		animClassCodePanel, codePanel, infoPanel, colorPanel,
		// Column 3
		flexDemoPanel, availableAnimsPanel, easingPanel, comingSoonPanel,
	)

	return root
}

func findWidgets(w *retained.Widget, counterText, animatedBox, animStatusText **retained.Widget) {
	if data := w.Data(); data != nil {
		switch data {
		case "counterText":
			*counterText = w
		case "animatedBox":
			*animatedBox = w
		case "animStatusText":
			*animStatusText = w
		}
	}
	for _, child := range w.Children() {
		findWidgets(child, counterText, animatedBox, animStatusText)
	}
}
