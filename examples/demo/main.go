// Example demonstrating the retained mode API with game loop
package main

import (
	"fmt"
	"log"
	"math"
	"runtime"
	"time"

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

func init() {
	// Lock the main goroutine to the main OS thread.
	// This is required on macOS because winit's EventLoop must be created
	// on the main thread.
	runtime.LockOSThread()
}

func main() {
	// Create the game loop with default config (60 FPS)
	config := ctd.DefaultLoopConfig()
	loop := ctd.NewLoop(config)

	// Get the tree to build our UI
	tree := loop.Tree()

	// Create the widget tree
	root := buildUI()
	tree.SetRoot(root)

	// Track some state for animations
	var animAngle float64
	var clickCount int

	// References to widgets we want to update
	var counterText *ctd.Widget
	var spinningBox *ctd.Widget
	var mouseLabel *ctd.Widget

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
	loop.OnFrame(func(frame *ctd.Frame) {
		// Update animation
		animAngle += frame.DeltaTime * 2.0 // 2 radians per second

		// Animate the spinning box
		if spinningBox != nil {
			// Oscillate the color
			t := (math.Sin(animAngle) + 1) / 2 // 0 to 1
			r := uint8(100 + t*155)
			g := uint8(50 + t*100)
			b := uint8(200 - t*100)
			spinningBox.SetBackgroundColor(ctd.RGBA(r, g, b, 255))

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
			float32(900), 10, 14, ctd.ColorWhite,
		)

		// Frame number
		frame.DrawText(
			fmt.Sprintf("Frame: %d", frame.Number),
			float32(900), 30, 12, ctd.ColorGray400,
		)

		// Draw a particle-like effect (immediate mode)
		numParticles := 20
		for i := 0; i < numParticles; i++ {
			angle := animAngle + float64(i)*math.Pi*2/float64(numParticles)
			radius := float32(50 + 20*math.Sin(animAngle*3+float64(i)*0.5))
			px := float32(700) + radius*float32(math.Cos(angle))
			py := float32(400) + radius*float32(math.Sin(angle))

			alpha := uint8(100 + 100*math.Sin(animAngle*2+float64(i)))
			frame.DrawRect(px-3, py-3, 6, 6, ctd.RGBA(255, 200, 100, alpha), 3)
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
func buildUI() *ctd.Widget {
	// Root container
	root := ctd.Container("").
		WithSize(1024, 768).
		WithBackground(ctd.Hex("#1a1a2e"))

	// Header - use padding for internal spacing
	header := ctd.HStack("",
		ctd.Text("Retained Mode Demo", "").
			WithTextStyle(ctd.ColorWhite, 24),
	).
		WithFrame(20, 20, 400, 50).
		WithBackground(ctd.Hex("#16213e")).
		WithCornerRadius(8).
		WithPadding(12)

	// Counter panel
	counterPanel := ctd.VStack("",
		ctd.Text("Interactive Counter", "").
			WithTextStyle(ctd.ColorGray300, 16),
		ctd.Text("Clicks: 0", ""). // Will be updated
						WithTextStyle(ctd.ColorWhite, 32).
						WithData("counterText"), // Tag for finding later
		ctd.Text("(click anywhere)", "").
			WithTextStyle(ctd.ColorGray500, 12),
	).
		WithFrame(20, 90, 220, 150).
		WithBackground(ctd.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)

	// Animated box panel
	animPanel := ctd.VStack("",
		ctd.Text("Animated Widget", "").
			WithTextStyle(ctd.ColorGray300, 14),
		ctd.Container("").
			WithSize(100, 100).
			WithBackground(ctd.ColorBlue500).
			WithCornerRadius(8).
			WithData("spinningBox"), // Tag for finding later
	).
		WithFrame(20, 260, 220, 160).
		WithBackground(ctd.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)

	// Mouse tracking panel
	mousePanel := ctd.VStack("",
		ctd.Text("Mouse Position", "").
			WithTextStyle(ctd.ColorGray300, 14),
		ctd.Text("Mouse: 0, 0", "").
			WithTextStyle(ctd.ColorGreen400, 18).
			WithData("mouseLabel"),
	).
		WithFrame(20, 440, 220, 80).
		WithBackground(ctd.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(6)

	// Info panel
	infoPanel := ctd.VStack("",
		ctd.Text("How it works:", "").
			WithTextStyle(ctd.ColorWhite, 16),
		ctd.Text("• Retained widgets update via channels", "").
			WithTextStyle(ctd.ColorGray400, 12),
		ctd.Text("• Game loop batches updates at 60 FPS", "").
			WithTextStyle(ctd.ColorGray400, 12),
		ctd.Text("• Immediate draws (particles, FPS) overlay", "").
			WithTextStyle(ctd.ColorGray400, 12),
		ctd.Text("• Single FFI call per frame", "").
			WithTextStyle(ctd.ColorGray400, 12),
	).
		WithFrame(20, 540, 300, 180).
		WithBackground(ctd.Hex("#16213e")).
		WithCornerRadius(8).
		WithPadding(16).
		WithGap(6)

	// Particle area label - positioned absolutely
	particleLabel := ctd.Text("Particle Effect Area →", "").
		WithFrame(500, 380, 180, 30).
		WithTextStyle(ctd.ColorGray500, 14).
		WithPositionMode(ctd.PositionAbsolute)

	// Add all to root
	root.
		WithChildren(header, counterPanel, animPanel, mousePanel, infoPanel, particleLabel)

	return root
}

// findWidgets recursively finds widgets by their data tags
func findWidgets(w *ctd.Widget, counterText, spinningBox, mouseLabel **ctd.Widget) {
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
func startBackgroundUpdates(widget *ctd.Widget) {
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
