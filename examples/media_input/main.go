// Example demonstrating microphone and camera input widgets.
// Shows device enumeration, permission handling, and live audio level visualization.
package main

import (
	"fmt"
	"log"
	"runtime"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)
	tree := loop.Tree()

	// State for UI updates
	var micLevelBar *retained.Widget
	var statusText *retained.Widget
	var micDeviceText *retained.Widget
	var camDeviceText *retained.Widget
	var cameraPreview *retained.Widget
	var micWidget *retained.Widget
	var camWidget *retained.Widget

	// Build the UI
	root := retained.VStack("flex-1 p-6 bg-gray-900",
		// Title
		retained.Text("Media Input Demo", "text-2xl font-bold text-white mb-4"),

		// Device lists section
		retained.HStack("w-full mb-4",
			// Microphone devices panel
			retained.VStack("flex-1 p-4 bg-gray-800 rounded-lg mr-2",
				retained.Text("Microphones", "text-lg font-semibold text-white mb-2"),
				buildDeviceText(&micDeviceText),
			),

			// Camera devices panel
			retained.VStack("flex-1 p-4 bg-gray-800 rounded-lg ml-2",
				retained.Text("Cameras", "text-lg font-semibold text-white mb-2"),
				buildDeviceText(&camDeviceText),
			),
		),

		// Microphone section
		retained.VStack("w-full p-4 bg-gray-800 rounded-lg mb-4",
			retained.Text("Microphone", "text-lg font-semibold text-white mb-2"),

			// Audio level visualization
			retained.HStack("items-center mb-2",
				retained.Text("Level:", "text-white mr-2"),
				buildLevelBar(&micLevelBar),
			),

			// Start/Stop buttons
			retained.HStack("gap-2 mb-2",
				retained.Button("Start Mic", "px-3 py-1 bg-green-600 hover:bg-green-700 text-white rounded").
					OnClick(func(e *retained.MouseEvent) {
						if micWidget != nil {
							if err := micWidget.MicrophoneStart(); err != nil {
								log.Printf("Failed to start microphone: %v", err)
							}
						}
					}),
				retained.Button("Stop Mic", "px-3 py-1 bg-red-600 hover:bg-red-700 text-white rounded").
					OnClick(func(e *retained.MouseEvent) {
						if micWidget != nil {
							if err := micWidget.MicrophoneStop(); err != nil {
								log.Printf("Failed to stop microphone: %v", err)
							}
						}
					}),
			),

			// Microphone widget (invisible - just captures audio)
			buildMicrophone(&micWidget, &micLevelBar, &statusText),
		),

		// Camera section
		retained.VStack("w-full p-4 bg-gray-800 rounded-lg mb-4",
			retained.Text("Camera", "text-lg font-semibold text-white mb-2"),

			// Camera preview display (Video widget receives frames from Camera)
			buildCameraPreview(&cameraPreview),

			// Start/Stop buttons
			retained.HStack("gap-2 mt-2 mb-2",
				retained.Button("Start Camera", "px-3 py-1 bg-green-600 hover:bg-green-700 text-white rounded").
					OnClick(func(e *retained.MouseEvent) {
						if camWidget != nil {
							if err := camWidget.CameraStart(); err != nil {
								log.Printf("Failed to start camera: %v", err)
							}
						}
					}),
				retained.Button("Stop Camera", "px-3 py-1 bg-red-600 hover:bg-red-700 text-white rounded").
					OnClick(func(e *retained.MouseEvent) {
						if camWidget != nil {
							if err := camWidget.CameraStop(); err != nil {
								log.Printf("Failed to stop camera: %v", err)
							}
						}
					}),
			),

			// Camera input (invisible data source - provides frames to Video widget)
			buildCamera(&camWidget, &cameraPreview, &statusText),
		),

		// Status section
		retained.VStack("w-full p-4 bg-gray-700 rounded-lg",
			buildStatusText(&statusText),
		),

		// Spacer to push content up (use flex-1 Container as spacer)
		retained.Container("flex-1"),

		// Help text
		retained.Text("Press ESC to quit", "text-gray-400 text-sm"),
	)

	tree.SetRoot(root)

	// Load device lists asynchronously
	go func() {
		// List microphone devices
		micDevices, err := retained.ListMicrophoneDevices()
		if err != nil {
			log.Printf("Error listing microphones: %v", err)
		} else {
			deviceText := ""
			for i, dev := range micDevices {
				defaultMarker := ""
				if dev.IsDefault {
					defaultMarker = " (default)"
				}
				deviceText += fmt.Sprintf("%d. %s%s\n", i+1, dev.Name, defaultMarker)
			}
			if deviceText == "" {
				deviceText = "No microphones found"
			}
			if micDeviceText != nil {
				micDeviceText.SetText(deviceText)
			}
		}

		// List camera devices
		camDevices, err := retained.ListCameraDevices()
		if err != nil {
			log.Printf("Error listing cameras: %v", err)
		} else {
			deviceText := ""
			for i, dev := range camDevices {
				defaultMarker := ""
				if dev.IsDefault {
					defaultMarker = " (default)"
				}
				deviceText += fmt.Sprintf("%d. %s%s\n", i+1, dev.Name, defaultMarker)
			}
			if deviceText == "" {
				deviceText = "No cameras found"
			}
			if camDeviceText != nil {
				camDeviceText.SetText(deviceText)
			}
		}
	}()

	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Media Input Demo"
	appConfig.Width = 800
	appConfig.Height = 700

	log.Println("Starting Media Input Demo...")
	log.Println("  - Microphone level bar shows audio input level")
	log.Println("  - Camera preview shows live video (when implemented)")
	log.Println("  - Press ESC to quit")
	log.Println("")
	log.Println("Note: You may need to grant microphone/camera permissions")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

// buildLevelBar creates the audio level visualization bar
func buildLevelBar(ref **retained.Widget) *retained.Widget {
	levelBar := retained.Container("w-2 h-4 bg-green-500 rounded")
	*ref = levelBar

	container := retained.HStack("w-52 h-4 bg-gray-600 rounded overflow-hidden",
		levelBar,
	)

	return container
}

// buildStatusText creates the status text widget
func buildStatusText(ref **retained.Widget) *retained.Widget {
	w := retained.Text("Initializing...", "text-white")
	*ref = w
	return w
}

// buildDeviceText creates a text widget for device list
func buildDeviceText(ref **retained.Widget) *retained.Widget {
	w := retained.Text("Loading devices...", "text-gray-300 text-sm")
	*ref = w
	return w
}

// buildCameraPreview creates a Video widget for displaying camera frames
func buildCameraPreview(ref **retained.Widget) *retained.Widget {
	w := retained.VideoStream(640, 480, "w-full h-64 rounded-lg bg-gray-700")
	*ref = w
	return w
}

// buildMicrophone creates a Microphone widget (invisible data source)
func buildMicrophone(ref **retained.Widget, micLevelBar, statusText **retained.Widget) *retained.Widget {
	w := retained.Microphone("").
		OnMicrophoneLevelChange(func(level float32) {
			// Update level bar width based on audio level (as percentage)
			if *micLevelBar != nil {
				// Convert level (0-1) to percentage (1% minimum for visibility)
				percent := level * 100
				if percent < 1 {
					percent = 1
				}
				if percent > 100 {
					percent = 100
				}
				(*micLevelBar).SetWidthPercent(percent)
			}
		}).
		OnMicrophoneStateChange(func(state int32) {
			stateStr := microphoneStateString(state)
			if *statusText != nil {
				(*statusText).SetText(fmt.Sprintf("Microphone: %s", stateStr))
			}
		}).
		OnMicrophoneError(func(err error) {
			log.Printf("Microphone error: %v", err)
			if *statusText != nil {
				(*statusText).SetText(fmt.Sprintf("Mic Error: %v", err))
			}
		})
	*ref = w
	return w
}

// buildCamera creates a Camera widget (invisible data source)
func buildCamera(ref **retained.Widget, cameraPreview, statusText **retained.Widget) *retained.Widget {
	w := retained.Camera("").
		OnCameraFrame(func(textureID uint32) {
			// Forward camera frames to the Video widget for display
			if *cameraPreview != nil {
				(*cameraPreview).SetTextureID(textureID)
			}
		}).
		OnCameraStateChange(func(state int32) {
			stateStr := cameraStateString(state)
			if *statusText != nil {
				(*statusText).SetText(fmt.Sprintf("Camera: %s", stateStr))
			}
		}).
		OnCameraError(func(err error) {
			log.Printf("Camera error: %v", err)
			if *statusText != nil {
				(*statusText).SetText(fmt.Sprintf("Camera Error: %v", err))
			}
		})
	*ref = w
	return w
}

// microphoneStateString converts state code to string
func microphoneStateString(state int32) string {
	switch ffi.AudioInputState(state) {
	case ffi.AudioInputStateIdle:
		return "Idle"
	case ffi.AudioInputStateRequestingPermission:
		return "Requesting Permission..."
	case ffi.AudioInputStateReady:
		return "Ready"
	case ffi.AudioInputStateCapturing:
		return "Capturing"
	case ffi.AudioInputStateStopped:
		return "Stopped"
	case ffi.AudioInputStateError:
		return "Error"
	default:
		return fmt.Sprintf("Unknown (%d)", state)
	}
}

// cameraStateString converts state code to string
func cameraStateString(state int32) string {
	switch ffi.VideoInputState(state) {
	case ffi.VideoInputStateIdle:
		return "Idle"
	case ffi.VideoInputStateRequestingPermission:
		return "Requesting Permission..."
	case ffi.VideoInputStateReady:
		return "Ready"
	case ffi.VideoInputStateCapturing:
		return "Capturing"
	case ffi.VideoInputStateStopped:
		return "Stopped"
	case ffi.VideoInputStateError:
		return "Error"
	default:
		return fmt.Sprintf("Unknown (%d)", state)
	}
}
