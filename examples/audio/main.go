// Example demonstrating audio widgets with playback controls
//
// This example shows how to:
// 1. Load audio from local files (bundled assets)
// 2. Load audio from URLs (async)
// 3. Control playback (play, pause, stop, seek)
// 4. Configure autoplay, looping, and volume
// 5. Handle audio events (ended, error)
package main

import (
	"fmt"
	"runtime"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	// Lock the main goroutine to the main OS thread.
	// Required on macOS for windowing.
	runtime.LockOSThread()
}

// Bundled audio file (use absolute path for reliable testing)
var bundledAudioPath = func() string {
	// Get absolute path relative to working directory
	return "examples/audio/example.m4a"
}()

// State for the audio player UI
var (
	audioWidget *retained.Widget
	statusText  *retained.Widget
	timeText    *retained.Widget
	volumeText  *retained.Widget
	currentVolume float32 = 1.0
)

func main() {
	// Create the game loop
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)

	// Build the UI
	root := buildUI()
	loop.Tree().SetRoot(root)

	// Handle resize
	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	// Handle escape to exit
	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		// Space bar toggles play/pause
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeySpace) {
			togglePlayPause()
			return true
		}
		return false
	})

	// Run the event loop
	fmt.Println("Audio Widget Example - Press ESC to exit")
	fmt.Println("")
	fmt.Println("Controls:")
	fmt.Println("  SPACE - Toggle play/pause")
	fmt.Println("  Click buttons to control playback")

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Audio Widget Example"
	appConfig.Width = 800
	appConfig.Height = 600

	if err := loop.Run(appConfig); err != nil {
		fmt.Printf("Error: %v\n", err)
	}
}

func togglePlayPause() {
	if audioWidget == nil {
		return
	}
	if audioWidget.AudioIsPlaying() {
		audioWidget.AudioPause()
		updateStatus("Paused")
	} else {
		audioWidget.AudioPlay()
		updateStatus("Playing")
	}
}

func updateStatus(text string) {
	if statusText != nil {
		statusText.SetText(text)
	}
}

func updateTime(currentMs, durationMs uint64) {
	if timeText != nil {
		current := formatTime(currentMs)
		duration := formatTime(durationMs)
		timeText.SetText(fmt.Sprintf("%s / %s", current, duration))
	}
}

func formatTime(ms uint64) string {
	seconds := ms / 1000
	minutes := seconds / 60
	seconds = seconds % 60
	return fmt.Sprintf("%d:%02d", minutes, seconds)
}

func buildUI() *retained.Widget {
	// Create the main audio widget using local file
	audioWidget = retained.AudioFromFile(bundledAudioPath, "").
		WithAudioLoop().
		OnAudioEnded(func() {
			fmt.Println("Audio ended!")
			updateStatus("Ended (looping)")
		}).
		OnAudioError(func(err error) {
			fmt.Printf("Audio error: %v\n", err)
			updateStatus(fmt.Sprintf("Error: %v", err))
		}).
		OnAudioTimeUpdate(func(currentMs, durationMs uint64) {
			updateTime(currentMs, durationMs)
		})

	// Create status text
	statusText = retained.Text("Loading...", "text-sm text-gray-400")
	timeText = retained.Text("0:00 / 0:00", "text-sm text-gray-400 font-mono")
	volumeText = retained.Text("Volume: 100%", "text-sm text-gray-400")

	return retained.VStack("flex-1 bg-gray-900 p-8",
		// Header
		retained.Text("Audio Widget Example", "text-4xl font-bold text-white mb-2"),
		retained.Text("Background music and sound effects", "text-lg text-gray-400 mb-8"),

		// Main audio player card
		retained.VStack("bg-gray-800 rounded-xl p-6 gap-4",
			// Audio info
			retained.HStack("items-center gap-4",
				retained.Text("Now Playing:", "text-lg font-semibold text-white"),
				retained.Text("Test Tone (440 Hz)", "text-lg text-gray-300"),
			),

			// Audio widget (invisible - just manages playback)
			audioWidget,

			// Status and time
			retained.HStack("items-center gap-4",
				statusText,
				retained.Container("flex-1"),
				timeText,
			),

			// Playback controls
			retained.HStack("gap-4 justify-center",
				retained.Button("Play", "px-6 py-2 bg-green-600 hover:bg-green-500 text-white rounded-lg font-medium").
					OnClick(func(e *retained.MouseEvent) {
						if audioWidget != nil {
							audioWidget.AudioPlay()
							updateStatus("Playing")
						}
					}),
				retained.Button("Pause", "px-6 py-2 bg-yellow-600 hover:bg-yellow-500 text-white rounded-lg font-medium").
					OnClick(func(e *retained.MouseEvent) {
						if audioWidget != nil {
							audioWidget.AudioPause()
							updateStatus("Paused")
						}
					}),
				retained.Button("Stop", "px-6 py-2 bg-red-600 hover:bg-red-500 text-white rounded-lg font-medium").
					OnClick(func(e *retained.MouseEvent) {
						if audioWidget != nil {
							audioWidget.AudioStop()
							updateStatus("Stopped")
						}
					}),
				retained.Button("Restart", "px-6 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium").
					OnClick(func(e *retained.MouseEvent) {
						if audioWidget != nil {
							audioWidget.AudioSeek(0)
							audioWidget.AudioPlay()
							updateStatus("Restarted")
						}
					}),
			),

			// Volume controls
			retained.HStack("gap-4 justify-center items-center mt-4",
				retained.Text("Volume:", "text-white"),
				retained.Button("-", "w-10 h-10 bg-gray-700 hover:bg-gray-600 text-white rounded-lg font-bold").
					OnClick(func(e *retained.MouseEvent) {
						if audioWidget != nil {
							currentVolume -= 0.1
							if currentVolume < 0 {
								currentVolume = 0
							}
							audioWidget.SetAudioVolume(currentVolume)
							volumeText.SetText(fmt.Sprintf("Volume: %d%%", int(currentVolume*100)))
						}
					}),
				volumeText,
				retained.Button("+", "w-10 h-10 bg-gray-700 hover:bg-gray-600 text-white rounded-lg font-bold").
					OnClick(func(e *retained.MouseEvent) {
						if audioWidget != nil {
							currentVolume += 0.1
							if currentVolume > 1.0 {
								currentVolume = 1.0
							}
							audioWidget.SetAudioVolume(currentVolume)
							volumeText.SetText(fmt.Sprintf("Volume: %d%%", int(currentVolume*100)))
						}
					}),
			),
		),

		// Additional examples section
		retained.Text("Audio Options", "text-2xl font-semibold text-white mt-8 mb-4"),

		retained.HStack("gap-6",
			// Autoplay example
			audioCard(
				"Autoplay",
				"Starts playing automatically when loaded.",
				retained.AudioFromFile(bundledAudioPath, "").
					WithAudioAutoplay().
					WithAudioVolume(0.3), // Lower volume for background
			),

			// Looping example
			audioCard(
				"Looping",
				"Audio loops continuously.",
				retained.AudioFromFile(bundledAudioPath, "").
					WithAudioLoop().
					WithAudioVolume(0.3),
			),

			// Low volume example
			audioCard(
				"Low Volume",
				"Audio at 25% volume.",
				retained.AudioFromFile(bundledAudioPath, "").
					WithAudioVolume(0.25),
			),
		),

		// Instructions
		retained.Text("Press SPACE to toggle play/pause on the main audio", "text-sm text-gray-500 mt-4"),
		retained.Text("Note: Audio uses system default output device", "text-sm text-gray-500"),

		// Spacer
		retained.Container("flex-1"),
	)
}

// audioCard creates a card with audio controls and description
func audioCard(title, description string, audio *retained.Widget) *retained.Widget {
	var playBtn *retained.Widget
	isPlaying := false

	playBtn = retained.Button("Play", "px-4 py-1 bg-blue-600 hover:bg-blue-500 text-white rounded text-sm").
		OnClick(func(e *retained.MouseEvent) {
			if isPlaying {
				audio.AudioPause()
				playBtn.SetText("Play")
				isPlaying = false
			} else {
				audio.AudioPlay()
				playBtn.SetText("Pause")
				isPlaying = true
			}
		})

	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3 w-48",
		retained.Text(title, "text-lg font-semibold text-white"),
		audio, // Invisible audio widget
		playBtn,
		retained.Text(description, "text-sm text-gray-400"),
	)
}

// Suppress unused import warning
var _ = time.Second
