//go:build js && wasm

// Package main demonstrates the Centered framework running in the browser.
// This uses the SAME retained mode API as iOS/Android/desktop demos.
package main

import (
	"fmt"
	"log"
	"time"

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

// Global state
var (
	demoLoop   *ctd.Loop
	clickCount int

	// Media state
	audioPlaying bool
	micRecording bool
	videoPlaying bool
	cameraActive bool
)

func main() {
	log.Println("=== Starting Centered Web Demo ===")

	config := ctd.DefaultLoopConfig()
	demoLoop = ctd.NewLoop(config)
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
	CounterText *ctd.Widget
	StatusText  *ctd.Widget
	Button1     *ctd.Widget
	Button2     *ctd.Widget
	Button3     *ctd.Widget

	// Form widgets
	NameInput      *ctd.Widget
	EmailInput     *ctd.Widget
	PasswordInput  *ctd.Widget
	MessageInput   *ctd.Widget
	SubmitButton   *ctd.Widget
	FormStatusText *ctd.Widget

	// Media widgets
	AudioWidget     *ctd.Widget
	AudioButton     *ctd.Widget
	AudioStatusText *ctd.Widget

	MicWidget     *ctd.Widget
	MicButton     *ctd.Widget
	MicStatusText *ctd.Widget
	MicLevelText  *ctd.Widget

	VideoWidget     *ctd.Widget
	VideoButton     *ctd.Widget
	VideoStatusText *ctd.Widget

	CameraWidget     *ctd.Widget
	CameraButton     *ctd.Widget
	CameraStatusText *ctd.Widget
}

func buildDemoUI() (*ctd.Widget, *DemoWidgetRefs) {
	refs := &DemoWidgetRefs{}

	// Title
	title := ctd.Text("Centered Web Demo", "text-white text-2xl font-bold")

	// Subtitle
	subtitle := ctd.Text("Same code runs on iOS, Android, Desktop, and Web!", "text-gray-400 text-base")

	// Counter card
	counterLabel := ctd.Text("Click Counter", "text-gray-400 text-sm")
	refs.CounterText = ctd.Text("0", "text-white text-6xl font-bold")

	// Button row
	refs.Button1 = ctd.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Blue (+1)", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button2 = ctd.Container("bg-green-500 hover:bg-green-600 active:bg-green-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Green (+2)", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.Button3 = ctd.Container("bg-purple-500 hover:bg-purple-600 active:bg-purple-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("Purple (+5)", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Status text
	refs.StatusText = ctd.Text("Click a button to interact", "text-gray-500 text-sm")

	// Counter card
	counterCard := ctd.VStack("bg-gray-800 rounded-2xl p-6 gap-4 w-full items-center",
		counterLabel,
		refs.CounterText,
		ctd.HStack("gap-4",
			refs.Button1,
			refs.Button2,
			refs.Button3,
		),
		refs.StatusText,
	)

	// Feature list
	featureCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Cross-Platform Features", "text-white text-lg font-semibold"),
		ctd.Text("✓ Tailwind CSS classes", "text-green-400 text-sm"),
		ctd.Text("✓ Same Go code on all platforms", "text-green-400 text-sm"),
		ctd.Text("✓ Mouse/touch interactions", "text-green-400 text-sm"),
		ctd.Text("✓ Hover/Active states", "text-green-400 text-sm"),
		ctd.Text("✓ Animations", "text-green-400 text-sm"),
		ctd.Text("✓ Scrolling (scroll down!)", "text-green-400 text-sm"),
		ctd.Text("✓ Bundled custom fonts", "text-green-400 text-sm"),
	)

	// Bundled fonts demo - uses font-serif from theme.toml which is a bundled TTF
	fontCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Bundled Fonts", "text-white text-lg font-semibold"),
		ctd.Text("Custom fonts loaded from TTF files", "text-gray-400 text-xs"),

		// System sans (default)
		ctd.VStack("gap-1 w-full",
			ctd.Text("font-sans (System)", "text-gray-500 text-xs"),
			ctd.Text("The quick brown fox jumps over the lazy dog", "text-white text-lg font-sans"),
		),

		// Bundled serif font
		ctd.VStack("gap-1 w-full",
			ctd.Text("font-serif (Bundled TTF)", "text-gray-500 text-xs"),
			ctd.Text("The quick brown fox jumps over the lazy dog", "text-white text-lg font-serif"),
		),

		// System mono
		ctd.VStack("gap-1 w-full",
			ctd.Text("font-mono (System)", "text-gray-500 text-xs"),
			ctd.Text("The quick brown fox jumps over the lazy dog", "text-white text-lg font-mono"),
		),
	)

	// Form section
	refs.NameInput = ctd.TextField("Your name", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500")
	refs.EmailInput = ctd.TextField("Email address", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500")
	refs.PasswordInput = ctd.TextField("Password", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500").
		SetPassword(true)
	refs.MessageInput = ctd.TextArea("Your message...", "w-full px-3 py-2 bg-gray-700 text-white rounded-lg border border-gray-600 focus:border-blue-500 h-24")

	refs.SubmitButton = ctd.Container("bg-blue-500 hover:bg-blue-600 active:bg-blue-700 rounded-lg px-6 py-2").
		WithChildren(
			ctd.Text("Submit Form", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	refs.FormStatusText = ctd.Text("Fill out the form and click submit", "text-gray-500 text-xs")

	formCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Form Inputs", "text-white text-lg font-semibold"),
		ctd.Text("Test text inputs, password fields, and text areas", "text-gray-400 text-xs"),

		// Name field
		ctd.VStack("gap-1 w-full",
			ctd.Text("Name", "text-gray-300 text-sm"),
			refs.NameInput,
		),

		// Email field
		ctd.VStack("gap-1 w-full",
			ctd.Text("Email", "text-gray-300 text-sm"),
			refs.EmailInput,
		),

		// Password field
		ctd.VStack("gap-1 w-full",
			ctd.Text("Password", "text-gray-300 text-sm"),
			refs.PasswordInput,
		),

		// Message field
		ctd.VStack("gap-1 w-full",
			ctd.Text("Message", "text-gray-300 text-sm"),
			refs.MessageInput,
		),

		// Submit button and status
		ctd.HStack("gap-3 items-center w-full",
			refs.SubmitButton,
			refs.FormStatusText,
		),
	)

	// Audio section
	refs.AudioWidget = ctd.Audio("https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3", "")
	refs.AudioWidget.SetAudioAutoplay(false)

	refs.AudioStatusText = ctd.Text("Click to play audio", "text-gray-400 text-xs")
	refs.AudioButton = ctd.Container("bg-orange-500 hover:bg-orange-600 active:bg-orange-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("🔊 Audio", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Microphone section
	refs.MicWidget = ctd.Microphone("")
	refs.MicWidget.SetMicrophoneAutoStart(false)

	refs.MicStatusText = ctd.Text("Click to start recording", "text-gray-400 text-xs")
	refs.MicLevelText = ctd.Text("Level: --", "text-gray-500 text-xs")
	refs.MicButton = ctd.Container("bg-red-500 hover:bg-red-600 active:bg-red-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("🎤 Microphone", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Video section - use a CORS-enabled video source
	refs.VideoWidget = ctd.VideoFromURL(
		"https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4",
		"w-full h-48 rounded-lg bg-gray-800 object-cover",
	).
		WithMuted().
		WithLoop().
		OnVideoError(func(err error) {
			log.Printf("VIDEO ERROR: %v", err)
			refs.VideoStatusText.SetText(fmt.Sprintf("Error: %v", err))
			refs.VideoStatusText.SetTextColor(ctd.ColorRed400)
		}).
		OnVideoEnded(func() {
			log.Printf("Video ended")
			refs.VideoStatusText.SetText("Video ended, click to replay")
			refs.VideoStatusText.SetTextColor(ctd.ColorGray400)
			videoPlaying = false
		})
	refs.VideoWidget.SetVideoAutoplay(false)

	refs.VideoStatusText = ctd.Text("Click to play video", "text-gray-400 text-xs")
	refs.VideoButton = ctd.Container("bg-cyan-500 hover:bg-cyan-600 active:bg-cyan-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("🎬 Video", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Camera section
	refs.CameraWidget = ctd.CameraWithResolution(640, 480, 30, "w-full h-48 rounded-lg bg-gray-700 object-contain")
	refs.CameraWidget.SetCameraAutoStart(false)

	refs.CameraStatusText = ctd.Text("Click to start camera", "text-gray-400 text-xs")
	refs.CameraButton = ctd.Container("bg-pink-500 hover:bg-pink-600 active:bg-pink-700 rounded-xl p-3").
		WithChildren(
			ctd.Text("📷 Camera", "text-white text-base font-semibold").
				WithPositionMode(ctd.PositionRelative),
		)

	// Media card
	mediaCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-3 w-full",
		ctd.Text("Media Testing", "text-white text-lg font-semibold"),
		ctd.Text("Test audio, video, mic, and camera", "text-gray-400 text-xs"),

		// Audio row
		refs.AudioWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.AudioButton,
			ctd.VStack("gap-1 flex-1",
				ctd.Text("Audio Playback", "text-white text-sm"),
				refs.AudioStatusText,
			),
		),

		// Microphone row
		refs.MicWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.MicButton,
			ctd.VStack("gap-1 flex-1",
				ctd.Text("Microphone", "text-white text-sm"),
				refs.MicStatusText,
				refs.MicLevelText,
			),
		),

		// Video section
		ctd.Text("Video Playback", "text-white text-sm"),
		refs.VideoWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.VideoButton,
			refs.VideoStatusText,
		),

		// Camera section
		ctd.Text("Camera Preview", "text-white text-sm"),
		refs.CameraWidget,
		ctd.HStack("gap-2 items-center w-full",
			refs.CameraButton,
			refs.CameraStatusText,
		),
	)

	// Platform support section
	platformCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Platform Support", "text-white text-lg font-semibold"),
		ctd.Text("✓ macOS", "text-blue-400 text-sm"),
		ctd.Text("✓ iOS", "text-blue-400 text-sm"),
		ctd.Text("✓ Android", "text-blue-400 text-sm"),
		ctd.Text("✓ Web (this demo!)", "text-green-400 text-sm"),
		ctd.Text("○ Windows (planned)", "text-gray-500 text-sm"),
		ctd.Text("○ Linux (planned)", "text-gray-500 text-sm"),
	)

	// Scrolling info section
	scrollCard := ctd.VStack("bg-gray-800 rounded-2xl p-4 gap-2 w-full",
		ctd.Text("Scrolling Demo", "text-white text-lg font-semibold"),
		ctd.Text("This content is scrollable!", "text-gray-400 text-sm"),
		ctd.Text("Use mouse wheel or touch/drag to scroll", "text-gray-400 text-sm"),
		ctd.Text("The same scroll behavior works on all platforms", "text-gray-400 text-sm"),
	)

	// Footer
	footer := ctd.Text("Built with Centered Framework - Same code, all platforms", "text-gray-600 text-xs")

	// Scrollable content
	scrollContent := ctd.VStack("gap-4 p-4 w-full",
		title,
		subtitle,
		counterCard,
		featureCard,
		fontCard,
		formCard,
		mediaCard,
		ctd.Text("↓ Scroll down for more ↓", "text-gray-500 text-sm text-center"),
		platformCard,
		scrollCard,
		footer,
	)

	// Root container with vertical scrolling
	root := ctd.VStack("bg-gray-900 w-full h-full overflow-y-auto flex flex-col").
		WithChildren(scrollContent)

	return root, refs
}

func setupDemoHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Button 1 - Blue
	refs.Button1.OnClick(func(e *ctd.MouseEvent) {
		clickCount++
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("Blue button clicked!")
		refs.StatusText.SetTextColor(ctd.ColorBlue400)

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
		refs.StatusText.SetText("Green button clicked! (+2)")
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
		refs.StatusText.SetText("Purple button clicked! (+5)")
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

	// Double click bonus
	refs.Button3.OnDoubleClick(func(e *ctd.MouseEvent) {
		clickCount += 20
		refs.CounterText.SetText(fmt.Sprintf("%d", clickCount))
		refs.StatusText.SetText("DOUBLE CLICK BONUS! (+20)")
		refs.StatusText.SetTextColor(ctd.ColorYellow400)
	})

	// Form handlers
	setupFormHandlers(refs, anims)

	// Media handlers
	setupMediaHandlers(refs, anims)
}

func setupFormHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Submit button handler
	refs.SubmitButton.OnClick(func(e *ctd.MouseEvent) {
		name := refs.NameInput.Text()
		email := refs.EmailInput.Text()
		password := refs.PasswordInput.Text()
		message := refs.MessageInput.Text()

		// Validate form
		if name == "" || email == "" || password == "" {
			refs.FormStatusText.SetText("Please fill in all required fields")
			refs.FormStatusText.SetTextColor(ctd.ColorRed400)
			return
		}

		// Show success
		log.Printf("Form submitted: name=%s, email=%s, password=%s, message=%s", name, email, password, message)
		refs.FormStatusText.SetText(fmt.Sprintf("Submitted! Hello, %s!", name))
		refs.FormStatusText.SetTextColor(ctd.ColorGreen400)

		// Animate button
		refs.SubmitButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})
}

func setupMediaHandlers(refs *DemoWidgetRefs, anims *ctd.AnimationRegistry) {
	// Audio playback handler
	refs.AudioButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Audio button clicked, playing=%v", audioPlaying)

		if !audioPlaying {
			refs.AudioWidget.AudioPlay()
			audioPlaying = true
			refs.AudioStatusText.SetText("Playing... click to pause")
			refs.AudioStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			refs.AudioWidget.AudioPause()
			audioPlaying = false
			refs.AudioStatusText.SetText("Paused, click to resume")
			refs.AudioStatusText.SetTextColor(ctd.ColorYellow400)
		}

		refs.AudioButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
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
			refs.MicStatusText.SetText("Recording... click to stop")
			refs.MicStatusText.SetTextColor(ctd.ColorRed400)
		} else {
			refs.MicWidget.MicrophoneStop()
			micRecording = false
			refs.MicStatusText.SetText("Stopped, click to record")
			refs.MicStatusText.SetTextColor(ctd.ColorGray400)
			refs.MicLevelText.SetText("Level: --")
		}

		refs.MicButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Video playback handler
	refs.VideoButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Video button clicked, playing=%v", videoPlaying)

		if !videoPlaying {
			err := refs.VideoWidget.VideoPlay()
			if err != nil {
				log.Printf("VideoPlay error: %v", err)
				refs.VideoStatusText.SetText(fmt.Sprintf("Play error: %v", err))
				refs.VideoStatusText.SetTextColor(ctd.ColorRed400)
				return
			}
			videoPlaying = true
			refs.VideoStatusText.SetText("Playing... click to pause")
			refs.VideoStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			err := refs.VideoWidget.VideoPause()
			if err != nil {
				log.Printf("VideoPause error: %v", err)
			}
			videoPlaying = false
			refs.VideoStatusText.SetText("Paused, click to resume")
			refs.VideoStatusText.SetTextColor(ctd.ColorYellow400)
		}

		refs.VideoButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})

	// Camera handler
	refs.CameraButton.OnClick(func(e *ctd.MouseEvent) {
		log.Printf("Camera button clicked, active=%v", cameraActive)

		if !cameraActive {
			refs.CameraWidget.CameraStart()
			cameraActive = true
			refs.CameraStatusText.SetText("Camera active, click to stop")
			refs.CameraStatusText.SetTextColor(ctd.ColorGreen400)
		} else {
			refs.CameraWidget.CameraStop()
			cameraActive = false
			refs.CameraStatusText.SetText("Stopped, click to start")
			refs.CameraStatusText.SetTextColor(ctd.ColorGray400)
		}

		refs.CameraButton.Animate(anims).
			Duration(100 * time.Millisecond).
			Easing(ctd.EaseOutBack)
	})
}
