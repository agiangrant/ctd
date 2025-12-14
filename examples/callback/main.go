package main

import (
	"bytes"
	"fmt"
	goimage "image"
	"image/color"
	"image/png"
	"log"

	"github.com/agiangrant/centered/internal/ffi"
)

// Application state
type AppState struct {
	mouseX, mouseY float64
	width, height  float64
	frameCount     int
	// Image textures
	testImageID ffi.TextureID
	imageLoaded bool
	// Text input state
	inputText       string
	cursorPos       int
	inputFocused    bool
	lastKeyInfo     string // Debug: last key event info
	// Scroll view state
	scrollY float32 // Vertical scroll offset (0 = top)
}

// generateTestImage creates a colorful gradient test pattern as PNG bytes
func generateTestImage(width, height int) []byte {
	img := goimage.NewRGBA(goimage.Rect(0, 0, width, height))

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			// Create a colorful gradient pattern
			r := uint8(x * 255 / width)
			g := uint8(y * 255 / height)
			b := uint8((x + y) * 255 / (width + height))
			img.Set(x, y, color.RGBA{r, g, b, 255})
		}
	}

	var buf bytes.Buffer
	png.Encode(&buf, img)
	return buf.Bytes()
}

// Helper to create a string pointer
func strPtr(s string) *string {
	return &s
}

func main() {
	fmt.Println("Centered Callback Example")
	fmt.Println("Version:", ffi.Version())

	// Create app config
	config := ffi.DefaultAppConfig()
	config.Title = "Centered - Callback Example"
	config.Width = 800
	config.Height = 600

	// Application state
	state := &AppState{
		width:        float64(config.Width),
		height:       float64(config.Height),
		inputText:    "",
		inputFocused: true, // Start with input focused
	}

	// Run the app with our event handler
	// This blocks until the window is closed
	err := ffi.Run(config, func(event ffi.Event) ffi.FrameResponse {
		return handleEvent(state, event)
	})

	if err != nil {
		log.Fatalf("App error: %v", err)
	}

	fmt.Println("App exited cleanly")
}

func handleEvent(state *AppState, event ffi.Event) ffi.FrameResponse {
	switch event.Type {
	case ffi.EventReady:
		fmt.Println("Window ready!")
		// Load test image
		if !state.imageLoaded {
			pngData := generateTestImage(128, 128)
			textureID, err := ffi.LoadImage(pngData)
			if err != nil {
				fmt.Printf("Failed to load test image: %v\n", err)
			} else {
				state.testImageID = textureID
				state.imageLoaded = true
				fmt.Printf("Loaded test image with texture ID: %d\n", textureID)
			}
		}
		return ffi.FrameResponse{RequestRedraw: true}

	case ffi.EventResized:
		state.width = event.Data1
		state.height = event.Data2
		fmt.Printf("Window resized to %.0fx%.0f\n", state.width, state.height)
		return ffi.FrameResponse{RequestRedraw: true}

	case ffi.EventRedrawRequested:
		state.frameCount++
		return renderFrame(state)

	case ffi.EventMouseMoved:
		state.mouseX = event.Data1
		state.mouseY = event.Data2
		// Request redraw to show mouse follower
		return ffi.FrameResponse{RequestRedraw: true}

	case ffi.EventMousePressed:
		button := int(event.Data1)
		fmt.Printf("Mouse button %d pressed at (%.0f, %.0f)\n", button, state.mouseX, state.mouseY)
		return ffi.FrameResponse{}

	case ffi.EventMouseReleased:
		button := int(event.Data1)
		fmt.Printf("Mouse button %d released\n", button)
		return ffi.FrameResponse{}

	case ffi.EventMouseWheel:
		_, deltaY := event.ScrollDelta()
		// winit provides normalized scroll deltas that match native behavior
		state.scrollY += float32(deltaY)
		// Clamp scroll position (content height is 600, viewport is ~200)
		const contentHeight = 600.0
		const viewportHeight = 200.0
		maxScroll := float32(contentHeight - viewportHeight)
		if state.scrollY < 0 {
			state.scrollY = 0
		}
		if state.scrollY > maxScroll {
			state.scrollY = maxScroll
		}
		return ffi.FrameResponse{RequestRedraw: true}

	case ffi.EventKeyPressed:
		keycode := ffi.Keycode(event.Keycode())
		mods := event.Modifiers()
		state.lastKeyInfo = fmt.Sprintf("Key: %d, Mods: %d", keycode, mods)

		// ESC to quit
		if keycode == ffi.KeyEscape {
			ffi.RequestExit()
			return ffi.FrameResponse{}
		}

		// Handle text editing keys
		if state.inputFocused {
			switch keycode {
			case ffi.KeyBackspace:
				if len(state.inputText) > 0 && state.cursorPos > 0 {
					// Delete character before cursor
					runes := []rune(state.inputText)
					state.inputText = string(runes[:state.cursorPos-1]) + string(runes[state.cursorPos:])
					state.cursorPos--
				}
			case ffi.KeyDelete:
				runes := []rune(state.inputText)
				if state.cursorPos < len(runes) {
					// Delete character after cursor
					state.inputText = string(runes[:state.cursorPos]) + string(runes[state.cursorPos+1:])
				}
			case ffi.KeyLeft:
				if state.cursorPos > 0 {
					// Move cursor left, or to beginning with Cmd/Ctrl
					if mods&ffi.ModSuper != 0 || mods&ffi.ModCtrl != 0 {
						state.cursorPos = 0
					} else {
						state.cursorPos--
					}
				}
			case ffi.KeyRight:
				runes := []rune(state.inputText)
				if state.cursorPos < len(runes) {
					// Move cursor right, or to end with Cmd/Ctrl
					if mods&ffi.ModSuper != 0 || mods&ffi.ModCtrl != 0 {
						state.cursorPos = len(runes)
					} else {
						state.cursorPos++
					}
				}
			case ffi.KeyHome:
				state.cursorPos = 0
			case ffi.KeyEnd:
				state.cursorPos = len([]rune(state.inputText))
			case ffi.KeyA:
				// Cmd+A / Ctrl+A: Select all (for now, just move cursor to end)
				if mods&ffi.ModSuper != 0 || mods&ffi.ModCtrl != 0 {
					state.cursorPos = len([]rune(state.inputText))
				}
			}
		}
		return ffi.FrameResponse{RequestRedraw: true}

	case ffi.EventCharInput:
		// Character input - add to text buffer
		if state.inputFocused {
			char := event.Char()
			// Only add printable characters (skip control chars)
			if char >= 32 && char != 127 {
				// Insert at cursor position
				runes := []rune(state.inputText)
				newRunes := make([]rune, 0, len(runes)+1)
				newRunes = append(newRunes, runes[:state.cursorPos]...)
				newRunes = append(newRunes, char)
				newRunes = append(newRunes, runes[state.cursorPos:]...)
				state.inputText = string(newRunes)
				state.cursorPos++
			}
		}
		return ffi.FrameResponse{RequestRedraw: true}

	case ffi.EventCloseRequested:
		fmt.Println("Close requested")
		return ffi.FrameResponse{}

	default:
		return ffi.FrameResponse{}
	}
}

func renderFrame(state *AppState) ffi.FrameResponse {
	// Text input font settings
	const inputFontName = "system"
	const inputFontSize = float32(16)

	// Get scale factor for HiDPI displays (e.g., 2.0 on Retina)
	scaleFactor := float32(ffi.GetScaleFactor())

	// Calculate cursor X position using text measurement
	// We need to measure at scaled font size since the renderer applies scale factor
	cursorX := float32(0)
	if state.cursorPos > 0 {
		scaledFontSize := inputFontSize * scaleFactor
		scaledWidth := ffi.MeasureTextToCursor(state.inputText, state.cursorPos, inputFontName, scaledFontSize)
		// Convert back to logical pixels
		cursorX = scaledWidth / scaleFactor
	}

	// Get font metrics for cursor height
	metrics := ffi.MeasureText("M", inputFontName, inputFontSize)
	cursorHeight := metrics.Height

	commands := []ffi.RenderCommand{
		// Clear background (dark gray-blue)
		ffi.Clear(26, 26, 38, 255),

		// ========== Text Input Demo (center) ==========
		// Card background
		ffi.RoundedRect(
			float32(state.width/2-175), float32(state.height/2-120),
			350, 240,
			ffi.RGB(45, 55, 72), // gray-800
			12,
		),

		// Title bar
		ffi.RoundedRect(
			float32(state.width/2-165), float32(state.height/2-110),
			330, 36,
			ffi.RGB(66, 153, 225), // blue-500
			8,
		),
		ffi.Text(
			"Text Input Demo",
			float32(state.width/2-155), float32(state.height/2-102),
			20, ffi.RGB(255, 255, 255),
		),

		// Input field label
		ffi.Text(
			"Type below (use arrow keys, backspace, delete):",
			float32(state.width/2-165), float32(state.height/2-65),
			13, ffi.RGB(148, 163, 184),
		),

		// Input field background
		ffi.RoundedRect(
			float32(state.width/2-165), float32(state.height/2-45),
			330, 40,
			ffi.RGB(30, 41, 59), // darker slate
			6,
		),

		// Input field border (indicates focus)
		ffi.RoundedRect(
			float32(state.width/2-166), float32(state.height/2-46),
			332, 42,
			ffi.RGB(99, 179, 237), // blue-400 border when focused
			6,
		),

		// Input field background (on top of border)
		ffi.RoundedRect(
			float32(state.width/2-164), float32(state.height/2-44),
			328, 38,
			ffi.RGB(30, 41, 59),
			5,
		),

		// Input text (without cursor character)
		ffi.TextWithLayout(
			state.inputText,
			float32(state.width/2-155), float32(state.height/2-32),
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			},
			ffi.RGB(226, 232, 240),
			ffi.SingleLineEllipsisLayout(310),
		),

		// Draw cursor as a thin rectangle (using text measurement for position)
		ffi.Rect(
			float32(state.width/2-155)+cursorX, float32(state.height/2-32),
			2, cursorHeight,
			ffi.RGB(99, 179, 237), // blue cursor
		),

		// Character count
		ffi.Text(
			fmt.Sprintf("Length: %d chars, Cursor: %d", len([]rune(state.inputText)), state.cursorPos),
			float32(state.width/2-165), float32(state.height/2+5),
			12, ffi.RGB(100, 116, 139),
		),

		// Last key info (debug)
		ffi.Text(
			fmt.Sprintf("Last: %s", state.lastKeyInfo),
			float32(state.width/2-165), float32(state.height/2+25),
			11, ffi.RGB(100, 116, 139),
		),

		// Keyboard shortcuts help
		ffi.Text(
			"Shortcuts: Cmd+Left/Right = start/end, ESC = quit",
			float32(state.width/2-165), float32(state.height/2+50),
			11, ffi.RGB(100, 116, 139),
		),

		// Frame counter and mouse position
		ffi.Text(
			fmt.Sprintf("Frame: %d | Mouse: (%.0f, %.0f)", state.frameCount, state.mouseX, state.mouseY),
			float32(state.width/2-165), float32(state.height/2+100),
			12, ffi.RGB(71, 85, 105),
		),

		// Draw mouse follower circle
		ffi.RoundedRect(
			float32(state.mouseX-15), float32(state.mouseY-15),
			30, 30,
			ffi.RGBA(236, 72, 153, 180), // pink-500 with alpha
			15, // fully rounded = circle
		),

		// ========== Clipping Demo (top-left) ==========
		// Background card for clipping showcase
		ffi.RoundedRect(40, 40, 200, 120, ffi.RGB(30, 41, 59), 8),
		ffi.Text("Clipping Demo:", 50, 50, 14, ffi.RGB(148, 163, 184)),

		// Push a clip region - content will be clipped to this box
		ffi.PushClip(50, 70, 180, 80),

		// This rect is larger than the clip region - it will be clipped
		ffi.RoundedRect(40, 60, 220, 60, ffi.RGB(239, 68, 68), 8), // red - overflows left/right

		// This text extends beyond clip - will be clipped
		ffi.Text("This text is clipped at the edges!", 55, 90, 14, ffi.RGB(255, 255, 255)),

		// A circle that partially overflows
		ffi.RoundedRect(180, 100, 80, 80, ffi.RGB(59, 130, 246), 40), // blue circle, partially visible

		// End the clip region
		ffi.PopClip(),

		// This text is outside the clip, so it renders normally
		ffi.Text("(outside clip)", 50, 170, 12, ffi.RGB(100, 116, 139)),

		// Font weight showcase on the right side
		ffi.Text("Font Weights:", float32(state.width-220), 50, 16, ffi.RGB(200, 200, 200)),

		// Thin (100)
		ffi.TextWithFont("Thin (100)", float32(state.width-220), 80,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 100,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Light (300)
		ffi.TextWithFont("Light (300)", float32(state.width-220), 105,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 300,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Regular (400)
		ffi.TextWithFont("Regular (400)", float32(state.width-220), 130,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Medium (500)
		ffi.TextWithFont("Medium (500)", float32(state.width-220), 155,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 500,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Semi Bold (600)
		ffi.TextWithFont("SemiBold (600)", float32(state.width-220), 180,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 600,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Bold (700)
		ffi.TextWithFont("Bold (700)", float32(state.width-220), 205,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 700,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Heavy (800)
		ffi.TextWithFont("Heavy (800)", float32(state.width-220), 230,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 800,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Black (900)
		ffi.TextWithFont("Black (900)", float32(state.width-220), 255,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 900,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Italic showcase
		ffi.Text("Font Styles:", float32(state.width-220), 295, 16, ffi.RGB(200, 200, 200)),

		ffi.TextWithFont("Normal Style", float32(state.width-220), 325,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		ffi.TextWithFont("Italic Style", float32(state.width-220), 350,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleItalic,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		ffi.TextWithFont("Bold Italic", float32(state.width-220), 375,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 700,
				Style:  ffi.FontStyleItalic,
				Size:   16,
			}, ffi.RGB(255, 255, 255)),

		// Letter spacing showcase (center-left)
		ffi.RoundedRect(320, 330, 200, 220, ffi.RGB(30, 41, 59), 8),

		ffi.Text("Letter Spacing:", 330, 340, 14, ffi.RGB(148, 163, 184)),

		// Tight tracking (-0.05em)
		ffi.TextWithLayout("Tight tracking", 330, 370,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   14,
			},
			ffi.RGB(226, 232, 240),
			ffi.TrackingLayout(-0.05),
		),

		// Normal (0em)
		ffi.TextWithLayout("Normal spacing", 330, 400,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   14,
			},
			ffi.RGB(226, 232, 240),
			ffi.TrackingLayout(0),
		),

		// Wide tracking (0.1em)
		ffi.TextWithLayout("Wide tracking", 330, 430,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   14,
			},
			ffi.RGB(167, 243, 208),
			ffi.TrackingLayout(0.1),
		),

		// Very wide tracking (0.2em)
		ffi.TextWithLayout("Very wide", 330, 460,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   14,
			},
			ffi.RGB(253, 186, 116),
			ffi.TrackingLayout(0.2),
		),

		// Word spacing demo
		ffi.Text("Word Spacing:", 330, 495, 12, ffi.RGB(100, 116, 139)),
		ffi.TextWithLayout("Words spaced apart", 330, 515,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   13,
			},
			ffi.RGB(147, 197, 253),
			ffi.SpacedTextLayout(0, 0.5),
		),

		// Multi-line text showcase (bottom left)
		ffi.RoundedRect(40, 330, 260, 220, ffi.RGB(30, 41, 59), 8), // slate-800 background

		ffi.Text("Multi-Line Text:", 50, 340, 14, ffi.RGB(148, 163, 184)),

		// Wrapped text with max width
		ffi.TextWithLayout(
			"This is a long paragraph of text that will automatically wrap to multiple lines when it exceeds the maximum width specified.",
			50, 365,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   14,
			},
			ffi.RGB(226, 232, 240), // slate-200
			ffi.WrappedTextLayout(240),
		),

		// Text with explicit newlines
		ffi.TextWithLayout(
			"Line 1: First line\nLine 2: Second line\nLine 3: Third line",
			50, 460,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   13,
			},
			ffi.RGB(147, 197, 253), // blue-300
			ffi.DefaultTextLayout(),
		),

		// Ellipsis overflow showcase (bottom right)
		ffi.RoundedRect(float32(state.width-280), 410, 260, 180, ffi.RGB(30, 41, 59), 8),

		ffi.Text("Text Overflow:", float32(state.width-270), 420, 14, ffi.RGB(148, 163, 184)),

		// Single line ellipsis
		ffi.Text("Single line:", float32(state.width-270), 450, 12, ffi.RGB(100, 116, 139)),
		ffi.TextWithLayout(
			"This text is too long to fit on one line",
			float32(state.width-270), 468,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   14,
			},
			ffi.RGB(226, 232, 240),
			ffi.SingleLineEllipsisLayout(240),
		),

		// Multi-line ellipsis (max 2 lines)
		ffi.Text("Max 2 lines:", float32(state.width-270), 500, 12, ffi.RGB(100, 116, 139)),
		ffi.TextWithLayout(
			"This is a longer paragraph that will wrap to multiple lines but will be truncated with an ellipsis after two lines of text are displayed.",
			float32(state.width-270), 518,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   13,
			},
			ffi.RGB(167, 243, 208), // emerald-200
			ffi.EllipsisTextLayout(240, 2),
		),

		// Height-based ellipsis
		ffi.Text("Max height (20px = 1 line):", float32(state.width-270), 560, 12, ffi.RGB(100, 116, 139)),
		ffi.TextWithLayout(
			"This text will be truncated based on maximum height constraint rather than line count, useful for fixed-size containers.",
			float32(state.width-270), 578,
			ffi.FontDescriptor{
				Source: ffi.FontSource{System: strPtr("system")},
				Weight: 400,
				Style:  ffi.FontStyleNormal,
				Size:   13,
			},
			ffi.RGB(253, 186, 116), // orange-300
			ffi.EllipsisHeightLayout(240, 20),
		),
	}

	// ========== Image Demo (top center) ==========
	if state.imageLoaded {
		// Background card for image showcase
		commands = append(commands,
			ffi.RoundedRect(260, 40, 220, 150, ffi.RGB(30, 41, 59), 8),
			ffi.Text("Image Demo:", 270, 50, 14, ffi.RGB(148, 163, 184)),

			// Draw the test image at 80x80 logical pixels
			ffi.Image(state.testImageID, 270, 70, 80, 80),

			// Draw scaled versions
			ffi.Image(state.testImageID, 360, 70, 50, 50),
			ffi.Image(state.testImageID, 420, 70, 50, 50),

			// Labels
			ffi.Text("80x80", 290, 155, 10, ffi.RGB(100, 116, 139)),
			ffi.Text("50x50", 370, 125, 10, ffi.RGB(100, 116, 139)),
			ffi.Text("50x50", 430, 125, 10, ffi.RGB(100, 116, 139)),
		)
	}

	// ========== Scroll View Demo (bottom center-right) ==========
	// Viewport position and size
	scrollViewX := float32(540)
	scrollViewY := float32(200)
	scrollViewWidth := float32(220)
	scrollViewHeight := float32(200)

	// Card background for scroll demo
	commands = append(commands,
		ffi.RoundedRect(scrollViewX-10, scrollViewY-30, scrollViewWidth+20, scrollViewHeight+50, ffi.RGB(30, 41, 59), 8),
		ffi.Text("Scroll View Demo:", scrollViewX, scrollViewY-20, 12, ffi.RGB(148, 163, 184)),
		ffi.Text(fmt.Sprintf("Y: %.0f", state.scrollY), scrollViewX+120, scrollViewY-20, 11, ffi.RGB(100, 116, 139)),

		// Scroll view border
		ffi.RoundedRect(scrollViewX-1, scrollViewY-1, scrollViewWidth+2, scrollViewHeight+2, ffi.RGB(59, 130, 246), 4),
		ffi.RoundedRect(scrollViewX, scrollViewY, scrollViewWidth, scrollViewHeight, ffi.RGB(26, 26, 38), 3),

		// Begin scroll view - all content after this will be offset and clipped
		ffi.BeginScrollView(scrollViewX, scrollViewY, scrollViewWidth, scrollViewHeight, 0, state.scrollY),
	)

	// Scrollable content - positioned relative to scroll view (0,0 = top-left of viewport)
	// When scrollY=0, content at y=0 appears at top of viewport
	// When scrollY=50, content at y=0 appears 50px above viewport (off screen)
	contentY := float32(0) // Starting Y position in content space
	for i := 0; i < 20; i++ {
		color := ffi.RGB(uint8(100+i*7), uint8(150-i*3), uint8(200-i*5))
		commands = append(commands,
			ffi.RoundedRect(10, contentY, scrollViewWidth-20, 25, color, 4),
			ffi.Text(fmt.Sprintf("Item %d - Scroll to see more!", i+1), 20, contentY+5, 12, ffi.RGB(255, 255, 255)),
		)
		contentY += 30
	}

	// End scroll view
	commands = append(commands, ffi.EndScrollView())

	// Scroll indicator (drawn outside scroll view)
	scrollIndicatorHeight := scrollViewHeight * scrollViewHeight / 600 // 600 = content height
	scrollIndicatorY := scrollViewY + (state.scrollY / 400 * (scrollViewHeight - scrollIndicatorHeight))
	commands = append(commands,
		ffi.RoundedRect(scrollViewX+scrollViewWidth-6, scrollIndicatorY, 4, scrollIndicatorHeight, ffi.RGBA(255, 255, 255, 100), 2),
	)

	return ffi.FrameResponse{
		ImmediateCommands: commands,
		RequestRedraw:     false, // Don't continuously redraw unless mouse moves
	}
}
