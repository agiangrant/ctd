package ctd

import "runtime"

// Platform represents the current operating system/platform
type Platform string

const (
	PlatformMacOS   Platform = "darwin"
	PlatformIOS     Platform = "ios"
	PlatformAndroid Platform = "android"
	PlatformLinux   Platform = "linux"
	PlatformWindows Platform = "windows"
	PlatformWeb     Platform = "js"
	PlatformUnknown Platform = "unknown"
)

// CurrentPlatform returns the platform the app is running on
func CurrentPlatform() Platform {
	switch runtime.GOOS {
	case "darwin":
		// On darwin, check if we're on iOS or macOS
		// iOS builds set GOARCH to arm64 and have specific build tags
		// For now, we detect based on architecture + build tag
		// The ios build tag is set during gomobile compilation
		return detectDarwinPlatform()
	case "android":
		return PlatformAndroid
	case "linux":
		return PlatformLinux
	case "windows":
		return PlatformWindows
	case "js":
		return PlatformWeb
	default:
		return PlatformUnknown
	}
}

// IsMobile returns true if running on iOS or Android
func IsMobile() bool {
	p := CurrentPlatform()
	return p == PlatformIOS || p == PlatformAndroid
}

// IsDesktop returns true if running on macOS, Linux, or Windows
func IsDesktop() bool {
	p := CurrentPlatform()
	return p == PlatformMacOS || p == PlatformLinux || p == PlatformWindows
}

// IsIOS returns true if running on iOS
func IsIOS() bool {
	return CurrentPlatform() == PlatformIOS
}

// IsAndroid returns true if running on Android
func IsAndroid() bool {
	return CurrentPlatform() == PlatformAndroid
}

// IsMacOS returns true if running on macOS
func IsMacOS() bool {
	return CurrentPlatform() == PlatformMacOS
}

// IsLinux returns true if running on Linux
func IsLinux() bool {
	return CurrentPlatform() == PlatformLinux
}

// IsWindows returns true if running on Windows
func IsWindows() bool {
	return CurrentPlatform() == PlatformWindows
}

// IsWeb returns true if running in a web browser (WASM)
func IsWeb() bool {
	return CurrentPlatform() == PlatformWeb
}

// SupportsHaptics returns true if the platform supports haptic feedback
func SupportsHaptics() bool {
	return IsMobile()
}

// SupportsSystemTray returns true if the platform supports system tray icons
func SupportsSystemTray() bool {
	return IsDesktop()
}

// SupportsMultiWindow returns true if the platform supports multiple windows
func SupportsMultiWindow() bool {
	return IsDesktop()
}

// SupportsFileDialog returns true if the platform supports native file dialogs
func SupportsFileDialog() bool {
	return IsDesktop()
}

// HasPhysicalKeyboard returns true if the platform typically has a physical keyboard
func HasPhysicalKeyboard() bool {
	return IsDesktop()
}
