// Example demonstrating video widgets with playback controls
//
// This example shows how to:
// 1. Load videos from local files (bundled assets)
// 2. Load videos from URLs (async)
// 3. Control playback (play, pause, seek)
// 4. Configure autoplay, looping, and muting
// 5. Handle video events (ended, error)
package main

import (
	"fmt"
	"runtime"

	"github.com/agiangrant/ctd"
	"github.com/agiangrant/ctd/internal/ffi"
)

func init() {
	// Lock the main goroutine to the main OS thread.
	// Required on macOS for windowing.
	runtime.LockOSThread()
}

// Test video URL - Big Buck Bunny (10 second clip, 1MB)
const testVideoURL = "https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4"

// Bundled video file (relative to executable)
const bundledVideoPath = "examples/video/example.mp4"

// State for the video player UI
var (
	videoWidget *ctd.Widget
	statusText  *ctd.Widget
)

func main() {
	// Create the game loop
	config := ctd.DefaultLoopConfig()
	config.TargetFPS = 30
	loop := ctd.NewLoop(config)

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
	fmt.Println("Video Widget Example - Press ESC to exit")
	fmt.Println("")
	fmt.Println("Controls:")
	fmt.Println("  SPACE - Toggle play/pause")
	fmt.Println("  Click buttons to control playback")

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Video Widget Example"
	appConfig.Width = 1024
	appConfig.Height = 768

	if err := loop.Run(appConfig); err != nil {
		fmt.Printf("Error: %v\n", err)
	}
}

func togglePlayPause() {
	if videoWidget == nil {
		return
	}
	if videoWidget.VideoIsPlaying() {
		videoWidget.VideoPause()
		updateStatus("Paused")
	} else {
		videoWidget.VideoPlay()
		updateStatus("Playing")
	}
}

func updateStatus(text string) {
	if statusText != nil {
		statusText.SetText(text)
	}
}

func buildUI() *ctd.Widget {
	// Create the main video widget using bundled file
	videoWidget = ctd.VideoFromFile(bundledVideoPath, "w-full h-80 rounded-lg bg-gray-800").
		WithLoop().
		OnVideoEnded(func() {
			fmt.Println("Video ended!")
			updateStatus("Ended (looping)")
		}).
		OnVideoError(func(err error) {
			fmt.Printf("Video error: %v\n", err)
			updateStatus(fmt.Sprintf("Error: %v", err))
		})

	// Create status text
	statusText = ctd.Text("Loading...", "text-sm text-gray-400")

	return ctd.VStack("flex-1 bg-gray-900 p-8",
		// Header
		ctd.Text("Video Widget Example", "text-4xl font-bold text-white mb-2"),
		ctd.Text("Bundled video file + URL examples", "text-lg text-gray-400 mb-8"),

		// Main video player
		ctd.VStack("bg-gray-800 rounded-xl p-6 gap-4",
			// Video container
			videoWidget,

			// Status and controls
			ctd.HStack("items-center gap-4",
				statusText,
				ctd.Container("flex-1"),
			),

			// Playback controls
			ctd.HStack("gap-4 justify-center",
				ctd.Button("Play", "px-6 py-2 bg-green-600 hover:bg-green-500 text-white rounded-lg font-medium").
					OnClick(func(e *ctd.MouseEvent) {
						if videoWidget != nil {
							videoWidget.VideoPlay()
							updateStatus("Playing")
						}
					}),
				ctd.Button("Pause", "px-6 py-2 bg-yellow-600 hover:bg-yellow-500 text-white rounded-lg font-medium").
					OnClick(func(e *ctd.MouseEvent) {
						if videoWidget != nil {
							videoWidget.VideoPause()
							updateStatus("Paused")
						}
					}),
				ctd.Button("Restart", "px-6 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium").
					OnClick(func(e *ctd.MouseEvent) {
						if videoWidget != nil {
							videoWidget.VideoSeek(0)
							videoWidget.VideoPlay()
							updateStatus("Restarted")
						}
					}),
			),
		),

		// Additional examples
		ctd.Text("Video Options", "text-2xl font-semibold text-white mt-8 mb-4"),

		ctd.HStack("gap-6",
			// Bundled file example
			videoCard(
				"Bundled File",
				"Loaded from local file (instant).",
				ctd.VideoFromFile(bundledVideoPath, "w-48 h-32 rounded-lg").
					WithAutoplay().
					WithMuted().
					WithLoop(),
			),

			// URL example with autoplay
			videoCard(
				"URL + Autoplay",
				"Loaded from URL (downloads first).",
				ctd.VideoFromURL(testVideoURL, "w-48 h-32 rounded-lg").
					WithAutoplay().
					WithMuted().
					WithLoop(),
			),

			// No loop example
			videoCard(
				"No Loop",
				"Video plays once then stops.",
				ctd.VideoFromFile(bundledVideoPath, "w-48 h-32 rounded-lg").
					WithAutoplay().
					WithMuted(),
			),
		),

		// Instructions
		ctd.Text("Press SPACE to toggle play/pause on the main video", "text-sm text-gray-500 mt-4"),

		// Spacer
		ctd.Container("flex-1"),
	)
}

// videoCard creates a card with a video and description
func videoCard(title, description string, video *ctd.Widget) *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text(title, "text-lg font-semibold text-white"),
		video,
		ctd.Text(description, "text-sm text-gray-400 max-w-xs"),
	)
}
