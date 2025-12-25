//go:build darwin || linux || ios || android

package ffi

import (
	"github.com/ebitengine/purego"
)

// openLibrary loads a dynamic library on Unix-like systems
func openLibrary(path string) (uintptr, error) {
	const RTLD_LAZY = 0x1
	return purego.Dlopen(path, RTLD_LAZY)
}

// getSymbol retrieves a symbol from the loaded library
func getSymbol(handle uintptr, name string) (uintptr, error) {
	return purego.Dlsym(handle, name)
}
