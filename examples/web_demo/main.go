//go:build js && wasm

// Package main demonstrates the Centered framework running in the browser.
// This uses the SAME retained mode API as iOS/Android/desktop demos.
package main

import (
	"fmt"
	"log"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

// Global state
var (
	demoLoop   *retained.Loop
	clickCount int

	// Media state
	audioPlaying bool
	micRecording bool
	videoPlaying bool
	cameraActive bool
)

func main() {
	log.Println("=== Starting Centered Web Demo ===")

	config := retained.DefaultLoopConfig()
	demoLoop = retained.NewLoop(config)
	tree := demoLoop.Tree()
	anims := demoLoop.Animations()

	// Build the UI using the same widgets as iOS/Android/desktop
	root, refs := buildDemoUI()
	tree.SetRoot(root)

	// Set up event handlers
	setupDemoHandlers(refs, anims)

	demoLoop.OnResize(func(width, height float32) {
		log.Printf("OnResize: %fx%f", width, height)
		root.SetSize(width, height)
	})

	demoLoop.OnEvent(func(event ffi.Event) bool {
		// Handle escape key to close
		if event.Type == ffi.EventKeyPressed && event.Data1 == 27 {
			return true // Request exit
		}
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Centered Web Demo"
	appConfig.Width = 800
	appConfig.Height = 600

	log.Println("Starting Centered Web Demo...")

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

	// Form widgets
	NameInput      *retained.Widget
	EmailInput     *retained.Widget
	PasswordInput  *retained.Widget
	MessageInput   *retained.Widget
	SubmitButton   *retained.Widget
	FormStatusText *retained.Widget

	// Media widgets
	AudioWidget     *retained.Widget
	AudioButton     *retained.Widget
	AudioStatusText *retained.Widget

	MicWidget     *retained.Widget
	MicButton     *retained.Widget
	MicStatusText *retained.Widget
	MicLevelText  *retained.Widget

	VideoWidget     *retained.Widget
	VideoButton     *retained.Widget
	VideoStatusText *retained.Widget

	CameraWidget     *retained.Widget
	CameraButton     *retained.Widget
	CameraStatusText *retained.Widget
}

func buildDemoUI() (*retained.Widget, *DemoWidgetRefs) {
	refs := &DemoWidgetRefs{}

	// Title
	title := retained.Text("Centered Web Demo", "text-white text-2xl font-bold")

	// Subtitle
	subtitle := retained.Text("Same code runs on iOS, Android, Desktop, and Web!", "text-gray-400 text-base")

	// Counter card
	counterLabel := retained.Text("Click Counter", "text-gray-400 text-sm")
	refs.CounterText = retained.Text("0", "text-white text-6xl font-bold")

	// Button row
	refs.Button1 = retained.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Blue (+1)", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.Button2 = retained.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Green (+2)", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.Button3 = retained.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-xl p-3").
		WithChildren(
			retained.Text("Purple (+5)", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Status text
	refs.StatusText = retained.Text("Click a button to interact", "text-gray-500 text-sm")

	// Counter card
	counterCard := retained.VStack("bg-gray-800 rounded-2xl p-6 gap-4 w-full items-center",
		counterLabel,
		refs.CounterText,
		retained.HStack("gap-4",
			refs.Button1,
			refs.Button2,
			refs.Button3,
		),
		refs.StatusText,
	)

	// Feature list
	featureCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		retained.Text("Cross-Platform Features", "text-white text-lg font-semibold"),
		retained.Text("✓ Tailwind CSS classes", "text-green-400 text-sm"),
		retained.Text("✓ Same Go code on all platforms", "text-green-400 text-sm"),
		retained.Text("✓ Mouse/touch interactions", "text-green-400 text-sm"),
		retained.Text("✓ Hover/Active states", "text-green-400 text-sm"),
		retained.Text("✓ Animations", "text-green-400 text-sm"),
		retained.Text("✓ Scrolling (scroll down!)", "text-green-400 text-sm"),
		retained.Text("✓ Bundled custom fonts", "text-green-400 text-sm"),
	)

	// Bundled fonts demo - uses font-serif from theme.toml which is a bundled TTF
	fontCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Bundled Fonts", "text-white text-lg font-semibold"),
		retained.Text("Custom fonts loaded from TTF files", "text-gray-400 text-xs"),

		// System sans (default)
		retained.VStack("gap-1 w-full",
			retained.Text("font-sans (System)", "text-gray-500 text-xs"),
			retained.Text("The quick brown fox jumps over the lazy dog", "text-white text-lg font-sans"),
		),

		// Bundled serif font
		retained.VStack("gap-1 w-full",
			retained.Text("font-serif (Bundled TTF)", "text-gray-500 text-xs"),
			retained.Text("The quick brown fox jumps over the lazy dog", "text-white text-lg font-serif"),
		),

		// System mono
		retained.VStack("gap-1 w-full",
			retained.Text("font-mono (System)", "text-gray-500 text-xs"),
			retained.Text("The quick brown fox jumps over the lazy dog", "text-white text-lg font-mono"),
		),
	)

	// Form section
	refs.NameInput = retained.TextField("Your name", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500")
	refs.EmailInput = retained.TextField("Email address", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500")
	refs.PasswordInput = retained.TextField("Password", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500").
		SetPassword(true)
	refs.MessageInput = retained.TextArea("Your message...", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500 h-24")

	refs.SubmitButton = retained.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-lg px-6 py-2").
		WithChildren(
			retained.Text("Submit Form", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	refs.FormStatusText = retained.Text("Fill out the form and click submit", "text-gray-500 text-xs")

	formCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Form Inputs", "text-white text-lg font-semibold"),
		retained.Text("Test text inputs, password fields, and text areas", "text-gray-400 text-xs"),

		// Name field
		retained.VStack("gap-1 w-full",
			retained.Text("Name", "text-gray-300 text-sm"),
			refs.NameInput,
		),

		// Email field
		retained.VStack("gap-1 w-full",
			retained.Text("Email", "text-gray-300 text-sm"),
			refs.EmailInput,
		),

		// Password field
		retained.VStack("gap-1 w-full",
			retained.Text("Password", "text-gray-300 text-sm"),
			refs.PasswordInput,
		),

		// Message field
		retained.VStack("gap-1 w-full",
			retained.Text("Message", "text-gray-300 text-sm"),
			refs.MessageInput,
		),

		// Submit button and status
		retained.HStack("gap-3 items-center w-full",
			refs.SubmitButton,
			refs.FormStatusText,
		),
	)

	// Audio section
	refs.AudioWidget = retained.Audio("https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3", "")
	refs.AudioWidget.SetAudioAutoplay(false)

	refs.AudioStatusText = retained.Text("Click to play audio", "text-gray-400 text-xs")
	refs.AudioButton = retained.Container("bg-orange-500 hover:bg-orange-600 active:bg-orange-700 rounded-xl p-3").
		WithChildren(
			retained.Text("🔊 Audio", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Microphone section
	refs.MicWidget = retained.Microphone("")
	refs.MicWidget.SetMicrophoneAutoStart(false)

	refs.MicStatusText = retained.Text("Click to start recording", "text-gray-400 text-xs")
	refs.MicLevelText = retained.Text("Level: --", "text-gray-500 text-xs")
	refs.MicButton = retained.Container("bg-red-500 hover:bg-red-600 active:bg-red-700 rounded-xl p-3").
		WithChildren(
			retained.Text("🎤 Microphone", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Video section - use a CORS-enabled video source
	refs.VideoWidget = retained.VideoFromURL(
		"https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4",
		"w-full h-48 rounded-lg bg-gray-800 object-cover",
	).
		WithMuted().
		WithLoop().
		OnVideoError(func(err error) {
			log.Printf("VIDEO ERROR: %v", err)
			refs.VideoStatusText.SetText(fmt.Sprintf("Error: %v", err))
			refs.VideoStatusText.SetTextColor(retained.ColorRed400)
		}).
		OnVideoEnded(func() {
			log.Printf("Video ended")
			refs.VideoStatusText.SetText("Video ended, click to replay")
			refs.VideoStatusText.SetTextColor(retained.ColorGray400)
			videoPlaying = false
		})
	refs.VideoWidget.SetVideoAutoplay(false)

	refs.VideoStatusText = retained.Text("Click to play video", "text-gray-400 text-xs")
	refs.VideoButton = retained.Container("bg-cyan-500 hover:bg-cyan-600 active:bg-cyan-700 rounded-xl p-3").
		WithChildren(
			retained.Text("🎬 Video", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Camera section
	refs.CameraWidget = retained.CameraWithResolution(640, 480, 30, "w-full h-48 rounded-lg bg-gray-700 object-contain")
	refs.CameraWidget.SetCameraAutoStart(false)

	refs.CameraStatusText = retained.Text("Click to start camera", "text-gray-400 text-xs")
	refs.CameraButton = retained.Container("bg-pink-500 hover:bg-pink-600 active:bg-pink-700 rounded-xl p-3").
		WithChildren(
			retained.Text("📷 Camera", "text-white text-base font-semibold").
				WithPositionMode(retained.PositionRelative),
		)

	// Media card
	mediaCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		retained.Text("Media Testing", "text-white text-lg font-semibold"),
		retained.Text("Test audio, video, mic, and camera", "text-gray-400 text-xs"),

		// Audio row
		refs.AudioWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.AudioButton,
			retained.VStack("gap-1 flex-1",
				retained.Text("Audio Playback", "text-white text-sm"),
				refs.AudioStatusText,
			),
		),

		// Microphone row
		refs.MicWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.MicButton,
			retained.VStack("gap-1 flex-1",
				retained.Text("Microphone", "text-white text-sm"),
				refs.MicStatusText,
				refs.MicLevelText,
			),
		),

		// Video section
		retained.Text("Video Playback", "text-white text-sm"),
		refs.VideoWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.VideoButton,
			refs.VideoStatusText,
		),

		// Camera section
		retained.Text("Camera Preview", "text-white text-sm"),
		refs.CameraWidget,
		retained.HStack("gap-2 items-center w-full",
			refs.CameraButton,
			refs.CameraStatusText,
		),
	)

	// Platform support section
	platformCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		retained.Text("Platform Support", "text-white text-lg font-semibold"),
		retained.Text("✓ macOS", "text-blue-400 text-sm"),
		retained.Text("✓ iOS", "text-blue-400 text-sm"),
		retained.Text("✓ Android", "text-blue-400 text-sm"),
		retained.Text("✓ Web (this demo!)", "text-green-400 text-sm"),
		retained.Text("○ Windows (planned)", "text-gray-500 text-sm"),
		retained.Text("○ Linux (planned)", "text-gray-500 text-sm"),
	)

	// Scrolling info section
	scrollCard := retained.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		retained.Text("Scrolling Demo", "text-white text-lg font-semibold"),
		retained.Text("This content is scrollable!", "text-gray-400 text-sm"),
		retained.Text("Use mouse wheel or touch/drag to scroll", "text-gray-400 text-sm"),
		retained.Text("The same scroll behavior works on all platforms", "text-gray-400 text-sm"),
	)

	// Footer
	footer := retained.Text("Built with Centered Framework - Same code, all platforms", "text-gray-600 text-xs")

	// Scrollable content
	scrollContent := retained.VStack("gap-4 p-4 w-full",
		title,
		subtitle,
		counterCard,
		featureCard,
		fontCard,
		formCard,
		mediaCard,
		retained.Text("↓ Scroll down for more ↓", "text-gray-500 text-sm text-center"),
		platformCard,
		scrollCard,
		footer,
	)

	// Root container with vertical scrolling
	root := retained.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

func setupDemoHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Button 1 - Blue
	refs.Button1.OnClick(func(e *retained.MouseEvent) {
		clickCount++
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Blue button clicked!")
		refs.StatusText.SetTextColor(retained.ColorBlue400)

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
		refs.StatusText.SetText("Green button clicked! (+2)")
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
		refs.StatusText.SetText("Purple button clicked! (+5)")
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

	// Double click bonus
	refs.Button3.OnDoubleClick(func(e *retained.MouseEvent) {
		clickCount += 20
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("DOUBLE CLICK BONUS! (+20)")
		refs.StatusText.SetTextColor(retained.ColorYellow400)
	})

	// Form handlers
	setupFormHandlers(refs, anims)

	// Media handlers
	setupMediaHandlers(refs, anims)
}

func setupFormHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Submit button handler
	refs.SubmitButton.OnClick(func(e *retained.MouseEvent) {
		name := refs.NameInput.Text()
		email := refs.EmailInput.Text()
		password := refs.PasswordInput.Text()
		message := refs.MessageInput.Text()

		// Validate form
		if name == "" || email == "" || password == "" {
			refs.FormStatusText.SetText("Please fill in all required fields")
			refs.FormStatusText.SetTextColor(retained.ColorRed400)
			return
		}

		// Show success
		log.Printf("Form submitted: name=%s, email=%s, password=%s, message=%s", name, email, password, message)
		refs.FormStatusText.SetText(fmt.Sprintf("Submitted! Hello, %s!", name))
		refs.FormStatusText.SetTextColor(retained.ColorGreen400)

		// Animate button
		refs.SubmitButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})
}

func setupMediaHandlers(refs *DemoWidgetRefs, anims *retained.AnimationRegistry) {
	// Audio playback handler
	refs.AudioButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Audio button clicked, playing=%v", audioPlaying)

		if !audioPlaying {
			refs.AudioWidget.AudioPlay()
			audioPlaying = true
			refs.AudioStatusText.SetText("Playing... click to pause")
			refs.AudioStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			refs.AudioWidget.AudioPause()
			audioPlaying = false
			refs.AudioStatusText.SetText("Paused, click to resume")
			refs.AudioStatusText.SetTextColor(retained.ColorYellow400)
		}

		refs.AudioButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Microphone handler
	refs.MicWidget.OnMicrophoneLevelChange(func(level float32) {
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
			refs.MicStatusText.SetText("Recording... click to stop")
			refs.MicStatusText.SetTextColor(retained.ColorRed400)
		} else {
			refs.MicWidget.MicrophoneStop()
			micRecording = false
			refs.MicStatusText.SetText("Stopped, click to record")
			refs.MicStatusText.SetTextColor(retained.ColorGray400)
			refs.MicLevelText.SetText("Level: --")
		}

		refs.MicButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Video playback handler
	refs.VideoButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Video button clicked, playing=%v", videoPlaying)

		if !videoPlaying {
			err := refs.VideoWidget.VideoPlay()
			if err != nil {
				log.Printf("VideoPlay error: %v", err)
				refs.VideoStatusText.SetText(fmt.Sprintf("Play error: %v", err))
				refs.VideoStatusText.SetTextColor(retained.ColorRed400)
				return
			}
			videoPlaying = true
			refs.VideoStatusText.SetText("Playing... click to pause")
			refs.VideoStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			err := refs.VideoWidget.VideoPause()
			if err != nil {
				log.Printf("VideoPause error: %v", err)
			}
			videoPlaying = false
			refs.VideoStatusText.SetText("Paused, click to resume")
			refs.VideoStatusText.SetTextColor(retained.ColorYellow400)
		}

		refs.VideoButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})

	// Camera handler
	refs.CameraButton.OnClick(func(e *retained.MouseEvent) {
		log.Printf("Camera button clicked, active=%v", cameraActive)

		if !cameraActive {
			refs.CameraWidget.CameraStart()
			cameraActive = true
			refs.CameraStatusText.SetText("Camera active, click to stop")
			refs.CameraStatusText.SetTextColor(retained.ColorGreen400)
		} else {
			refs.CameraWidget.CameraStop()
			cameraActive = false
			refs.CameraStatusText.SetText("Stopped, click to start")
			refs.CameraStatusText.SetTextColor(retained.ColorGray400)
		}

		refs.CameraButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(retained.EaseOutBack)
	})
}
