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

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

// Dummy export required by gomobile
func Dummy() {}

// Global state for the demo
var (
	demoLoop   *ctd.Loop
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
	CounterText *ctd.Widget
	StatusText  *ctd.Widget
	Button1     *ctd.Widget
	Button2     *ctd.Widget
	Button3     *ctd.Widget
	// Device input buttons
	MicButton    *ctd.Widget
	CameraButton *ctd.Widget
	AudioButton  *ctd.Widget
	MicStatus    *ctd.Widget
	CameraStatus *ctd.Widget
	AudioStatus  *ctd.Widget
	// Video playback
	VideoWidget *ctd.Widget
	VideoButton *ctd.Widget
	VideoStatus *ctd.Widget
}

func buildDemoUI() (*ctd.Widget, *DemoWidgetRefs) {
	refs := &DemoWidgetRefs{}

	// Title
	title := ctd.Text("Centered Android Demo", "text-white text-2xl font-bold")

	// Subtitle
	subtitle := ctd.Text("Same Go code on iOS, Android, and Desktop", "text-gray-400 text-base")

	// Counter card
	refs.CounterText = ctd.Text("0", "text-white text-6xl font-bold")
	refs.StatusText = ctd.Text("Tap a button to interact", "text-gray-500 text-sm")

	counterCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 items-center w-full",
		ctd.Text("Tap Counter", "text-gray-400 text-sm"),
		refs.CounterText,
		refs.StatusText,
	)

	// Button row - using Container for styled buttons
	refs.Button1 = ctd.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Blue", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button2 = ctd.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Show Keyboard", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button3 = ctd.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Haptic", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	buttonRow := ctd.HStack("gap-3 w-full justify-center",
		refs.Button1,
		refs.Button2,
		refs.Button3,
	)

	// Text input field
	textField := ctd.TextField("Type something...", "w-full px-4 py-3 bg-gray-700 rounded-lg text-white")

	inputCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Text Input", "text-gray-400 text-sm"),
		textField,
	)

	// Feature list
	featureCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Features", "text-white text-lg font-semibold"),
		ctd.Text("✓ Tailwind CSS classes", "text-green-400 text-sm"),
		ctd.Text("✓ Touch interactions", "text-green-400 text-sm"),
		ctd.Text("✓ Hover/Active states", "text-green-400 text-sm"),
		ctd.Text("✓ Go + Rust engine", "text-green-400 text-sm"),
		ctd.Text("✓ Custom bundled fonts", "text-green-400 text-sm"),
		ctd.Text("✓ Cross-platform", "text-green-400 text-sm"),
	)

	// Bundled Fonts Demo - uses font-serif from theme.toml which is a bundled TTF
	fontCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Bundled Fonts", "text-white text-lg font-semibold"),
		ctd.Text("Custom fonts loaded from TTF files", "text-gray-400 text-xs"),

		// System sans (default)
		ctd.VStack("gap-1 w-full",
			ctd.Text("font-sans (System)", "text-gray-500 text-xs"),
			ctd.Text("The quick brown fox", "text-white text-lg font-sans"),
		),

		// Bundled serif font
		ctd.VStack("gap-1 w-full",
			ctd.Text("font-serif (Bundled TTF)", "text-gray-500 text-xs"),
			ctd.Text("The quick brown fox", "text-white text-lg font-serif"),
		),

		// System mono
		ctd.VStack("gap-1 w-full",
			ctd.Text("font-mono (System)", "text-gray-500 text-xs"),
			ctd.Text("The quick brown fox", "text-white text-lg font-mono"),
		),
	)

	// Device Input Testing Card
	refs.MicStatus = ctd.Text("Microphone: Not started", "text-gray-500 text-xs")
	refs.CameraStatus = ctd.Text("Camera: Not started", "text-gray-500 text-xs")
	refs.AudioStatus = ctd.Text("Audio: Not loaded", "text-gray-500 text-xs")

	refs.MicButton = ctd.Container("bg-red-500 hover:bg-red-600 active:bg-red-700 rounded-xl p-3 flex-1").
		WithChildren(
			ctd.Text("🎤 Mic", "text-white text-sm font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.CameraButton = ctd.Container("bg-orange-500 hover:bg-orange-600 active:bg-orange-700 rounded-xl p-3 flex-1").
		WithChildren(
			ctd.Text("📷 Camera", "text-white text-sm font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.AudioButton = ctd.Container("bg-pink-500 hover:bg-pink-600 active:bg-pink-700 rounded-xl p-3 flex-1").
		WithChildren(
			ctd.Text("🔊 Audio", "text-white text-sm font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	deviceInputRow := ctd.HStack("gap-2 w-full",
		refs.MicButton,
		refs.CameraButton,
		refs.AudioButton,
	)

	deviceCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Device Input/Output", "text-white text-lg font-semibold"),
		deviceInputRow,
		refs.MicStatus,
		refs.CameraStatus,
		refs.AudioStatus,
	)

	// Video player widget
	refs.VideoWidget = ctd.VideoFromURL(
		"https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4",
		"w-full h-48 rounded-lg bg-gray-800",
	).
		WithMuted().
		WithLoop().
		OnVideoError(func(err error) {
			log.Printf("VIDEO ERROR: %v", err)
			refs.VideoStatus.SetText(fmt.Sprintf("Error: %v", err))
			refs.VideoStatus.SetTextColor(ctd.ColorRed400)
		}).
		OnVideoEnded(func() {
			log.Printf("Video ended")
			refs.VideoStatus.SetText("Video ended, tap to replay")
			refs.VideoStatus.SetTextColor(ctd.ColorGray400)
			videoPlaying = false
		})
	refs.VideoWidget.SetVideoAutoplay(false)
	log.Printf("Video widget created")

	refs.VideoStatus = ctd.Text("Tap to play video", "text-gray-400 text-xs")
	refs.VideoButton = ctd.Container("bg-cyan-500 hover:bg-cyan-600 active:bg-cyan-700 rounded-xl p-3 flex-1").
		WithChildren(
			ctd.Text("🎬 Video", "text-white text-sm font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	videoCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Video Playback", "text-white text-lg font-semibold"),
		refs.VideoWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.VideoButton,
			refs.VideoStatus,
		),
	)

	// Footer
	footer := ctd.Text("Built with Centered Framework", "text-gray-600 text-xs")

	// Scrollable content container
	scrollContent := ctd.VStack("gap-4 p-4 w-full",
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
	root := ctd.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

func setupDemoHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Button 1: Increment counter
	refs.Button1.OnClick(func(e *ctd.MouseEvent) {
		clickCount++
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Blue button tapped!")
		refs.StatusText.SetTextColor(ctd.ColorBlue400)

		// Animate
		refs.Button1.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)

		// Reset status after a delay
		go func() {
			time.Sleep(1 * time.Second)
			refs.StatusText.SetText("Tap a button to interact")
			refs.StatusText.SetTextColor(ctd.ColorGray500)
		}()
	})

	// Button 2: Show keyboard
	refs.Button2.OnClick(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Showing keyboard...")
		refs.StatusText.SetTextColor(ctd.ColorGreen400)
		ffi.KeyboardShow()

		refs.Button2.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Button 3: Haptic feedback
	refs.Button3.OnClick(func(e *ctd.MouseEvent) {
		refs.StatusText.SetText("Haptic feedback!")
		refs.StatusText.SetTextColor(ctd.ColorPurple400)
		ffi.HapticFeedback(ffi.HapticImpactMedium)

		refs.Button3.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Microphone button: Request permission and start/stop recording
	micRecording := false
	refs.MicButton.OnClick(func(e *ctd.MouseEvent) {
		refs.MicButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)

		if !micRecording {
			// Create audio input if needed
			if audioInputID == 0 {
				audioInputID = ffi.AudioInputCreate()
			}

			// Request permission and start
			err := ffi.AudioInputRequestPermission(audioInputID)
			if err != nil {
				refs.MicStatus.SetText(fmt.Sprintf("Mic: Permission error - %v", err))
				refs.MicStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			if !ffi.AudioInputHasPermission(audioInputID) {
				refs.MicStatus.SetText("Mic: Permission denied")
				refs.MicStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			// Open and start (44100 Hz, mono)
			err = ffi.AudioInputOpen(audioInputID, "", 44100, 1)
			if err != nil {
				refs.MicStatus.SetText(fmt.Sprintf("Mic: Open error - %v", err))
				refs.MicStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			err = ffi.AudioInputStart(audioInputID)
			if err != nil {
				refs.MicStatus.SetText(fmt.Sprintf("Mic: Start error - %v", err))
				refs.MicStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			micRecording = true
			refs.MicStatus.SetText("Mic: Recording...")
			refs.MicStatus.SetTextColor(ctd.ColorGreen400)
		} else {
			// Stop recording
			ffi.AudioInputStop(audioInputID)
			ffi.AudioInputClose(audioInputID)
			micRecording = false
			refs.MicStatus.SetText("Mic: Stopped")
			refs.MicStatus.SetTextColor(ctd.ColorGray500)
		}
	})

	// Camera button: Request permission and start/stop camera
	cameraRunning := false
	refs.CameraButton.OnClick(func(e *ctd.MouseEvent) {
		refs.CameraButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)

		if !cameraRunning {
			// Create video input if needed
			if videoInputID == 0 {
				videoInputID = ffi.VideoInputCreate()
			}

			// Request permission
			err := ffi.VideoInputRequestPermission(videoInputID)
			if err != nil {
				refs.CameraStatus.SetText(fmt.Sprintf("Camera: Permission error - %v", err))
				refs.CameraStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			if !ffi.VideoInputHasPermission(videoInputID) {
				refs.CameraStatus.SetText("Camera: Permission denied")
				refs.CameraStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			// Open and start (default camera, 720p)
			err = ffi.VideoInputOpen(videoInputID, "", 1280, 720, 30)
			if err != nil {
				refs.CameraStatus.SetText(fmt.Sprintf("Camera: Open error - %v", err))
				refs.CameraStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			err = ffi.VideoInputStart(videoInputID)
			if err != nil {
				refs.CameraStatus.SetText(fmt.Sprintf("Camera: Start error - %v", err))
				refs.CameraStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			cameraRunning = true
			refs.CameraStatus.SetText("Camera: Running (720p)")
			refs.CameraStatus.SetTextColor(ctd.ColorGreen400)
		} else {
			// Stop camera
			ffi.VideoInputStop(videoInputID)
			ffi.VideoInputClose(videoInputID)
			cameraRunning = false
			refs.CameraStatus.SetText("Camera: Stopped")
			refs.CameraStatus.SetTextColor(ctd.ColorGray500)
		}
	})

	// Audio button: Load and play a test sound
	audioPlaying := false
	refs.AudioButton.OnClick(func(e *ctd.MouseEvent) {
		refs.AudioButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)

		if !audioPlaying {
			// Create audio player if needed
			if audioPlayerID == 0 {
				audioPlayerID = ffi.AudioCreate()
			}

			// Load a test audio file (same as iOS demo)
			err := ffi.AudioLoadURL(audioPlayerID, "https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3")
			if err != nil {
				refs.AudioStatus.SetText(fmt.Sprintf("Audio: Load error - %v", err))
				refs.AudioStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			err = ffi.AudioPlay(audioPlayerID)
			if err != nil {
				refs.AudioStatus.SetText(fmt.Sprintf("Audio: Play error - %v", err))
				refs.AudioStatus.SetTextColor(ctd.ColorRed400)
				return
			}

			audioPlaying = true
			refs.AudioStatus.SetText("Audio: Playing...")
			refs.AudioStatus.SetTextColor(ctd.ColorGreen400)
		} else {
			// Stop audio
			ffi.AudioStop(audioPlayerID)
			audioPlaying = false
			refs.AudioStatus.SetText("Audio: Stopped")
			refs.AudioStatus.SetTextColor(ctd.ColorGray500)
		}
	})

	// Video playback handler
	refs.VideoButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Video button clicked, playing=%v", videoPlaying)

		refs.VideoButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)

		if !videoPlaying {
			err := refs.VideoWidget.VideoPlay()
			if err != nil {
				log.Printf("VideoPlay error: %v", err)
				refs.VideoStatus.SetText(fmt.Sprintf("Play error: %v", err))
				refs.VideoStatus.SetTextColor(ctd.ColorRed400)
				return
			}
			videoPlaying = true
			refs.VideoStatus.SetText("Playing... tap to pause")
			refs.VideoStatus.SetTextColor(ctd.ColorGreen400)
		} else {
			err := refs.VideoWidget.VideoPause()
			if err != nil {
				log.Printf("VideoPause error: %v", err)
			}
			videoPlaying = false
			refs.VideoStatus.SetText("Paused, tap to resume")
			refs.VideoStatus.SetTextColor(ctd.ColorYellow400)
		}
	})
}
