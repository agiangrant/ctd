//go:build ios || android

// Mobile entry points for iOS and Android
//
// iOS Build:
//   gomobile bind -target ios -o UnifiedApp.xcframework ./examples/unified/app
//
// Android Build:
//   gomobile bind -target android -o unified_app.aar ./examples/unified/app
//
// These exports are called from the native platform code (main.m / MainActivity.java)
package app

import (
	"log"
	"runtime"
)

// Dummy export required by gomobile
func Dummy() {}

// IOSMain is called from main.m on iOS
func IOSMain() {
	runtime.LockOSThread()
	log.Println("Starting CTD Unified App (iOS)")
	application := New()
	if err := application.Run(); err != nil {
		log.Fatal(err)
	}
}

// AndroidMain is called from the native activity on Android
func AndroidMain() {
	runtime.LockOSThread()
	log.Println("Starting CTD Unified App (Android)")
	application := New()
	if err := application.Run(); err != nil {
		log.Fatal(err)
	}
}
