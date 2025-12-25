//go:build windows

package ffi

import (
	"fmt"

	"golang.org/x/sys/windows"
)

var winDLL *windows.DLL

// openLibrary loads a dynamic library on Windows
func openLibrary(path string) (uintptr, error) {
	dll, err := windows.LoadDLL(path)
	if err != nil {
		return 0, fmt.Errorf("LoadDLL failed: %w", err)
	}
	winDLL = dll
	// Return the actual HMODULE handle, not a pointer to the DLL struct
	return uintptr(dll.Handle), nil
}

// getSymbol retrieves a symbol from the loaded library on Windows
func getSymbol(handle uintptr, name string) (uintptr, error) {
	if winDLL == nil {
		return 0, fmt.Errorf("library not loaded")
	}
	proc, err := winDLL.FindProc(name)
	if err != nil {
		return 0, fmt.Errorf("FindProc(%s) failed: %w", name, err)
	}
	return proc.Addr(), nil
}
