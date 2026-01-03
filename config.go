package ctd

import "github.com/agiangrant/ctd/internal/ffi"

// AppConfig configures the application window and behavior.
// This is a re-export of ffi.AppConfig for consumer convenience.
type AppConfig = ffi.AppConfig

// TransportMode specifies how Go communicates with the Rust engine.
// This is a re-export of ffi.TransportMode for consumer convenience.
type TransportMode = ffi.TransportMode

const (
	// TransportSharedMemory uses binary protocol with dual buffers for maximum performance.
	// This is the default for production use.
	TransportSharedMemory = ffi.TransportSharedMemory

	// TransportFFI uses direct CGO calls with JSON encoding for easier debugging.
	// Use this during development when you need to trace individual calls.
	TransportFFI = ffi.TransportFFI
)

// DefaultAppConfig returns sensible defaults for a new application window.
func DefaultAppConfig() AppConfig {
	return ffi.DefaultAppConfig()
}
