//go:build !js

// Package ffi provides Go bindings to the Centered Rust engine via purego.
// This implementation uses purego for FFI, eliminating the need for CGo.
// This enables cross-compilation and mobile platform support.
package ffi

import (
	"encoding/json"
	"fmt"
	"log"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
)

// ============================================================================
// Library Loading
// ============================================================================

var (
	libHandle   uintptr
	libOnce     sync.Once
	libErr      error
	initialized bool
)

// Library function pointers (populated by initLibrary)
var (
	// Core app functions
	fnAppRun           func(config uintptr, callback uintptr) int32
	fnAppRequestExit   func()
	fnAppRequestRedraw func() int32
	fnEngineVersion    func() uintptr

	// Window control functions
	fnWindowMinimize        func() int32
	fnWindowToggleMaximize  func() int32
	fnWindowEnterFullscreen func() int32
	fnWindowExitFullscreen  func() int32
	fnWindowToggleFullscreen func() int32
	fnWindowClose           func() int32
	fnWindowSetTitle        func(title uintptr) int32

	// Image/texture functions
	fnLoadImage      func(dataPtr uintptr, dataLen uint64) int32
	fnLoadImageFile  func(path uintptr) int32
	fnUnloadImage    func(textureID uint32) int32
	fnGetTextureSize func(textureID uint32, widthOut uintptr, heightOut uintptr) int32

	// Text measurement functions
	fnMeasureText         func(text uintptr, fontName uintptr, fontSize float32) TextMeasurementC
	fnMeasureTextPtr      func(text uintptr, fontName uintptr, fontSize float32, out uintptr) int32 // iOS-compatible version
	fnMeasureTextWidth    func(text uintptr, fontName uintptr, fontSize float32) float32
	fnMeasureTextToCursor func(text uintptr, charIndex uint32, fontName uintptr, fontSize float32) float32
	fnMeasureTextWithFont func(text uintptr, fontJSON uintptr) float32
	fnGetScaleFactor      func() float64

	// Safe area insets (iOS/Android)
	fnGetSafeAreaInsetsPtr func(out uintptr) int32

	// Audio playback functions
	fnAudioCreate     func() uint32
	fnAudioDestroy    func(playerID uint32)
	fnAudioLoadURL    func(playerID uint32, url uintptr) int32
	fnAudioLoadFile   func(playerID uint32, path uintptr) int32
	fnAudioPlay       func(playerID uint32) int32
	fnAudioPause      func(playerID uint32) int32
	fnAudioStop       func(playerID uint32) int32
	fnAudioSeek       func(playerID uint32, timestampMs uint64) int32
	fnAudioSetLooping func(playerID uint32, looping bool) int32
	fnAudioSetVolume  func(playerID uint32, volume float32) int32
	fnAudioGetState   func(playerID uint32) int32
	fnAudioGetTime    func(playerID uint32) uint64
	fnAudioGetInfo    func(playerID uint32, durationOut uintptr, sampleRateOut uintptr, channelsOut uintptr) int32
	fnAudioGetVolume  func(playerID uint32) float32
	fnAudioIsLooping  func(playerID uint32) int32
	fnAudioUpdate     func(playerID uint32) int32

	// Video playback functions
	fnVideoCreate       func() uint32
	fnVideoDestroy      func(playerID uint32)
	fnVideoLoadURL      func(playerID uint32, url uintptr) int32
	fnVideoLoadFile     func(playerID uint32, path uintptr) int32
	fnVideoInitStream   func(playerID uint32, width uint32, height uint32) int32
	fnVideoPushFrame    func(playerID uint32, width uint32, height uint32, dataPtr uintptr, dataLen uint64, timestampMs uint64) int32
	fnVideoPlay         func(playerID uint32) int32
	fnVideoPause        func(playerID uint32) int32
	fnVideoSeek         func(playerID uint32, timestampMs uint64) int32
	fnVideoSetLooping   func(playerID uint32, looping bool) int32
	fnVideoSetMuted     func(playerID uint32, muted bool) int32
	fnVideoSetVolume    func(playerID uint32, volume float32) int32
	fnVideoGetState     func(playerID uint32) int32
	fnVideoGetTime      func(playerID uint32) uint64
	fnVideoGetInfo      func(playerID uint32, widthOut uintptr, heightOut uintptr, durationOut uintptr) int32
	fnVideoUpdate       func(playerID uint32) int32
	fnVideoGetTextureID func(playerID uint32) uint32

	// Audio input functions
	fnAudioInputCreate            func() uint32
	fnAudioInputDestroy           func(inputID uint32)
	fnAudioInputRequestPermission func(inputID uint32) int32
	fnAudioInputHasPermission     func(inputID uint32) int32
	fnAudioInputListDevices       func(inputID uint32) uintptr
	fnAudioInputOpen              func(inputID uint32, deviceID uintptr, sampleRate uint32, channels uint32) int32
	fnAudioInputStart             func(inputID uint32) int32
	fnAudioInputStop              func(inputID uint32) int32
	fnAudioInputClose             func(inputID uint32)
	fnAudioInputGetState          func(inputID uint32) int32
	fnAudioInputGetLevel          func(inputID uint32) float32

	// Video input functions
	fnVideoInputCreate            func() uint32
	fnVideoInputDestroy           func(inputID uint32)
	fnVideoInputRequestPermission func(inputID uint32) int32
	fnVideoInputHasPermission     func(inputID uint32) int32
	fnVideoInputListDevices       func(inputID uint32) uintptr
	fnVideoInputOpen              func(inputID uint32, deviceID uintptr, width uint32, height uint32, frameRate uint32) int32
	fnVideoInputStart             func(inputID uint32) int32
	fnVideoInputStop              func(inputID uint32) int32
	fnVideoInputClose             func(inputID uint32)
	fnVideoInputGetState          func(inputID uint32) int32
	fnVideoInputGetDimensions     func(inputID uint32, widthOut uintptr, heightOut uintptr) int32
	fnVideoInputGetFrameTexture   func(inputID uint32, existingTextureID uint32) int32

	// System functions
	fnSystemDarkMode func() int32
	fnFreeString     func(ptr uintptr)

	// iOS-specific functions (only loaded on iOS)
	fnIosSetReadyCallback func(callback uintptr)
	fnIosMain             func(argc int32, argv uintptr) int32

	// Android-specific functions (only loaded on Android)
	fnAndroidSetReadyCallback func(callback uintptr)

	// Clipboard functions (Rust implementation)
	fnClipboardGet func() uintptr
	fnClipboardSet func(text uintptr)

	// Keyboard functions (iOS)
	fnKeyboardShow      func()
	fnKeyboardHide      func()
	fnKeyboardIsVisible func() int32

	// Haptic feedback functions (iOS)
	fnHapticFeedback func(style int32)

	// System preferences functions
	fnGetNaturalScrolling func() int32

	// File dialog functions (Rust implementation)
	fnFileDialogOpen       func(title uintptr, directory uintptr, filters uintptr, multiple int32) uintptr
	fnFileDialogSave       func(title uintptr, directory uintptr, filters uintptr) uintptr
	fnFileDialogResultFree func(result uintptr)

	// Tray icon functions (Rust implementation)
	fnTrayIconCreate             func() int32
	fnTrayIconDestroy            func()
	fnTrayIconSetIconFile        func(path uintptr) int32
	fnTrayIconSetIconData        func(data uintptr, length uint64) int32
	fnTrayIconSetTooltip         func(tooltip uintptr)
	fnTrayIconSetTitle           func(title uintptr)
	fnTrayIconClearMenu          func()
	fnTrayIconAddMenuItem        func(label uintptr, enabled int32, checked int32, isSeparator int32) int32
	fnTrayIconSetMenuItemEnabled func(index int32, enabled int32)
	fnTrayIconSetMenuItemChecked func(index int32, checked int32)
	fnTrayIconSetMenuItemLabel   func(index int32, label uintptr)
	fnTrayIconSetVisible         func(visible int32)
	fnTrayIconIsVisible          func() int32
	fnTrayIconSetCallback        func(callback uintptr)

	// Batch execution (for shared memory transport)
	fnExecuteBatch func(requestPtr uintptr, requestLen uintptr, responsePtr uintptr, responseCapacity uintptr, responseLenOut uintptr) int32
)

// TextMeasurementC matches the C struct layout for text measurement results
type TextMeasurementC struct {
	Width   float32
	Height  float32
	Ascent  float32
	Descent float32
}

// SafeAreaInsetsC matches the C struct layout for safe area insets (iOS/Android)
type SafeAreaInsetsC struct {
	Top    float32
	Left   float32
	Bottom float32
	Right  float32
}

// AppEventC matches the C struct layout for events from Rust
type AppEventC struct {
	EventType   uint8
	_           [7]byte // padding
	Data1       float64
	Data2       float64
	ScaleFactor float64
}

// FrameResponseC matches the C struct layout for frame responses to Rust
type FrameResponseC struct {
	ImmediateCommands uintptr
	WidgetDelta       uintptr
	RequestRedraw     bool
}

// AppConfigC matches the C struct layout for app configuration
type AppConfigC struct {
	Title                 uintptr
	Width                 uint32
	Height                uint32
	VSync                 bool
	LowPowerGPU           bool
	AllowSoftwareFallback bool
	TargetFPS             uint32
	UserData              uintptr
	Decorations           bool
	Transparent           bool
	Resizable             bool
	AlwaysOnTop           bool
	MinWidth              uint32
	MinHeight             uint32
	MaxWidth              uint32
	MaxHeight             uint32
	X                     int32
	Y                     int32
	CornerRadius          float32
	ShowNativeControls    bool
	EnableMinimize        bool
	EnableMaximize        bool
}

// getLibraryPath returns the path to the dynamic library
func getLibraryPath() string {
	// Check environment variable first
	if path := os.Getenv("CENTERED_LIB_PATH"); path != "" {
		return path
	}

	// Try to find the library relative to the executable or working directory
	var libName string
	switch runtime.GOOS {
	case "darwin", "ios":
		libName = "libcentered_engine.dylib"
	case "linux", "android":
		libName = "libcentered_engine.so"
	case "windows":
		libName = "centered_engine.dll"
	default:
		libName = "libcentered_engine.so"
	}

	// Check common locations
	searchPaths := []string{
		// Current directory
		libName,
		// Relative to executable
		filepath.Join(".", libName),
		// engine/target/release (development)
		filepath.Join("engine", "target", "release", libName),
		// engine/target/debug (development)
		filepath.Join("engine", "target", "debug", libName),
	}

	// Also check relative to the executable
	if execPath, err := os.Executable(); err == nil {
		execDir := filepath.Dir(execPath)
		searchPaths = append(searchPaths,
			filepath.Join(execDir, libName),
			filepath.Join(execDir, "..", "lib", libName),
		)
		// iOS app bundle locations
		if runtime.GOOS == "ios" || runtime.GOOS == "darwin" {
			searchPaths = append(searchPaths,
				filepath.Join(execDir, "Frameworks", libName),
				filepath.Join(execDir, "..", "Frameworks", libName),
			)
		}
	}

	for _, path := range searchPaths {
		if _, err := os.Stat(path); err == nil {
			absPath, err := filepath.Abs(path)
			if err == nil {
				return absPath
			}
			return path
		}
	}

	// Default to library name (let the system find it)
	return libName
}

// initLibrary loads the dynamic library and registers all function pointers
func initLibrary() error {
	libOnce.Do(func() {
		log.Printf("ffi: runtime.GOOS = %s, runtime.GOARCH = %s", runtime.GOOS, runtime.GOARCH)
		libPath := getLibraryPath()
		log.Printf("ffi: attempting to load library from: %s", libPath)

		var flags int
		switch runtime.GOOS {
		case "darwin", "ios":
			flags = 0x1 // RTLD_LAZY
		default:
			flags = 0x1 // RTLD_LAZY
		}

		libHandle, libErr = purego.Dlopen(libPath, flags)
		if libErr != nil {
			libErr = fmt.Errorf("failed to load centered engine library from %s: %w", libPath, libErr)
			return
		}

		// Register all function pointers
		registerCoreFunctions()
		registerIOSFunctions()
		registerAndroidFunctions()
		registerImageFunctions()
		registerTextFunctions()
		registerAudioFunctions()
		registerVideoFunctions()
		registerAudioInputFunctions()
		registerVideoInputFunctions()
		registerSystemFunctions()

		initialized = true
	})

	return libErr
}

func registerCoreFunctions() {
	purego.RegisterLibFunc(&fnAppRun, libHandle, "centered_app_run")
	purego.RegisterLibFunc(&fnAppRequestExit, libHandle, "centered_app_request_exit")
	purego.RegisterLibFunc(&fnAppRequestRedraw, libHandle, "centered_app_request_redraw")
	purego.RegisterLibFunc(&fnEngineVersion, libHandle, "centered_engine_version")

	// Window control functions
	purego.RegisterLibFunc(&fnWindowMinimize, libHandle, "centered_window_minimize")
	purego.RegisterLibFunc(&fnWindowToggleMaximize, libHandle, "centered_window_toggle_maximize")
	purego.RegisterLibFunc(&fnWindowEnterFullscreen, libHandle, "centered_window_enter_fullscreen")
	purego.RegisterLibFunc(&fnWindowExitFullscreen, libHandle, "centered_window_exit_fullscreen")
	purego.RegisterLibFunc(&fnWindowToggleFullscreen, libHandle, "centered_window_toggle_fullscreen")
	purego.RegisterLibFunc(&fnWindowClose, libHandle, "centered_window_close")
	purego.RegisterLibFunc(&fnWindowSetTitle, libHandle, "centered_window_set_title")
}

func registerIOSFunctions() {
	if runtime.GOOS != "ios" {
		return
	}
	purego.RegisterLibFunc(&fnIosSetReadyCallback, libHandle, "centered_ios_set_ready_callback")
	purego.RegisterLibFunc(&fnIosMain, libHandle, "centered_ios_main")
}

func registerAndroidFunctions() {
	if runtime.GOOS != "android" {
		return
	}
	purego.RegisterLibFunc(&fnAndroidSetReadyCallback, libHandle, "centered_android_set_ready_callback")
}

func registerImageFunctions() {
	purego.RegisterLibFunc(&fnLoadImage, libHandle, "centered_backend_load_image")
	purego.RegisterLibFunc(&fnLoadImageFile, libHandle, "centered_backend_load_image_file")
	purego.RegisterLibFunc(&fnUnloadImage, libHandle, "centered_backend_unload_image")
	purego.RegisterLibFunc(&fnGetTextureSize, libHandle, "centered_backend_get_texture_size")
}

func registerTextFunctions() {
	// fnMeasureText returns a struct by value, which purego only supports on darwin (not ios).
	// On iOS, use the pointer-based version instead.
	if runtime.GOOS == "darwin" {
		purego.RegisterLibFunc(&fnMeasureText, libHandle, "centered_measure_text")
	}
	// Always register the pointer-based version for iOS compatibility
	purego.RegisterLibFunc(&fnMeasureTextPtr, libHandle, "centered_measure_text_ptr")
	purego.RegisterLibFunc(&fnMeasureTextWidth, libHandle, "centered_measure_text_width")
	purego.RegisterLibFunc(&fnMeasureTextToCursor, libHandle, "centered_measure_text_to_cursor")
	purego.RegisterLibFunc(&fnMeasureTextWithFont, libHandle, "centered_measure_text_with_font")
	purego.RegisterLibFunc(&fnGetScaleFactor, libHandle, "centered_get_scale_factor")
	// Safe area insets (iOS/Android) - use pointer version for iOS compatibility
	purego.RegisterLibFunc(&fnGetSafeAreaInsetsPtr, libHandle, "centered_get_safe_area_insets_ptr")
}

func registerAudioFunctions() {
	purego.RegisterLibFunc(&fnAudioCreate, libHandle, "centered_audio_create")
	purego.RegisterLibFunc(&fnAudioDestroy, libHandle, "centered_audio_destroy")
	purego.RegisterLibFunc(&fnAudioLoadURL, libHandle, "centered_audio_load_url")
	purego.RegisterLibFunc(&fnAudioLoadFile, libHandle, "centered_audio_load_file")
	purego.RegisterLibFunc(&fnAudioPlay, libHandle, "centered_audio_play")
	purego.RegisterLibFunc(&fnAudioPause, libHandle, "centered_audio_pause")
	purego.RegisterLibFunc(&fnAudioStop, libHandle, "centered_audio_stop")
	purego.RegisterLibFunc(&fnAudioSeek, libHandle, "centered_audio_seek")
	purego.RegisterLibFunc(&fnAudioSetLooping, libHandle, "centered_audio_set_looping")
	purego.RegisterLibFunc(&fnAudioSetVolume, libHandle, "centered_audio_set_volume")
	purego.RegisterLibFunc(&fnAudioGetState, libHandle, "centered_audio_get_state")
	purego.RegisterLibFunc(&fnAudioGetTime, libHandle, "centered_audio_get_time")
	purego.RegisterLibFunc(&fnAudioGetInfo, libHandle, "centered_audio_get_info")
	purego.RegisterLibFunc(&fnAudioGetVolume, libHandle, "centered_audio_get_volume")
	purego.RegisterLibFunc(&fnAudioIsLooping, libHandle, "centered_audio_is_looping")
	purego.RegisterLibFunc(&fnAudioUpdate, libHandle, "centered_audio_update")
}

func registerVideoFunctions() {
	purego.RegisterLibFunc(&fnVideoCreate, libHandle, "centered_video_create")
	purego.RegisterLibFunc(&fnVideoDestroy, libHandle, "centered_video_destroy")
	purego.RegisterLibFunc(&fnVideoLoadURL, libHandle, "centered_video_load_url")
	purego.RegisterLibFunc(&fnVideoLoadFile, libHandle, "centered_video_load_file")
	purego.RegisterLibFunc(&fnVideoInitStream, libHandle, "centered_video_init_stream")
	purego.RegisterLibFunc(&fnVideoPushFrame, libHandle, "centered_video_push_frame")
	purego.RegisterLibFunc(&fnVideoPlay, libHandle, "centered_video_play")
	purego.RegisterLibFunc(&fnVideoPause, libHandle, "centered_video_pause")
	purego.RegisterLibFunc(&fnVideoSeek, libHandle, "centered_video_seek")
	purego.RegisterLibFunc(&fnVideoSetLooping, libHandle, "centered_video_set_looping")
	purego.RegisterLibFunc(&fnVideoSetMuted, libHandle, "centered_video_set_muted")
	purego.RegisterLibFunc(&fnVideoSetVolume, libHandle, "centered_video_set_volume")
	purego.RegisterLibFunc(&fnVideoGetState, libHandle, "centered_video_get_state")
	purego.RegisterLibFunc(&fnVideoGetTime, libHandle, "centered_video_get_time")
	purego.RegisterLibFunc(&fnVideoGetInfo, libHandle, "centered_video_get_info")
	purego.RegisterLibFunc(&fnVideoUpdate, libHandle, "centered_video_update")
	purego.RegisterLibFunc(&fnVideoGetTextureID, libHandle, "centered_video_get_texture_id")
}

func registerAudioInputFunctions() {
	purego.RegisterLibFunc(&fnAudioInputCreate, libHandle, "centered_audio_input_create")
	purego.RegisterLibFunc(&fnAudioInputDestroy, libHandle, "centered_audio_input_destroy")
	purego.RegisterLibFunc(&fnAudioInputRequestPermission, libHandle, "centered_audio_input_request_permission")
	purego.RegisterLibFunc(&fnAudioInputHasPermission, libHandle, "centered_audio_input_has_permission")
	purego.RegisterLibFunc(&fnAudioInputListDevices, libHandle, "centered_audio_input_list_devices")
	purego.RegisterLibFunc(&fnAudioInputOpen, libHandle, "centered_audio_input_open")
	purego.RegisterLibFunc(&fnAudioInputStart, libHandle, "centered_audio_input_start")
	purego.RegisterLibFunc(&fnAudioInputStop, libHandle, "centered_audio_input_stop")
	purego.RegisterLibFunc(&fnAudioInputClose, libHandle, "centered_audio_input_close")
	purego.RegisterLibFunc(&fnAudioInputGetState, libHandle, "centered_audio_input_get_state")
	purego.RegisterLibFunc(&fnAudioInputGetLevel, libHandle, "centered_audio_input_get_level")
}

func registerVideoInputFunctions() {
	purego.RegisterLibFunc(&fnVideoInputCreate, libHandle, "centered_video_input_create")
	purego.RegisterLibFunc(&fnVideoInputDestroy, libHandle, "centered_video_input_destroy")
	purego.RegisterLibFunc(&fnVideoInputRequestPermission, libHandle, "centered_video_input_request_permission")
	purego.RegisterLibFunc(&fnVideoInputHasPermission, libHandle, "centered_video_input_has_permission")
	purego.RegisterLibFunc(&fnVideoInputListDevices, libHandle, "centered_video_input_list_devices")
	purego.RegisterLibFunc(&fnVideoInputOpen, libHandle, "centered_video_input_open")
	purego.RegisterLibFunc(&fnVideoInputStart, libHandle, "centered_video_input_start")
	purego.RegisterLibFunc(&fnVideoInputStop, libHandle, "centered_video_input_stop")
	purego.RegisterLibFunc(&fnVideoInputClose, libHandle, "centered_video_input_close")
	purego.RegisterLibFunc(&fnVideoInputGetState, libHandle, "centered_video_input_get_state")
	purego.RegisterLibFunc(&fnVideoInputGetDimensions, libHandle, "centered_video_input_get_dimensions")
	purego.RegisterLibFunc(&fnVideoInputGetFrameTexture, libHandle, "centered_video_input_get_frame_texture")
}

func registerSystemFunctions() {
	purego.RegisterLibFunc(&fnSystemDarkMode, libHandle, "centered_system_dark_mode")
	purego.RegisterLibFunc(&fnFreeString, libHandle, "centered_free_string")

	// These will be registered once implemented in Rust
	// For now, we'll check if they exist and skip if not available
	registerOptionalFunc(&fnClipboardGet, "centered_clipboard_get")
	registerOptionalFunc(&fnClipboardSet, "centered_clipboard_set")
	registerOptionalFunc(&fnKeyboardShow, "centered_keyboard_show")
	registerOptionalFunc(&fnKeyboardHide, "centered_keyboard_hide")
	registerOptionalFunc(&fnKeyboardIsVisible, "centered_keyboard_is_visible")
	registerOptionalFunc(&fnHapticFeedback, "centered_haptic_feedback")
	registerOptionalFunc(&fnGetNaturalScrolling, "centered_get_natural_scrolling")
	registerOptionalFunc(&fnFileDialogOpen, "centered_file_dialog_open")
	registerOptionalFunc(&fnFileDialogSave, "centered_file_dialog_save")
	registerOptionalFunc(&fnFileDialogResultFree, "centered_file_dialog_result_free")
	registerOptionalFunc(&fnTrayIconCreate, "centered_tray_icon_create")
	registerOptionalFunc(&fnTrayIconDestroy, "centered_tray_icon_destroy")
	registerOptionalFunc(&fnTrayIconSetIconFile, "centered_tray_icon_set_icon_file")
	registerOptionalFunc(&fnTrayIconSetIconData, "centered_tray_icon_set_icon_data")
	registerOptionalFunc(&fnTrayIconSetTooltip, "centered_tray_icon_set_tooltip")
	registerOptionalFunc(&fnTrayIconSetTitle, "centered_tray_icon_set_title")
	registerOptionalFunc(&fnTrayIconClearMenu, "centered_tray_icon_clear_menu")
	registerOptionalFunc(&fnTrayIconAddMenuItem, "centered_tray_icon_add_menu_item")
	registerOptionalFunc(&fnTrayIconSetMenuItemEnabled, "centered_tray_icon_set_menu_item_enabled")
	registerOptionalFunc(&fnTrayIconSetMenuItemChecked, "centered_tray_icon_set_menu_item_checked")
	registerOptionalFunc(&fnTrayIconSetMenuItemLabel, "centered_tray_icon_set_menu_item_label")
	registerOptionalFunc(&fnTrayIconSetVisible, "centered_tray_icon_set_visible")
	registerOptionalFunc(&fnTrayIconIsVisible, "centered_tray_icon_is_visible")
	registerOptionalFunc(&fnTrayIconSetCallback, "centered_tray_icon_set_callback")

	// Batch execution for shared memory transport
	purego.RegisterLibFunc(&fnExecuteBatch, libHandle, "centered_execute_batch")
}

// registerOptionalFunc attempts to register a function, ignoring errors if not found
func registerOptionalFunc[T any](fn *T, name string) {
	defer func() {
		// Recover from panic if symbol not found
		recover()
	}()
	purego.RegisterLibFunc(fn, libHandle, name)
}

// ============================================================================
// String Helpers for FFI
// ============================================================================

// goString converts a C string pointer to a Go string
func goString(ptr uintptr) string {
	if ptr == 0 {
		return ""
	}
	// Find the null terminator
	var length int
	for {
		b := *(*byte)(unsafe.Pointer(ptr + uintptr(length)))
		if b == 0 {
			break
		}
		length++
		if length > 1<<20 { // Safety limit: 1MB
			break
		}
	}
	if length == 0 {
		return ""
	}
	bytes := make([]byte, length)
	for i := 0; i < length; i++ {
		bytes[i] = *(*byte)(unsafe.Pointer(ptr + uintptr(i)))
	}
	return string(bytes)
}

// ============================================================================
// Event Types and Constants
// ============================================================================

// EventType represents the type of event from the engine
type EventType uint8

const (
	EventReady                EventType = 0
	EventRedrawRequested      EventType = 1
	EventResized              EventType = 2
	EventCloseRequested       EventType = 3
	EventMouseMoved           EventType = 4
	EventMousePressed         EventType = 5
	EventMouseReleased        EventType = 6
	EventKeyPressed           EventType = 7
	EventKeyReleased          EventType = 8
	EventCharInput            EventType = 9
	EventMouseWheel           EventType = 10
	EventSuspended            EventType = 11
	EventResumed              EventType = 12
	EventKeyboardFrameChanged EventType = 13
)

// Modifier flags for keyboard events (stored in Data2)
type Modifiers uint32

const (
	ModShift Modifiers = 1 << iota
	ModCtrl
	ModAlt
	ModSuper
)

// Common keycodes - stable cross-platform values
type Keycode uint32

const (
	// Letters A-Z = 0-25
	KeyA Keycode = 0
	KeyB Keycode = 1
	KeyC Keycode = 2
	KeyD Keycode = 3
	KeyE Keycode = 4
	KeyF Keycode = 5
	KeyG Keycode = 6
	KeyH Keycode = 7
	KeyI Keycode = 8
	KeyJ Keycode = 9
	KeyK Keycode = 10
	KeyL Keycode = 11
	KeyM Keycode = 12
	KeyN Keycode = 13
	KeyO Keycode = 14
	KeyP Keycode = 15
	KeyQ Keycode = 16
	KeyR Keycode = 17
	KeyS Keycode = 18
	KeyT Keycode = 19
	KeyU Keycode = 20
	KeyV Keycode = 21
	KeyW Keycode = 22
	KeyX Keycode = 23
	KeyY Keycode = 24
	KeyZ Keycode = 25

	// Numbers 0-9 = 26-35
	Key0 Keycode = 26
	Key1 Keycode = 27
	Key2 Keycode = 28
	Key3 Keycode = 29
	Key4 Keycode = 30
	Key5 Keycode = 31
	Key6 Keycode = 32
	Key7 Keycode = 33
	Key8 Keycode = 34
	Key9 Keycode = 35

	// Function keys F1-F12 = 36-47
	KeyF1  Keycode = 36
	KeyF2  Keycode = 37
	KeyF3  Keycode = 38
	KeyF4  Keycode = 39
	KeyF5  Keycode = 40
	KeyF6  Keycode = 41
	KeyF7  Keycode = 42
	KeyF8  Keycode = 43
	KeyF9  Keycode = 44
	KeyF10 Keycode = 45
	KeyF11 Keycode = 46
	KeyF12 Keycode = 47

	// Navigation = 48-55
	KeyUp       Keycode = 48
	KeyDown     Keycode = 49
	KeyLeft     Keycode = 50
	KeyRight    Keycode = 51
	KeyHome     Keycode = 52
	KeyEnd      Keycode = 53
	KeyPageUp   Keycode = 54
	KeyPageDown Keycode = 55

	// Editing = 56-62
	KeyBackspace Keycode = 56
	KeyDelete    Keycode = 57
	KeyInsert    Keycode = 58
	KeyEnter     Keycode = 59
	KeyTab       Keycode = 60
	KeyEscape    Keycode = 61
	KeySpace     Keycode = 62

	// Punctuation = 63-73
	KeyMinus        Keycode = 63
	KeyEqual        Keycode = 64
	KeyLeftBracket  Keycode = 65
	KeyRightBracket Keycode = 66
	KeyBackslash    Keycode = 67
	KeySemicolon    Keycode = 68
	KeyQuote        Keycode = 69
	KeyBackquote    Keycode = 70
	KeyComma        Keycode = 71
	KeyPeriod       Keycode = 72
	KeySlash        Keycode = 73

	// Numpad = 100-115
	KeyNumpad0        Keycode = 100
	KeyNumpad1        Keycode = 101
	KeyNumpad2        Keycode = 102
	KeyNumpad3        Keycode = 103
	KeyNumpad4        Keycode = 104
	KeyNumpad5        Keycode = 105
	KeyNumpad6        Keycode = 106
	KeyNumpad7        Keycode = 107
	KeyNumpad8        Keycode = 108
	KeyNumpad9        Keycode = 109
	KeyNumpadAdd      Keycode = 110
	KeyNumpadSubtract Keycode = 111
	KeyNumpadMultiply Keycode = 112
	KeyNumpadDivide   Keycode = 113
	KeyNumpadDecimal  Keycode = 114
	KeyNumpadEnter    Keycode = 115

	// Modifier keys
	KeyShiftLeft    Keycode = 200
	KeyShiftRight   Keycode = 201
	KeyControlLeft  Keycode = 202
	KeyControlRight Keycode = 203
	KeyAltLeft      Keycode = 204
	KeyAltRight     Keycode = 205
	KeySuperLeft    Keycode = 206
	KeySuperRight   Keycode = 207

	// Other
	KeyCapsLock    Keycode = 300
	KeyNumLock     Keycode = 301
	KeyScrollLock  Keycode = 302
	KeyPrintScreen Keycode = 303
	KeyPause       Keycode = 304
	KeyContextMenu Keycode = 305

	KeyUnknown Keycode = 999
)

// ============================================================================
// Event Type
// ============================================================================

// Event represents an event from the Rust engine
type Event struct {
	Type        EventType
	Data1       float64
	Data2       float64
	ScaleFactor float64
}

// Keycode returns the keycode for KeyPressed/KeyReleased events
func (e Event) Keycode() uint32 {
	return uint32(e.Data1)
}

// Modifiers returns the modifier flags for keyboard events
func (e Event) Modifiers() Modifiers {
	return Modifiers(uint32(e.Data2))
}

// HasShift returns true if Shift was held
func (e Event) HasShift() bool {
	return e.Modifiers()&ModShift != 0
}

// HasCtrl returns true if Ctrl was held
func (e Event) HasCtrl() bool {
	return e.Modifiers()&ModCtrl != 0
}

// HasAlt returns true if Alt/Option was held
func (e Event) HasAlt() bool {
	return e.Modifiers()&ModAlt != 0
}

// HasSuper returns true if Cmd/Win/Super was held
func (e Event) HasSuper() bool {
	return e.Modifiers()&ModSuper != 0
}

// Char returns the character for CharInput events
func (e Event) Char() rune {
	if e.Type != EventCharInput {
		return 0
	}
	return rune(e.Data1)
}

// MouseX returns the X coordinate for mouse events
func (e Event) MouseX() float64 {
	return e.Data1
}

// MouseY returns the Y coordinate for mouse events
func (e Event) MouseY() float64 {
	return e.Data2
}

// MouseButton returns the button number for mouse button events
func (e Event) MouseButton() int {
	return int(e.Data1)
}

// Width returns the width for Resized events
func (e Event) Width() float64 {
	return e.Data1
}

// Height returns the height for Resized events
func (e Event) Height() float64 {
	return e.Data2
}

// ScrollDelta returns the scroll delta for MouseWheel events
func (e Event) ScrollDelta() (float64, float64) {
	return e.Data1, e.Data2
}

// ============================================================================
// Frame Response and Handler
// ============================================================================

// FrameResponse is returned by the event handler
type FrameResponse struct {
	ImmediateCommands []RenderCommand
	WidgetDelta       interface{}
	RequestRedraw     bool
	RedrawAfterMs     uint32 // Schedule a redraw after N milliseconds (0 = no delayed redraw)
	Exit              bool   // Request app exit
}

// EventHandler is called for each event from the engine
type EventHandler func(event Event) FrameResponse

// ============================================================================
// App Configuration
// ============================================================================

// AppConfig configures the application window
type AppConfig struct {
	Title                 string
	Width                 uint32
	Height                uint32
	VSync                 bool
	LowPowerGPU           bool
	AllowSoftwareFallback bool
	// TargetFPS is the target frames per second (default: 60)
	// Use lower values (e.g., 30) for lighter apps to save battery
	// Use higher values (e.g., 120) for games on high refresh rate displays
	TargetFPS uint32
	Transport TransportMode

	Decorations bool
	Transparent bool
	Resizable   bool
	AlwaysOnTop bool

	MinWidth  uint32
	MinHeight uint32
	MaxWidth  uint32
	MaxHeight uint32

	X int32
	Y int32

	CornerRadius       float32
	ShowNativeControls bool
	EnableMinimize     bool
	EnableMaximize     bool
}

// DefaultAppConfig returns sensible defaults
func DefaultAppConfig() AppConfig {
	return AppConfig{
		Title:                 "Centered App",
		Width:                 1024,
		Height:                768,
		VSync:                 true,
		LowPowerGPU:           false,
		AllowSoftwareFallback: false,
		TargetFPS:             60,
		Transport:             TransportSharedMemory,

		Decorations: true,
		Transparent: false,
		Resizable:   true,
		AlwaysOnTop: false,

		MinWidth:  0,
		MinHeight: 0,
		MaxWidth:  0,
		MaxHeight: 0,

		X: -2147483648, // math.MinInt32
		Y: -2147483648,

		CornerRadius:       10.0,
		ShowNativeControls: true,
		EnableMinimize:     true,
		EnableMaximize:     true,
	}
}

// ============================================================================
// Global Handler for Callback
// ============================================================================

var (
	globalHandler EventHandler
	globalMutex   sync.Mutex
	callbackPtr   uintptr

	// iOS-specific state
	iosReadyCallbackPtr uintptr
	iosStoredConfig     *AppConfig

	// Android-specific state
	androidReadyCallbackPtr uintptr
	androidStoredConfig     *AppConfig
)

// appCallback is the callback function called from Rust
func appCallback(eventPtr uintptr, responsePtr uintptr, userData uintptr) {
	globalMutex.Lock()
	handler := globalHandler
	globalMutex.Unlock()

	if handler == nil {
		return
	}

	if eventPtr == 0 {
		return
	}

	// Read event from memory
	event := (*AppEventC)(unsafe.Pointer(eventPtr))

	goEvent := Event{
		Type:        EventType(event.EventType),
		Data1:       event.Data1,
		Data2:       event.Data2,
		ScaleFactor: event.ScaleFactor,
	}

	// Call the Go handler
	goResponse := handler(goEvent)

	// Write response to memory
	response := (*FrameResponseC)(unsafe.Pointer(responsePtr))
	response.RequestRedraw = goResponse.RequestRedraw
	response.ImmediateCommands = 0
	response.WidgetDelta = 0

	// Render immediate commands
	if len(goResponse.ImmediateCommands) > 0 {
		// On iOS, always use JSON response path (shared memory transport uses
		// global backend which doesn't work with iOS thread-local backend)
		if runtime.GOOS == "ios" {
			jsonBytes, err := json.Marshal(goResponse.ImmediateCommands)
			if err == nil {
				// Allocate string for Rust
				b := append(jsonBytes, 0)
				response.ImmediateCommands = uintptr(unsafe.Pointer(&b[0]))
				runtime.KeepAlive(b)
			}
		} else {
			transport := GetTransport()
			if transport != nil && transport.Mode() == TransportSharedMemory {
				_ = RenderFrameBinary(goResponse.ImmediateCommands)
			} else {
				jsonBytes, err := json.Marshal(goResponse.ImmediateCommands)
				if err == nil {
					// Allocate string for Rust
					b := append(jsonBytes, 0)
					response.ImmediateCommands = uintptr(unsafe.Pointer(&b[0]))
					runtime.KeepAlive(b)
				}
			}
		}
	}

	// Serialize widget delta if present
	if goResponse.WidgetDelta != nil {
		jsonBytes, err := json.Marshal(goResponse.WidgetDelta)
		if err == nil {
			b := append(jsonBytes, 0)
			response.WidgetDelta = uintptr(unsafe.Pointer(&b[0]))
			runtime.KeepAlive(b)
		}
	}
}

// iosReadyCallback is called by Rust when the iOS app is ready (after didFinishLaunching).
// This function then calls fnAppRun to register the event callback with the iOS backend.
func iosReadyCallback() {
	log.Println("[FFI] iOS app ready, registering event callback")

	config := iosStoredConfig
	if config == nil {
		log.Println("[FFI] ERROR: iosStoredConfig is nil")
		return
	}

	// Create callback
	callbackPtr = purego.NewCallback(appCallback)

	// Prepare config struct
	titleBytes := append([]byte(config.Title), 0)
	titlePtr := uintptr(unsafe.Pointer(&titleBytes[0]))

	cConfig := AppConfigC{
		Title:                 titlePtr,
		Width:                 config.Width,
		Height:                config.Height,
		VSync:                 config.VSync,
		LowPowerGPU:           config.LowPowerGPU,
		AllowSoftwareFallback: config.AllowSoftwareFallback,
		TargetFPS:             config.TargetFPS,
		UserData:              0,
		Decorations:           config.Decorations,
		Transparent:           config.Transparent,
		Resizable:             config.Resizable,
		AlwaysOnTop:           config.AlwaysOnTop,
		MinWidth:              config.MinWidth,
		MinHeight:             config.MinHeight,
		MaxWidth:              config.MaxWidth,
		MaxHeight:             config.MaxHeight,
		X:                     config.X,
		Y:                     config.Y,
		CornerRadius:          config.CornerRadius,
		ShowNativeControls:    config.ShowNativeControls,
		EnableMinimize:        config.EnableMinimize,
		EnableMaximize:        config.EnableMaximize,
	}

	// Keep titleBytes alive
	runtime.KeepAlive(titleBytes)

	// Register callback with iOS backend (this doesn't block on iOS)
	result := fnAppRun(uintptr(unsafe.Pointer(&cConfig)), callbackPtr)
	if result != 0 {
		log.Printf("[FFI] ERROR: fnAppRun returned %d", result)
	}
}

// runIOS handles the iOS-specific startup flow.
// On iOS, Rust owns the UIApplicationMain event loop. The flow is:
// 1. Store config for later use by the ready callback
// 2. Register Go's ready callback with Rust
// 3. Call centered_ios_main (which calls UIApplicationMain - never returns)
// 4. When iOS app is ready, Rust calls our iosReadyCallback
// 5. iosReadyCallback calls fnAppRun to register the event handler
func runIOS(config AppConfig) error {
	log.Println("[FFI] Running iOS app")

	// Store config for the ready callback to use later
	iosStoredConfig = &config

	// Create and register the ready callback
	iosReadyCallbackPtr = purego.NewCallback(iosReadyCallback)
	fnIosSetReadyCallback(iosReadyCallbackPtr)

	log.Println("[FFI] Calling centered_ios_main")

	// Call iOS main - this calls UIApplicationMain and never returns
	// The argc/argv are not used by our iOS implementation
	result := fnIosMain(0, 0)
	if result != 0 {
		return &AppError{Code: int(result)}
	}

	return nil
}

// androidReadyCallback is called by Rust when the Android app is ready (after window init).
// This function then calls fnAppRun to register the event callback with the Android backend.
func androidReadyCallback() {
	log.Println("[FFI] Android app ready, registering event callback")

	config := androidStoredConfig
	if config == nil {
		log.Println("[FFI] ERROR: androidStoredConfig is nil")
		return
	}

	// Create callback
	callbackPtr = purego.NewCallback(appCallback)

	// Prepare config struct
	titleBytes := append([]byte(config.Title), 0)
	titlePtr := uintptr(unsafe.Pointer(&titleBytes[0]))

	cConfig := AppConfigC{
		Title:                 titlePtr,
		Width:                 config.Width,
		Height:                config.Height,
		VSync:                 config.VSync,
		LowPowerGPU:           config.LowPowerGPU,
		AllowSoftwareFallback: config.AllowSoftwareFallback,
		TargetFPS:             config.TargetFPS,
		Resizable:             config.Resizable,
		AlwaysOnTop:           config.AlwaysOnTop,
		MinWidth:              config.MinWidth,
		MinHeight:             config.MinHeight,
		MaxWidth:              config.MaxWidth,
		MaxHeight:             config.MaxHeight,
		X:                     config.X,
		Y:                     config.Y,
		CornerRadius:          config.CornerRadius,
		ShowNativeControls:    config.ShowNativeControls,
		EnableMinimize:        config.EnableMinimize,
		EnableMaximize:        config.EnableMaximize,
	}

	// Keep titleBytes alive
	runtime.KeepAlive(titleBytes)

	// Register callback with Android backend
	result := fnAppRun(uintptr(unsafe.Pointer(&cConfig)), callbackPtr)
	if result != 0 {
		log.Printf("[FFI] ERROR: fnAppRun returned %d", result)
	}
}

// runAndroid handles the Android-specific startup flow.
// On Android, Rust owns the android-activity event loop. The flow is:
// 1. Store config for later use by the ready callback
// 2. Register Go's ready callback with Rust
// 3. Block forever (event loop runs in Rust's android_main)
// 4. When Android window is ready, Rust calls our androidReadyCallback
// 5. androidReadyCallback calls fnAppRun to register the event handler
func runAndroid(config AppConfig) error {
	log.Println("[FFI] Running Android app")

	// Store config for the ready callback to use later
	androidStoredConfig = &config

	// Create and register the ready callback
	androidReadyCallbackPtr = purego.NewCallback(androidReadyCallback)
	fnAndroidSetReadyCallback(androidReadyCallbackPtr)

	log.Println("[FFI] Android ready callback registered, waiting for window init...")

	// On Android, the event loop is already running in Rust's android_main.
	// We just need to block here to keep the goroutine alive.
	// The callback will be invoked from Rust when the window is ready.
	select {}
}

// ============================================================================
// Core Functions
// ============================================================================

// Run starts the application with the given configuration and event handler
func Run(config AppConfig, handler EventHandler) error {
	if err := initLibrary(); err != nil {
		return err
	}

	// Initialize transport
	SetTransportMode(config.Transport)
	if err := InitTransport(); err != nil {
		return err
	}
	defer CloseTransport()

	// Store handler globally
	globalMutex.Lock()
	globalHandler = handler
	globalMutex.Unlock()

	defer func() {
		globalMutex.Lock()
		globalHandler = nil
		globalMutex.Unlock()
	}()

	// iOS has a different startup flow
	if runtime.GOOS == "ios" {
		return runIOS(config)
	}

	// Android has a different startup flow (similar to iOS)
	if runtime.GOOS == "android" {
		return runAndroid(config)
	}

	// Desktop path (macOS, Windows, Linux) - uses winit
	// Create callback
	callbackPtr = purego.NewCallback(appCallback)

	// Prepare config struct
	titleBytes := append([]byte(config.Title), 0)
	titlePtr := uintptr(unsafe.Pointer(&titleBytes[0]))

	cConfig := AppConfigC{
		Title:                 titlePtr,
		Width:                 config.Width,
		Height:                config.Height,
		VSync:                 config.VSync,
		LowPowerGPU:           config.LowPowerGPU,
		AllowSoftwareFallback: config.AllowSoftwareFallback,
		TargetFPS:             config.TargetFPS,
		UserData:              0,
		Decorations:           config.Decorations,
		Transparent:           config.Transparent,
		Resizable:             config.Resizable,
		AlwaysOnTop:           config.AlwaysOnTop,
		MinWidth:              config.MinWidth,
		MinHeight:             config.MinHeight,
		MaxWidth:              config.MaxWidth,
		MaxHeight:             config.MaxHeight,
		X:                     config.X,
		Y:                     config.Y,
		CornerRadius:          config.CornerRadius,
		ShowNativeControls:    config.ShowNativeControls,
		EnableMinimize:        config.EnableMinimize,
		EnableMaximize:        config.EnableMaximize,
	}

	// Keep titleBytes alive
	runtime.KeepAlive(titleBytes)

	// Run the app
	result := fnAppRun(uintptr(unsafe.Pointer(&cConfig)), callbackPtr)
	if result != 0 {
		return &AppError{Code: int(result)}
	}

	return nil
}

// RequestExit requests the application to exit
func RequestExit() {
	if !initialized {
		return
	}
	fnAppRequestExit()
}

// RequestRedraw requests a redraw from any goroutine
func RequestRedraw() {
	if !initialized {
		return
	}
	fnAppRequestRedraw()
}

// ============================================================================
// Window Control Functions
// ============================================================================

// WindowMinimize minimizes the window.
// Safe to call from any goroutine.
func WindowMinimize() {
	if !initialized {
		return
	}
	fnWindowMinimize()
}

// WindowToggleMaximize toggles the maximize state of the window.
// If maximized, it will restore to previous size. If not maximized, it will maximize.
// Safe to call from any goroutine.
func WindowToggleMaximize() {
	if !initialized {
		return
	}
	fnWindowToggleMaximize()
}

// WindowEnterFullscreen enters borderless fullscreen mode on the primary monitor.
// Safe to call from any goroutine.
func WindowEnterFullscreen() {
	if !initialized {
		return
	}
	fnWindowEnterFullscreen()
}

// WindowExitFullscreen exits fullscreen mode.
// Safe to call from any goroutine.
func WindowExitFullscreen() {
	if !initialized {
		return
	}
	fnWindowExitFullscreen()
}

// WindowToggleFullscreen toggles fullscreen mode.
// If in fullscreen, exits fullscreen. If not in fullscreen, enters fullscreen.
// Safe to call from any goroutine.
func WindowToggleFullscreen() {
	if !initialized {
		return
	}
	fnWindowToggleFullscreen()
}

// WindowClose requests the window to close, triggering a clean shutdown.
// Safe to call from any goroutine.
func WindowClose() {
	if !initialized {
		return
	}
	fnWindowClose()
}

// WindowSetTitle sets the window title.
// Safe to call from any goroutine.
func WindowSetTitle(title string) {
	if !initialized {
		return
	}
	titleBytes := append([]byte(title), 0)
	fnWindowSetTitle(uintptr(unsafe.Pointer(&titleBytes[0])))
	runtime.KeepAlive(titleBytes)
}

// Version returns the engine version string
func Version() string {
	if !initialized {
		if err := initLibrary(); err != nil {
			return ""
		}
	}
	ptr := fnEngineVersion()
	return goString(ptr)
}

// AppError represents an error from the engine
type AppError struct {
	Code int
}

func (e *AppError) Error() string {
	switch e.Code {
	case -1:
		return "null config"
	case -2:
		return "failed to create event loop"
	case -3:
		return "event loop error"
	default:
		return "unknown error"
	}
}

// ============================================================================
// Render Commands
// ============================================================================

// RenderCommand represents a single rendering operation
type RenderCommand struct {
	DrawRect        *DrawRectCmd        `json:"DrawRect,omitempty"`
	DrawText        *DrawTextCmd        `json:"DrawText,omitempty"`
	DrawImage       *DrawImageCmd       `json:"DrawImage,omitempty"`
	DrawShadow      *DrawShadowCmd      `json:"DrawShadow,omitempty"`
	Clear           *ClearCmd           `json:"Clear,omitempty"`
	PushClip        *PushClipCmd        `json:"PushClip,omitempty"`
	PopClip         *struct{}           `json:"PopClip,omitempty"`
	BeginScrollView *BeginScrollViewCmd `json:"BeginScrollView,omitempty"`
	EndScrollView   *struct{}           `json:"EndScrollView,omitempty"`
	SetOpacity      *float32            `json:"SetOpacity,omitempty"`
}

type BeginScrollViewCmd struct {
	X             float32  `json:"x"`
	Y             float32  `json:"y"`
	Width         float32  `json:"width"`
	Height        float32  `json:"height"`
	ScrollX       float32  `json:"scroll_x"`
	ScrollY       float32  `json:"scroll_y"`
	ContentWidth  *float32 `json:"content_width"`
	ContentHeight *float32 `json:"content_height"`
}

type DrawRectCmd struct {
	X           float32    `json:"x"`
	Y           float32    `json:"y"`
	Width       float32    `json:"width"`
	Height      float32    `json:"height"`
	Color       uint32     `json:"color"`
	CornerRadii [4]float32 `json:"corner_radii"`
	Rotation    float32    `json:"rotation,omitempty"`
	Border      *Border    `json:"border,omitempty"`
	Gradient    *Gradient  `json:"gradient,omitempty"`
}

type DrawImageCmd struct {
	X           float32     `json:"x"`
	Y           float32     `json:"y"`
	Width       float32     `json:"width"`
	Height      float32     `json:"height"`
	TextureID   uint32      `json:"texture_id"`
	SourceRect  *[4]float32 `json:"source_rect,omitempty"`
	CornerRadii [4]float32  `json:"corner_radii,omitempty"`
}

type Border struct {
	Width float32 `json:"width"`
	Color uint32  `json:"color"`
	Style string  `json:"style"`
}

type Gradient struct {
	Linear *LinearGradient `json:"Linear,omitempty"`
	Radial *RadialGradient `json:"Radial,omitempty"`
}

type LinearGradient struct {
	Angle float32        `json:"angle"`
	Stops []GradientStop `json:"stops"`
}

type RadialGradient struct {
	CenterX float32        `json:"center_x"`
	CenterY float32        `json:"center_y"`
	Stops   []GradientStop `json:"stops"`
}

type GradientStop struct {
	Position float32 `json:"position"`
	Color    uint32  `json:"color"`
}

type DrawTextCmd struct {
	X      float32          `json:"x"`
	Y      float32          `json:"y"`
	Text   string           `json:"text"`
	Font   FontDescriptor   `json:"font"`
	Color  uint32           `json:"color"`
	Layout TextLayoutConfig `json:"layout"`
}

type FontDescriptor struct {
	Source FontSource `json:"source"`
	Weight uint16     `json:"weight"`
	Style  FontStyle  `json:"style"`
	Size   float32    `json:"size"`
}

type FontSource struct {
	System  *string `json:"System,omitempty"`
	Bundled *string `json:"Bundled,omitempty"`
}

type FontStyle string

const (
	FontStyleNormal FontStyle = "Normal"
	FontStyleItalic FontStyle = "Italic"
)

type TextLayoutConfig struct {
	MaxWidth      *float32      `json:"max_width,omitempty"`
	MaxHeight     *float32      `json:"max_height,omitempty"`
	MaxLines      *int          `json:"max_lines,omitempty"`
	LineHeight    float32       `json:"line_height"`
	LetterSpacing float32       `json:"letter_spacing"`
	WordSpacing   float32       `json:"word_spacing"`
	Alignment     TextAlign     `json:"alignment"`
	VerticalAlign VerticalAlign `json:"vertical_align"`
	WordBreak     WordBreak     `json:"word_break"`
	Overflow      TextOverflow  `json:"overflow"`
	WhiteSpace    WhiteSpace    `json:"white_space"`
}

type TextAlign string

const (
	TextAlignLeft    TextAlign = "Left"
	TextAlignCenter  TextAlign = "Center"
	TextAlignRight   TextAlign = "Right"
	TextAlignJustify TextAlign = "Justify"
)

type VerticalAlign string

const (
	VerticalAlignTop      VerticalAlign = "Top"
	VerticalAlignMiddle   VerticalAlign = "Middle"
	VerticalAlignBottom   VerticalAlign = "Bottom"
	VerticalAlignBaseline VerticalAlign = "Baseline"
)

type WordBreak string

const (
	WordBreakNormal    WordBreak = "Normal"
	WordBreakBreakAll  WordBreak = "BreakAll"
	WordBreakKeepAll   WordBreak = "KeepAll"
	WordBreakBreakWord WordBreak = "BreakWord"
)

type TextOverflow string

const (
	TextOverflowClip     TextOverflow = "Clip"
	TextOverflowEllipsis TextOverflow = "Ellipsis"
	TextOverflowWrap     TextOverflow = "Wrap"
)

type WhiteSpace string

const (
	WhiteSpaceNormal  WhiteSpace = "Normal"
	WhiteSpaceNoWrap  WhiteSpace = "NoWrap"
	WhiteSpacePre     WhiteSpace = "Pre"
	WhiteSpacePreWrap WhiteSpace = "PreWrap"
)

type DrawShadowCmd struct {
	X           float32    `json:"x"`
	Y           float32    `json:"y"`
	Width       float32    `json:"width"`
	Height      float32    `json:"height"`
	Blur        float32    `json:"blur"`
	Color       uint32     `json:"color"`
	OffsetX     float32    `json:"offset_x"`
	OffsetY     float32    `json:"offset_y"`
	CornerRadii [4]float32 `json:"corner_radii"`
}

type ClearCmd struct {
	R uint8 `json:"r"`
	G uint8 `json:"g"`
	B uint8 `json:"b"`
	A uint8 `json:"a"`
}

type PushClipCmd struct {
	X      float32 `json:"x"`
	Y      float32 `json:"y"`
	Width  float32 `json:"width"`
	Height float32 `json:"height"`
}

// ============================================================================
// Command Builders
// ============================================================================

func Rect(x, y, width, height float32, color uint32) RenderCommand {
	return RenderCommand{
		DrawRect: &DrawRectCmd{
			X: x, Y: y, Width: width, Height: height,
			Color:       color,
			CornerRadii: [4]float32{0, 0, 0, 0},
		},
	}
}

func RoundedRect(x, y, width, height float32, color uint32, radius float32) RenderCommand {
	return RenderCommand{
		DrawRect: &DrawRectCmd{
			X: x, Y: y, Width: width, Height: height,
			Color:       color,
			CornerRadii: [4]float32{radius, radius, radius, radius},
		},
	}
}

func Shadow(x, y, width, height, blur float32, color uint32, offsetX, offsetY float32, radii [4]float32) RenderCommand {
	return RenderCommand{
		DrawShadow: &DrawShadowCmd{
			X: x, Y: y, Width: width, Height: height,
			Blur: blur, Color: color,
			OffsetX: offsetX, OffsetY: offsetY,
			CornerRadii: radii,
		},
	}
}

func Clear(r, g, b, a uint8) RenderCommand {
	return RenderCommand{
		Clear: &ClearCmd{R: r, G: g, B: b, A: a},
	}
}

func PushClip(x, y, width, height float32) RenderCommand {
	return RenderCommand{
		PushClip: &PushClipCmd{X: x, Y: y, Width: width, Height: height},
	}
}

func PopClip() RenderCommand {
	return RenderCommand{
		PopClip: &struct{}{},
	}
}

func BeginScrollView(x, y, width, height, scrollX, scrollY float32) RenderCommand {
	return RenderCommand{
		BeginScrollView: &BeginScrollViewCmd{
			X: x, Y: y, Width: width, Height: height,
			ScrollX: scrollX, ScrollY: scrollY,
		},
	}
}

func BeginScrollViewWithContent(x, y, width, height, scrollX, scrollY, contentWidth, contentHeight float32) RenderCommand {
	return RenderCommand{
		BeginScrollView: &BeginScrollViewCmd{
			X: x, Y: y, Width: width, Height: height,
			ScrollX: scrollX, ScrollY: scrollY,
			ContentWidth: &contentWidth, ContentHeight: &contentHeight,
		},
	}
}

func EndScrollView() RenderCommand {
	return RenderCommand{
		EndScrollView: &struct{}{},
	}
}

func Text(text string, x, y float32, size float32, color uint32) RenderCommand {
	fontName := "system"
	return RenderCommand{
		DrawText: &DrawTextCmd{
			X: x, Y: y, Text: text, Color: color,
			Font: FontDescriptor{
				Source: FontSource{System: &fontName},
				Weight: 400, Style: FontStyleNormal, Size: size,
			},
			Layout: DefaultTextLayout(),
		},
	}
}

func TextWithFont(text string, x, y float32, font FontDescriptor, color uint32) RenderCommand {
	return RenderCommand{
		DrawText: &DrawTextCmd{
			X: x, Y: y, Text: text, Color: color,
			Font: font, Layout: DefaultTextLayout(),
		},
	}
}

func TextWithLayout(text string, x, y float32, font FontDescriptor, color uint32, layout TextLayoutConfig) RenderCommand {
	return RenderCommand{
		DrawText: &DrawTextCmd{
			X: x, Y: y, Text: text, Color: color,
			Font: font, Layout: layout,
		},
	}
}

func SystemFont(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 400, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontBold(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 700, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontItalic(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 400, Style: FontStyleItalic, Size: size,
	}
}

func SystemFontBoldItalic(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 700, Style: FontStyleItalic, Size: size,
	}
}

func SystemFontWithWeight(name string, size float32, weight uint16) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: weight, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontLight(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 300, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontMedium(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 500, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontSemiBold(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 600, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontHeavy(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 800, Style: FontStyleNormal, Size: size,
	}
}

func SystemFontBlack(name string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{System: &name},
		Weight: 900, Style: FontStyleNormal, Size: size,
	}
}

func BundledFont(path string, size float32) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{Bundled: &path},
		Weight: 400, Style: FontStyleNormal, Size: size,
	}
}

func BundledFontWithWeight(path string, size float32, weight uint16) FontDescriptor {
	return FontDescriptor{
		Source: FontSource{Bundled: &path},
		Weight: weight, Style: FontStyleNormal, Size: size,
	}
}

// LoadBundledFont preloads a bundled font from the given path.
// On native platforms, fonts are loaded lazily by the engine, so this is a no-op.
// On web, this must be called before using the font to register it with the browser.
func LoadBundledFont(path string) error {
	// No-op on native - fonts load lazily when first used
	return nil
}

// LoadBundledFontFromData preloads a font from raw byte data.
// On native platforms, this is a no-op (fonts load lazily).
// On web, this registers the font data with the browser's FontFace API.
func LoadBundledFontFromData(name string, data []byte) error {
	// No-op on native - fonts load lazily when first used
	return nil
}

// IsFontLoaded checks if a bundled font has been loaded.
// On native platforms, this always returns true since fonts load lazily.
// On web, returns true only if LoadBundledFont was called for this path.
func IsFontLoaded(path string) bool {
	// On native, fonts are loaded on-demand by the engine
	return true
}

func DefaultTextLayout() TextLayoutConfig {
	return TextLayoutConfig{
		LineHeight: 1.5, LetterSpacing: 0.0, WordSpacing: 0.0,
		Alignment: TextAlignLeft, VerticalAlign: VerticalAlignTop,
		WordBreak: WordBreakNormal, Overflow: TextOverflowWrap,
		WhiteSpace: WhiteSpaceNormal,
	}
}

func CenteredTextLayout() TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.Alignment = TextAlignCenter
	return layout
}

func WrappedTextLayout(maxWidth float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	return layout
}

func WrappedCenteredTextLayout(maxWidth float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.Alignment = TextAlignCenter
	return layout
}

func EllipsisTextLayout(maxWidth float32, maxLines int) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxLines = &maxLines
	layout.Overflow = TextOverflowEllipsis
	return layout
}

func SingleLineEllipsisLayout(maxWidth float32) TextLayoutConfig {
	maxLines := 1
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxLines = &maxLines
	layout.Overflow = TextOverflowEllipsis
	return layout
}

func EllipsisHeightLayout(maxWidth, maxHeight float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxHeight = &maxHeight
	layout.Overflow = TextOverflowEllipsis
	return layout
}

func ClippedTextLayout(maxWidth, maxHeight float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxHeight = &maxHeight
	layout.Overflow = TextOverflowClip
	return layout
}

func SpacedTextLayout(letterSpacing, wordSpacing float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.LetterSpacing = letterSpacing
	layout.WordSpacing = wordSpacing
	return layout
}

func TrackingLayout(letterSpacing float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.LetterSpacing = letterSpacing
	return layout
}

// ============================================================================
// Color Helpers
// ============================================================================

func RGBA(r, g, b, a uint8) uint32 {
	return uint32(r)<<24 | uint32(g)<<16 | uint32(b)<<8 | uint32(a)
}

func RGB(r, g, b uint8) uint32 {
	return RGBA(r, g, b, 255)
}

func HexColor(hex uint32) uint32 {
	return (hex << 8) | 0xFF
}

// ============================================================================
// Image/Texture Management
// ============================================================================

type TextureID uint32

type ImageError struct {
	Code    int
	Message string
}

func (e *ImageError) Error() string {
	return e.Message
}

func imageErrorMessage(code int) string {
	switch code {
	case -1:
		return "invalid parameters"
	case -2:
		return "backend not initialized"
	case -3:
		return "failed to decode image"
	case -4:
		return "failed to upload to GPU"
	default:
		return "unknown image error"
	}
}

func LoadImage(data []byte) (TextureID, error) {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0, err
		}
	}
	if len(data) == 0 {
		return 0, &ImageError{Code: -1, Message: "empty image data"}
	}

	result := fnLoadImage(uintptr(unsafe.Pointer(&data[0])), uint64(len(data)))
	if result < 0 {
		return 0, &ImageError{Code: int(result), Message: imageErrorMessage(int(result))}
	}
	return TextureID(result), nil
}

func LoadImageFile(path string) (TextureID, error) {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0, err
		}
	}

	pathBytes := append([]byte(path), 0)
	result := fnLoadImageFile(uintptr(unsafe.Pointer(&pathBytes[0])))
	runtime.KeepAlive(pathBytes)

	if result < 0 {
		return 0, &ImageError{Code: int(result), Message: imageErrorMessage(int(result))}
	}
	return TextureID(result), nil
}

func UnloadImage(id TextureID) error {
	if !initialized {
		return nil
	}
	result := fnUnloadImage(uint32(id))
	if result < 0 {
		return &ImageError{Code: int(result), Message: imageErrorMessage(int(result))}
	}
	return nil
}

func GetTextureSize(id TextureID) (uint32, uint32, error) {
	if !initialized {
		return 0, 0, &ImageError{Code: -2, Message: "not initialized"}
	}

	var width, height uint32
	result := fnGetTextureSize(uint32(id), uintptr(unsafe.Pointer(&width)), uintptr(unsafe.Pointer(&height)))
	if result < 0 {
		return 0, 0, &ImageError{Code: int(result), Message: imageErrorMessage(int(result))}
	}
	return width, height, nil
}

// ============================================================================
// Image Command Builders
// ============================================================================

func Image(textureID TextureID, x, y, width, height float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID),
		},
	}
}

func ImageWithCornerRadii(textureID TextureID, x, y, width, height float32, cornerRadii [4]float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID), CornerRadii: cornerRadii,
		},
	}
}

func ImageWithSourceRect(textureID TextureID, x, y, width, height float32, sourceRect [4]float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID), SourceRect: &sourceRect,
		},
	}
}

func ImageWithSourceRectAndCornerRadii(textureID TextureID, x, y, width, height float32, sourceRect [4]float32, cornerRadii [4]float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID), SourceRect: &sourceRect, CornerRadii: cornerRadii,
		},
	}
}

func Sprite(textureID TextureID, x, y, width, height float32, spriteX, spriteY, cols, rows int) RenderCommand {
	spriteWidth := 1.0 / float32(cols)
	spriteHeight := 1.0 / float32(rows)
	srcX := float32(spriteX) * spriteWidth
	srcY := float32(spriteY) * spriteHeight

	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID:  uint32(textureID),
			SourceRect: &[4]float32{srcX, srcY, spriteWidth, spriteHeight},
		},
	}
}

// ============================================================================
// Text Measurement
// ============================================================================

type TextMeasurement struct {
	Width   float32
	Height  float32
	Ascent  float32
	Descent float32
}

func MeasureText(text string, fontName string, fontSize float32) TextMeasurement {
	if !initialized {
		if err := initLibrary(); err != nil {
			return TextMeasurement{}
		}
	}

	textBytes := append([]byte(text), 0)
	fontBytes := append([]byte(fontName), 0)

	// On iOS, use the pointer-based version since purego doesn't support struct returns
	if runtime.GOOS == "ios" {
		var resultC TextMeasurementC
		fnMeasureTextPtr(
			uintptr(unsafe.Pointer(&textBytes[0])),
			uintptr(unsafe.Pointer(&fontBytes[0])),
			fontSize,
			uintptr(unsafe.Pointer(&resultC)),
		)
		runtime.KeepAlive(textBytes)
		runtime.KeepAlive(fontBytes)
		return TextMeasurement{
			Width:   resultC.Width,
			Height:  resultC.Height,
			Ascent:  resultC.Ascent,
			Descent: resultC.Descent,
		}
	}

	// On darwin (macOS), use the direct struct return version
	result := fnMeasureText(
		uintptr(unsafe.Pointer(&textBytes[0])),
		uintptr(unsafe.Pointer(&fontBytes[0])),
		fontSize,
	)

	runtime.KeepAlive(textBytes)
	runtime.KeepAlive(fontBytes)

	return TextMeasurement{
		Width:   result.Width,
		Height:  result.Height,
		Ascent:  result.Ascent,
		Descent: result.Descent,
	}
}

func MeasureTextWidth(text string, fontName string, fontSize float32) float32 {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}

	textBytes := append([]byte(text), 0)
	fontBytes := append([]byte(fontName), 0)

	result := fnMeasureTextWidth(
		uintptr(unsafe.Pointer(&textBytes[0])),
		uintptr(unsafe.Pointer(&fontBytes[0])),
		fontSize,
	)

	runtime.KeepAlive(textBytes)
	runtime.KeepAlive(fontBytes)

	return result
}

type TextMeasurementRequest struct {
	Text     string
	FontName string
	FontSize float32
}

func MeasureTextWidthBatch(measurements []TextMeasurementRequest) []float32 {
	if len(measurements) == 0 {
		return nil
	}

	// Calculate payload size
	payloadSize := 4
	for _, m := range measurements {
		payloadSize += 4 + len(m.Text) + 4 + len(m.FontName) + 4
	}

	// Build payload
	payload := make([]byte, payloadSize)
	offset := 0

	PutUint32(payload[offset:], uint32(len(measurements)))
	offset += 4

	for _, m := range measurements {
		PutUint32(payload[offset:], uint32(len(m.Text)))
		offset += 4
		copy(payload[offset:], m.Text)
		offset += len(m.Text)

		PutUint32(payload[offset:], uint32(len(m.FontName)))
		offset += 4
		copy(payload[offset:], m.FontName)
		offset += len(m.FontName)

		PutFloat32(payload[offset:], m.FontSize)
		offset += 4
	}

	transport := GetTransport()
	respType, respPayload, err := transport.Execute(CmdMeasureTextBatch, payload)
	if err != nil || respType != RespFloat32Array {
		return make([]float32, len(measurements))
	}

	if len(respPayload) < 4 {
		return make([]float32, len(measurements))
	}

	count := int(GetUint32(respPayload[0:4]))
	if count != len(measurements) || len(respPayload) < 4+count*4 {
		return make([]float32, len(measurements))
	}

	widths := make([]float32, count)
	for i := 0; i < count; i++ {
		widths[i] = GetFloat32(respPayload[4+i*4:])
	}
	return widths
}

func MeasureTextToCursor(text string, charIndex int, fontName string, fontSize float32) float32 {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}

	textBytes := append([]byte(text), 0)
	fontBytes := append([]byte(fontName), 0)

	result := fnMeasureTextToCursor(
		uintptr(unsafe.Pointer(&textBytes[0])),
		uint32(charIndex),
		uintptr(unsafe.Pointer(&fontBytes[0])),
		fontSize,
	)

	runtime.KeepAlive(textBytes)
	runtime.KeepAlive(fontBytes)

	return result
}

func MeasureTextWithFont(text string, font FontDescriptor) float32 {
	if text == "" {
		return 0
	}
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}

	textBytes := append([]byte(text), 0)
	fontJSON, err := json.Marshal(font)
	if err != nil {
		return 0
	}
	fontJSONBytes := append(fontJSON, 0)

	result := fnMeasureTextWithFont(
		uintptr(unsafe.Pointer(&textBytes[0])),
		uintptr(unsafe.Pointer(&fontJSONBytes[0])),
	)

	runtime.KeepAlive(textBytes)
	runtime.KeepAlive(fontJSONBytes)

	return result
}

func GetScaleFactor() float64 {
	if !initialized {
		return 1.0
	}
	return fnGetScaleFactor()
}

// ============================================================================
// Safe Area Insets (iOS/Android)
// ============================================================================

// SafeAreaInsets represents the insets (in logical pixels) for areas that should
// avoid system UI elements like the notch, status bar, and home indicator on iOS,
// or the navigation bar and status bar on Android.
//
// On desktop platforms, all values are 0.
type SafeAreaInsets struct {
	Top    float32 // Distance from top edge to avoid (notch, status bar)
	Left   float32 // Distance from left edge to avoid (landscape notch)
	Bottom float32 // Distance from bottom edge to avoid (home indicator)
	Right  float32 // Distance from right edge to avoid (landscape notch)
}

// GetSafeAreaInsets returns the current safe area insets.
// This is primarily useful on iOS and Android where system UI elements
// occupy screen space that content should avoid.
//
// For games or apps that want full-screen edge-to-edge rendering,
// the engine renders into unsafe areas by default. Use these insets
// to position UI elements (like buttons, text) within the safe area.
//
// Example usage:
//
//	insets := ffi.GetSafeAreaInsets()
//	// Position a toolbar at the bottom, above the home indicator
//	toolbarY := windowHeight - toolbarHeight - insets.Bottom
//
// On desktop platforms, this returns (0, 0, 0, 0).
func GetSafeAreaInsets() SafeAreaInsets {
	if !initialized || fnGetSafeAreaInsetsPtr == nil {
		return SafeAreaInsets{}
	}
	var result SafeAreaInsetsC
	fnGetSafeAreaInsetsPtr(uintptr(unsafe.Pointer(&result)))
	return SafeAreaInsets{
		Top:    result.Top,
		Left:   result.Left,
		Bottom: result.Bottom,
		Right:  result.Right,
	}
}

// ============================================================================
// Clipboard Functions
// ============================================================================

func ClipboardGetString() string {
	if !initialized || fnClipboardGet == nil {
		return ""
	}
	ptr := fnClipboardGet()
	if ptr == 0 {
		return ""
	}
	result := goString(ptr)
	return result
}

func ClipboardSetString(text string) {
	if !initialized || fnClipboardSet == nil {
		return
	}
	textBytes := append([]byte(text), 0)
	fnClipboardSet(uintptr(unsafe.Pointer(&textBytes[0])))
	runtime.KeepAlive(textBytes)
}

// ============================================================================
// Keyboard Functions (iOS)
// ============================================================================

// KeyboardShow shows the software keyboard on iOS.
// On other platforms, this is a no-op.
func KeyboardShow() {
	if !initialized || fnKeyboardShow == nil {
		return
	}
	fnKeyboardShow()
}

// KeyboardHide hides the software keyboard on iOS.
// On other platforms, this is a no-op.
func KeyboardHide() {
	if !initialized || fnKeyboardHide == nil {
		return
	}
	fnKeyboardHide()
}

// KeyboardIsVisible returns true if the software keyboard is visible on iOS.
// On other platforms, always returns false.
func KeyboardIsVisible() bool {
	if !initialized || fnKeyboardIsVisible == nil {
		return false
	}
	return fnKeyboardIsVisible() != 0
}

// ============================================================================
// Haptic Feedback Functions (iOS)
// ============================================================================

// HapticStyle represents the type of haptic feedback
type HapticStyle int32

const (
	// Impact feedback styles
	HapticImpactLight  HapticStyle = 0
	HapticImpactMedium HapticStyle = 1
	HapticImpactHeavy  HapticStyle = 2
	HapticImpactSoft   HapticStyle = 3 // iOS 13+
	HapticImpactRigid  HapticStyle = 4 // iOS 13+

	// Selection feedback
	HapticSelection HapticStyle = 10

	// Notification feedback
	HapticNotificationSuccess HapticStyle = 20
	HapticNotificationWarning HapticStyle = 21
	HapticNotificationError   HapticStyle = 22
)

// HapticFeedback triggers haptic feedback on iOS.
// On other platforms, this is a no-op.
func HapticFeedback(style HapticStyle) {
	if !initialized || fnHapticFeedback == nil {
		return
	}
	fnHapticFeedback(int32(style))
}

// ============================================================================
// System Preferences Functions
// ============================================================================

// GetNaturalScrolling returns true if natural scrolling is enabled in system preferences.
// On macOS, this checks the "com.apple.swipescrolldirection" preference.
// On iOS, always returns true (touch scrolling is always natural).
// On other platforms, defaults to true.
func GetNaturalScrolling() bool {
	if !initialized || fnGetNaturalScrolling == nil {
		return true // Default to natural scrolling
	}
	return fnGetNaturalScrolling() != 0
}

// ============================================================================
// File Dialog Functions
// ============================================================================

type FileFilter struct {
	Name       string
	Extensions []string
}

func OpenFileDialog(title, directory string, filters []FileFilter, multiple bool) ([]string, bool) {
	if !initialized || fnFileDialogOpen == nil {
		return nil, false
	}

	var titlePtr, dirPtr, filtersPtr uintptr
	var titleBytes, dirBytes, filtersBytes []byte

	if title != "" {
		titleBytes = append([]byte(title), 0)
		titlePtr = uintptr(unsafe.Pointer(&titleBytes[0]))
	}
	if directory != "" {
		dirBytes = append([]byte(directory), 0)
		dirPtr = uintptr(unsafe.Pointer(&dirBytes[0]))
	}
	if len(filters) > 0 {
		var exts []string
		for _, f := range filters {
			exts = append(exts, f.Extensions...)
		}
		if len(exts) > 0 {
			filterStr := strings.Join(exts, ",")
			filtersBytes = append([]byte(filterStr), 0)
			filtersPtr = uintptr(unsafe.Pointer(&filtersBytes[0]))
		}
	}

	multipleInt := int32(0)
	if multiple {
		multipleInt = 1
	}

	result := fnFileDialogOpen(titlePtr, dirPtr, filtersPtr, multipleInt)

	runtime.KeepAlive(titleBytes)
	runtime.KeepAlive(dirBytes)
	runtime.KeepAlive(filtersBytes)

	if result == 0 {
		return nil, false
	}
	defer fnFileDialogResultFree(result)

	// Parse result (JSON array of paths)
	jsonStr := goString(result)
	var paths []string
	if err := json.Unmarshal([]byte(jsonStr), &paths); err != nil {
		return nil, false
	}
	return paths, len(paths) > 0
}

func SaveFileDialog(title, directory string, filters []FileFilter) (string, bool) {
	if !initialized || fnFileDialogSave == nil {
		return "", false
	}

	var titlePtr, dirPtr, filtersPtr uintptr
	var titleBytes, dirBytes, filtersBytes []byte

	if title != "" {
		titleBytes = append([]byte(title), 0)
		titlePtr = uintptr(unsafe.Pointer(&titleBytes[0]))
	}
	if directory != "" {
		dirBytes = append([]byte(directory), 0)
		dirPtr = uintptr(unsafe.Pointer(&dirBytes[0]))
	}
	if len(filters) > 0 {
		var exts []string
		for _, f := range filters {
			exts = append(exts, f.Extensions...)
		}
		if len(exts) > 0 {
			filterStr := strings.Join(exts, ",")
			filtersBytes = append([]byte(filterStr), 0)
			filtersPtr = uintptr(unsafe.Pointer(&filtersBytes[0]))
		}
	}

	result := fnFileDialogSave(titlePtr, dirPtr, filtersPtr)

	runtime.KeepAlive(titleBytes)
	runtime.KeepAlive(dirBytes)
	runtime.KeepAlive(filtersBytes)

	if result == 0 {
		return "", false
	}
	defer fnFileDialogResultFree(result)

	path := goString(result)
	return path, path != ""
}

// ============================================================================
// Tray Icon Functions
// ============================================================================

var trayMenuCallback func(index int)
var trayCallbackPtr uintptr

func trayCallback(index int32) {
	if trayMenuCallback != nil {
		trayMenuCallback(int(index))
	}
}

func TrayIconCreate() error {
	if !initialized || fnTrayIconCreate == nil {
		return fmt.Errorf("tray icon not available")
	}
	result := fnTrayIconCreate()
	if result < 0 {
		return fmt.Errorf("failed to create tray icon: %d", result)
	}
	return nil
}

func TrayIconDestroy() {
	if !initialized || fnTrayIconDestroy == nil {
		return
	}
	fnTrayIconDestroy()
}

func TrayIconSetIconFile(path string) error {
	if !initialized || fnTrayIconSetIconFile == nil {
		return fmt.Errorf("tray icon not available")
	}
	pathBytes := append([]byte(path), 0)
	result := fnTrayIconSetIconFile(uintptr(unsafe.Pointer(&pathBytes[0])))
	runtime.KeepAlive(pathBytes)

	if result < 0 {
		return fmt.Errorf("failed to set tray icon: %d", result)
	}
	return nil
}

func TrayIconSetIconData(data []byte) error {
	if !initialized || fnTrayIconSetIconData == nil {
		return fmt.Errorf("tray icon not available")
	}
	if len(data) == 0 {
		return fmt.Errorf("empty image data")
	}

	result := fnTrayIconSetIconData(uintptr(unsafe.Pointer(&data[0])), uint64(len(data)))
	if result < 0 {
		return fmt.Errorf("failed to set tray icon: %d", result)
	}
	return nil
}

func TrayIconSetTooltip(tooltip string) {
	if !initialized || fnTrayIconSetTooltip == nil {
		return
	}
	tooltipBytes := append([]byte(tooltip), 0)
	fnTrayIconSetTooltip(uintptr(unsafe.Pointer(&tooltipBytes[0])))
	runtime.KeepAlive(tooltipBytes)
}

func TrayIconSetTitle(title string) {
	if !initialized || fnTrayIconSetTitle == nil {
		return
	}
	titleBytes := append([]byte(title), 0)
	fnTrayIconSetTitle(uintptr(unsafe.Pointer(&titleBytes[0])))
	runtime.KeepAlive(titleBytes)
}

func TrayIconClearMenu() {
	if !initialized || fnTrayIconClearMenu == nil {
		return
	}
	fnTrayIconClearMenu()
}

func TrayIconAddMenuItem(label string, enabled, checked bool) int {
	if !initialized || fnTrayIconAddMenuItem == nil {
		return -1
	}
	labelBytes := append([]byte(label), 0)
	enabledInt := int32(0)
	if enabled {
		enabledInt = 1
	}
	checkedInt := int32(0)
	if checked {
		checkedInt = 1
	}

	result := fnTrayIconAddMenuItem(uintptr(unsafe.Pointer(&labelBytes[0])), enabledInt, checkedInt, 0)
	runtime.KeepAlive(labelBytes)
	return int(result)
}

func TrayIconAddSeparator() int {
	if !initialized || fnTrayIconAddMenuItem == nil {
		return -1
	}
	return int(fnTrayIconAddMenuItem(0, 0, 0, 1))
}

func TrayIconSetMenuItemEnabled(index int, enabled bool) {
	if !initialized || fnTrayIconSetMenuItemEnabled == nil {
		return
	}
	enabledInt := int32(0)
	if enabled {
		enabledInt = 1
	}
	fnTrayIconSetMenuItemEnabled(int32(index), enabledInt)
}

func TrayIconSetMenuItemChecked(index int, checked bool) {
	if !initialized || fnTrayIconSetMenuItemChecked == nil {
		return
	}
	checkedInt := int32(0)
	if checked {
		checkedInt = 1
	}
	fnTrayIconSetMenuItemChecked(int32(index), checkedInt)
}

func TrayIconSetMenuItemLabel(index int, label string) {
	if !initialized || fnTrayIconSetMenuItemLabel == nil {
		return
	}
	labelBytes := append([]byte(label), 0)
	fnTrayIconSetMenuItemLabel(int32(index), uintptr(unsafe.Pointer(&labelBytes[0])))
	runtime.KeepAlive(labelBytes)
}

func TrayIconSetVisible(visible bool) {
	if !initialized || fnTrayIconSetVisible == nil {
		return
	}
	visibleInt := int32(0)
	if visible {
		visibleInt = 1
	}
	fnTrayIconSetVisible(visibleInt)
}

func TrayIconIsVisible() bool {
	if !initialized || fnTrayIconIsVisible == nil {
		return false
	}
	return fnTrayIconIsVisible() != 0
}

func TrayIconSetMenuCallback(callback func(index int)) {
	trayMenuCallback = callback
	if fnTrayIconSetCallback != nil {
		trayCallbackPtr = purego.NewCallback(trayCallback)
		fnTrayIconSetCallback(trayCallbackPtr)
	}
}

// ============================================================================
// System Preferences
// ============================================================================

func SystemDarkMode() bool {
	if !initialized {
		if err := initLibrary(); err != nil {
			return false
		}
	}
	result := fnSystemDarkMode()
	return result == 1
}

// ============================================================================
// Audio Playback API
// ============================================================================

type AudioPlayerID uint32

type AudioState int32

const (
	AudioStateIdle    AudioState = 0
	AudioStateLoading AudioState = 1
	AudioStatePlaying AudioState = 2
	AudioStatePaused  AudioState = 3
	AudioStateEnded   AudioState = 4
	AudioStateError   AudioState = 5
)

func (s AudioState) String() string {
	switch s {
	case AudioStateIdle:
		return "idle"
	case AudioStateLoading:
		return "loading"
	case AudioStatePlaying:
		return "playing"
	case AudioStatePaused:
		return "paused"
	case AudioStateEnded:
		return "ended"
	case AudioStateError:
		return "error"
	default:
		return "unknown"
	}
}

type AudioInfo struct {
	DurationMs uint64
	SampleRate uint32
	Channels   uint32
}

type AudioError struct {
	Code    int
	Message string
}

func (e *AudioError) Error() string {
	return e.Message
}

func audioErrorMessage(code int) string {
	switch code {
	case -1:
		return "invalid parameters"
	case -2:
		return "player not found"
	case -3:
		return "audio operation failed"
	default:
		return "unknown audio error"
	}
}

func AudioCreate() AudioPlayerID {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}
	return AudioPlayerID(fnAudioCreate())
}

func AudioDestroy(id AudioPlayerID) {
	if !initialized {
		return
	}
	fnAudioDestroy(uint32(id))
}

func AudioLoadURL(id AudioPlayerID, url string) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	urlBytes := append([]byte(url), 0)
	result := fnAudioLoadURL(uint32(id), uintptr(unsafe.Pointer(&urlBytes[0])))
	runtime.KeepAlive(urlBytes)

	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioLoadFile(id AudioPlayerID, path string) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	pathBytes := append([]byte(path), 0)
	result := fnAudioLoadFile(uint32(id), uintptr(unsafe.Pointer(&pathBytes[0])))
	runtime.KeepAlive(pathBytes)

	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioPlay(id AudioPlayerID) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioPlay(uint32(id))
	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioPause(id AudioPlayerID) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioPause(uint32(id))
	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioStop(id AudioPlayerID) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioStop(uint32(id))
	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioSeek(id AudioPlayerID, timestampMs uint64) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioSeek(uint32(id), timestampMs)
	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioSetLooping(id AudioPlayerID, looping bool) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioSetLooping(uint32(id), looping)
	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioSetVolume(id AudioPlayerID, volume float32) error {
	if !initialized {
		return &AudioError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioSetVolume(uint32(id), volume)
	if result < 0 {
		return &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}
	return nil
}

func AudioGetState(id AudioPlayerID) AudioState {
	if !initialized {
		return AudioStateError
	}
	result := fnAudioGetState(uint32(id))
	if result < 0 {
		return AudioStateError
	}
	return AudioState(result)
}

func AudioGetTime(id AudioPlayerID) uint64 {
	if !initialized {
		return 0
	}
	return fnAudioGetTime(uint32(id))
}

func AudioGetInfo(id AudioPlayerID) (*AudioInfo, error) {
	if !initialized {
		return nil, &AudioError{Code: -2, Message: "not initialized"}
	}

	var durationMs uint64
	var sampleRate, channels uint32

	result := fnAudioGetInfo(uint32(id),
		uintptr(unsafe.Pointer(&durationMs)),
		uintptr(unsafe.Pointer(&sampleRate)),
		uintptr(unsafe.Pointer(&channels)))

	if result < 0 {
		return nil, &AudioError{Code: int(result), Message: audioErrorMessage(int(result))}
	}

	return &AudioInfo{
		DurationMs: durationMs,
		SampleRate: sampleRate,
		Channels:   channels,
	}, nil
}

func AudioGetVolume(id AudioPlayerID) float32 {
	if !initialized {
		return 0
	}
	return fnAudioGetVolume(uint32(id))
}

func AudioIsLooping(id AudioPlayerID) bool {
	if !initialized {
		return false
	}
	return fnAudioIsLooping(uint32(id)) == 1
}

func AudioUpdate(id AudioPlayerID) bool {
	if !initialized {
		return false
	}
	return fnAudioUpdate(uint32(id)) == 1
}

// ============================================================================
// Video Playback API
// ============================================================================

type VideoPlayerID uint32

type VideoState int32

const (
	VideoStateIdle    VideoState = 0
	VideoStateLoading VideoState = 1
	VideoStatePlaying VideoState = 2
	VideoStatePaused  VideoState = 3
	VideoStateEnded   VideoState = 4
	VideoStateError   VideoState = 5
)

func (s VideoState) String() string {
	switch s {
	case VideoStateIdle:
		return "idle"
	case VideoStateLoading:
		return "loading"
	case VideoStatePlaying:
		return "playing"
	case VideoStatePaused:
		return "paused"
	case VideoStateEnded:
		return "ended"
	case VideoStateError:
		return "error"
	default:
		return "unknown"
	}
}

type VideoInfo struct {
	Width      uint32
	Height     uint32
	DurationMs uint64
}

type VideoError struct {
	Code    int
	Message string
}

func (e *VideoError) Error() string {
	return e.Message
}

func videoErrorMessage(code int) string {
	switch code {
	case -1:
		return "invalid parameters"
	case -2:
		return "player not found"
	case -3:
		return "video operation failed"
	case -4:
		return "failed to create texture"
	case -5:
		return "failed to update texture"
	default:
		return "unknown video error"
	}
}

func VideoCreate() VideoPlayerID {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}
	return VideoPlayerID(fnVideoCreate())
}

func VideoDestroy(id VideoPlayerID) {
	if !initialized {
		return
	}
	fnVideoDestroy(uint32(id))
}

func VideoLoadURL(id VideoPlayerID, url string) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	urlBytes := append([]byte(url), 0)
	result := fnVideoLoadURL(uint32(id), uintptr(unsafe.Pointer(&urlBytes[0])))
	runtime.KeepAlive(urlBytes)

	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoLoadFile(id VideoPlayerID, path string) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	pathBytes := append([]byte(path), 0)
	result := fnVideoLoadFile(uint32(id), uintptr(unsafe.Pointer(&pathBytes[0])))
	runtime.KeepAlive(pathBytes)

	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoInitStream(id VideoPlayerID, width, height uint32) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoInitStream(uint32(id), width, height)
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoPushFrame(id VideoPlayerID, width, height uint32, data []byte, timestampMs uint64) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	if len(data) == 0 {
		return &VideoError{Code: -1, Message: "empty frame data"}
	}

	result := fnVideoPushFrame(uint32(id), width, height,
		uintptr(unsafe.Pointer(&data[0])), uint64(len(data)), timestampMs)

	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoPlay(id VideoPlayerID) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoPlay(uint32(id))
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoPause(id VideoPlayerID) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoPause(uint32(id))
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoSeek(id VideoPlayerID, timestampMs uint64) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoSeek(uint32(id), timestampMs)
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoSetLooping(id VideoPlayerID, looping bool) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoSetLooping(uint32(id), looping)
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoSetMuted(id VideoPlayerID, muted bool) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoSetMuted(uint32(id), muted)
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoSetVolume(id VideoPlayerID, volume float32) error {
	if !initialized {
		return &VideoError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoSetVolume(uint32(id), volume)
	if result < 0 {
		return &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}
	return nil
}

func VideoGetState(id VideoPlayerID) VideoState {
	if !initialized {
		return VideoStateError
	}
	result := fnVideoGetState(uint32(id))
	if result < 0 {
		return VideoStateError
	}
	return VideoState(result)
}

func VideoGetTime(id VideoPlayerID) uint64 {
	if !initialized {
		return 0
	}
	return fnVideoGetTime(uint32(id))
}

func VideoGetInfo(id VideoPlayerID) (*VideoInfo, error) {
	if !initialized {
		return nil, &VideoError{Code: -2, Message: "not initialized"}
	}

	var width, height uint32
	var durationMs uint64

	result := fnVideoGetInfo(uint32(id),
		uintptr(unsafe.Pointer(&width)),
		uintptr(unsafe.Pointer(&height)),
		uintptr(unsafe.Pointer(&durationMs)))

	if result < 0 {
		return nil, &VideoError{Code: int(result), Message: videoErrorMessage(int(result))}
	}

	return &VideoInfo{
		Width:      width,
		Height:     height,
		DurationMs: durationMs,
	}, nil
}

func VideoUpdate(id VideoPlayerID) TextureID {
	if !initialized {
		return 0
	}
	result := fnVideoUpdate(uint32(id))
	if result <= 0 {
		return 0
	}
	return TextureID(result)
}

func VideoGetTextureID(id VideoPlayerID) TextureID {
	if !initialized {
		return 0
	}
	return TextureID(fnVideoGetTextureID(uint32(id)))
}

// ============================================================================
// Audio Input (Microphone)
// ============================================================================

type AudioInputID uint32

type AudioInputState int32

const (
	AudioInputStateIdle                 AudioInputState = 0
	AudioInputStateRequestingPermission AudioInputState = 1
	AudioInputStateReady                AudioInputState = 2
	AudioInputStateCapturing            AudioInputState = 3
	AudioInputStateStopped              AudioInputState = 4
	AudioInputStateError                AudioInputState = 5
)

type AudioInputDevice struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	IsDefault bool   `json:"is_default"`
}

type AudioInputError struct {
	Code    int
	Message string
}

func (e *AudioInputError) Error() string {
	return e.Message
}

func audioInputErrorMessage(code int) string {
	switch code {
	case -1:
		return "invalid parameter"
	case -2:
		return "input not found"
	case -3:
		return "input operation failed"
	default:
		return "unknown audio input error"
	}
}

func AudioInputCreate() AudioInputID {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}
	return AudioInputID(fnAudioInputCreate())
}

func AudioInputDestroy(id AudioInputID) {
	if !initialized {
		return
	}
	fnAudioInputDestroy(uint32(id))
}

func AudioInputRequestPermission(id AudioInputID) error {
	if !initialized {
		return &AudioInputError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioInputRequestPermission(uint32(id))
	if result != 0 {
		if result == 1 {
			return &AudioInputError{Code: 1, Message: "microphone permission required"}
		}
		return &AudioInputError{Code: int(result), Message: audioInputErrorMessage(int(result))}
	}
	return nil
}

func AudioInputHasPermission(id AudioInputID) bool {
	if !initialized {
		return false
	}
	return fnAudioInputHasPermission(uint32(id)) == 1
}

func AudioInputListDevices(id AudioInputID) ([]AudioInputDevice, error) {
	if !initialized {
		return nil, &AudioInputError{Code: -2, Message: "not initialized"}
	}

	ptr := fnAudioInputListDevices(uint32(id))
	if ptr == 0 {
		return nil, &AudioInputError{Code: -1, Message: "failed to list devices"}
	}
	defer fnFreeString(ptr)

	jsonStr := goString(ptr)
	var devices []AudioInputDevice
	if err := json.Unmarshal([]byte(jsonStr), &devices); err != nil {
		return nil, err
	}
	return devices, nil
}

func AudioInputOpen(id AudioInputID, deviceID string, sampleRate, channels uint32) error {
	if !initialized {
		return &AudioInputError{Code: -2, Message: "not initialized"}
	}

	var devicePtr uintptr
	var deviceBytes []byte
	if deviceID != "" {
		deviceBytes = append([]byte(deviceID), 0)
		devicePtr = uintptr(unsafe.Pointer(&deviceBytes[0]))
	}

	result := fnAudioInputOpen(uint32(id), devicePtr, sampleRate, channels)
	runtime.KeepAlive(deviceBytes)

	if result < 0 {
		return &AudioInputError{Code: int(result), Message: audioInputErrorMessage(int(result))}
	}
	return nil
}

func AudioInputStart(id AudioInputID) error {
	if !initialized {
		return &AudioInputError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioInputStart(uint32(id))
	if result < 0 {
		return &AudioInputError{Code: int(result), Message: audioInputErrorMessage(int(result))}
	}
	return nil
}

func AudioInputStop(id AudioInputID) error {
	if !initialized {
		return &AudioInputError{Code: -2, Message: "not initialized"}
	}
	result := fnAudioInputStop(uint32(id))
	if result < 0 {
		return &AudioInputError{Code: int(result), Message: audioInputErrorMessage(int(result))}
	}
	return nil
}

func AudioInputClose(id AudioInputID) {
	if !initialized {
		return
	}
	fnAudioInputClose(uint32(id))
}

func AudioInputGetState(id AudioInputID) AudioInputState {
	if !initialized {
		return AudioInputStateError
	}
	result := fnAudioInputGetState(uint32(id))
	if result < 0 {
		return AudioInputStateError
	}
	return AudioInputState(result)
}

func AudioInputGetLevel(id AudioInputID) float32 {
	if !initialized {
		return 0
	}
	return fnAudioInputGetLevel(uint32(id))
}

// ============================================================================
// Video Input (Camera)
// ============================================================================

type VideoInputID uint32

type VideoInputState int32

const (
	VideoInputStateIdle                 VideoInputState = 0
	VideoInputStateRequestingPermission VideoInputState = 1
	VideoInputStateReady                VideoInputState = 2
	VideoInputStateCapturing            VideoInputState = 3
	VideoInputStateStopped              VideoInputState = 4
	VideoInputStateError                VideoInputState = 5
)

type CameraPosition int32

const (
	CameraPositionUnspecified CameraPosition = 0
	CameraPositionBack        CameraPosition = 1
	CameraPositionFront       CameraPosition = 2
	CameraPositionExternal    CameraPosition = 3
)

type VideoInputDevice struct {
	ID          string      `json:"id"`
	Name        string      `json:"name"`
	Position    int32       `json:"position"`
	IsDefault   bool        `json:"is_default"`
	Resolutions [][2]uint32 `json:"resolutions"`
}

type VideoInputError struct {
	Code    int
	Message string
}

func (e *VideoInputError) Error() string {
	return e.Message
}

func videoInputErrorMessage(code int) string {
	switch code {
	case -1:
		return "invalid parameter"
	case -2:
		return "input not found"
	case -3:
		return "input operation failed"
	default:
		return "unknown video input error"
	}
}

func VideoInputCreate() VideoInputID {
	if !initialized {
		if err := initLibrary(); err != nil {
			return 0
		}
	}
	return VideoInputID(fnVideoInputCreate())
}

func VideoInputDestroy(id VideoInputID) {
	if !initialized {
		return
	}
	fnVideoInputDestroy(uint32(id))
}

func VideoInputRequestPermission(id VideoInputID) error {
	if !initialized {
		return &VideoInputError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoInputRequestPermission(uint32(id))
	if result != 0 {
		if result == 1 {
			return &VideoInputError{Code: 1, Message: "camera permission required"}
		}
		return &VideoInputError{Code: int(result), Message: videoInputErrorMessage(int(result))}
	}
	return nil
}

func VideoInputHasPermission(id VideoInputID) bool {
	if !initialized {
		return false
	}
	return fnVideoInputHasPermission(uint32(id)) == 1
}

func VideoInputListDevices(id VideoInputID) ([]VideoInputDevice, error) {
	if !initialized {
		return nil, &VideoInputError{Code: -2, Message: "not initialized"}
	}

	ptr := fnVideoInputListDevices(uint32(id))
	if ptr == 0 {
		return nil, &VideoInputError{Code: -1, Message: "failed to list devices"}
	}
	defer fnFreeString(ptr)

	jsonStr := goString(ptr)
	var devices []VideoInputDevice
	if err := json.Unmarshal([]byte(jsonStr), &devices); err != nil {
		return nil, err
	}
	return devices, nil
}

func VideoInputOpen(id VideoInputID, deviceID string, width, height, frameRate uint32) error {
	if !initialized {
		return &VideoInputError{Code: -2, Message: "not initialized"}
	}

	var devicePtr uintptr
	var deviceBytes []byte
	if deviceID != "" {
		deviceBytes = append([]byte(deviceID), 0)
		devicePtr = uintptr(unsafe.Pointer(&deviceBytes[0]))
	}

	result := fnVideoInputOpen(uint32(id), devicePtr, width, height, frameRate)
	runtime.KeepAlive(deviceBytes)

	if result < 0 {
		return &VideoInputError{Code: int(result), Message: videoInputErrorMessage(int(result))}
	}
	return nil
}

func VideoInputStart(id VideoInputID) error {
	if !initialized {
		return &VideoInputError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoInputStart(uint32(id))
	if result < 0 {
		return &VideoInputError{Code: int(result), Message: videoInputErrorMessage(int(result))}
	}
	return nil
}

func VideoInputStop(id VideoInputID) error {
	if !initialized {
		return &VideoInputError{Code: -2, Message: "not initialized"}
	}
	result := fnVideoInputStop(uint32(id))
	if result < 0 {
		return &VideoInputError{Code: int(result), Message: videoInputErrorMessage(int(result))}
	}
	return nil
}

func VideoInputClose(id VideoInputID) {
	if !initialized {
		return
	}
	fnVideoInputClose(uint32(id))
}

func VideoInputGetState(id VideoInputID) VideoInputState {
	if !initialized {
		return VideoInputStateError
	}
	result := fnVideoInputGetState(uint32(id))
	if result < 0 {
		return VideoInputStateError
	}
	return VideoInputState(result)
}

func VideoInputGetDimensions(id VideoInputID) (uint32, uint32, error) {
	if !initialized {
		return 0, 0, &VideoInputError{Code: -2, Message: "not initialized"}
	}

	var width, height uint32
	result := fnVideoInputGetDimensions(uint32(id),
		uintptr(unsafe.Pointer(&width)),
		uintptr(unsafe.Pointer(&height)))

	if result < 0 {
		return 0, 0, &VideoInputError{Code: int(result), Message: videoInputErrorMessage(int(result))}
	}
	return width, height, nil
}

func VideoInputGetFrameTexture(id VideoInputID, existingTextureID uint32) (uint32, error) {
	if !initialized {
		return 0, &VideoInputError{Code: -2, Message: "not initialized"}
	}

	result := fnVideoInputGetFrameTexture(uint32(id), existingTextureID)
	if result < 0 {
		switch result {
		case -1:
			return 0, &VideoInputError{Code: -1, Message: "backend not initialized"}
		case -2:
			return 0, &VideoInputError{Code: -2, Message: "video input not found"}
		case -3:
			return 0, &VideoInputError{Code: -3, Message: "no frame available"}
		case -4:
			return 0, &VideoInputError{Code: -4, Message: "texture upload failed"}
		default:
			return 0, &VideoInputError{Code: int(result), Message: "unknown error"}
		}
	}
	return uint32(result), nil
}

// ============================================================================
// Binary Render Command Serialization
// ============================================================================

func SerializeRenderCommands(commands []RenderCommand) []byte {
	buf := make([]byte, 0, len(commands)*64+4)
	buf = appendU32(buf, uint32(len(commands)))

	for _, cmd := range commands {
		if cmd.Clear != nil {
			buf = append(buf, 0x00)
			buf = append(buf, cmd.Clear.R, cmd.Clear.G, cmd.Clear.B, cmd.Clear.A)
		} else if cmd.DrawRect != nil {
			buf = append(buf, 0x01)
			buf = appendF32(buf, cmd.DrawRect.X)
			buf = appendF32(buf, cmd.DrawRect.Y)
			buf = appendF32(buf, cmd.DrawRect.Width)
			buf = appendF32(buf, cmd.DrawRect.Height)
			buf = appendU32(buf, cmd.DrawRect.Color)
			buf = appendF32(buf, cmd.DrawRect.CornerRadii[0])
			buf = appendF32(buf, cmd.DrawRect.CornerRadii[1])
			buf = appendF32(buf, cmd.DrawRect.CornerRadii[2])
			buf = appendF32(buf, cmd.DrawRect.CornerRadii[3])
			buf = appendF32(buf, cmd.DrawRect.Rotation)

			var flags byte
			if cmd.DrawRect.Border != nil {
				flags |= 0x01
			}
			if cmd.DrawRect.Gradient != nil {
				flags |= 0x02
			}
			buf = append(buf, flags)

			if cmd.DrawRect.Border != nil {
				buf = appendF32(buf, cmd.DrawRect.Border.Width)
				buf = appendU32(buf, cmd.DrawRect.Border.Color)
				var style byte
				switch cmd.DrawRect.Border.Style {
				case "Dashed":
					style = 1
				case "Dotted":
					style = 2
				default:
					style = 0
				}
				buf = append(buf, style)
			}

			if cmd.DrawRect.Gradient != nil {
				if cmd.DrawRect.Gradient.Linear != nil {
					buf = append(buf, 0)
					buf = appendF32(buf, cmd.DrawRect.Gradient.Linear.Angle)
					buf = append(buf, byte(len(cmd.DrawRect.Gradient.Linear.Stops)))
					for _, stop := range cmd.DrawRect.Gradient.Linear.Stops {
						buf = appendF32(buf, stop.Position)
						buf = appendU32(buf, stop.Color)
					}
				} else if cmd.DrawRect.Gradient.Radial != nil {
					buf = append(buf, 1)
					buf = appendF32(buf, cmd.DrawRect.Gradient.Radial.CenterX)
					buf = appendF32(buf, cmd.DrawRect.Gradient.Radial.CenterY)
					buf = append(buf, byte(len(cmd.DrawRect.Gradient.Radial.Stops)))
					for _, stop := range cmd.DrawRect.Gradient.Radial.Stops {
						buf = appendF32(buf, stop.Position)
						buf = appendU32(buf, stop.Color)
					}
				}
			}
		} else if cmd.DrawText != nil {
			buf = append(buf, 0x02)
			buf = appendF32(buf, cmd.DrawText.X)
			buf = appendF32(buf, cmd.DrawText.Y)
			buf = appendString(buf, cmd.DrawText.Text)

			var sourceType byte
			var fontName string
			if cmd.DrawText.Font.Source.Bundled != nil {
				sourceType = 1
				fontName = *cmd.DrawText.Font.Source.Bundled
			} else if cmd.DrawText.Font.Source.System != nil {
				sourceType = 0
				fontName = *cmd.DrawText.Font.Source.System
			} else {
				sourceType = 0
				fontName = "system"
			}
			buf = append(buf, sourceType)
			buf = appendString(buf, fontName)
			buf = appendU16(buf, cmd.DrawText.Font.Weight)
			var style byte
			if cmd.DrawText.Font.Style == FontStyleItalic {
				style = 1
			}
			buf = append(buf, style)
			buf = appendF32(buf, cmd.DrawText.Font.Size)

			buf = appendU32(buf, cmd.DrawText.Color)

			var layoutFlags byte
			if cmd.DrawText.Layout.MaxWidth != nil {
				layoutFlags |= 0x01
			}
			if cmd.DrawText.Layout.MaxHeight != nil {
				layoutFlags |= 0x02
			}
			if cmd.DrawText.Layout.MaxLines != nil {
				layoutFlags |= 0x04
			}
			buf = append(buf, layoutFlags)

			if cmd.DrawText.Layout.MaxWidth != nil {
				buf = appendF32(buf, *cmd.DrawText.Layout.MaxWidth)
			}
			if cmd.DrawText.Layout.MaxHeight != nil {
				buf = appendF32(buf, *cmd.DrawText.Layout.MaxHeight)
			}
			if cmd.DrawText.Layout.MaxLines != nil {
				buf = appendU32(buf, uint32(*cmd.DrawText.Layout.MaxLines))
			}

			buf = appendF32(buf, cmd.DrawText.Layout.LineHeight)
			buf = appendF32(buf, cmd.DrawText.Layout.LetterSpacing)
			buf = appendF32(buf, cmd.DrawText.Layout.WordSpacing)

			var alignment byte
			switch cmd.DrawText.Layout.Alignment {
			case TextAlignCenter:
				alignment = 1
			case TextAlignRight:
				alignment = 2
			case TextAlignJustify:
				alignment = 3
			default:
				alignment = 0
			}
			buf = append(buf, alignment)

			var vertAlign byte
			switch cmd.DrawText.Layout.VerticalAlign {
			case VerticalAlignMiddle:
				vertAlign = 1
			case VerticalAlignBottom:
				vertAlign = 2
			case VerticalAlignBaseline:
				vertAlign = 3
			default:
				vertAlign = 0
			}
			buf = append(buf, vertAlign)

			var wordBreak byte
			switch cmd.DrawText.Layout.WordBreak {
			case WordBreakBreakAll:
				wordBreak = 1
			case WordBreakKeepAll:
				wordBreak = 2
			case WordBreakBreakWord:
				wordBreak = 3
			default:
				wordBreak = 0
			}
			buf = append(buf, wordBreak)

			var overflow byte
			switch cmd.DrawText.Layout.Overflow {
			case TextOverflowEllipsis:
				overflow = 1
			case TextOverflowWrap:
				overflow = 2
			default:
				overflow = 0
			}
			buf = append(buf, overflow)

			var whiteSpace byte
			switch cmd.DrawText.Layout.WhiteSpace {
			case WhiteSpaceNoWrap:
				whiteSpace = 1
			case WhiteSpacePre:
				whiteSpace = 2
			case WhiteSpacePreWrap:
				whiteSpace = 3
			default:
				whiteSpace = 0
			}
			buf = append(buf, whiteSpace)
		} else if cmd.DrawImage != nil {
			buf = append(buf, 0x03)
			buf = appendF32(buf, cmd.DrawImage.X)
			buf = appendF32(buf, cmd.DrawImage.Y)
			buf = appendF32(buf, cmd.DrawImage.Width)
			buf = appendF32(buf, cmd.DrawImage.Height)
			buf = appendU32(buf, cmd.DrawImage.TextureID)

			var flags byte
			if cmd.DrawImage.SourceRect != nil {
				flags |= 0x01
			}
			buf = append(buf, flags)

			if cmd.DrawImage.SourceRect != nil {
				buf = appendF32(buf, cmd.DrawImage.SourceRect[0])
				buf = appendF32(buf, cmd.DrawImage.SourceRect[1])
				buf = appendF32(buf, cmd.DrawImage.SourceRect[2])
				buf = appendF32(buf, cmd.DrawImage.SourceRect[3])
			}

			buf = appendF32(buf, cmd.DrawImage.CornerRadii[0])
			buf = appendF32(buf, cmd.DrawImage.CornerRadii[1])
			buf = appendF32(buf, cmd.DrawImage.CornerRadii[2])
			buf = appendF32(buf, cmd.DrawImage.CornerRadii[3])
		} else if cmd.DrawShadow != nil {
			buf = append(buf, 0x04)
			buf = appendF32(buf, cmd.DrawShadow.X)
			buf = appendF32(buf, cmd.DrawShadow.Y)
			buf = appendF32(buf, cmd.DrawShadow.Width)
			buf = appendF32(buf, cmd.DrawShadow.Height)
			buf = appendF32(buf, cmd.DrawShadow.Blur)
			buf = appendU32(buf, cmd.DrawShadow.Color)
			buf = appendF32(buf, cmd.DrawShadow.OffsetX)
			buf = appendF32(buf, cmd.DrawShadow.OffsetY)
			buf = appendF32(buf, cmd.DrawShadow.CornerRadii[0])
			buf = appendF32(buf, cmd.DrawShadow.CornerRadii[1])
			buf = appendF32(buf, cmd.DrawShadow.CornerRadii[2])
			buf = appendF32(buf, cmd.DrawShadow.CornerRadii[3])
		} else if cmd.PushClip != nil {
			buf = append(buf, 0x05)
			buf = appendF32(buf, cmd.PushClip.X)
			buf = appendF32(buf, cmd.PushClip.Y)
			buf = appendF32(buf, cmd.PushClip.Width)
			buf = appendF32(buf, cmd.PushClip.Height)
		} else if cmd.PopClip != nil {
			buf = append(buf, 0x06)
		} else if cmd.BeginScrollView != nil {
			buf = append(buf, 0x07)
			buf = appendF32(buf, cmd.BeginScrollView.X)
			buf = appendF32(buf, cmd.BeginScrollView.Y)
			buf = appendF32(buf, cmd.BeginScrollView.Width)
			buf = appendF32(buf, cmd.BeginScrollView.Height)
			buf = appendF32(buf, cmd.BeginScrollView.ScrollX)
			buf = appendF32(buf, cmd.BeginScrollView.ScrollY)

			var flags byte
			if cmd.BeginScrollView.ContentWidth != nil {
				flags |= 0x01
			}
			if cmd.BeginScrollView.ContentHeight != nil {
				flags |= 0x02
			}
			buf = append(buf, flags)

			if cmd.BeginScrollView.ContentWidth != nil {
				buf = appendF32(buf, *cmd.BeginScrollView.ContentWidth)
			}
			if cmd.BeginScrollView.ContentHeight != nil {
				buf = appendF32(buf, *cmd.BeginScrollView.ContentHeight)
			}
		} else if cmd.EndScrollView != nil {
			buf = append(buf, 0x08)
		} else if cmd.SetOpacity != nil {
			buf = append(buf, 0x09)
			buf = appendF32(buf, *cmd.SetOpacity)
		}
	}

	return buf
}

func RenderFrameBinary(commands []RenderCommand) error {
	if len(commands) == 0 {
		return nil
	}

	payload := SerializeRenderCommands(commands)
	transport := GetTransport()
	respType, respPayload, err := transport.Execute(CmdRenderFrame, payload)
	if err != nil {
		return err
	}
	if respType == RespError {
		if len(respPayload) > 0 {
			return fmt.Errorf("render error: %s", string(respPayload))
		}
		return fmt.Errorf("render error")
	}
	return nil
}

// ============================================================================
// Binary Serialization Helpers
// ============================================================================

func appendU16(buf []byte, v uint16) []byte {
	return append(buf, byte(v), byte(v>>8))
}

func appendU32(buf []byte, v uint32) []byte {
	return append(buf, byte(v), byte(v>>8), byte(v>>16), byte(v>>24))
}

func appendF32(buf []byte, v float32) []byte {
	bits := math.Float32bits(v)
	return appendU32(buf, bits)
}

func appendString(buf []byte, s string) []byte {
	buf = appendU32(buf, uint32(len(s)))
	return append(buf, s...)
}
