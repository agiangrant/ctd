//go:build ios

package ctd

// detectDarwinPlatform returns iOS on iOS builds
func detectDarwinPlatform() Platform {
	return PlatformIOS
}
