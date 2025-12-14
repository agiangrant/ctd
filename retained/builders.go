package retained

// Builder helpers for common widget patterns.
// These provide a fluent API for constructing UI trees.

// Container creates a generic container widget.
func Container(classes string) *Widget {
	w := NewWidget(KindContainer)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// Flex creates a flexible container that respects flex-direction from classes.
// Use this for responsive layouts where direction changes based on breakpoints.
// Example: Flex("flex flex-row md:flex-col gap-4", child1, child2)
func Flex(classes string, children ...*Widget) *Widget {
	w := NewWidget(KindContainer)
	if classes != "" {
		w.SetClasses(classes)
	}
	for _, child := range children {
		w.AddChild(child)
	}
	return w
}

// VStack creates a vertical stack container.
// Children are laid out top-to-bottom.
func VStack(classes string, children ...*Widget) *Widget {
	w := NewWidget(KindVStack)
	if classes != "" {
		w.SetClasses(classes)
	}
	for _, child := range children {
		w.AddChild(child)
	}
	return w
}

// HStack creates a horizontal stack container.
// Children are laid out left-to-right.
func HStack(classes string, children ...*Widget) *Widget {
	w := NewWidget(KindHStack)
	if classes != "" {
		w.SetClasses(classes)
	}
	for _, child := range children {
		w.AddChild(child)
	}
	return w
}

// ZStack creates a depth stack container.
// Children are layered on top of each other.
func ZStack(classes string, children ...*Widget) *Widget {
	w := NewWidget(KindZStack)
	if classes != "" {
		w.SetClasses(classes)
	}
	for _, child := range children {
		w.AddChild(child)
	}
	return w
}

// ScrollView creates a scrollable container.
func ScrollView(classes string, children ...*Widget) *Widget {
	w := NewWidget(KindScrollView)
	w.scrollEnabled = true
	w.showScrollIndicators = true
	if classes != "" {
		w.SetClasses(classes)
	}
	for _, child := range children {
		w.AddChild(child)
	}
	return w
}

// Text creates a text widget.
func Text(text string, classes string) *Widget {
	w := NewWidget(KindText)
	w.SetText(text)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// Button creates a button widget.
func Button(text string, classes string) *Widget {
	w := NewWidget(KindButton)
	w.SetText(text)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// Image creates an image widget from a pre-loaded texture ID.
// The textureID should be loaded via ffi.LoadImage() or ffi.LoadImageFile().
func Image(textureID uint32, classes string) *Widget {
	w := NewWidget(KindImage)
	w.SetImageTextureID(textureID)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// ImageFromSource creates an image widget from a source path or URL.
// For bundled files: "assets/icon.png" (relative to working directory or executable)
// For URLs: "https://example.com/image.png" (will load asynchronously)
// The image will be loaded automatically when first rendered.
func ImageFromSource(source string, classes string) *Widget {
	w := NewWidget(KindImage)
	w.SetImageSource(source)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// ImageFromURL creates an image widget that loads from a URL asynchronously.
// Alias for ImageFromSource with a URL.
func ImageFromURL(url string, classes string) *Widget {
	return ImageFromSource(url, classes)
}

// ImageFromFile creates an image widget from a bundled file path.
// Alias for ImageFromSource with a file path.
func ImageFromFile(path string, classes string) *Widget {
	return ImageFromSource(path, classes)
}

// Video creates a video widget from a source path or URL.
// For local files: "/path/to/video.mp4"
// For URLs: "https://example.com/video.mp4"
// The video will be loaded automatically when first rendered.
func Video(source string, classes string) *Widget {
	w := NewWidget(KindVideo)
	w.videoSource = source
	w.videoVolume = 1.0 // Default volume
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// VideoFromURL creates a video widget that loads from a URL.
// Alias for Video with a URL.
func VideoFromURL(url string, classes string) *Widget {
	return Video(url, classes)
}

// VideoFromFile creates a video widget from a local file path.
// Alias for Video with a file path.
func VideoFromFile(path string, classes string) *Widget {
	return Video(path, classes)
}

// VideoStream creates a video widget for live/streaming content.
// Use PushFrame() to push video frames from external sources (WebRTC, etc.).
func VideoStream(width, height uint32, classes string) *Widget {
	w := NewWidget(KindVideo)
	w.videoNaturalW = width
	w.videoNaturalH = height
	w.videoVolume = 1.0
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// Audio creates an audio widget from a source path or URL.
// For local files: "/path/to/audio.mp3"
// For URLs: "https://example.com/audio.mp3"
// The audio will be loaded automatically when first rendered.
// Note: Audio widgets have no visual representation - they are invisible
// but can be controlled programmatically or via UI controls you build.
func Audio(source string, classes string) *Widget {
	w := NewWidget(KindAudio)
	w.audioSource = source
	w.audioVolume = 1.0 // Default volume
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// AudioFromURL creates an audio widget that loads from a URL.
// Alias for Audio with a URL.
func AudioFromURL(url string, classes string) *Widget {
	return Audio(url, classes)
}

// AudioFromFile creates an audio widget from a local file path.
// Alias for Audio with a file path.
func AudioFromFile(path string, classes string) *Widget {
	return Audio(path, classes)
}

// Microphone creates a microphone (audio input) widget for capturing audio.
// The microphone will request permission and start capturing when the widget is rendered.
// Note: Microphone widgets have no default visual representation - they capture audio
// and provide level data that you can visualize however you want.
func Microphone(classes string) *Widget {
	w := NewWidget(KindMicrophone)
	w.micAutoStart = true // Auto-start when ready by default
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// MicrophoneWithDevice creates a microphone widget that uses a specific device.
// deviceID: device ID from ListMicrophoneDevices (empty string for default device)
func MicrophoneWithDevice(deviceID string, classes string) *Widget {
	w := Microphone(classes)
	w.micDeviceID = deviceID
	return w
}

// Camera creates a camera (video input) widget for capturing video.
// The camera will request permission and start capturing when the widget is rendered.
// The captured video is displayed as the widget's content.
func Camera(classes string) *Widget {
	w := NewWidget(KindCamera)
	w.camAutoStart = true // Auto-start when ready by default
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// CameraWithDevice creates a camera widget that uses a specific device.
// deviceID: device ID from ListCameraDevices (empty string for default device)
func CameraWithDevice(deviceID string, classes string) *Widget {
	w := Camera(classes)
	w.camDeviceID = deviceID
	return w
}

// CameraWithResolution creates a camera widget with a specific resolution preference.
func CameraWithResolution(width, height, frameRate uint32, classes string) *Widget {
	w := Camera(classes)
	w.camWidth = width
	w.camHeight = height
	w.camFrameRate = frameRate
	return w
}

// TextField creates a single-line text input widget.
func TextField(placeholder string, classes string) *Widget {
	w := NewWidget(KindTextField)
	InitTextField(w, placeholder, false)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// Custom creates a custom widget with the specified type name.
func Custom(typeName string, classes string) *Widget {
	w := NewWidget(KindCustom)
	w.SetData(typeName)
	if classes != "" {
		w.SetClasses(classes)
	}
	return w
}

// Clipboard creates a clipboard widget for accessing and monitoring the system clipboard.
// This is a non-rendering widget that provides clipboard access as a data source.
func Clipboard() *Widget {
	w := NewWidget(KindClipboard)
	return w
}

// FilePicker creates a file picker widget for opening native file dialogs.
// This is a non-rendering widget that triggers native file open/save dialogs.
func FilePicker() *Widget {
	w := NewWidget(KindFilePicker)
	return w
}

// FilePickerWithFilters creates a file picker with predefined file type filters.
func FilePickerWithFilters(filters []FileFilter) *Widget {
	w := FilePicker()
	w.SetFilePickerFilters(filters)
	return w
}

// ImageFilePicker creates a file picker configured for image files.
func ImageFilePicker() *Widget {
	return FilePickerWithFilters([]FileFilter{
		{Name: "Images", Extensions: []string{"png", "jpg", "jpeg", "gif", "webp", "bmp"}},
	})
}

// TrayIcon creates a system tray icon widget for desktop applications.
// This is a non-rendering widget that creates an icon in the system tray (macOS menu bar).
// On mobile platforms this is a no-op.
func TrayIcon() *Widget {
	w := NewWidget(KindTrayIcon)
	w.trayVisible = true // Visible by default
	return w
}

// TrayIconWithTitle creates a tray icon with a text title (shown in menu bar).
func TrayIconWithTitle(title string) *Widget {
	w := TrayIcon()
	w.trayTitle = title
	return w
}

// TrayIconWithIcon creates a tray icon with an icon from a file path.
// The icon should be a template image (monochrome) for proper dark/light mode support.
func TrayIconWithIcon(iconPath string) *Widget {
	w := TrayIcon()
	w.trayIconPath = iconPath
	return w
}

// TrayIconWithIconData creates a tray icon with an icon from raw PNG data.
// The icon should be a template image (monochrome) for proper dark/light mode support.
func TrayIconWithIconData(iconData []byte) *Widget {
	w := TrayIcon()
	w.trayIconData = iconData
	return w
}

// TrayIconWithMenu creates a tray icon with a predefined menu.
func TrayIconWithMenu(items []MenuItem) *Widget {
	w := TrayIcon()
	w.trayMenu = items
	return w
}

// ============================================================================
// Fluent Builder Pattern
// ============================================================================

// With returns the widget for chaining.
func (w *Widget) With(fn func(*Widget)) *Widget {
	fn(w)
	return w
}

// WithChildren adds children to the widget.
func (w *Widget) WithChildren(children ...*Widget) *Widget {
	for _, child := range children {
		w.AddChild(child)
	}
	return w
}

// WithFrame sets position and size.
func (w *Widget) WithFrame(x, y, width, height float32) *Widget {
	return w.SetFrame(x, y, width, height)
}

// WithSize sets width and height.
func (w *Widget) WithSize(width, height float32) *Widget {
	return w.SetSize(width, height)
}

// WithPosition sets x and y.
func (w *Widget) WithPosition(x, y float32) *Widget {
	return w.SetPosition(x, y)
}

// WithBackground sets the background color.
func (w *Widget) WithBackground(color uint32) *Widget {
	return w.SetBackgroundColor(color)
}

// WithBorder sets border width and color.
func (w *Widget) WithBorder(width float32, color uint32) *Widget {
	return w.SetBorder(width, color)
}

// WithCornerRadius sets uniform corner radius.
func (w *Widget) WithCornerRadius(radius float32) *Widget {
	return w.SetCornerRadius(radius)
}

// WithOpacity sets opacity (0.0 to 1.0).
func (w *Widget) WithOpacity(opacity float32) *Widget {
	return w.SetOpacity(opacity)
}

// WithText sets text content.
func (w *Widget) WithText(text string) *Widget {
	return w.SetText(text)
}

// WithTextStyle sets text color and size.
func (w *Widget) WithTextStyle(color uint32, size float32) *Widget {
	w.SetTextColor(color)
	w.SetFontSize(size)
	return w
}

// WithScroll enables scrolling with content dimensions.
func (w *Widget) WithScroll(contentWidth, contentHeight float32) *Widget {
	w.mu.Lock()
	w.scrollEnabled = true
	w.contentWidth = contentWidth
	w.contentHeight = contentHeight
	w.mu.Unlock()
	return w
}

// WithPadding sets uniform padding on all sides.
func (w *Widget) WithPadding(padding float32) *Widget {
	return w.SetPaddingAll(padding, padding, padding, padding)
}

// WithPaddingXY sets horizontal and vertical padding.
func (w *Widget) WithPaddingXY(horizontal, vertical float32) *Widget {
	return w.SetPaddingAll(vertical, horizontal, vertical, horizontal)
}

// WithGap sets the spacing between children.
func (w *Widget) WithGap(gap float32) *Widget {
	return w.SetGap(gap)
}

// WithPositionMode sets how the widget is positioned.
func (w *Widget) WithPositionMode(pos Position) *Widget {
	return w.SetPositionMode(pos)
}

// WithData sets custom application data.
func (w *Widget) WithData(data any) *Widget {
	return w.SetData(data)
}

// WithAutoplay enables autoplay for video widgets.
func (w *Widget) WithAutoplay() *Widget {
	w.mu.Lock()
	w.videoAutoplay = true
	w.mu.Unlock()
	return w
}

// WithLoop enables looping for video widgets.
func (w *Widget) WithLoop() *Widget {
	w.mu.Lock()
	w.videoLoop = true
	w.mu.Unlock()
	return w
}

// WithMuted mutes audio for video widgets.
func (w *Widget) WithMuted() *Widget {
	w.mu.Lock()
	w.videoMuted = true
	w.mu.Unlock()
	return w
}

// WithVolume sets audio volume for video widgets (0.0 - 1.0).
func (w *Widget) WithVolume(volume float32) *Widget {
	w.mu.Lock()
	w.videoVolume = volume
	w.mu.Unlock()
	return w
}

// OnVideoEnded sets a callback for when video playback ends.
func (w *Widget) OnVideoEnded(fn func()) *Widget {
	w.mu.Lock()
	w.onVideoEnded = fn
	w.mu.Unlock()
	return w
}

// OnVideoError sets a callback for video errors.
func (w *Widget) OnVideoError(fn func(error)) *Widget {
	w.mu.Lock()
	w.onVideoError = fn
	w.mu.Unlock()
	return w
}

// WithAudioAutoplay enables autoplay for audio widgets.
func (w *Widget) WithAudioAutoplay() *Widget {
	w.mu.Lock()
	w.audioAutoplay = true
	w.mu.Unlock()
	return w
}

// WithAudioLoop enables looping for audio widgets.
func (w *Widget) WithAudioLoop() *Widget {
	w.mu.Lock()
	w.audioLoop = true
	w.mu.Unlock()
	return w
}

// WithAudioVolume sets audio volume for audio widgets (0.0 - 1.0).
func (w *Widget) WithAudioVolume(volume float32) *Widget {
	w.mu.Lock()
	w.audioVolume = volume
	w.mu.Unlock()
	return w
}

// OnAudioEnded sets a callback for when audio playback ends.
func (w *Widget) OnAudioEnded(fn func()) *Widget {
	w.mu.Lock()
	w.onAudioEnded = fn
	w.mu.Unlock()
	return w
}

// OnAudioError sets a callback for audio errors.
func (w *Widget) OnAudioError(fn func(error)) *Widget {
	w.mu.Lock()
	w.onAudioError = fn
	w.mu.Unlock()
	return w
}

// OnAudioTimeUpdate sets a callback for audio time updates.
// The callback receives (currentMs, durationMs).
func (w *Widget) OnAudioTimeUpdate(fn func(uint64, uint64)) *Widget {
	w.mu.Lock()
	w.onAudioTimeUpdate = fn
	w.mu.Unlock()
	return w
}

// ============================================================================
// Color Helpers
// ============================================================================

// RGB creates a color from RGB values (alpha = 255).
func RGB(r, g, b uint8) uint32 {
	return uint32(r)<<24 | uint32(g)<<16 | uint32(b)<<8 | 0xFF
}

// RGBA creates a color from RGBA values.
func RGBA(r, g, b, a uint8) uint32 {
	return uint32(r)<<24 | uint32(g)<<16 | uint32(b)<<8 | uint32(a)
}

// Hex parses a hex color string (e.g., "#FF5500" or "FF5500").
// Returns 0 on invalid input.
func Hex(s string) uint32 {
	if len(s) > 0 && s[0] == '#' {
		s = s[1:]
	}

	if len(s) != 6 && len(s) != 8 {
		return 0
	}

	var color uint32
	for _, c := range s {
		color <<= 4
		switch {
		case c >= '0' && c <= '9':
			color |= uint32(c - '0')
		case c >= 'a' && c <= 'f':
			color |= uint32(c - 'a' + 10)
		case c >= 'A' && c <= 'F':
			color |= uint32(c - 'A' + 10)
		default:
			return 0
		}
	}

	// Add alpha if not provided
	if len(s) == 6 {
		color = (color << 8) | 0xFF
	}

	return color
}

// ============================================================================
// Common Colors (Tailwind-inspired)
// ============================================================================

var (
	ColorTransparent = uint32(0x00000000)
	ColorWhite       = uint32(0xFFFFFFFF)
	ColorBlack       = uint32(0x000000FF)

	// Grays
	ColorGray50  = uint32(0xF9FAFBFF)
	ColorGray100 = uint32(0xF3F4F6FF)
	ColorGray200 = uint32(0xE5E7EBFF)
	ColorGray300 = uint32(0xD1D5DBFF)
	ColorGray400 = uint32(0x9CA3AFFF)
	ColorGray500 = uint32(0x6B7280FF)
	ColorGray600 = uint32(0x4B5563FF)
	ColorGray700 = uint32(0x374151FF)
	ColorGray800 = uint32(0x1F2937FF)
	ColorGray900 = uint32(0x111827FF)

	// Blues
	ColorBlue50  = uint32(0xEFF6FFFF)
	ColorBlue100 = uint32(0xDBEAFEFF)
	ColorBlue200 = uint32(0xBFDBFEFF)
	ColorBlue300 = uint32(0x93C5FDFF)
	ColorBlue400 = uint32(0x60A5FAFF)
	ColorBlue500 = uint32(0x3B82F6FF)
	ColorBlue600 = uint32(0x2563EBFF)
	ColorBlue700 = uint32(0x1D4ED8FF)
	ColorBlue800 = uint32(0x1E40AFFF)
	ColorBlue900 = uint32(0x1E3A8AFF)

	// Reds
	ColorRed50  = uint32(0xFEF2F2FF)
	ColorRed100 = uint32(0xFEE2E2FF)
	ColorRed200 = uint32(0xFECACAFF)
	ColorRed300 = uint32(0xFCA5A5FF)
	ColorRed400 = uint32(0xF87171FF)
	ColorRed500 = uint32(0xEF4444FF)
	ColorRed600 = uint32(0xDC2626FF)
	ColorRed700 = uint32(0xB91C1CFF)
	ColorRed800 = uint32(0x991B1BFF)
	ColorRed900 = uint32(0x7F1D1DFF)

	// Greens
	ColorGreen50  = uint32(0xF0FDF4FF)
	ColorGreen100 = uint32(0xDCFCE7FF)
	ColorGreen200 = uint32(0xBBF7D0FF)
	ColorGreen300 = uint32(0x86EFACFF)
	ColorGreen400 = uint32(0x4ADE80FF)
	ColorGreen500 = uint32(0x22C55EFF)
	ColorGreen600 = uint32(0x16A34AFF)
	ColorGreen700 = uint32(0x15803DFF)
	ColorGreen800 = uint32(0x166534FF)
	ColorGreen900 = uint32(0x14532DFF)

	// Yellows
	ColorYellow50  = uint32(0xFEFCE8FF)
	ColorYellow100 = uint32(0xFEF9C3FF)
	ColorYellow200 = uint32(0xFEF08AFF)
	ColorYellow300 = uint32(0xFDE047FF)
	ColorYellow400 = uint32(0xFACC15FF)
	ColorYellow500 = uint32(0xEAB308FF)
	ColorYellow600 = uint32(0xCA8A04FF)
	ColorYellow700 = uint32(0xA16207FF)
	ColorYellow800 = uint32(0x854D0EFF)
	ColorYellow900 = uint32(0x713F12FF)

	// Purples
	ColorPurple50  = uint32(0xFAF5FFFF)
	ColorPurple100 = uint32(0xF3E8FFFF)
	ColorPurple200 = uint32(0xE9D5FFFF)
	ColorPurple300 = uint32(0xD8B4FEFF)
	ColorPurple400 = uint32(0xC084FCFF)
	ColorPurple500 = uint32(0xA855F7FF)
	ColorPurple600 = uint32(0x9333EAFF)
	ColorPurple700 = uint32(0x7E22CEFF)
	ColorPurple800 = uint32(0x6B21A8FF)
	ColorPurple900 = uint32(0x581C87FF)
)
