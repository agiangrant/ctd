// Top-level build file for Centered Android Demo
//
// NOTE: Rust and Go builds are handled by the Taskfile (task android:build, task android:go)
// This Gradle project only handles the final APK assembly.
//
// Build workflow:
//   task android:build   # Build Rust engine for Android
//   task android:go      # Build Go code with gomobile
//   task android:apk     # Build APK (or just: cd examples/android_demo && ./gradlew assembleDebug)

plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
}
