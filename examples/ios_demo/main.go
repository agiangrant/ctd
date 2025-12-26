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

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

// Dummy export required by gomobile
func Dummy() {}

// Global state for the demo
var (
	demoLoop   *ctd.Loop
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

	config := ctd.DefaultLoopConfig()
	demoLoop = ctd.NewLoop(config)
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
	CounterText *ctd.Widget
	StatusText  *ctd.Widget
	Button1     *ctd.Widget
	Button2     *ctd.Widget
	Button3     *ctd.Widget

	// Media widgets
	AudioWidget      *ctd.Widget // Audio player widget
	AudioButton      *ctd.Widget
	AudioStatusText  *ctd.Widget
	MicWidget        *ctd.Widget // Microphone widget
	MicButton        *ctd.Widget
	MicStatusText    *ctd.Widget
	MicLevelText     *ctd.Widget
	VideoWidget      *ctd.Widget // Video player widget
	VideoButton      *ctd.Widget
	VideoStatusText  *ctd.Widget
	CameraWidget     *ctd.Widget // Camera widget
	CameraButton     *ctd.Widget
	CameraStatusText *ctd.Widget

	// iOS Features widgets
	ClipboardCopyButton  *ctd.Widget
	ClipboardPasteButton *ctd.Widget
	ClipboardStatusText  *ctd.Widget
	HapticLightButton    *ctd.Widget
	HapticMediumButton   *ctd.Widget
	HapticHeavyButton    *ctd.Widget
	HapticSelectionBtn   *ctd.Widget
	HapticSuccessButton  *ctd.Widget
	HapticStatusText     *ctd.Widget
	KeyboardInput        *ctd.Widget
	KeyboardShowButton   *ctd.Widget
	KeyboardHideButton   *ctd.Widget
	KeyboardStatusText   *ctd.Widget
}

func buildDemoUI() (*ctd.Widget, *DemoWidgetRefs) {
	refs := &DemoWidgetRefs{}

	// Title
	title := ctd.Text("Centered iOS Demo", "text-white text-2xl font-bold")

	// Subtitle
	subtitle := ctd.Text("Tailwind-style UI on iOS with Go", "text-gray-400 text-base")

	// Counter card
	counterCard := ctd.Container("bg-gray-800 rounded-2xl p-4")

	counterLabel := ctd.Text("Tap Counter", "text-gray-400 text-sm")

	refs.CounterText = ctd.Text("0", "text-white text-6xl font-bold")

	// Button row
	refs.Button1 = ctd.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Blue", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button2 = ctd.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Green", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button3 = ctd.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Purple", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Status text
	refs.StatusText = ctd.Text("Tap a button to interact", "text-gray-500 text-sm")

	// Feature list
	featureCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Features", "text-white text-lg font-semibold"),
		ctd.Text("✓ Tailwind CSS classes", "text-green-400 text-sm"),
		ctd.Text("✓ Touch interactions", "text-green-400 text-sm"),
		ctd.Text("✓ Hover/Active states", "text-green-400 text-sm"),
		ctd.Text("✓ Animations", "text-green-400 text-sm"),
		ctd.Text("✓ Go + Rust engine", "text-green-400 text-sm"),
		ctd.Text("✓ No CGO required", "text-green-400 text-sm"),
	)

	// Media Testing Section
	// Audio widget (invisible - just plays audio)
	refs.AudioWidget = ctd.Audio("https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3", "")
	refs.AudioWidget.SetAudioAutoplay(false) // Don't auto-play

	refs.AudioStatusText = ctd.Text("Tap to play audio", "text-gray-400 text-xs")
	refs.AudioButton = ctd.Container("bg-orange-500 hover:bg-orange-600 active:bg-orange-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("🔊 Audio", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Microphone widget (invisible - just captures audio)
	refs.MicWidget = ctd.Microphone("")
	refs.MicWidget.SetMicrophoneAutoStart(false) // Don't auto-start

	refs.MicStatusText = ctd.Text("Tap to start recording", "text-gray-400 text-xs")
	refs.MicLevelText = ctd.Text("Level: --", "text-gray-500 text-xs")
	refs.MicButton = ctd.Container("bg-red-500 hover:bg-red-600 active:bg-red-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("🎤 Microphone", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Video widget - displays video
	// Using URL since iOS apps are sandboxed and can't access relative file paths
	// Big Buck Bunny test video (small 10s clip)
	refs.VideoWidget = ctd.VideoFromURL(
		"https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4",
		"w-full h-48 rounded-lg bg-gray-800",
	).
		WithMuted(). // Mute by default on mobile
		WithLoop().  // Loop the short clip
		OnVideoError(func(err error) {
			log.Printf("VIDEO ERROR: %v", err)
			refs.VideoStatusText.SetText(fmt.Sprintf("Error: %v", err))
			refs.VideoStatusText.SetTextColor(ctd.ColorRed400)
		}).
		OnVideoEnded(func() {
			log.Printf("Video ended")
			refs.VideoStatusText.SetText("Video ended, tap to replay")
			refs.VideoStatusText.SetTextColor(ctd.ColorGray400)
			videoPlaying = false
		})
	refs.VideoWidget.SetVideoAutoplay(false) // Don't auto-play
	log.Printf("Video widget created with source: %s", refs.VideoWidget.VideoSource())

	refs.VideoStatusText = ctd.Text("Tap to play video", "text-gray-400 text-xs")
	refs.VideoButton = ctd.Container("bg-cyan-500 hover:bg-cyan-600 active:bg-cyan-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("🎬 Video", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Camera widget - displays camera feed
	refs.CameraWidget = ctd.CameraWithResolution(640, 480, 30, "w-full h-48 rounded-lg bg-gray-700")
	refs.CameraWidget.SetCameraAutoStart(false) // Don't auto-start

	refs.CameraStatusText = ctd.Text("Tap to start camera", "text-gray-400 text-xs")
	refs.CameraButton = ctd.Container("bg-pink-500 hover:bg-pink-600 active:bg-pink-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("📷 Camera", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	mediaCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Media Testing", "text-white text-lg font-semibold"),
		ctd.Text("Test audio, video, mic, and camera", "text-gray-400 text-xs"),

		// Audio row (widget is invisible, just include it in tree)
		refs.AudioWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.AudioButton,
			ctd.VStack("gap-1 flex-1",
				ctd.Text("Audio Playback", "text-white text-sm"),
				refs.AudioStatusText,
			),
		),

		// Microphone row (widget is invisible, just include it in tree)
		refs.MicWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.MicButton,
			ctd.VStack("gap-1 flex-1",
				ctd.Text("Microphone", "text-white text-sm"),
				refs.MicStatusText,
				refs.MicLevelText,
			),
		),

		// Video section with display
		ctd.Text("Video Playback", "text-white text-sm"),
		refs.VideoWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.VideoButton,
			refs.VideoStatusText,
		),

		// Camera section with display
		ctd.Text("Camera Preview", "text-white text-sm"),
		refs.CameraWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.CameraButton,
			refs.CameraStatusText,
		),
	)

	// iOS Features Testing Section
	refs.ClipboardStatusText = ctd.Text("Tap Copy or Paste", "text-gray-400 text-xs")
	refs.ClipboardCopyButton = ctd.Container("bg-indigo-500 hover:bg-indigo-600 active:bg-indigo-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("📋 Copy", "text-white text-sm font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)
	refs.ClipboardPasteButton = ctd.Container("bg-indigo-500 hover:bg-indigo-600 active:bg-indigo-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("📄 Paste", "text-white text-sm font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.HapticStatusText = ctd.Text("Tap a button to feel haptic", "text-gray-400 text-xs")
	refs.HapticLightButton = ctd.Container("bg-teal-500 hover:bg-teal-600 active:bg-teal-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Light", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)
	refs.HapticMediumButton = ctd.Container("bg-teal-500 hover:bg-teal-600 active:bg-teal-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Medium", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)
	refs.HapticHeavyButton = ctd.Container("bg-teal-500 hover:bg-teal-600 active:bg-teal-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Heavy", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)
	refs.HapticSelectionBtn = ctd.Container("bg-amber-500 hover:bg-amber-600 active:bg-amber-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Select", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)
	refs.HapticSuccessButton = ctd.Container("bg-emerald-500 hover:bg-emerald-600 active:bg-emerald-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Success", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.KeyboardStatusText = ctd.Text("Tap input to show keyboard", "text-gray-400 text-xs")
	refs.KeyboardInput = ctd.TextField("Type here...", "bg-gray-700 text-white rounded-lg p-3 w-full")
	refs.KeyboardShowButton = ctd.Container("bg-sky-500 hover:bg-sky-600 active:bg-sky-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Show KB", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)
	refs.KeyboardHideButton = ctd.Container("bg-sky-500 hover:bg-sky-600 active:bg-sky-700 rounded-xl p-2").
		WithChildren(
			ctd.Text("Hide KB", "text-white text-xs font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	iosFeaturesCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("iOS Features", "text-white text-lg font-semibold"),
		ctd.Text("Test clipboard, haptics, and keyboard", "text-gray-400 text-xs"),

		// Clipboard section
		ctd.Text("Clipboard", "text-white text-sm"),
		ctd.HStack("gap-2 items-center w-full",
			refs.ClipboardCopyButton,
			refs.ClipboardPasteButton,
			refs.ClipboardStatusText,
		),

		// Haptic feedback section
		ctd.Text("Haptic Feedback", "text-white text-sm"),
		ctd.HStack("gap-2 items-center w-full flex-wrap",
			refs.HapticLightButton,
			refs.HapticMediumButton,
			refs.HapticHeavyButton,
			refs.HapticSelectionBtn,
			refs.HapticSuccessButton,
		),
		refs.HapticStatusText,

		// Keyboard section
		ctd.Text("Keyboard Input", "text-white text-sm"),
		refs.KeyboardInput,
		ctd.HStack("gap-2 items-center w-full",
			refs.KeyboardShowButton,
			refs.KeyboardHideButton,
			refs.KeyboardStatusText,
		),
	)

	// Footer
	footer := ctd.Text("Built with Centered Framework", "text-gray-600 text-xs")

	// Scrollable content container
	scrollContent := ctd.VStack("gap-2 p-4 w-full",
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
		ctd.Text("Scroll down for more →", "text-gray-500 text-sm mt-4"),
		ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
			ctd.Text("More Features", "text-white text-lg font-semibold"),
			ctd.Text("✓ Touch scrolling", "text-green-400 text-sm"),
			ctd.Text("✓ Momentum scrolling", "text-green-400 text-sm"),
			ctd.Text("✓ Gesture recognition", "text-green-400 text-sm"),
		),
		ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
			ctd.Text("Platform Support", "text-white text-lg font-semibold"),
			ctd.Text("✓ macOS", "text-blue-400 text-sm"),
			ctd.Text("✓ iOS", "text-blue-400 text-sm"),
			ctd.Text("○ Android (planned)", "text-gray-500 text-sm"),
			ctd.Text("○ Windows (planned)", "text-gray-500 text-sm"),
			ctd.Text("○ Linux (planned)", "text-gray-500 text-sm"),
		),
		footer,
	)

	// Root container with vertical scrolling enabled
	// Note: flex-col is needed so the container properly calculates content height for scrolling
	root := ctd.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

func setupDemoHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Button 1 - Blue
	refs.Button1.OnClick(func(e *ctd.MouseEvent) {
		clickCount++
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Blue button tapped!")
		refs.StatusText.SetTextColor(ctd.ColorBlue400)

		// Animate
		refs.Button1.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack).
			OnComplete(func() {
				refs.Button1.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	refs.Button1.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering blue button")
		refs.StatusText.SetTextColor(ctd.ColorBlue400)
	})

	// Button 2 - Green
	refs.Button2.OnClick(func(e *ctd.MouseEvent) {
		clickCount += 2
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Green button tapped! (+2)")
		refs.StatusText.SetTextColor(ctd.ColorGreen400)

		refs.Button2.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack).
			OnComplete(func() {
				refs.Button2.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	refs.Button2.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering green button")
		refs.StatusText.SetTextColor(ctd.ColorGreen400)
	})

	// Button 3 - Purple
	refs.Button3.OnClick(func(e *ctd.MouseEvent) {
		clickCount += 5
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Purple button tapped! (+5)")
		refs.StatusText.SetTextColor(ctd.ColorPurple400)

		refs.Button3.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack).
			OnComplete(func() {
				refs.Button3.Animate(anims).
					Duration(100 * time.Millisecond)
			})
	})

	refs.Button3.OnMouseEnter(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Hovering purple button")
		refs.StatusText.SetTextColor(ctd.ColorPurple400)
	})

	// Double tap on any button for bonus
	refs.Button3.OnDoubleClick(func(e *ctd.MouseEvent) {
		clickCount += 20
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("DOUBLE TAP BONUS! (+20)")
		refs.StatusText.SetTextColor(ctd.ColorYellow400)
	})

	// Media handlers
	setupMediaHandlers(refs, anims)

	// iOS Features handlers
	setupIOSFeaturesHandlers(refs, anims)
}

func setupMediaHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Audio playback handler - uses widget API
	refs.AudioButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Audio button clicked, playing=%v", audioPlaying)

		if !audioPlaying {
			refs.AudioWidget.AudioPlay()
			audioPlaying = true
			refs.AudioStatusText.SetText("Playing... tap to pause")
			refs.AudioStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			refs.AudioWidget.AudioPause()
			audioPlaying = false
			refs.AudioStatusText.SetText("Paused, tap to resume")
			refs.AudioStatusText.SetTextColor(ctd.ColorYellow400)
		}

		refs.AudioButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
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
			refs.MicLevelText.SetTextColor(ctd.ColorRed400)
		} else if level > 0.5 {
			refs.MicLevelText.SetTextColor(ctd.ColorYellow400)
		} else {
			refs.MicLevelText.SetTextColor(ctd.ColorGreen400)
		}
	})

	refs.MicButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Mic button clicked, recording=%v", micRecording)

		if !micRecording {
			refs.MicWidget.MicrophoneStart()
			micRecording = true
			refs.MicStatusText.SetText("Recording... tap to stop")
			refs.MicStatusText.SetTextColor(ctd.ColorRed400)
		} else {
			refs.MicWidget.MicrophoneStop()
			micRecording = false
			refs.MicStatusText.SetText("Stopped, tap to record")
			refs.MicStatusText.SetTextColor(ctd.ColorGray400)
			refs.MicLevelText.SetText("Level: --")
		}

		refs.MicButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Video playback handler - uses widget API
	refs.VideoButton.OnClick(func(e *ctd.MouseEvent) {
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
				refs.VideoStatusText.SetTextColor(ctd.ColorRed400)
				return
			}
			videoPlaying = true
			refs.VideoStatusText.SetText("Playing... tap to pause")
			refs.VideoStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			err := refs.VideoWidget.VideoPause()
			if err != nil {
				log.Printf("VideoPause error: %v", err)
			}
			videoPlaying = false
			refs.VideoStatusText.SetText("Paused, tap to resume")
			refs.VideoStatusText.SetTextColor(ctd.ColorYellow400)
		}

		refs.VideoButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Camera handler - uses widget API
	refs.CameraButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Camera button clicked, active=%v", cameraActive)

		if !cameraActive {
			refs.CameraWidget.CameraStart()
			cameraActive = true
			refs.CameraStatusText.SetText("Camera active, tap to stop")
			refs.CameraStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			refs.CameraWidget.CameraStop()
			cameraActive = false
			refs.CameraStatusText.SetText("Stopped, tap to start")
			refs.CameraStatusText.SetTextColor(ctd.ColorGray400)
		}

		refs.CameraButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})
}

func setupIOSFeaturesHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Clipboard - Copy button
	refs.ClipboardCopyButton.OnClick(func(e *ctd.MouseEvent) {
		testText := fmt.Sprintf("Centered iOS Demo - copied at %s", time.Now().Format("15:04:05"))
		ffi.ClipboardSetString(testText)
		refs.ClipboardStatusText.SetText("Copied!")
		refs.ClipboardStatusText.SetTextColor(ctd.ColorGreen400)
		log.Printf("Clipboard: copied '%s'", testText)

		// Haptic feedback for copy
		ffi.HapticFeedback(ffi.HapticImpactLight)

		refs.ClipboardCopyButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Clipboard - Paste button
	refs.ClipboardPasteButton.OnClick(func(e *ctd.MouseEvent) {
		text := ffi.ClipboardGetString()
		if text != "" {
			// Truncate for display
			displayText := text
			if len(displayText) > 30 {
				displayText = displayText[:27] + "..."
			}
			refs.ClipboardStatusText.SetText(fmt.Sprintf("Pasted: %s", displayText))
			refs.ClipboardStatusText.SetTextColor(ctd.ColorBlue400)
			log.Printf("Clipboard: pasted '%s'", text)
		} else {
			refs.ClipboardStatusText.SetText("Clipboard empty")
			refs.ClipboardStatusText.SetTextColor(ctd.ColorYellow400)
		}

		// Haptic feedback for paste
		ffi.HapticFeedback(ffi.HapticImpactLight)

		refs.ClipboardPasteButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Haptic - Light
	refs.HapticLightButton.OnClick(func(e *ctd.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticImpactLight)
		refs.HapticStatusText.SetText("Light impact!")
		refs.HapticStatusText.SetTextColor(ctd.ColorBlue400)
		log.Printf("Haptic: light impact")
	})

	// Haptic - Medium
	refs.HapticMediumButton.OnClick(func(e *ctd.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticImpactMedium)
		refs.HapticStatusText.SetText("Medium impact!")
		refs.HapticStatusText.SetTextColor(ctd.ColorBlue400)
		log.Printf("Haptic: medium impact")
	})

	// Haptic - Heavy
	refs.HapticHeavyButton.OnClick(func(e *ctd.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticImpactHeavy)
		refs.HapticStatusText.SetText("Heavy impact!")
		refs.HapticStatusText.SetTextColor(ctd.ColorBlue400)
		log.Printf("Haptic: heavy impact")
	})

	// Haptic - Selection
	refs.HapticSelectionBtn.OnClick(func(e *ctd.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticSelection)
		refs.HapticStatusText.SetText("Selection feedback!")
		refs.HapticStatusText.SetTextColor(ctd.ColorYellow400)
		log.Printf("Haptic: selection")
	})

	// Haptic - Success notification
	refs.HapticSuccessButton.OnClick(func(e *ctd.MouseEvent) {
		ffi.HapticFeedback(ffi.HapticNotificationSuccess)
		refs.HapticStatusText.SetText("Success notification!")
		refs.HapticStatusText.SetTextColor(ctd.ColorGreen400)
		log.Printf("Haptic: success notification")
	})

	// Keyboard - Show button
	refs.KeyboardShowButton.OnClick(func(e *ctd.MouseEvent) {
		ffi.KeyboardShow()
		if ffi.KeyboardIsVisible() {
			refs.KeyboardStatusText.SetText("Keyboard shown")
			refs.KeyboardStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			refs.KeyboardStatusText.SetText("Show keyboard requested")
			refs.KeyboardStatusText.SetTextColor(ctd.ColorYellow400)
		}
	})

	// Keyboard - Hide button
	refs.KeyboardHideButton.OnClick(func(e *ctd.MouseEvent) {
		ffi.KeyboardHide()
		refs.KeyboardStatusText.SetText("Keyboard hidden")
		refs.KeyboardStatusText.SetTextColor(ctd.ColorGray400)
	})

	// Keyboard input - focus handler (also shows keyboard)
	refs.KeyboardInput.OnFocus(func(e *ctd.FocusEvent) {
		ffi.KeyboardShow()
		refs.KeyboardStatusText.SetText("Input focused, keyboard shown")
		refs.KeyboardStatusText.SetTextColor(ctd.ColorGreen400)
	})

	// When input loses focus, hide keyboard
	refs.KeyboardInput.OnBlur(func(e *ctd.FocusEvent) {
		ffi.KeyboardHide()
		refs.KeyboardStatusText.SetText("Input unfocused, keyboard hidden")
		refs.KeyboardStatusText.SetTextColor(ctd.ColorGray400)
	})
}

// Main is exported for testing on desktop
func Main() {
	StartDemo()
}
