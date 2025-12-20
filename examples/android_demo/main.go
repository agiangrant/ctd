// Package android_demo demonstrates running Go + Rust FFI on Android.
//
// Build with: gomobile bind -target android -o centered_go.aar .
//
// Then integrate centered_go.aar into an Android Gradle project.
//
// Architecture:
// - CenteredActivity extends NativeActivity
// - NativeActivity loads libcentered_engine.so and libcentered_go.so
// - android_main() in Rust starts the android-activity event loop
// - When ready, Rust calls Go's ready callback
// - Go registers the event handler for rendering
package android_demo

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

	// Device input IDs
	audioInputID  ffi.AudioInputID
	videoInputID  ffi.VideoInputID
	audioPlayerID ffi.AudioPlayerID

	// Media playback state
	videoPlaying bool
)

// AndroidMain is the entry point for the Android app.
// This is called from the native activity initialization.
// It handles the full app lifecycle via ffi.Run().
func AndroidMain() {
	StartDemo()
}

// StartDemo initializes and starts the demo app.
// On Android, this is called via AndroidMain().
// On desktop/iOS, the same code structure works (platform-agnostic).
func StartDemo() {
	log.Printf("=== Starting Centered Android Demo ===")
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
		// Handle back button, etc.
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Centered Android Demo"
	appConfig.Width = 360  // Common Android logical width
	appConfig.Height = 780 // Common Android logical height

	log.Println("Starting Centered Android Demo...")

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
	// Device input buttons
	MicButton    *retained.Widget
	CameraButton *retained.Widget
	AudioButton  *retained.Widget
	MicStatus    *retained.Widget
	CameraStatus *retained.Widget
	AudioStatus  *retained.Widget
	// Video playback
	VideoWidget *retained.Widget
	VideoButton *retained.Widget
	VideoStatus *retained.Widget
}

func buildDemoUI() (*retained.Widget, *DemoWidgetRefs) {
	refs := &DemoWidgetRefs{}

	// Title
	title := retained.Text("Centered Android Demo", "text-white text-2xl font-bold")

	// Subtitle
	subtitle := retained.Text("Same Go code on iOS, Android, and Desktop", "text-gray-400 text-base")

	// Counter card
	refs.CounterText = retained.Text("0", "text-white text-6xl font-bold")
	refs.StatusText = retained.Text("Tap a button to interact", "text-gray-500 text-sm")

	counterCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 items-center w-full",
		retained.Text("Tap Counter", "text-gray-400 text-sm"),
		refs.CounterText,
		refs.StatusText,
	)

	// Button row - using Container for styled buttons
	refs.Button1 = retained.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Blue", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.Button2 = retained.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Show Keyboard", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.Button3 = retained.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Haptic", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	buttonRow := retained.HStack("gap-3 w-full justify-center",
		refs.Button1,
		refs.Button2,
		refs.Button3,
	)

	// Text input field
	textField := retained.TextField("Type something...", "w-full px-4 py-3 bg-gray-700 rounded-lg text-white")

	inputCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		retained.Text("Text Input", "text-gray-400 text-sm"),
		textField,
	)

	// Feature list
	featureCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		retained.Text("Features", "text-white text-lg font-semibold"),
		retained.Text("✓ Tailwind CSS classes", "text-green-400 text-sm"),
		retained.Text("✓ Touch interactions", "text-green-400 text-sm"),
		retained.Text("✓ Hover/Active states", "text-green-400 text-sm"),
		retained.Text("✓ Go + Rust engine", "text-green-400 text-sm"),
		retained.Text("✓ Custom bundled fonts", "text-green-400 text-sm"),
		retained.Text("✓ Cross-platform", "text-green-400 text-sm"),
	)

	// Bundled Fonts Demo - uses font-serif from theme.toml which is a bundled TTF
	fontCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Bundled Fonts", "text-white text-lg font-semibold"),
		retained.Text("Custom fonts loaded from TTF files", "text-gray-400 text-xs"),

		// System sans (default)
		retained.VStack("gap-1 w-full",
			retained.Text("font-sans (System)", "text-gray-500 text-xs"),
			retained.Text("The quick brown fox", "text-white text-lg font-sans"),
		),

		// Bundled serif font
		retained.VStack("gap-1 w-full",
			retained.Text("font-serif (Bundled TTF)", "text-gray-500 text-xs"),
			retained.Text("The quick brown fox", "text-white text-lg font-serif"),
		),

		// System mono
		retained.VStack("gap-1 w-full",
			retained.Text("font-mono (System)", "text-gray-500 text-xs"),
			retained.Text("The quick brown fox", "text-white text-lg font-mono"),
		),
	)

	// Device Input Testing Card
	refs.MicStatus = retained.Text("Microphone: Not started", "text-gray-500 text-xs")
	refs.CameraStatus = retained.Text("Camera: Not started", "text-gray-500 text-xs")
	refs.AudioStatus = retained.Text("Audio: Not loaded", "text-gray-500 text-xs")

	refs.MicButton = retained.Container("bg-red-500 hover:bg-red-600 active:bg-red-700 rounded-xl p-3 flex-1").
		WithChildren(
			retained.Text("🎤 Mic", "text-white text-sm font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.CameraButton = retained.Container("bg-orange-500 hover:bg-orange-600 active:bg-orange-700 rounded-xl p-3 flex-1").
		WithChildren(
			retained.Text("📷 Camera", "text-white text-sm font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.AudioButton = retained.Container("bg-pink-500 hover:bg-pink-600 active:bg-pink-700 rounded-xl p-3 flex-1").
		WithChildren(
			retained.Text("🔊 Audio", "text-white text-sm font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	deviceInputRow := retained.HStack("gap-2 w-full",
		refs.MicButton,
		refs.CameraButton,
		refs.AudioButton,
	)

	deviceCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Device Input/Output", "text-white text-lg font-semibold"),
		deviceInputRow,
		refs.MicStatus,
		refs.CameraStatus,
		refs.AudioStatus,
	)

	// Video player widget
	refs.VideoWidget = retained.VideoFromURL(
		"https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4",
		"w-full h-48 rounded-lg bg-gray-800",
	).
		WithMuted().
		WithLoop().
		OnVideoError(func(err error) {
			log.Printf("VIDEO ERROR: %v", err)
			refs.VideoStatus.SetText(fmt.Sprintf("Error: %v", err))
			refs.VideoStatus.SetTextColor(retained.ColorRed400)
		}).
		OnVideoEnded(func() {
			log.Printf("Video ended")
			refs.VideoStatus.SetText("Video ended, tap to replay")
			refs.VideoStatus.SetTextColor(retained.ColorGray400)
			videoPlaying = false
		})
	refs.VideoWidget.SetVideoAutoplay(false)
	log.Printf("Video widget created")

	refs.VideoStatus = retained.Text("Tap to play video", "text-gray-400 text-xs")
	refs.VideoButton = retained.Container("bg-cyan-500 hover:bg-cyan-600 active:bg-cyan-700 rounded-xl p-3 flex-1").
		WithChildren(
			retained.Text("🎬 Video", "text-white text-sm font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	videoCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Video Playback", "text-white text-lg font-semibold"),
		refs.VideoWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.VideoButton,
			refs.VideoStatus,
		),
	)

	// Footer
	footer := retained.Text("Built with Centered Framework", "text-gray-600 text-xs")

	// Scrollable content container
	scrollContent := retained.VStack("gap-4 p-4 w-full",
		title,
		subtitle,
		counterCard,
		buttonRow,
		inputCard,
		fontCard,
		deviceCard,
		videoCard,
		featureCard,
		footer,
	)

	// Root container with vertical scrolling enabled
	root := retained.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

func setupDemoHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Button 1: Increment counter
	refs.Button1.OnClick(func(e *retained.MouseEvent) {
		clickCount++
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Blue button tapped!")
		refs.StatusText.SetTextColor(retained.ColorBlue400)

		// Animate
		refs.Button1.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)

		// Reset status after a delay
		go func() {
			time.Sleep(1 * time.Second)
			refs.StatusText.SetText("Tap a button to interact")
			refs.StatusText.SetTextColor(retained.ColorGray500)
		}()
	})

	// Button 2: Show keyboard
	refs.Button2.OnClick(func(e *retained.MouseEvent) {
		refs.StatusText.SetText("Showing keyboard...")
		refs.StatusText.SetTextColor(retained.ColorGreen400)
		ffi.KeyboardShow()

		refs.Button2.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Button 3: Haptic feedback
	refs.Button3.OnClick(func(e *retained.MouseEvent) {
		refs.StatusText.SetText("Haptic feedback!")
		refs.StatusText.SetTextColor(retained.ColorPurple400)
		ffi.HapticFeedback(ffi.HapticImpactMedium)

		refs.Button3.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Microphone button: Request permission and start/stop recording
	micRecording := false
	refs.MicButton.OnClick(func(e *retained.MouseEvent) {
		refs.MicButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)

		if !micRecording {
			// Create audio input if needed
			if audioInputID == 0 {
				audioInputID = ffi.AudioInputCreate()
			}

			// Request permission and start
			err := ffi.AudioInputRequestPermission(audioInputID)
			if err != nil {
				refs.MicStatus.SetText(fmt.Sprintf("Mic: Permission error - %v", err))
				refs.MicStatus.SetTextColor(retained.ColorRed400)
				return
			}

			if !ffi.AudioInputHasPermission(audioInputID) {
				refs.MicStatus.SetText("Mic: Permission denied")
				refs.MicStatus.SetTextColor(retained.ColorRed400)
				return
			}

			// Open and start (44100 Hz, mono)
			err = ffi.AudioInputOpen(audioInputID, "", 44100, 1)
			if err != nil {
				refs.MicStatus.SetText(fmt.Sprintf("Mic: Open error - %v", err))
				refs.MicStatus.SetTextColor(retained.ColorRed400)
				return
			}

			err = ffi.AudioInputStart(audioInputID)
			if err != nil {
				refs.MicStatus.SetText(fmt.Sprintf("Mic: Start error - %v", err))
				refs.MicStatus.SetTextColor(retained.ColorRed400)
				return
			}

			micRecording = true
			refs.MicStatus.SetText("Mic: Recording...")
			refs.MicStatus.SetTextColor(retained.ColorGreen400)
		} else {
			// Stop recording
			ffi.AudioInputStop(audioInputID)
			ffi.AudioInputClose(audioInputID)
			micRecording = false
			refs.MicStatus.SetText("Mic: Stopped")
			refs.MicStatus.SetTextColor(retained.ColorGray500)
		}
	})

	// Camera button: Request permission and start/stop camera
	cameraRunning := false
	refs.CameraButton.OnClick(func(e *retained.MouseEvent) {
		refs.CameraButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)

		if !cameraRunning {
			// Create video input if needed
			if videoInputID == 0 {
				videoInputID = ffi.VideoInputCreate()
			}

			// Request permission
			err := ffi.VideoInputRequestPermission(videoInputID)
			if err != nil {
				refs.CameraStatus.SetText(fmt.Sprintf("Camera: Permission error - %v", err))
				refs.CameraStatus.SetTextColor(retained.ColorRed400)
				return
			}

			if !ffi.VideoInputHasPermission(videoInputID) {
				refs.CameraStatus.SetText("Camera: Permission denied")
				refs.CameraStatus.SetTextColor(retained.ColorRed400)
				return
			}

			// Open and start (default camera, 720p)
			err = ffi.VideoInputOpen(videoInputID, "", 1280, 720, 30)
			if err != nil {
				refs.CameraStatus.SetText(fmt.Sprintf("Camera: Open error - %v", err))
				refs.CameraStatus.SetTextColor(retained.ColorRed400)
				return
			}

			err = ffi.VideoInputStart(videoInputID)
			if err != nil {
				refs.CameraStatus.SetText(fmt.Sprintf("Camera: Start error - %v", err))
				refs.CameraStatus.SetTextColor(retained.ColorRed400)
				return
			}

			cameraRunning = true
			refs.CameraStatus.SetText("Camera: Running (720p)")
			refs.CameraStatus.SetTextColor(retained.ColorGreen400)
		} else {
			// Stop camera
			ffi.VideoInputStop(videoInputID)
			ffi.VideoInputClose(videoInputID)
			cameraRunning = false
			refs.CameraStatus.SetText("Camera: Stopped")
			refs.CameraStatus.SetTextColor(retained.ColorGray500)
		}
	})

	// Audio button: Load and play a test sound
	audioPlaying := false
	refs.AudioButton.OnClick(func(e *retained.MouseEvent) {
		refs.AudioButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)

		if !audioPlaying {
			// Create audio player if needed
			if audioPlayerID == 0 {
				audioPlayerID = ffi.AudioCreate()
			}

			// Load a test audio file (same as iOS demo)
			err := ffi.AudioLoadURL(audioPlayerID, "https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3")
			if err != nil {
				refs.AudioStatus.SetText(fmt.Sprintf("Audio: Load error - %v", err))
				refs.AudioStatus.SetTextColor(retained.ColorRed400)
				return
			}

			err = ffi.AudioPlay(audioPlayerID)
			if err != nil {
				refs.AudioStatus.SetText(fmt.Sprintf("Audio: Play error - %v", err))
				refs.AudioStatus.SetTextColor(retained.ColorRed400)
				return
			}

			audioPlaying = true
			refs.AudioStatus.SetText("Audio: Playing...")
			refs.AudioStatus.SetTextColor(retained.ColorGreen400)
		} else {
			// Stop audio
			ffi.AudioStop(audioPlayerID)
			audioPlaying = false
			refs.AudioStatus.SetText("Audio: Stopped")
			refs.AudioStatus.SetTextColor(retained.ColorGray500)
		}
	})

	// Video playback handler
	refs.VideoButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Video button clicked, playing=%v", videoPlaying)

		refs.VideoButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)

		if !videoPlaying {
			err := refs.VideoWidget.VideoPlay()
			if err != nil {
				log.Printf("VideoPlay error: %v", err)
				refs.VideoStatus.SetText(fmt.Sprintf("Play error: %v", err))
				refs.VideoStatus.SetTextColor(retained.ColorRed400)
				return
			}
			videoPlaying = true
			refs.VideoStatus.SetText("Playing... tap to pause")
			refs.VideoStatus.SetTextColor(retained.ColorGreen400)
		} else {
			err := refs.VideoWidget.VideoPause()
			if err != nil {
				log.Printf("VideoPause error: %v", err)
			}
			videoPlaying = false
			refs.VideoStatus.SetText("Paused, tap to resume")
			refs.VideoStatus.SetTextColor(retained.ColorYellow400)
		}
	})
}
