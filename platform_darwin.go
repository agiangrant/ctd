//go:build darwin && !ios

package ctd

// detectDarwinPlatform returns macOS on non-iOS darwin builds
func detectDarwinPlatform() Platform {
	return PlatformMacOS
}
