// Example demonstrating Tailwind CSS class integration with retained mode
// and the new animation system for automatic 60 FPS mode switching.
// Now includes class-based animations (animate-pulse, animate-bounce, etc.)!
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

func main() {
	config := ctd.DefaultLoopConfig()
	loop := ctd.NewLoop(config)
	tree := loop.Tree()
	anims := loop.Animations()

	root := buildUI()
	tree.SetRoot(root)

	// Initialize animations from animate-* classes in the widget tree
	// This automatically starts looping animations like animate-pulse, animate-bounce
	loop.InitAnimations()

	var clickCount int
	var counterText *ctd.Widget
	var animatedBox *ctd.Widget
	var animStatusText *ctd.Widget

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
					Easing(ctd.EaseOutBack).
					OnComplete(func() {
						// Shrink back
						animatedBox.Animate(anims).
							Duration(150 * time.Millisecond).
							Easing(ctd.EaseOutCubic).
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
	loop.OnFrame(func(frame *ctd.Frame) {
		activeCount := anims.Count()

		// Only draw FPS overlay when animations are active
		// This allows the system to drop to event-driven mode when idle
		if activeCount > 0 {
			fps := 1.0 / frame.DeltaTime
			frame.DrawText(fmt.Sprintf("FPS: %.1f", fps), 900, 10, 14, ctd.ColorWhite)
			frame.DrawText(fmt.Sprintf("Active anims: %d", activeCount), 900, 30, 12, ctd.ColorGray400)
			frame.DrawText(fmt.Sprintf("Cache: %d styles", ctd.StyleCacheSize()), 900, 50, 12, ctd.ColorGray400)
		}

		// Update status text (this is a retained widget, not immediate draw)
		if animStatusText != nil {
			if activeCount > 0 {
				animStatusText.SetText(fmt.Sprintf("60 FPS mode (%d anims)", activeCount))
				animStatusText.SetTextColor(ctd.ColorGreen400)
			} else {
				animStatusText.SetText("Event-driven mode (idle)")
				animStatusText.SetTextColor(ctd.ColorGray500)
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
func buildUI() *ctd.Widget {
	// Root container with Tailwind classes
	root := ctd.Container("bg-gray-900").
		WithSize(1024, 768)

	// Header with Tailwind styling - spans full width
	header := ctd.HStack("bg-gray-800 rounded-lg p-4",
		ctd.Text("Tailwind + Animation Demo", "text-white text-2xl"),
	).WithFrame(20, 20, 984, 50)

	// ============ Column 1 (x=20, width=300) ============

	// Class-based animations demo panel - FEATURED at top!
	// Using absolute positioned boxes for reliable layout with animations
	animClassPanel := ctd.Container("bg-indigo-900 rounded-xl").
		WithFrame(20, 90, 300, 220)

	// Title and subtitle
	animTitle := ctd.Text("Class-Based Animations", "text-white text-lg").
		WithFrame(36, 106, 200, 24)
	animSubtitle := ctd.Text("Just add animate-* classes!", "text-indigo-300 text-xs").
		WithFrame(36, 132, 200, 16)

	// Row of animated boxes - each positioned absolutely with spacing
	// animate-pulse - opacity pulses like a heartbeat
	pulseBox := ctd.Container("bg-blue-500 rounded-lg animate-pulse").
		WithFrame(36, 156, 50, 50)
	pulseLabel := ctd.Text("pulse", "text-gray-400 text-xs").
		WithFrame(36, 210, 50, 16)

	// animate-bounce - bounces up and down
	bounceBox := ctd.Container("bg-green-500 rounded-lg animate-bounce").
		WithFrame(96, 156, 50, 50)
	bounceLabel := ctd.Text("bounce", "text-gray-400 text-xs").
		WithFrame(92, 210, 50, 16)

	// animate-spin - rotates (color shift placeholder)
	spinBox := ctd.Container("bg-purple-500 rounded-lg animate-spin").
		WithFrame(156, 156, 50, 50)
	spinLabel := ctd.Text("spin", "text-gray-400 text-xs").
		WithFrame(160, 210, 50, 16)

	// animate-ping - scales up and fades
	pingBox := ctd.Container("bg-red-500 rounded-lg animate-ping").
		WithFrame(216, 156, 50, 50)
	pingLabel := ctd.Text("ping", "text-gray-400 text-xs").
		WithFrame(220, 210, 50, 16)

	// Custom animation row - fast pulse with elastic easing!
	fastBox := ctd.Container("bg-yellow-500 rounded-lg animate-[pulse_500ms_elastic]").
		WithFrame(36, 236, 50, 50)
	fastLabel := ctd.Text("fast", "text-gray-400 text-xs").
		WithFrame(36, 290, 50, 16)
	fastSyntax := ctd.Text("animate-[pulse_500ms_elastic]", "text-indigo-200 text-xs").
		WithFrame(96, 254, 200, 16)

	// Counter panel
	counterPanel := ctd.VStack("bg-blue-900 rounded-xl p-4 gap-2",
		ctd.Text("Interactive Counter", "text-gray-300 text-sm"),
		ctd.Text("Clicks: 0", "text-white text-2xl").
			WithData("counterText"),
		ctd.Text("(click anywhere to bounce)", "text-gray-500 text-xs"),
	).WithFrame(20, 310, 300, 110)

	// Animated box panel - click to animate
	animPanel := ctd.VStack("bg-blue-900 rounded-xl p-4 gap-2",
		ctd.Text("Click-Triggered Animation", "text-gray-300 text-sm"),
		ctd.Container("bg-blue-500 rounded-lg w-[80px] h-[80px]").
			WithData("animatedBox"),
		ctd.Text("Bounces on click", "text-gray-500 text-xs"),
	).WithFrame(20, 440, 300, 150)

	// Animation status panel
	statusPanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		ctd.Text("Render Mode:", "text-white text-sm"),
		ctd.Text("Checking...", "text-gray-500 text-base").
			WithData("animStatusText"),
		ctd.Text("When animations active: 60 FPS", "text-gray-600 text-xs"),
		ctd.Text("When idle: event-driven", "text-gray-600 text-xs"),
	).WithFrame(20, 610, 300, 110)

	// ============ Column 2 (x=340, width=320) ============

	// Code example for class-based animations
	animClassCodePanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		ctd.Text("Class-Based Usage:", "text-yellow-400 text-sm"),
		ctd.Text("ctd.Container(", "text-gray-300 text-xs"),
		ctd.Text("  \"bg-blue-500 animate-pulse\"", "text-green-400 text-xs"),
		ctd.Text(")", "text-gray-300 text-xs"),
		ctd.Text("", "text-gray-300 text-xs"),
		ctd.Text("loop.InitAnimations() // auto-start", "text-gray-400 text-xs"),
	).WithFrame(340, 90, 320, 140)

	// Programmatic animation example
	codePanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		ctd.Text("Programmatic Animation:", "text-yellow-400 text-sm"),
		ctd.Text("box.Animate(anims).", "text-gray-300 text-xs"),
		ctd.Text("  Duration(200*time.Millisecond).", "text-gray-300 text-xs"),
		ctd.Text("  Easing(EaseOutBack).", "text-gray-300 text-xs"),
		ctd.Text("  Size(120, 120)", "text-gray-300 text-xs"),
	).WithFrame(340, 250, 320, 130)

	// Info panel showing animation features
	infoPanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		ctd.Text("Animation Features:", "text-white text-sm"),
		ctd.Text("• Automatic 60 FPS mode switching", "text-gray-400 text-xs"),
		ctd.Text("• Easing: cubic, back, elastic, bounce", "text-gray-400 text-xs"),
		ctd.Text("• Properties: color, size, position, opacity", "text-gray-400 text-xs"),
		ctd.Text("• Looping + OnComplete callbacks", "text-gray-400 text-xs"),
		ctd.Text("• Single FFI call per frame", "text-gray-400 text-xs"),
	).WithFrame(340, 400, 320, 140)

	// Color palette demo
	colorPanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-2",
		ctd.Text("Color Palette (Tailwind)", "text-white text-sm"),
		ctd.HStack("gap-2",
			ctd.Container("bg-red-500 rounded w-[35px] h-[35px]"),
			ctd.Container("bg-orange-500 rounded w-[35px] h-[35px]"),
			ctd.Container("bg-yellow-500 rounded w-[35px] h-[35px]"),
			ctd.Container("bg-green-500 rounded w-[35px] h-[35px]"),
			ctd.Container("bg-blue-500 rounded w-[35px] h-[35px]"),
			ctd.Container("bg-purple-500 rounded w-[35px] h-[35px]"),
		),
	).WithFrame(340, 560, 320, 90)

	// ============ Column 3 (x=680, width=320) ============

	// FLEXBOX DEMO - showcase new layout features!
	flexDemoPanel := ctd.Container("bg-emerald-900 rounded-xl flex flex-col gap-2 p-4").
		WithFrame(680, 90, 320, 180)

	flexTitle := ctd.Text("Flexbox Layout Demo", "text-white text-lg").
		WithSize(300, 24)

	// Row of boxes using flex-row with justify-between
	flexRow := ctd.Container("flex flex-row justify-between items-center gap-2").
		WithSize(280, 50)
	for i := 0; i < 4; i++ {
		colors := []string{"bg-red-500", "bg-yellow-500", "bg-green-500", "bg-blue-500"}
		box := ctd.Container(colors[i] + " rounded").
			WithSize(60, 40)
		flexRow.AddChild(box)
	}

	// Row with justify-center
	flexCenterRow := ctd.Container("flex flex-row justify-center gap-4").
		WithSize(280, 40)
	for i := 0; i < 3; i++ {
		box := ctd.Container("bg-purple-500 rounded").
			WithSize(40, 30)
		flexCenterRow.AddChild(box)
	}

	flexLabel := ctd.Text("flex flex-row justify-between", "text-emerald-300 text-xs").
		WithSize(280, 16)

	flexDemoPanel.WithChildren(
		flexTitle,
		flexRow,
		flexLabel,
		flexCenterRow,
		ctd.Text("flex flex-row justify-center", "text-emerald-300 text-xs").WithSize(280, 16),
	)

	// Available animations list
	availableAnimsPanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		ctd.Text("Available Animations:", "text-white text-sm"),
		ctd.Text("animate-pulse    - Opacity fade in/out", "text-gray-400 text-xs"),
		ctd.Text("animate-bounce   - Vertical bounce", "text-gray-400 text-xs"),
		ctd.Text("animate-spin     - Color rotation*", "text-gray-400 text-xs"),
		ctd.Text("animate-ping     - Scale + fade out", "text-gray-400 text-xs"),
		ctd.Text("animate-none     - Stop animation", "text-gray-400 text-xs"),
		ctd.Text("", "text-gray-600 text-xs"),
		ctd.Text("*spin uses color shift (no rotation yet)", "text-gray-600 text-xs"),
	).WithFrame(680, 290, 320, 160)

	// Easing functions list
	easingPanel := ctd.VStack("bg-gray-800 rounded-lg p-4 gap-1",
		ctd.Text("Easing Functions:", "text-white text-sm"),
		ctd.Text("EaseLinear       - Constant speed", "text-gray-400 text-xs"),
		ctd.Text("EaseInOutCubic   - Smooth start/end", "text-gray-400 text-xs"),
		ctd.Text("EaseOutBack      - Overshoot bounce", "text-gray-400 text-xs"),
		ctd.Text("EaseOutElastic   - Elastic wobble", "text-gray-400 text-xs"),
		ctd.Text("EaseOutBounce    - Bouncing stop", "text-gray-400 text-xs"),
	).WithFrame(680, 470, 320, 140)

	// Coming soon panel
	comingSoonPanel := ctd.VStack("bg-gray-700 rounded-lg p-4 gap-1",
		ctd.Text("Flexbox Classes:", "text-emerald-400 text-sm"),
		ctd.Text("• flex flex-row flex-col", "text-gray-400 text-xs"),
		ctd.Text("• justify-start/center/end/between/around", "text-gray-400 text-xs"),
		ctd.Text("• items-start/center/end/stretch", "text-gray-400 text-xs"),
		ctd.Text("• flex-grow flex-shrink", "text-gray-400 text-xs"),
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

func findWidgets(w *ctd.Widget, counterText, animatedBox, animStatusText **ctd.Widget) {
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
