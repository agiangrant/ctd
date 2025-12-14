// Example demonstrating the retained mode API with game loop
package main

import (
	"fmt"
	"log"
	"math"
	"runtime"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	// Lock the main goroutine to the main OS thread.
	// This is required on macOS because winit's EventLoop must be created
	// on the main thread.
	runtime.LockOSThread()
}

func main() {
	// Create the game loop with default config (60 FPS)
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)

	// Get the tree to build our UI
	tree := loop.Tree()

	// Create the widget tree
	root := buildUI()
	tree.SetRoot(root)

	// Track some state for animations
	var animAngle float64
	var clickCount int

	// References to widgets we want to update
	var counterText *retained.Widget
	var spinningBox *retained.Widget
	var mouseLabel *retained.Widget

	// Find our interactive widgets
	findWidgets(root, &counterText, &spinningBox, &mouseLabel)

	// Handle resize
	loop.OnResize(func(width, height float32) {
		// Update root size
		root.SetSize(width, height)
	})

	// Handle input events
	loop.OnEvent(func(event ffi.Event) bool {
		switch event.Type {
		case ffi.EventMouseMoved:
			if mouseLabel != nil {
				mouseLabel.SetText(fmt.Sprintf("Mouse: %.0f, %.0f", event.MouseX(), event.MouseY()))
			}
			return true

		case ffi.EventMousePressed:
			clickCount++
			if counterText != nil {
				counterText.SetText(fmt.Sprintf("Clicks: %d", clickCount))
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

	// Frame callback - runs at 60 FPS
	loop.OnFrame(func(frame *retained.Frame) {
		// Update animation
		animAngle += frame.DeltaTime * 2.0 // 2 radians per second

		// Animate the spinning box
		if spinningBox != nil {
			// Oscillate the color
			t := (math.Sin(animAngle) + 1) / 2 // 0 to 1
			r := uint8(100 + t*155)
			g := uint8(50 + t*100)
			b := uint8(200 - t*100)
			spinningBox.SetBackgroundColor(retained.RGBA(r, g, b, 255))

			// Oscillate the size
			scale := float32(0.8 + 0.4*math.Sin(animAngle*0.5))
			baseSize := float32(100)
			spinningBox.SetSize(baseSize*scale, baseSize*scale)
		}

		// Draw some immediate mode overlays
		// FPS counter in top-right
		fps := 1.0 / frame.DeltaTime
		frame.DrawText(
			fmt.Sprintf("FPS: %.1f", fps),
			float32(900), 10, 14, retained.ColorWhite,
		)

		// Frame number
		frame.DrawText(
			fmt.Sprintf("Frame: %d", frame.Number),
			float32(900), 30, 12, retained.ColorGray400,
		)

		// Draw a particle-like effect (immediate mode)
		numParticles := 20
		for i := 0; i < numParticles; i++ {
			angle := animAngle + float64(i)*math.Pi*2/float64(numParticles)
			radius := float32(50 + 20*math.Sin(animAngle*3+float64(i)*0.5))
			px := float32(700) + radius*float32(math.Cos(angle))
			py := float32(400) + radius*float32(math.Sin(angle))

			alpha := uint8(100 + 100*math.Sin(animAngle*2+float64(i)))
			frame.DrawRect(px-3, py-3, 6, 6, retained.RGBA(255, 200, 100, alpha), 3)
		}
	})

	// Run the application
	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Retained Mode Demo"
	appConfig.Width = 1024
	appConfig.Height = 768

	log.Println("Starting retained mode demo...")
	log.Println("  - Click anywhere to increment counter")
	log.Println("  - Press ESC to quit")
	log.Println("  - Watch the spinning box and particle effects")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

// buildUI creates the widget tree
func buildUI() *retained.Widget {
	// Root container
	root := retained.Container("").
		WithSize(1024, 768).
		WithBackground(retained.Hex("#1a1a2e"))

	// Header - use padding for internal spacing
	header := retained.HStack("",
		retained.Text("Retained Mode Demo", "").
			WithTextStyle(retained.ColorWhite, 24),
	).
		WithFrame(20, 20, 400, 50).
		WithBackground(retained.Hex("#16213e")).
		WithCornerRadius(8).
		WithPadding(12)

	// Counter panel
	counterPanel := retained.VStack("",
		retained.Text("Interactive Counter", "").
			WithTextStyle(retained.ColorGray300, 16),
		retained.Text("Clicks: 0", ""). // Will be updated
						WithTextStyle(retained.ColorWhite, 32).
						WithData("counterText"), // Tag for finding later
		retained.Text("(click anywhere)", "").
			WithTextStyle(retained.ColorGray500, 12),
	).
		WithFrame(20, 90, 220, 150).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)

	// Animated box panel
	animPanel := retained.VStack("",
		retained.Text("Animated Widget", "").
			WithTextStyle(retained.ColorGray300, 14),
		retained.Container("").
			WithSize(100, 100).
			WithBackground(retained.ColorBlue500).
			WithCornerRadius(8).
			WithData("spinningBox"), // Tag for finding later
	).
		WithFrame(20, 260, 220, 160).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)

	// Mouse tracking panel
	mousePanel := retained.VStack("",
		retained.Text("Mouse Position", "").
			WithTextStyle(retained.ColorGray300, 14),
		retained.Text("Mouse: 0, 0", "").
			WithTextStyle(retained.ColorGreen400, 18).
			WithData("mouseLabel"),
	).
		WithFrame(20, 440, 220, 80).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(6)

	// Info panel
	infoPanel := retained.VStack("",
		retained.Text("How it works:", "").
			WithTextStyle(retained.ColorWhite, 16),
		retained.Text("• Retained widgets update via channels", "").
			WithTextStyle(retained.ColorGray400, 12),
		retained.Text("• Game loop batches updates at 60 FPS", "").
			WithTextStyle(retained.ColorGray400, 12),
		retained.Text("• Immediate draws (particles, FPS) overlay", "").
			WithTextStyle(retained.ColorGray400, 12),
		retained.Text("• Single FFI call per frame", "").
			WithTextStyle(retained.ColorGray400, 12),
	).
		WithFrame(20, 540, 300, 180).
		WithBackground(retained.Hex("#16213e")).
		WithCornerRadius(8).
		WithPadding(16).
		WithGap(6)

	// Particle area label - positioned absolutely
	particleLabel := retained.Text("Particle Effect Area →", "").
		WithFrame(500, 380, 180, 30).
		WithTextStyle(retained.ColorGray500, 14).
		WithPositionMode(retained.PositionAbsolute)

	// Add all to root
	root.
		WithChildren(header, counterPanel, animPanel, mousePanel, infoPanel, particleLabel)

	return root
}

// findWidgets recursively finds widgets by their data tags
func findWidgets(w *retained.Widget, counterText, spinningBox, mouseLabel **retained.Widget) {
	if data := w.Data(); data != nil {
		switch data {
		case "counterText":
			*counterText = w
		case "spinningBox":
			*spinningBox = w
		case "mouseLabel":
			*mouseLabel = w
		}
	}

	for _, child := range w.Children() {
		findWidgets(child, counterText, spinningBox, mouseLabel)
	}
}

// Demonstrate concurrent updates from goroutines
func startBackgroundUpdates(widget *retained.Widget) {
	go func() {
		ticker := time.NewTicker(100 * time.Millisecond)
		defer ticker.Stop()

		var value float32
		for range ticker.C {
			value += 0.1
			if value > 1 {
				value = 0
			}
			// This update flows through the sharded channel pool
			// and gets batched with the next frame
			widget.SetOpacity(0.5 + value*0.5)
		}
	}()
}
