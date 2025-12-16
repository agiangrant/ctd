// Package ios_demo demonstrates running Go + Rust FFI on iOS.
//
// Build with: gomobile bind -target iossimulator -o CenteredDemo.xcframework .
//
// Then integrate CenteredDemo.xcframework into an Xcode project.
//
// Architecture:
// - main.m calls IOSMain() which is the entry point
// - IOSMain() calls ffi.Run() which handles the iOS-specific flow:
//  1. Registers Go's ready callback with Rust
//  2. Calls centered_ios_main() which starts UIApplicationMain (never returns)
//  3. When iOS app is ready, Rust calls Go's ready callback
//  4. Go's callback registers the event handler for rendering
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

	// Media state (just toggle flags now, widgets handle the rest)
	audioPlaying bool
	micRecording bool
	videoPlaying bool
	cameraActive bool
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

	// Media widgets
	AudioWidget      *retained.Widget // Audio player widget
	AudioButton      *retained.Widget
	AudioStatusText  *retained.Widget
	MicWidget        *retained.Widget // Microphone widget
	MicButton        *retained.Widget
	MicStatusText    *retained.Widget
	MicLevelText     *retained.Widget
	VideoWidget      *retained.Widget // Video player widget
	VideoButton      *retained.Widget
	VideoStatusText  *retained.Widget
	CameraWidget     *retained.Widget // Camera widget
	CameraButton     *retained.Widget
	CameraStatusText *retained.Widget

	// iOS Features widgets
	ClipboardCopyButton  *retained.Widget
	ClipboardPasteButton *retained.Widget
	ClipboardStatusText  *retained.Widget
	HapticLightButton    *retained.Widget
	HapticMediumButton   *retained.Widget
	HapticHeavyButton    *retained.Widget
	HapticSelectionBtn   *retained.Widget
	HapticSuccessButton  *retained.Widget
	HapticStatusText     *retained.Widget
	KeyboardInput        *retained.Widget
	KeyboardShowButton   *retained.Widget
	KeyboardHideButton   *retained.Widget
	KeyboardStatusText   *retained.Widget
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

	// Media Testing Section
	// Audio widget (invisible - just plays audio)
	refs.AudioWidget = retained.Audio("https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3", "")
	refs.AudioWidget.SetAudioAutoplay(false) // Don't auto-play

	refs.AudioStatusText = retained.Text("Tap to play audio", "text-gray-400 text-xs")
	refs.AudioButton = retained.Container("bg-orange-500 hover:bg-orange-600 active:bg-orange-700 rounded-xl p-3").
		WithChildren(
			retained.Text("🔊 Audio", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Microphone widget (invisible - just captures audio)
	refs.MicWidget = retained.Microphone("")
	refs.MicWidget.SetMicrophoneAutoStart(false) // Don't auto-start

	refs.MicStatusText = retained.Text("Tap to start recording", "text-gray-400 text-xs")
	refs.MicLevelText = retained.Text("Level: --", "text-gray-500 text-xs")
	refs.MicButton = retained.Container("bg-red-500 hover:bg-red-600 active:bg-red-700 rounded-xl p-3").
		WithChildren(
			retained.Text("🎤 Microphone", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Video widget - displays video
	// Using URL since iOS apps are sandboxed and can't access relative file paths
	// Big Buck Bunny test video (small 10s clip)
	refs.VideoWidget = retained.VideoFromURL(
		"https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4",
		"w-full h-48 rounded-lg bg-gray-800",
	).
		WithMuted(). // Mute by default on mobile
		WithLoop().  // Loop the short clip
		OnVideoError(func(err error) {
			log.Printf("VIDEO ERROR: %v", err)
			refs.VideoStatusText.SetText(fmt.Sprintf("Error: %v", err))
			refs.VideoStatusText.SetTextColor(retained.ColorRed400)
		}).
		OnVideoEnded(func() {
			log.Printf("Video ended")
			refs.VideoStatusText.SetText("Video ended, tap to replay")
			refs.VideoStatusText.SetTextColor(retained.ColorGray400)
			videoPlaying = false
		})
	refs.VideoWidget.SetVideoAutoplay(false) // Don't auto-play
	log.Printf("Video widget created with source: %s", refs.VideoWidget.VideoSource())

	refs.VideoStatusText = retained.Text("Tap to play video", "text-gray-400 text-xs")
	refs.VideoButton = retained.Container("bg-cyan-500 hover:bg-cyan-600 active:bg-cyan-700 rounded-xl p-3").
		WithChildren(
			retained.Text("🎬 Video", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Camera widget - displays camera feed
	refs.CameraWidget = retained.CameraWithResolution(640, 480, 30, "w-full h-48 rounded-lg bg-gray-700")
	refs.CameraWidget.SetCameraAutoStart(false) // Don't auto-start

	refs.CameraStatusText = retained.Text("Tap to start camera", "text-gray-400 text-xs")
	refs.CameraButton = retained.Container("bg-pink-500 hover:bg-pink-600 active:bg-pink-700 rounded-xl p-3").
		WithChildren(
			retained.Text("📷 Camera", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	mediaCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Media Testing", "text-white text-lg font-semibold"),
		retained.Text("Test audio, video, mic, and camera", "text-gray-400 text-xs"),

		// Audio row (widget is invisible, just include it in tree)
		refs.AudioWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.AudioButton,
			retained.VStack("gap-1 flex-1",
				retained.Text("Audio Playback", "text-white text-sm"),
				refs.AudioStatusText,
			),
		),

		// Microphone row (widget is invisible, just include it in tree)
		refs.MicWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.MicButton,
			retained.VStack("gap-1 flex-1",
				retained.Text("Microphone", "text-white text-sm"),
				refs.MicStatusText,
				refs.MicLevelText,
			),
		),

		// Video section with display
		retained.Text("Video Playback", "text-white text-sm"),
		refs.VideoWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.VideoButton,
			refs.VideoStatusText,
		),

		// Camera section with display
		retained.Text("Camera Preview", "text-white text-sm"),
		refs.CameraWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.CameraButton,
			refs.CameraStatusText,
		),
	)

	// iOS Features Testing Section
	refs.ClipboardStatusText = retained.Text("Tap Copy or Paste", "text-gray-400 text-xs")
	refs.ClipboardCopyButton = retained.Container("bg-indigo-500 hover:bg-indigo-600 active:bg-indigo-700 rounded-xl p-3").
		WithChildren(
			retained.Text("📋 Copy", "text-white text-sm font-semibold").
				WithPositionMode(retained.PositionRelative),
		)
	refs.ClipboardPasteButton = retained.Container("bg-indigo-500 hover:bg-indigo-600 active:bg-indigo-700 rounded-xl p-3").
		WithChildren(
			retained.Text("📄 Paste", "text-white text-sm font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.HapticStatusText = retained.Text("Tap a button to feel haptic", "text-gray-400 text-xs")
	refs.HapticLightButton = retained.Container("bg-teal-500 hover:bg-teal-600 active:bg-teal-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Light", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)
	refs.HapticMediumButton = retained.Container("bg-teal-500 hover:bg-teal-600 active:bg-teal-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Medium", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)
	refs.HapticHeavyButton = retained.Container("bg-teal-500 hover:bg-teal-600 active:bg-teal-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Heavy", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)
	refs.HapticSelectionBtn = retained.Container("bg-amber-500 hover:bg-amber-600 active:bg-amber-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Select", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)
	refs.HapticSuccessButton = retained.Container("bg-emerald-500 hover:bg-emerald-600 active:bg-emerald-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Success", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.KeyboardStatusText = retained.Text("Tap input to show keyboard", "text-gray-400 text-xs")
	refs.KeyboardInput = retained.TextField("Type here...", "bg-gray-700 text-white rounded-lg p-3 w-full")
	refs.KeyboardShowButton = retained.Container("bg-sky-500 hover:bg-sky-600 active:bg-sky-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Show KB", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)
	refs.KeyboardHideButton = retained.Container("bg-sky-500 hover:bg-sky-600 active:bg-sky-700 rounded-xl p-2").
		WithChildren(
			retained.Text("Hide KB", "text-white text-xs font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	iosFeaturesCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("iOS Features", "text-white text-lg font-semibold"),
		retained.Text("Test clipboard, haptics, and keyboard", "text-gray-400 text-xs"),

		// Clipboard section
		retained.Text("Clipboard", "text-white text-sm"),
		retained.HStack("gap-2 items-center w-full",
			refs.ClipboardCopyButton,
			refs.ClipboardPasteButton,
			refs.ClipboardStatusText,
		),

		// Haptic feedback section
		retained.Text("Haptic Feedback", "text-white text-sm"),
		retained.HStack("gap-2 items-center w-full flex-wrap",
			refs.HapticLightButton,
			refs.HapticMediumButton,
			refs.HapticHeavyButton,
			refs.HapticSelectionBtn,
			refs.HapticSuccessButton,
		),
		refs.HapticStatusText,

		// Keyboard section
		retained.Text("Keyboard Input", "text-white text-sm"),
		refs.KeyboardInput,
		retained.HStack("gap-2 items-center w-full",
			refs.KeyboardShowButton,
			refs.KeyboardHideButton,
			refs.KeyboardStatusText,
		),
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
		// iOS Features testing section (clipboard, haptics, keyboard)
		iosFeaturesCard,
		// Media testing section
		mediaCard,
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

	// Media handlers
	setupMediaHandlers(refs, anims)

	// iOS Features handlers
	setupIOSFeaturesHandlers(refs, anims)
}

func setupMediaHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Audio playback handler - uses widget API
	refs.AudioButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Audio button clicked, playing=%v", audioPlaying)

		if !audioPlaying {
			refs.AudioWidget.AudioPlay()
			audioPlaying = true
			refs.AudioStatusText.SetText("Playing... tap to pause")
			refs.AudioStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			refs.AudioWidget.AudioPause()
			audioPlaying = false
			refs.AudioStatusText.SetText("Paused, tap to resume")
			refs.AudioStatusText.SetTextColor(retained.ColorYellow400)
		}

		refs.AudioButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Microphone handler - uses widget API
	refs.MicWidget.OnMicrophoneLevelChange(func(level float32) {
		// Update level display
		bars := int(level * 20)
		barStr := ""
		for i := 0; i < 20; i++ {
			if i < bars {
				barStr += "█"
			} else {
				barStr += "░"
			}
		}
		refs.MicLevelText.SetText(fmt.Sprintf("Level: %s %.0f%%", barStr, level*100))

		// Color based on level
		if level > 0.8 {
			refs.MicLevelText.SetTextColor(retained.ColorRed400)
		} else if level > 0.5 {
			refs.MicLevelText.SetTextColor(retained.ColorYellow400)
		} else {
			refs.MicLevelText.SetTextColor(retained.ColorGreen400)
		}
	})

	refs.MicButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Mic button clicked, recording=%v", micRecording)

		if !micRecording {
			refs.MicWidget.MicrophoneStart()
			micRecording = true
			refs.MicStatusText.SetText("Recording... tap to stop")
			refs.MicStatusText.SetTextColor(retained.ColorRed400)
		} else {
			refs.MicWidget.MicrophoneStop()
			micRecording = false
			refs.MicStatusText.SetText("Stopped, tap to record")
			refs.MicStatusText.SetTextColor(retained.ColorGray400)
			refs.MicLevelText.SetText("Level: --")
		}

		refs.MicButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Video playback handler - uses widget API
	refs.VideoButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Video button clicked, playing=%v", videoPlaying)
		log.Printf("Video state: playerID=%d, textureID=%d, loading=%v, error=%v",
			refs.VideoWidget.VideoPlayerID(),
			refs.VideoWidget.VideoTextureID(),
			refs.VideoWidget.VideoLoading(),
			refs.VideoWidget.VideoError())

		if !videoPlaying {
			err := refs.VideoWidget.VideoPlay()
			if err != nil {
				log.Printf("VideoPlay error: %v", err)
				refs.VideoStatusText.SetText(fmt.Sprintf("Play error: %v", err))
				refs.VideoStatusText.SetTextColor(retained.ColorRed400)
				return
			}
			videoPlaying = true
			refs.VideoStatusText.SetText("Playing... tap to pause")
			refs.VideoStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			err := refs.VideoWidget.VideoPause()
			if err != nil {
				log.Printf("VideoPause error: %v", err)
			}
			videoPlaying = false
			refs.VideoStatusText.SetText("Paused, tap to resume")
			refs.VideoStatusText.SetTextColor(retained.ColorYellow400)
		}

		refs.VideoButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Camera handler - uses widget API
	refs.CameraButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Camera button clicked, active=%v", cameraActive)

		if !cameraActive {
			refs.CameraWidget.CameraStart()
			cameraActive = true
			refs.CameraStatusText.SetText("Camera active, tap to stop")
			refs.CameraStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			refs.CameraWidget.CameraStop()
			cameraActive = false
			refs.CameraStatusText.SetText("Stopped, tap to start")
			refs.CameraStatusText.SetTextColor(retained.ColorGray400)
		}

		refs.CameraButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})
}

func setupIOSFeaturesHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Clipboard - Copy button
	refs.ClipboardCopyButton.OnClick(func(e *retained.MouseEvent) {
		testText := fmt.Sprintf("Centered iOS Demo - copied at %s", time.Now().Format("15:04:05"))
		ffi.ClipboardSetString(testText)
		refs.ClipboardStatusText.SetText("Copied!")
		refs.ClipboardStatusText.SetTextColor(retained.ColorGreen400)
		log.Printf("Clipboard: copied '%s'", testText)

		// Haptic feedback for copy
		ffi.HapticFeedback(ffi.HapticImpactLight)

		refs.ClipboardCopyButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Clipboard - Paste button
	refs.ClipboardPasteButton.OnClick(func(e *retained.MouseEvent) {
		text := ffi.ClipboardGetString()
		if text != "" {
			// Truncate for display
			displayText := text
			if len(displayText) > 30 {
				displayText = displayText[:27] + "..."
			}
			refs.ClipboardStatusText.SetText(fmt.Sprintf("Pasted: %s", displayText))
			refs.ClipboardStatusText.SetTextColor(retained.ColorBlue400)
			log.Printf("Clipboard: pasted '%s'", text)
		} else {
			refs.ClipboardStatusText.SetText("Clipboard empty")
			refs.ClipboardStatusText.SetTextColor(retained.ColorYellow400)
		}

		// Haptic feedback for paste
		ffi.HapticFeedback(ffi.HapticImpactLight)

		refs.ClipboardPasteButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Haptic - Light
	refs.HapticLightButton.OnClick(func(e *retained.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticImpactLight)
		refs.HapticStatusText.SetText("Light impact!")
		refs.HapticStatusText.SetTextColor(retained.ColorBlue400)
		log.Printf("Haptic: light impact")
	})

	// Haptic - Medium
	refs.HapticMediumButton.OnClick(func(e *retained.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticImpactMedium)
		refs.HapticStatusText.SetText("Medium impact!")
		refs.HapticStatusText.SetTextColor(retained.ColorBlue400)
		log.Printf("Haptic: medium impact")
	})

	// Haptic - Heavy
	refs.HapticHeavyButton.OnClick(func(e *retained.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticImpactHeavy)
		refs.HapticStatusText.SetText("Heavy impact!")
		refs.HapticStatusText.SetTextColor(retained.ColorBlue400)
		log.Printf("Haptic: heavy impact")
	})

	// Haptic - Selection
	refs.HapticSelectionBtn.OnClick(func(e *retained.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticSelection)
		refs.HapticStatusText.SetText("Selection feedback!")
		refs.HapticStatusText.SetTextColor(retained.ColorYellow400)
		log.Printf("Haptic: selection")
	})

	// Haptic - Success notification
	refs.HapticSuccessButton.OnClick(func(e *retained.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticNotificationSuccess)
		refs.HapticStatusText.SetText("Success notification!")
		refs.HapticStatusText.SetTextColor(retained.ColorGreen400)
		log.Printf("Haptic: success notification")
	})

	// Keyboard - Show button
	refs.KeyboardShowButton.OnClick(func(e *retained.MouseEvent) {
		ffi.KeyboardShow()
		if ffi.KeyboardIsVisible() {
			refs.KeyboardStatusText.SetText("Keyboard shown")
			refs.KeyboardStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			refs.KeyboardStatusText.SetText("Show keyboard requested")
			refs.KeyboardStatusText.SetTextColor(retained.ColorYellow400)
		}
	})

	// Keyboard - Hide button
	refs.KeyboardHideButton.OnClick(func(e *retained.MouseEvent) {
		ffi.KeyboardHide()
		refs.KeyboardStatusText.SetText("Keyboard hidden")
		refs.KeyboardStatusText.SetTextColor(retained.ColorGray400)
	})

	// Keyboard input - focus handler (also shows keyboard)
	refs.KeyboardInput.OnFocus(func(e *retained.FocusEvent) {
		ffi.KeyboardShow()
		refs.KeyboardStatusText.SetText("Input focused, keyboard shown")
		refs.KeyboardStatusText.SetTextColor(retained.ColorGreen400)
	})

	// When input loses focus, hide keyboard
	refs.KeyboardInput.OnBlur(func(e *retained.FocusEvent) {
		ffi.KeyboardHide()
		refs.KeyboardStatusText.SetText("Input unfocused, keyboard hidden")
		refs.KeyboardStatusText.SetTextColor(retained.ColorGray400)
	})
}

// Main is exported for testing on desktop
func Main() {
	StartDemo()
}
