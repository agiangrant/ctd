package retained

import (
	"container/list"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/tw"
)

// textMeasureCache is an LRU cache for text width measurements.
// This prevents repeated FFI calls for the same text/font combinations.
type textMeasureCache struct {
	mu      sync.Mutex
	maxSize int
	cache   map[string]*list.Element
	lru     *list.List // Front = most recently used
}

type cacheEntry struct {
	key   string
	width float32
}

// newTextMeasureCache creates a new LRU cache with the specified max size.
func newTextMeasureCache(maxSize int) *textMeasureCache {
	return &textMeasureCache{
		maxSize: maxSize,
		cache:   make(map[string]*list.Element),
		lru:     list.New(),
	}
}

// get retrieves a cached width, returning (width, true) on hit or (0, false) on miss.
func (c *textMeasureCache) get(key string) (float32, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if elem, ok := c.cache[key]; ok {
		c.lru.MoveToFront(elem)
		return elem.Value.(*cacheEntry).width, true
	}
	return 0, false
}

// put stores a width in the cache, evicting old entries if needed.
func (c *textMeasureCache) put(key string, width float32) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if elem, ok := c.cache[key]; ok {
		c.lru.MoveToFront(elem)
		elem.Value.(*cacheEntry).width = width
		return
	}

	// Evict oldest entries if at capacity
	for c.lru.Len() >= c.maxSize {
		oldest := c.lru.Back()
		if oldest != nil {
			c.lru.Remove(oldest)
			delete(c.cache, oldest.Value.(*cacheEntry).key)
		}
	}

	// Add new entry
	entry := &cacheEntry{key: key, width: width}
	elem := c.lru.PushFront(entry)
	c.cache[key] = elem
}

// clear removes all entries from the cache.
func (c *textMeasureCache) clear() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.cache = make(map[string]*list.Element)
	c.lru.Init()
}

// globalTextCache is the shared text measurement cache.
// Default max size of 10,000 entries should handle most apps.
// For a book reader with lots of text, this prevents memory from growing unbounded.
var globalTextCache = newTextMeasureCache(10000)

// httpClient is a shared HTTP client for fetching images from URLs.
// Using a shared client enables connection pooling and reuse.
var httpClient = &http.Client{
	Timeout: 30 * time.Second,
}

// pendingTextureUpload represents image data ready to be uploaded to the GPU.
// GPU operations must happen on the main thread, so we queue them.
type pendingTextureUpload struct {
	widget *Widget
	data   []byte
	err    error
}

// pendingUploads is a channel for queueing texture uploads to the main thread.
// Buffered to avoid blocking the loading goroutines.
var pendingUploads = make(chan pendingTextureUpload, 100)

func init() {
	// Wire up the text measurement function to use FFI
	// This enables the Widget.TextWidth() method to measure text accurately
	SetMeasureTextWidthFunc(ffi.MeasureTextWidth)

	// Wire up the extended measurement function that supports font families
	SetMeasureTextWidthExtFunc(measureTextWithFontFamily)

	// Wire up the full metrics measurement function (width + height)
	// This enables accurate layout for fonts with non-standard metrics
	SetMeasureTextMetricsExtFunc(measureTextMetricsWithFontFamily)
}

// measureTextWithFontFamily measures text width, resolving fontFamily through theme fonts.
// This enables accurate measurement for bundled fonts configured in theme.toml.
//
// For text with newlines, it measures each line segment separately and returns the
// maximum width, since each segment renders as its own line.
func measureTextWithFontFamily(text string, fontName string, fontFamily string, fontSize float32) float32 {
	if text == "" {
		return 0
	}
	if fontSize == 0 {
		fontSize = 16 // default
	}

	// Split text by newlines and measure each segment separately.
	// Return the max width since each segment is its own line.
	segments := strings.Split(text, "\n")
	var maxWidth float32

	for _, segment := range segments {
		width := measureSingleLineText(segment, fontName, fontFamily, fontSize)
		if width > maxWidth {
			maxWidth = width
		}
	}

	return maxWidth
}

// measureSingleLineText measures a single line of text (no newlines) with caching.
func measureSingleLineText(text string, fontName string, fontFamily string, fontSize float32) float32 {
	if text == "" {
		return 0
	}

	// Build cache key from all font parameters
	cacheKey := fmt.Sprintf("%s|%s|%s|%.1f", text, fontName, fontFamily, fontSize)

	// Check cache first
	if width, ok := globalTextCache.get(cacheKey); ok {
		return width
	}

	// Cache miss - measure via FFI
	var width float32

	// If fontFamily is set, resolve it through theme fonts and use the full descriptor
	if fontFamily != "" {
		fonts := getThemeFonts()
		if config, ok := fonts[fontFamily]; ok {
			var font ffi.FontDescriptor
			if config.IsBundled {
				font = ffi.BundledFont(config.Value, fontSize)
			} else {
				font = ffi.SystemFont(config.Value, fontSize)
			}
			width = ffi.MeasureTextWithFont(text, font)
			globalTextCache.put(cacheKey, width)
			return width
		}
	}

	// Fall back to system font measurement
	if fontName == "" {
		fontName = "system"
	}
	width = ffi.MeasureTextWidth(text, fontName, fontSize)
	globalTextCache.put(cacheKey, width)
	return width
}

// measureTextMetricsWithFontFamily measures text and returns both width and height.
// This resolves fontFamily through theme fonts and uses actual font metrics for height.
// The height returned is the actual font height (ascent + descent), not fontSize.
func measureTextMetricsWithFontFamily(text string, fontName string, fontFamily string, fontSize float32) (float32, float32) {
	if fontSize == 0 {
		fontSize = 16 // default
	}

	// For width, use the existing function which handles newlines correctly
	width := measureTextWithFontFamily(text, fontName, fontFamily, fontSize)

	// For height, we need to get actual font metrics
	// We only need to measure once per font (not per text), so use empty string
	height := measureFontHeight(fontName, fontFamily, fontSize)

	return width, height
}

// measureFontHeight returns the actual font height (ascent + descent) for a given font.
// This is cached since font metrics don't change based on text content.
func measureFontHeight(fontName string, fontFamily string, fontSize float32) float32 {
	// Build cache key for font height (no text needed)
	cacheKey := fmt.Sprintf("height|%s|%s|%.1f", fontName, fontFamily, fontSize)

	// Check cache first
	if height, ok := globalTextCache.get(cacheKey); ok {
		return height
	}

	var height float32

	// If fontFamily is set, resolve it through theme fonts and use the full descriptor
	if fontFamily != "" {
		fonts := getThemeFonts()
		if config, ok := fonts[fontFamily]; ok {
			var font ffi.FontDescriptor
			if config.IsBundled {
				font = ffi.BundledFont(config.Value, fontSize)
			} else {
				font = ffi.SystemFont(config.Value, fontSize)
			}
			metrics := ffi.MeasureTextMetricsWithFont("", font)
			height = metrics.Height
			if height == 0 {
				height = fontSize // fallback if measurement fails
			}
			globalTextCache.put(cacheKey, height)
			return height
		}
	}

	// Fall back to system font measurement
	if fontName == "" {
		fontName = "system"
	}
	metrics := ffi.MeasureText("", fontName, fontSize)
	height = metrics.Height
	if height == 0 {
		height = fontSize // fallback if measurement fails
	}
	globalTextCache.put(cacheKey, height)
	return height
}

// textAlignToFFI converts widget textAlign string to FFI TextAlign enum.
// Handles "start" and "end" as aliases for "left" and "right" (LTR assumed).
func textAlignToFFI(align string) ffi.TextAlign {
	switch align {
	case "center":
		return ffi.TextAlignCenter
	case "right", "end":
		return ffi.TextAlignRight
	case "justify":
		return ffi.TextAlignJustify
	default: // "left", "start", or empty
		return ffi.TextAlignLeft
	}
}

// themeFontsCache caches the theme fonts for efficient lookup.
// Initialized on first use from tw.ThemeFonts().
var (
	themeFontsCache     map[string]tw.FontFamilyConfig
	themeFontsCacheOnce sync.Once
)

// getThemeFonts returns the cached theme fonts.
func getThemeFonts() map[string]tw.FontFamilyConfig {
	themeFontsCacheOnce.Do(func() {
		themeFontsCache = tw.ThemeFonts()
	})
	return themeFontsCache
}

// resolveFontDescriptor creates an ffi.FontDescriptor for the given widget.
// It resolves fontFamily through ThemeFonts() to get the actual font source.
func resolveFontDescriptor(w *Widget) ffi.FontDescriptor {
	fontSize := w.fontSize
	if fontSize == 0 {
		fontSize = 16 // default
	}

	// If fontFamily is set, look it up in theme fonts
	if w.fontFamily != "" {
		fonts := getThemeFonts()
		if config, ok := fonts[w.fontFamily]; ok {
			if config.IsBundled {
				return ffi.BundledFont(config.Value, fontSize)
			}
			return ffi.SystemFont(config.Value, fontSize)
		}
	}

	// Fall back to fontName (directly set) or system default
	fontName := w.fontName
	if fontName == "" {
		fontName = "system"
	}
	return ffi.SystemFont(fontName, fontSize)
}

// textCommand creates a text render command respecting font family settings.
// This should be used instead of ffi.Text() for widgets that support font family.
func textCommand(w *Widget, text string, x, y float32, color uint32) ffi.RenderCommand {
	// If widget has a font family set, use TextWithFont for proper resolution
	if w.fontFamily != "" {
		font := resolveFontDescriptor(w)
		return ffi.TextWithFont(text, x, y, font, color)
	}
	// Otherwise use simple Text() with fontName or default
	return ffi.Text(text, x, y, w.fontSize, color)
}

// ColorScheme represents the app's color scheme preference.
type ColorScheme int

const (
	// ColorSchemeSystem follows the OS dark/light mode setting.
	ColorSchemeSystem ColorScheme = iota
	// ColorSchemeLight forces light mode regardless of OS setting.
	ColorSchemeLight
	// ColorSchemeDark forces dark mode regardless of OS setting.
	ColorSchemeDark
)

// LoopConfig configures the game loop behavior.
type LoopConfig struct {
	// TargetFPS is the desired frames per second (default: 60).
	TargetFPS int

	// TreeConfig configures the widget tree.
	TreeConfig TreeConfig

	// Breakpoints configures the responsive design breakpoint thresholds.
	// If nil, uses tw.ThemeBreakpoints() from theme.toml.
	Breakpoints *tw.BreakpointConfig

	// ColorScheme sets the app's color scheme preference.
	// Default is ColorSchemeSystem (follow OS setting).
	ColorScheme ColorScheme
}

// DefaultLoopConfig returns sensible defaults.
func DefaultLoopConfig() LoopConfig {
	return LoopConfig{
		TargetFPS:  60,
		TreeConfig: DefaultTreeConfig(),
	}
}

// Frame provides context for each game loop iteration.
type Frame struct {
	// Number is the monotonically increasing frame counter.
	Number uint64

	// DeltaTime is seconds since the previous frame.
	DeltaTime float64

	// Time is seconds since loop start.
	Time float64

	// Tree provides access to the widget tree.
	Tree *Tree

	// Immediate commands to render this frame (on top of retained widgets).
	immediateCommands []ffi.RenderCommand
	immediateMu       sync.Mutex

	// Layer for immediate draws (default 0, higher = on top).
	currentLayer int
}

// SetLayer sets the z-order layer for subsequent immediate draws.
func (f *Frame) SetLayer(z int) {
	f.currentLayer = z
}

// Draw adds an immediate render command at the current layer.
func (f *Frame) Draw(cmd ffi.RenderCommand) {
	f.immediateMu.Lock()
	defer f.immediateMu.Unlock()
	f.immediateCommands = append(f.immediateCommands, cmd)
}

// DrawRect draws an immediate rectangle.
func (f *Frame) DrawRect(x, y, width, height float32, color uint32, radius float32) {
	f.Draw(ffi.RoundedRect(x, y, width, height, color, radius))
}

// DrawText draws immediate text.
func (f *Frame) DrawText(text string, x, y, size float32, color uint32) {
	f.Draw(ffi.Text(text, x, y, size, color))
}

// Clear adds a clear command.
func (f *Frame) Clear(r, g, b, a uint8) {
	f.Draw(ffi.Clear(r, g, b, a))
}

// Loop manages the retained+immediate mode game loop.
type Loop struct {
	tree       *Tree
	config     LoopConfig
	animations *AnimationRegistry
	events     *EventDispatcher

	// Timing
	targetFrameTime time.Duration
	startTime       time.Time
	lastFrameTime   time.Time

	// State
	running atomic.Bool
	paused  atomic.Bool

	// Mouse state for events that need position (like wheel)
	mouseX, mouseY float32

	// Responsive design
	breakpoints    tw.BreakpointConfig
	windowWidth    float32
	windowHeight   float32
	lastBreakpoint tw.Breakpoint // Track breakpoint changes for style reapplication

	// Color scheme / dark mode
	colorScheme ColorScheme
	darkMode    bool // Computed from colorScheme + OS preference

	// Scroll settings
	naturalScrolling bool // Cached from system preferences

	// Keyboard avoidance (iOS)
	keyboardHeight float32 // Height of on-screen keyboard in logical points (0 when hidden)

	// Event handlers
	onFrame  func(*Frame)
	onEvent  func(ffi.Event) bool // Return true to consume event
	onResize func(width, height float32)

	// Stats
	frameCount    atomic.Uint64
	droppedFrames atomic.Uint64

	// Deferred overlay commands (rendered on top of all widgets)
	// Used for select dropdowns, popups, tooltips, etc.
	deferredOverlays []ffi.RenderCommand

	// Scroll context stack for sticky positioning
	// When rendering inside scroll views, this tracks the viewport and scroll offset
	// so sticky elements can adjust their position appropriately
	scrollContextStack []scrollContext

	// Pending texture unloads - textures that should be unloaded AFTER the frame is rendered
	// This is needed because camera frames may be uploaded during tree traversal,
	// but old textures may still be referenced by render commands generated earlier
	pendingTextureUnloads []uint32
}

// scrollContext tracks the current scroll view for sticky positioning
type scrollContext struct {
	// Viewport bounds (where the scroll view appears on screen)
	viewportX, viewportY float32
	viewportW, viewportH float32
	// Current scroll offset (how much content has scrolled)
	scrollX, scrollY float32
	// Content start position (where content begins before scrolling)
	contentStartX, contentStartY float32
}

// NewLoop creates a game loop with the specified configuration.
func NewLoop(config LoopConfig) *Loop {
	if config.TargetFPS < 1 {
		config.TargetFPS = 60
	}

	// Set breakpoints from config or use theme defaults
	breakpoints := tw.ThemeBreakpoints()
	if config.Breakpoints != nil {
		breakpoints = *config.Breakpoints
	}

	tree := NewTree(config.TreeConfig)
	events := NewEventDispatcher(tree)
	tree.SetEventDispatcher(events) // Allow widgets to request focus changes

	// Initialize dark mode based on color scheme setting
	darkMode := false
	switch config.ColorScheme {
	case ColorSchemeSystem:
		darkMode = ffi.SystemDarkMode()
	case ColorSchemeDark:
		darkMode = true
	case ColorSchemeLight:
		darkMode = false
	}

	loop := &Loop{
		tree:             tree,
		config:           config,
		animations:       NewAnimationRegistry(),
		events:           events,
		targetFrameTime:  time.Second / time.Duration(config.TargetFPS),
		breakpoints:      breakpoints,
		colorScheme:      config.ColorScheme,
		darkMode:         darkMode,
		naturalScrolling: ffi.GetNaturalScrolling(),
	}

	// Set dark mode provider so widgets can check dark mode state
	tree.SetDarkModeProvider(func() bool {
		return loop.darkMode
	})

	return loop
}

// Animations returns the animation registry for this loop.
// Use this to create animations on widgets.
func (l *Loop) Animations() *AnimationRegistry {
	return l.animations
}

// Tree returns the widget tree.
func (l *Loop) Tree() *Tree {
	return l.tree
}

// Events returns the event dispatcher for this loop.
// Use this to programmatically focus widgets, check hover state, etc.
func (l *Loop) Events() *EventDispatcher {
	return l.events
}

// Breakpoints returns the current breakpoint configuration.
func (l *Loop) Breakpoints() tw.BreakpointConfig {
	return l.breakpoints
}

// WindowSize returns the current window dimensions.
func (l *Loop) WindowSize() (width, height float32) {
	return l.windowWidth, l.windowHeight
}

// ActiveBreakpoint returns which breakpoint is currently active based on window width.
func (l *Loop) ActiveBreakpoint() tw.Breakpoint {
	return l.breakpoints.ActiveBreakpoint(l.windowWidth)
}

// DarkMode returns whether dark mode is currently active.
func (l *Loop) DarkMode() bool {
	return l.darkMode
}

// darkModeFFI returns the dark mode value for FFI (0=light, 1=dark, 2=auto).
func (l *Loop) darkModeFFI() uint8 {
	switch l.colorScheme {
	case ColorSchemeLight:
		return 0
	case ColorSchemeDark:
		return 1
	default: // ColorSchemeSystem
		return 2 // auto - Rust will detect and respond to OS theme changes
	}
}

// response creates a FrameResponse with DarkMode already set.
func (l *Loop) response(requestRedraw bool) ffi.FrameResponse {
	return ffi.FrameResponse{
		RequestRedraw: requestRedraw,
		DarkMode:      l.darkModeFFI(),
	}
}

// ColorScheme returns the current color scheme setting.
func (l *Loop) ColorScheme() ColorScheme {
	return l.colorScheme
}

// SetColorScheme changes the color scheme and updates darkMode accordingly.
// This triggers a full re-render with the new color scheme.
func (l *Loop) SetColorScheme(scheme ColorScheme) {
	l.colorScheme = scheme

	// Recompute dark mode
	switch scheme {
	case ColorSchemeSystem:
		l.darkMode = ffi.SystemDarkMode()
	case ColorSchemeDark:
		l.darkMode = true
	case ColorSchemeLight:
		l.darkMode = false
	}

	// Re-apply styles to all widgets with the new dark mode setting
	if l.tree.Root() != nil {
		l.reapplyStylesRecursive(l.tree.Root())
		InvalidateTreeLayout(l.tree.Root())
	}
}

// reapplyStylesRecursive re-applies computed styles to a widget and its children.
// This is called when dark mode changes to update all widgets with new style values.
func (l *Loop) reapplyStylesRecursive(w *Widget) {
	w.mu.Lock()
	styles := w.computedStyles
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.Unlock()

	// Re-apply styles with current dark mode setting
	if styles != nil {
		resolved := styles.ResolveWithDarkMode(l.darkMode)
		w.mu.Lock()
		applyStyleProperties(w, &resolved)
		w.mu.Unlock()
	}

	// Recursively process children
	for _, child := range children {
		l.reapplyStylesRecursive(child)
	}

	releaseWidgetSlice(children)
}

// RefreshSystemDarkMode re-checks the OS dark mode setting.
// Only has effect when ColorScheme is ColorSchemeSystem.
// Call this if you suspect the OS setting has changed.
func (l *Loop) RefreshSystemDarkMode() {
	if l.colorScheme == ColorSchemeSystem {
		newDarkMode := ffi.SystemDarkMode()
		if newDarkMode != l.darkMode {
			l.darkMode = newDarkMode
			if l.tree.Root() != nil {
				l.reapplyStylesRecursive(l.tree.Root())
				InvalidateTreeLayout(l.tree.Root())
			}
		}
	}
}

// OnFrame sets the callback for each frame tick.
// This is where you add immediate draw commands and handle game logic.
func (l *Loop) OnFrame(fn func(*Frame)) {
	l.onFrame = fn
}

// OnEvent sets the callback for input events.
// Return true to consume the event (prevent default handling).
func (l *Loop) OnEvent(fn func(ffi.Event) bool) {
	l.onEvent = fn
}

// OnResize sets the callback for window resize.
func (l *Loop) OnResize(fn func(width, height float32)) {
	l.onResize = fn
}

// Run starts the game loop. This blocks until the window is closed.
func (l *Loop) Run(appConfig ffi.AppConfig) error {
	l.running.Store(true)
	l.startTime = time.Now()
	l.lastFrameTime = l.startTime

	// Set dark mode in app config based on loop's color scheme
	appConfig.DarkMode = l.darkModeFFI()

	// Preload bundled fonts from theme configuration.
	// On Android, copy fonts to app files directory first.
	// On web, this registers fonts with the browser.
	// On other native platforms, fonts load lazily.
	preloadBundledFonts()

	return ffi.Run(appConfig, l.handleEvent)
}

// preloadBundledFonts loads all bundled fonts from the theme configuration.
// On Android, fonts are loaded from assets (copied during build via ctd build-android).
// On web, this registers fonts with the browser's FontFace API.
// On other native platforms, fonts load lazily from the filesystem.
func preloadBundledFonts() {
	fonts := getThemeFonts()

	for name, config := range fonts {
		if !config.IsBundled {
			continue
		}

		// Call LoadBundledFont (no-op on native, registers on web)
		if err := ffi.LoadBundledFont(config.Value); err != nil {
			fmt.Printf("Warning: failed to load bundled font '%s' (%s): %v\n", name, config.Value, err)
		}
	}
}

// handleEvent is the FFI callback that drives the loop.
func (l *Loop) handleEvent(event ffi.Event) ffi.FrameResponse {
	// Let user handle event first (they can consume it)
	if l.onEvent != nil && l.onEvent(event) {
		// User consumed the event - only redraw if they modified widget state
		return l.response(l.tree.HasPendingUpdates())
	}

	switch event.Type {
	case ffi.EventReady:
		width, height := float32(event.Data1), float32(event.Data2)
		l.windowWidth = width
		l.windowHeight = height
		// Invalidate layout since window size affects w-full, h-full, etc.
		InvalidateTreeLayout(l.tree.Root())
		if l.onResize != nil {
			l.onResize(width, height)
		}
		return l.response(true)

	case ffi.EventResized:
		width, height := float32(event.Data1), float32(event.Data2)
		l.windowWidth = width
		l.windowHeight = height

		// Check if breakpoint changed - if so, reapply styles for all widgets
		newBreakpoint := l.breakpoints.ActiveBreakpoint(width)
		if newBreakpoint != l.lastBreakpoint {
			l.lastBreakpoint = newBreakpoint
			reapplyStylesForBreakpoint(l.tree.Root(), width, l.breakpoints, l.darkMode)
		}

		// Invalidate layout since window size affects w-full, h-full, etc.
		InvalidateTreeLayout(l.tree.Root())
		if l.onResize != nil {
			l.onResize(width, height)
		}
		return l.response(true)

	case ffi.EventRedrawRequested:
		return l.tick()

	case ffi.EventCloseRequested:
		l.running.Store(false)
		l.tree.Close()
		return l.response(false)

	case ffi.EventMouseMoved:
		x, y := float32(event.MouseX()), float32(event.MouseY())
		l.mouseX, l.mouseY = x, y
		hoverChanged := l.events.DispatchMouseMove(x, y, l.convertModifiers(event.Modifiers()))
		// Request redraw if hover state changed OR if event handlers modified widget state
		needsRedraw := hoverChanged || l.tree.HasPendingUpdates()
		return l.response(needsRedraw)

	case ffi.EventMousePressed:
		var x, y float32
		var button MouseButton
		if runtime.GOOS == "ios" || runtime.GOOS == "android" || runtime.GOOS == "js" {
			// On mobile (iOS/Android) and web (js), events send x/y coordinates directly
			x, y = float32(event.MouseX()), float32(event.MouseY())
			if runtime.GOOS == "js" {
				button = l.convertMouseButton(event.MouseButton())
			} else {
				button = MouseButtonLeft // Touch is always "left click"
			}
			// Update cached position for consistency
			l.mouseX, l.mouseY = x, y
		} else {
			// On desktop, MousePressed events don't include position (winit limitation)
			// Use the last known mouse position from MouseMoved events
			x, y = l.mouseX, l.mouseY
			button = l.convertMouseButton(event.MouseButton())
		}
		l.events.DispatchMouseDown(x, y, button, l.convertModifiers(event.Modifiers()))
		// Always redraw on press to show active state
		return l.response(true)

	case ffi.EventMouseReleased:
		var x, y float32
		var button MouseButton
		if runtime.GOOS == "ios" || runtime.GOOS == "android" || runtime.GOOS == "js" {
			// On mobile (iOS/Android) and web (js), events send x/y coordinates directly
			x, y = float32(event.MouseX()), float32(event.MouseY())
			if runtime.GOOS == "js" {
				button = l.convertMouseButton(event.MouseButton())
			} else {
				button = MouseButtonLeft // Touch is always "left click"
			}
			// Update cached position for consistency
			l.mouseX, l.mouseY = x, y
		} else {
			// On desktop, MouseReleased events don't include position (winit limitation)
			// Use the last known mouse position from MouseMoved events
			x, y = l.mouseX, l.mouseY
			button = l.convertMouseButton(event.MouseButton())
		}
		l.events.DispatchMouseUp(x, y, button, l.convertModifiers(event.Modifiers()))
		// Always redraw on release to clear active state
		return l.response(true)

	case ffi.EventMouseWheel:
		deltaX, deltaY := event.ScrollDelta()
		// On web (js), the wheel event includes mouse position
		if runtime.GOOS == "js" {
			l.mouseX = float32(event.MouseX())
			l.mouseY = float32(event.MouseY())
		}
		// On desktop, macOS/Windows provide scroll deltas with natural scrolling direction
		// when the system preference is enabled. When natural scrolling is disabled (traditional),
		// the delta direction is inverted. Our scroll logic expects natural scrolling direction,
		// so we negate the delta when traditional scrolling is in use.
		// Web always uses natural scrolling.
		if runtime.GOOS != "ios" && runtime.GOOS != "js" && !l.naturalScrolling {
			deltaX = -deltaX
			deltaY = -deltaY
		}
		l.events.DispatchMouseWheel(l.mouseX, l.mouseY, float32(deltaX), float32(deltaY), 0)
		// Only redraw if event handlers modified widget state (e.g., scroll position)
		return l.response(l.tree.HasPendingUpdates())

	case ffi.EventKeyPressed:
		keyCode := event.Keycode()
		key := l.keycodeToString(keyCode)
		mods := l.convertModifiers(event.Modifiers())
		// TODO: detect repeat (not currently exposed by FFI)
		l.events.DispatchKeyDown(keyCode, key, mods, false)
		// Only redraw if event handlers modified widget state
		return l.response(l.tree.HasPendingUpdates())

	case ffi.EventKeyReleased:
		keyCode := event.Keycode()
		key := l.keycodeToString(keyCode)
		mods := l.convertModifiers(event.Modifiers())
		l.events.DispatchKeyUp(keyCode, key, mods)
		// Only redraw if event handlers modified widget state
		return l.response(l.tree.HasPendingUpdates())

	case ffi.EventCharInput:
		char := event.Char()
		mods := l.convertModifiers(event.Modifiers())
		l.events.DispatchKeyPress(char, mods)
		// Only redraw if event handlers modified widget state
		return l.response(l.tree.HasPendingUpdates())

	case ffi.EventKeyboardFrameChanged:
		keyboardHeight := float32(event.Data1)
		animationDuration := time.Duration(event.Data2 * float64(time.Second))
		l.keyboardHeight = keyboardHeight
		// Scroll to keep focused input visible when keyboard appears
		if keyboardHeight > 0 {
			l.scrollToKeepFocusedInputVisible(animationDuration)
		}
		return l.response(l.tree.HasPendingUpdates())
	}

	return l.response(false)
}

// scrollToKeepFocusedInputVisible scrolls the nearest scrollable parent to ensure
// the focused text input is visible above the keyboard.
func (l *Loop) scrollToKeepFocusedInputVisible(animationDuration time.Duration) {
	// Get focused widget from event dispatcher
	focused := l.events.FocusedWidget()
	if focused == nil {
		return
	}

	// Check if it's a text input widget
	kind := focused.Kind()
	if kind != KindTextField && kind != KindTextArea {
		return
	}

	// Find the nearest scrollable parent
	scrollParent := findScrollableParent(focused)
	if scrollParent == nil {
		return
	}

	// Use the scroll animation utility with keyboard height
	cfg := ScrollToConfig{
		Duration: animationDuration,
		Easing:   EaseOutCubic,
		Padding:  20,
	}

	ScrollToWidgetWithKeyboard(scrollParent, focused, l.animations, cfg, l.keyboardHeight)
}

// findScrollableParent walks up the widget tree to find the nearest scrollable container.
func findScrollableParent(w *Widget) *Widget {
	parent := w.Parent()
	for parent != nil {
		parent.mu.RLock()
		isScrollable := parent.overflowY == "scroll" || parent.overflowY == "auto" ||
			(parent.kind == KindScrollView && parent.scrollEnabled)
		parent.mu.RUnlock()

		if isScrollable {
			return parent
		}
		parent = parent.Parent()
	}
	return nil
}

// convertMouseButton converts FFI mouse button to our MouseButton type.
func (l *Loop) convertMouseButton(button int) MouseButton {
	switch button {
	case 0:
		return MouseButtonLeft
	case 1:
		return MouseButtonRight
	case 2:
		return MouseButtonMiddle
	default:
		return MouseButtonNone
	}
}

// convertModifiers converts FFI modifiers to our Modifiers type.
func (l *Loop) convertModifiers(mods ffi.Modifiers) Modifiers {
	var result Modifiers
	if mods&ffi.ModShift != 0 {
		result |= ModShift
	}
	if mods&ffi.ModCtrl != 0 {
		result |= ModCtrl
	}
	if mods&ffi.ModAlt != 0 {
		result |= ModAlt
	}
	if mods&ffi.ModSuper != 0 {
		result |= ModSuper
	}
	return result
}

// keycodeToString converts a keycode to a human-readable string.
func (l *Loop) keycodeToString(keyCode uint32) string {
	switch ffi.Keycode(keyCode) {
	case ffi.KeyA:
		return "a"
	case ffi.KeyB:
		return "b"
	case ffi.KeyC:
		return "c"
	case ffi.KeyD:
		return "d"
	case ffi.KeyE:
		return "e"
	case ffi.KeyF:
		return "f"
	case ffi.KeyG:
		return "g"
	case ffi.KeyH:
		return "h"
	case ffi.KeyI:
		return "i"
	case ffi.KeyJ:
		return "j"
	case ffi.KeyK:
		return "k"
	case ffi.KeyL:
		return "l"
	case ffi.KeyM:
		return "m"
	case ffi.KeyN:
		return "n"
	case ffi.KeyO:
		return "o"
	case ffi.KeyP:
		return "p"
	case ffi.KeyQ:
		return "q"
	case ffi.KeyR:
		return "r"
	case ffi.KeyS:
		return "s"
	case ffi.KeyT:
		return "t"
	case ffi.KeyU:
		return "u"
	case ffi.KeyV:
		return "v"
	case ffi.KeyW:
		return "w"
	case ffi.KeyX:
		return "x"
	case ffi.KeyY:
		return "y"
	case ffi.KeyZ:
		return "z"
	case ffi.Key0:
		return "0"
	case ffi.Key1:
		return "1"
	case ffi.Key2:
		return "2"
	case ffi.Key3:
		return "3"
	case ffi.Key4:
		return "4"
	case ffi.Key5:
		return "5"
	case ffi.Key6:
		return "6"
	case ffi.Key7:
		return "7"
	case ffi.Key8:
		return "8"
	case ffi.Key9:
		return "9"
	case ffi.KeyUp:
		return "ArrowUp"
	case ffi.KeyDown:
		return "ArrowDown"
	case ffi.KeyLeft:
		return "ArrowLeft"
	case ffi.KeyRight:
		return "ArrowRight"
	case ffi.KeyHome:
		return "Home"
	case ffi.KeyEnd:
		return "End"
	case ffi.KeyPageUp:
		return "PageUp"
	case ffi.KeyPageDown:
		return "PageDown"
	case ffi.KeyBackspace:
		return "Backspace"
	case ffi.KeyDelete:
		return "Delete"
	case ffi.KeyEnter:
		return "Enter"
	case ffi.KeyTab:
		return "Tab"
	case ffi.KeyEscape:
		return "Escape"
	case ffi.KeySpace:
		return "Space"
	case ffi.KeyF1:
		return "F1"
	case ffi.KeyF2:
		return "F2"
	case ffi.KeyF3:
		return "F3"
	case ffi.KeyF4:
		return "F4"
	case ffi.KeyF5:
		return "F5"
	case ffi.KeyF6:
		return "F6"
	case ffi.KeyF7:
		return "F7"
	case ffi.KeyF8:
		return "F8"
	case ffi.KeyF9:
		return "F9"
	case ffi.KeyF10:
		return "F10"
	case ffi.KeyF11:
		return "F11"
	case ffi.KeyF12:
		return "F12"
	default:
		return ""
	}
}

// tick executes one frame of the game loop.
func (l *Loop) tick() ffi.FrameResponse {
	// Process any pending texture uploads on the main thread
	// This is safe to call here since we're on the main thread
	processPendingTextureUploads()

	// Process pending texture unloads from previous frame
	// These are old camera frame textures that were replaced during tree traversal
	// but couldn't be unloaded immediately because render commands still referenced them
	l.processPendingTextureUnloads()

	if l.paused.Load() {
		// When paused, still request redraw if animations or momentum scrolling are active
		return l.response(l.animations.HasActive() || l.events.IsMomentumScrolling())
	}

	now := time.Now()
	deltaTime := now.Sub(l.lastFrameTime).Seconds()
	totalTime := now.Sub(l.startTime).Seconds()
	l.lastFrameTime = now

	// Tick all animations - this updates widget properties
	// Animations modify widgets which will be included in the single FFI batch
	hasActiveAnimations := l.animations.Tick(now)

	// Update momentum scrolling (for touch/swipe scrolling)
	hasMomentumScrolling := l.events.UpdateMomentumScroll()

	// Update cursor blink for focused text input widgets
	cursorBlinkChanged, msUntilNextBlink := l.updateCursorBlink()

	// Check if any video or audio is playing or streaming (needs continuous redraws)
	// Consolidated into single tree traversal for efficiency
	hasPlayingVideo, hasPlayingAudio, hasStreamingVideo := l.hasPlayingMedia()

	// Compute layout pass (only runs if layout is dirty)
	// This resolves w-full, h-full, flex-1, percentages, etc.
	layoutChanged := ComputeLayout(l.tree.Root(), l.windowWidth, l.windowHeight)

	// After layout changes (e.g., window resize), clamp scroll positions
	// to ensure they don't exceed the new bounds
	if layoutChanged {
		ClampScrollPositions(l.tree.Root())
		// Also sync hit-testing bounds from layout immediately so that
		// touch/mouse events work correctly before the next render completes
		SyncBoundsFromLayout(l.tree.Root())
	}

	frameNum := l.tree.IncrementFrame()
	l.frameCount.Add(1)

	// Create frame context
	frame := &Frame{
		Number:            frameNum,
		DeltaTime:         deltaTime,
		Time:              totalTime,
		Tree:              l.tree,
		immediateCommands: make([]ffi.RenderCommand, 0, 64),
	}

	// Call user frame callback (for immediate mode draws)
	if l.onFrame != nil {
		l.onFrame(frame)
	}

	// Collect widget updates from all channels
	updates := l.tree.CollectUpdates()

	// Deduplicate updates
	deltas := l.tree.DeduplicateUpdates(updates)

	// Build render commands - single FFI call with all batched updates
	commands := l.buildRenderCommands(frame, deltas)

	// Determine if we need to keep requesting redraws:
	// - Active animations need continuous 60 FPS
	// - User's onFrame callback with immediate draws needs continuous updates
	// - Cursor blink changed (include in this frame)
	// - Playing videos/audio need continuous frame updates for time tracking
	// - Streaming video (receiving live frames) needs continuous updates
	// - Otherwise, only redraw when something changes (events will trigger redraws)
	hasImmediateDraws := len(frame.immediateCommands) > 0
	needsContinuousRedraw := hasActiveAnimations || hasMomentumScrolling || cursorBlinkChanged || hasPlayingVideo || hasPlayingAudio || hasStreamingVideo || (l.onFrame != nil && hasImmediateDraws)

	// For cursor blink, use delayed redraw instead of continuous polling
	// This allows CPU to sleep between blinks instead of running at 60 FPS
	var redrawAfterMs uint32
	if !needsContinuousRedraw && msUntilNextBlink > 0 {
		redrawAfterMs = msUntilNextBlink
	}

	return ffi.FrameResponse{
		ImmediateCommands: commands,
		RequestRedraw:     needsContinuousRedraw,
		RedrawAfterMs:     redrawAfterMs,
		DarkMode:          l.darkModeFFI(),
	}
}

// updateCursorBlink updates the cursor blink state for focused text input widgets.
// Returns:
// - blinkChanged: true if the cursor visibility actually toggled (requires redraw)
// - msUntilNextBlink: milliseconds until next blink toggle (0 if no text input focused)
func (l *Loop) updateCursorBlink() (blinkChanged bool, msUntilNextBlink uint32) {
	focused := l.events.FocusedWidget()
	if focused == nil {
		return false, 0
	}

	// Check if focused widget is a text input
	focused.mu.RLock()
	textBuffer := focused.textBuffer
	kind := focused.kind
	focused.mu.RUnlock()

	if textBuffer == nil || (kind != KindTextField && kind != KindTextArea) {
		return false, 0
	}

	// Update blink state - returns true when visibility actually toggled
	blinkChanged = textBuffer.UpdateBlink()

	// Calculate time until next blink
	timeUntilBlink := textBuffer.TimeUntilNextBlink()
	if timeUntilBlink > 0 {
		msUntilNextBlink = uint32(timeUntilBlink.Milliseconds())
		// Ensure at least 1ms to avoid busy loop
		if msUntilNextBlink == 0 {
			msUntilNextBlink = 1
		}
	}

	return blinkChanged, msUntilNextBlink
}

// hasPlayingMedia checks if any video or audio widget in the tree is currently playing or streaming.
// Returns (hasPlayingVideo, hasPlayingAudio, hasStreamingVideo) to determine if continuous redraws are needed.
// hasStreamingVideo is true when a Video widget is receiving live frames (no file source but has texture).
// This consolidates multiple tree traversals into one for efficiency.
func (l *Loop) hasPlayingMedia() (hasPlayingVideo, hasPlayingAudio, hasStreamingVideo bool) {
	root := l.tree.Root()
	if root == nil {
		return false, false, false
	}
	return hasPlayingMediaInTree(root)
}

// hasPlayingMediaInTree recursively checks if any video or audio widget is active.
// Returns as soon as all are found to avoid unnecessary traversal.
func hasPlayingMediaInTree(w *Widget) (hasPlayingVideo, hasPlayingAudio, hasStreamingVideo bool) {
	w.mu.RLock()
	kind := w.kind
	videoPlayerID := w.videoPlayerID
	videoState := w.videoState
	videoTextureID := w.videoTextureID
	videoSource := w.videoSource
	audioPlayerID := w.audioPlayerID
	audioState := w.audioState
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()
	defer releaseWidgetSlice(children)

	// Check if this widget is a playing video (file playback)
	if kind == KindVideo && videoPlayerID != 0 && videoState == int32(ffi.VideoStatePlaying) {
		hasPlayingVideo = true
	}
	// Check if this widget is receiving live video frames (streaming, no file source)
	if kind == KindVideo && videoSource == "" && videoTextureID != 0 {
		hasStreamingVideo = true
	}
	if kind == KindAudio && audioPlayerID != 0 && audioState == int32(ffi.AudioStatePlaying) {
		hasPlayingAudio = true
	}

	// If we found all, no need to traverse further
	if hasPlayingVideo && hasPlayingAudio && hasStreamingVideo {
		return hasPlayingVideo, hasPlayingAudio, hasStreamingVideo
	}

	// Check children
	for _, child := range children {
		childPlayingVideo, childPlayingAudio, childStreamingVideo := hasPlayingMediaInTree(child)
		hasPlayingVideo = hasPlayingVideo || childPlayingVideo
		hasPlayingAudio = hasPlayingAudio || childPlayingAudio
		hasStreamingVideo = hasStreamingVideo || childStreamingVideo
		// Early exit if we found all
		if hasPlayingVideo && hasPlayingAudio && hasStreamingVideo {
			return hasPlayingVideo, hasPlayingAudio, hasStreamingVideo
		}
	}
	return hasPlayingVideo, hasPlayingAudio, hasStreamingVideo
}

// buildRenderCommands converts the tree + immediate draws to render commands.
func (l *Loop) buildRenderCommands(frame *Frame, deltas map[WidgetID]*WidgetDelta) []ffi.RenderCommand {
	var commands []ffi.RenderCommand

	// Clear deferred overlays from previous frame
	l.deferredOverlays = l.deferredOverlays[:0]

	// Start with clear (can be overridden by user)
	commands = append(commands, ffi.Clear(26, 26, 38, 255))

	// Update event dispatcher's frame counter for bounds validation
	l.events.SetCurrentFrame(frame.Number)

	// Render retained widget tree with layout
	root := l.tree.Root()
	if root != nil {
		// Root widget uses its own x/y as the starting position
		commands = l.renderWidgetAt(commands, root, root.x, root.y, frame.Number)
	}

	// Append deferred overlays (dropdowns, popups, tooltips - rendered on top of everything)
	commands = append(commands, l.deferredOverlays...)

	// Append immediate commands (layered on top)
	commands = append(commands, frame.immediateCommands...)

	return commands
}

// renderWidgetAt renders a widget at the given computed position.
// parentX, parentY represent the top-left corner where this widget should be placed.
// frameNum is used for bounds caching to enable O(1) hit testing.
func (l *Loop) renderWidgetAt(commands []ffi.RenderCommand, w *Widget, parentX, parentY float32, frameNum uint64) []ffi.RenderCommand {
	w.mu.RLock()

	if !w.visible {
		w.mu.RUnlock()
		return commands
	}

	// Use computed layout if available, otherwise fall back to legacy calculation
	var renderX, renderY, widgetWidth, widgetHeight float32
	position := w.position
	stickyTop := w.posTop // sticky threshold (e.g., top-0 = 0, top-2 = 8px)

	if w.computedLayout.Valid {
		// Use pre-computed layout values (efficient path)
		renderX = w.computedLayout.X
		renderY = w.computedLayout.Y
		widgetWidth = w.computedLayout.Width
		widgetHeight = w.computedLayout.Height
	} else {
		// Legacy path: compute position on-the-fly (for backwards compatibility)
		switch position {
		case PositionStatic, PositionRelative:
			renderX = parentX + w.x
			renderY = parentY + w.y
			if position == PositionStatic {
				renderX = parentX
				renderY = parentY
			}
		case PositionAbsolute:
			renderX = w.x
			renderY = w.y
		case PositionFixed:
			renderX = w.x
			renderY = w.y
		}
		widgetWidth = w.width
		widgetHeight = w.height
	}

	w.mu.RUnlock()

	// Handle sticky positioning when inside a scroll view
	// Sticky elements "stick" at their threshold when they would scroll past it
	if position == PositionSticky && len(l.scrollContextStack) > 0 {
		ctx := l.scrollContextStack[len(l.scrollContextStack)-1]

		// Get the sticky threshold (default to 0 if not set, like top-0)
		threshold := float32(0)
		if stickyTop != nil {
			threshold = *stickyTop
		}

		// Check if element would scroll above its sticky threshold within its viewport
		// Element's position relative to viewport top
		contentRelY := renderY - ctx.viewportY

		// If contentRelY < scrollY + threshold, element should stick
		if contentRelY < ctx.scrollY+threshold {
			// Element should stick - set renderY so it appears at threshold within viewport
			// After Rust applies scroll offset (-scrollY), this becomes viewportY + threshold
			renderY = ctx.viewportY + threshold + ctx.scrollY
		}
	}

	// Update bounds outside the lock to avoid potential deadlock with updateBounds
	w.updateBounds(renderX, renderY, widgetWidth, widgetHeight, frameNum)

	// Re-acquire lock for the rest of rendering
	w.mu.RLock()

	// Render background BEFORE scroll view so it stays fixed
	// Use computed layout dimensions (widgetWidth/widgetHeight) instead of raw w.width/w.height
	// because computed layout accounts for auto-sizing, percentage sizing, etc.
	if w.backgroundColor != nil {
		// Apply opacity to the background color's alpha channel
		bgColor := *w.backgroundColor
		if w.opacity < 1.0 {
			alpha := float32(bgColor&0xFF) * w.opacity
			bgColor = (bgColor & 0xFFFFFF00) | uint32(alpha)
		}
		cmd := ffi.RenderCommand{
			DrawRect: &ffi.DrawRectCmd{
				X:           renderX,
				Y:           renderY,
				Width:       widgetWidth,
				Height:      widgetHeight,
				Color:       bgColor,
				CornerRadii: w.cornerRadius,
				Rotation:    w.rotation,
			},
		}
		// Add border if set
		if w.borderColor != nil && w.borderWidth > 0 {
			cmd.DrawRect.Border = &ffi.Border{
				Width: w.borderWidth,
				Color: *w.borderColor,
				Style: "Solid",
			}
		}
		commands = append(commands, cmd)
	} else if w.borderColor != nil && w.borderWidth > 0 {
		// Border only (no fill)
		commands = append(commands, ffi.RenderCommand{
			DrawRect: &ffi.DrawRectCmd{
				X:           renderX,
				Y:           renderY,
				Width:       widgetWidth,
				Height:      widgetHeight,
				Color:       0x00000000, // Transparent
				CornerRadii: w.cornerRadius,
				Rotation:    w.rotation,
				Border: &ffi.Border{
					Width: w.borderWidth,
					Color: *w.borderColor,
					Style: "Solid",
				},
			},
		})
	}

	// Handle scroll view or overflow scroll/auto containers AFTER background
	isScrollable := (w.kind == KindScrollView && w.scrollEnabled) ||
		w.overflowY == "scroll" || w.overflowY == "auto" ||
		w.overflowX == "scroll" || w.overflowX == "auto"
	if isScrollable {
		commands = append(commands, ffi.BeginScrollView(
			renderX, renderY, widgetWidth, widgetHeight,
			w.scrollX, w.scrollY,
		))
		// Push scroll context for sticky positioning
		// Content starts at the viewport position (before scroll offset is applied)
		l.scrollContextStack = append(l.scrollContextStack, scrollContext{
			viewportX:     renderX,
			viewportY:     renderY,
			viewportW:     widgetWidth,
			viewportH:     widgetHeight,
			scrollX:       w.scrollX,
			scrollY:       w.scrollY,
			contentStartX: renderX + w.padding[3],
			contentStartY: renderY + w.padding[0],
		})
		// Note: defer won't work correctly with slice append, so we handle EndScrollView below
	} else if w.overflowX == "hidden" || w.overflowX == "clip" ||
		w.overflowY == "hidden" || w.overflowY == "clip" {
		// For hidden/clip, just use a clip rect (no scroll offset)
		commands = append(commands, ffi.PushClip(renderX, renderY, widgetWidth, widgetHeight))
	}

	// Render text if present (text is drawn inside the widget's bounds with padding)
	if w.text != "" && (w.kind == KindText || w.kind == KindButton) {
		textX := renderX + w.padding[3] // left padding
		textY := renderY + w.padding[0] // top padding
		contentWidth := widgetWidth - w.padding[1] - w.padding[3]

		// Resolve font descriptor (handles fontFamily -> bundled/system font)
		font := resolveFontDescriptor(w)

		// Use text layout with wrapping for Text widgets (not buttons)
		if w.kind == KindText && contentWidth > 0 {
			layout := ffi.WrappedTextLayout(contentWidth)
			lh := w.lineHeight
			if lh == 0 {
				lh = 1.4
			}
			layout.LineHeight = lh
			// Apply text alignment
			layout.Alignment = textAlignToFFI(w.textAlign)
			commands = append(commands, ffi.TextWithLayout(
				w.text,
				textX, textY,
				font,
				w.textColor,
				layout,
			))
		} else {
			// Buttons use single-line text with alignment
			layout := ffi.DefaultTextLayout()
			layout.Alignment = textAlignToFFI(w.textAlign)
			commands = append(commands, ffi.TextWithLayout(
				w.text,
				textX, textY,
				font,
				w.textColor,
				layout,
			))
		}
	}

	// Render image widgets - release lock first since renderImage may trigger async loading
	if w.kind == KindImage {
		w.mu.RUnlock()
		commands = l.renderImage(commands, w, renderX, renderY, widgetWidth, widgetHeight)
		w.mu.RLock()
	}

	// Render video widgets - release lock first since renderVideo may trigger loading/updates
	if w.kind == KindVideo {
		w.mu.RUnlock()
		commands = l.renderVideo(commands, w, renderX, renderY, widgetWidth, widgetHeight)
		w.mu.RLock()
	}

	// Handle audio widgets - no visual rendering, just playback management
	if w.kind == KindAudio {
		w.mu.RUnlock()
		l.updateAudio(w)
		w.mu.RLock()
	}

	// Handle microphone widgets - no visual rendering, just capture management
	if w.kind == KindMicrophone {
		w.mu.RUnlock()
		l.updateMicrophone(w)
		w.mu.RLock()
	}

	// Handle camera widgets - update state and render preview
	if w.kind == KindCamera {
		w.mu.RUnlock()
		l.updateCamera(w)
		commands = l.renderCamera(commands, w, renderX, renderY, widgetWidth, widgetHeight)
		w.mu.RLock()
	}

	// Handle clipboard widgets - non-rendering data source for clipboard access
	if w.kind == KindClipboard {
		w.mu.RUnlock()
		l.updateClipboard(w)
		w.mu.RLock()
	}

	// Handle tray icon widgets - non-rendering data source for system tray
	if w.kind == KindTrayIcon {
		w.mu.RUnlock()
		l.updateTrayIcon(w)
		w.mu.RLock()
	}

	// Render text input fields
	if w.kind == KindTextField || w.kind == KindTextArea {
		textX := renderX + w.padding[3]
		textY := renderY + w.padding[0]
		contentWidth := widgetWidth - w.padding[1] - w.padding[3]
		lineHeight := w.fontSize * 1.5

		// Get text and cursor info from buffer
		displayText := ""
		cursorPos := 0
		selStart, selEnd := 0, 0
		isPlaceholder := false

		if w.textBuffer != nil {
			// Use DisplayText() which handles password masking
			displayText = w.textBuffer.DisplayText()
			cursorPos = w.textBuffer.Cursor()
			selStart, selEnd = w.textBuffer.Selection()
			if w.textBuffer.Text() == "" && w.textBuffer.Placeholder() != "" {
				displayText = w.textBuffer.Placeholder()
				isPlaceholder = true
			}
		}

		// Determine text color
		textColor := w.textColor
		if isPlaceholder {
			// Priority for placeholder color:
			// 1. Tailwind placeholder: variant (from computedStyles.Placeholder)
			// 2. SetPlaceholderColor() on textBuffer
			// 3. Default gray
			if w.computedStyles != nil && w.computedStyles.Placeholder.TextColor != nil {
				textColor = *w.computedStyles.Placeholder.TextColor
			} else if w.textBuffer != nil && w.textBuffer.PlaceholderColor() != 0 {
				textColor = w.textBuffer.PlaceholderColor()
			} else {
				textColor = 0x9CA3AFFF // gray-400 for placeholder
			}
		}

		// For TextArea, use word wrapping; for TextField, single line
		if w.kind == KindTextArea {
			// Calculate wrapped lines
			lines := WrapText(displayText, contentWidth, w.fontSize, "system")

			// Calculate content dimensions
			contentHeight := widgetHeight - w.padding[0] - w.padding[2]
			totalTextHeight := float32(len(lines)) * lineHeight

			// Find which row the cursor is on
			cursorRow, cursorColX := CursorPositionInWrappedText(lines, cursorPos, w.fontSize, "system")
			cursorRowTop := float32(cursorRow) * lineHeight
			cursorRowBottom := cursorRowTop + lineHeight

			// Only auto-scroll when cursor position has changed (not on manual scroll)
			if cursorPos != w.lastCursorPos {
				w.lastCursorPos = cursorPos
				// Auto-scroll to keep cursor visible
				if cursorRowBottom > w.scrollY+contentHeight {
					// Cursor is below visible area - scroll down
					w.scrollY = cursorRowBottom - contentHeight
				}
				if cursorRowTop < w.scrollY {
					// Cursor is above visible area - scroll up
					w.scrollY = cursorRowTop
				}
			}

			// Clamp scroll to valid range (always do this)
			maxScroll := totalTextHeight - contentHeight
			if maxScroll < 0 {
				maxScroll = 0
			}
			if w.scrollY > maxScroll {
				w.scrollY = maxScroll
			}
			if w.scrollY < 0 {
				w.scrollY = 0
			}

			// Begin scroll view for text content (handles clipping and scroll offset)
			commands = append(commands, ffi.BeginScrollView(
				textX, textY, contentWidth, contentHeight,
				0, w.scrollY, // Only vertical scrolling
			))

			// Draw selection background for each line
			if selStart != selEnd && !isPlaceholder {
				for lineIdx, line := range lines {
					lineY := textY + float32(lineIdx)*lineHeight

					// Check if selection overlaps with this line
					if selEnd > line.StartIndex && selStart < line.EndIndex {
						// Calculate selection bounds within this line
						lineSelStart := selStart - line.StartIndex
						if lineSelStart < 0 {
							lineSelStart = 0
						}
						lineSelEnd := selEnd - line.StartIndex
						if lineSelEnd > len([]rune(line.Text)) {
							lineSelEnd = len([]rune(line.Text))
						}

						selStartX := float32(0)
						if lineSelStart > 0 {
							selStartX = ffi.MeasureTextToCursor(line.Text, lineSelStart, "system", w.fontSize)
						}
						selEndX := ffi.MeasureTextToCursor(line.Text, lineSelEnd, "system", w.fontSize)

						commands = append(commands, ffi.RoundedRect(
							textX+selStartX, lineY,
							selEndX-selStartX, w.fontSize*1.2,
							0x3B82F640, // blue-500 with alpha
							0,
						))
					}
				}
			}

			// Draw each line of text
			for lineIdx, line := range lines {
				if line.Text != "" {
					lineY := textY + float32(lineIdx)*lineHeight
					commands = append(commands, textCommand(w,
						line.Text,
						textX, lineY,
						textColor,
					))
				}
			}

			// Draw cursor if focused
			cursorVisible := w.textBuffer != nil && w.textBuffer.CursorVisible()
			if w.focused && !isPlaceholder && cursorVisible {
				cursorY := textY + cursorRowTop
				commands = append(commands, ffi.RoundedRect(
					textX+cursorColX, cursorY,
					2, w.fontSize*1.2,
					w.textColor,
					1,
				))
			}

			// End scroll view
			commands = append(commands, ffi.EndScrollView())

			// Draw scroll bar if content overflows
			if totalTextHeight > contentHeight {
				scrollBarWidth := float32(8)
				scrollBarX := renderX + widgetWidth - scrollBarWidth // Flush with right border
				scrollBarTrackY := renderY + w.padding[0]
				scrollBarTrackHeight := widgetHeight - w.padding[0] - w.padding[2]

				// Calculate scroll bar thumb size and position
				viewportRatio := contentHeight / totalTextHeight
				thumbHeight := scrollBarTrackHeight * viewportRatio
				if thumbHeight < 20 {
					thumbHeight = 20 // Minimum thumb size
				}

				// Calculate thumb position based on scroll
				maxScroll := totalTextHeight - contentHeight
				scrollRatio := float32(0)
				if maxScroll > 0 {
					scrollRatio = w.scrollY / maxScroll
				}
				thumbY := scrollBarTrackY + (scrollBarTrackHeight-thumbHeight)*scrollRatio

				// Draw scroll bar track (subtle background)
				commands = append(commands, ffi.RoundedRect(
					scrollBarX, scrollBarTrackY,
					scrollBarWidth, scrollBarTrackHeight,
					0x00000020, // Very subtle track
					3,
				))

				// Draw scroll bar thumb
				commands = append(commands, ffi.RoundedRect(
					scrollBarX, thumbY,
					scrollBarWidth, thumbHeight,
					0x6B728080, // gray-500 with alpha
					3,
				))
			}
		} else {
			// TextField: single line rendering (original behavior)
			// Draw selection background if text is selected
			if selStart != selEnd && !isPlaceholder {
				selStartX := ffi.MeasureTextToCursor(displayText, selStart, "system", w.fontSize)
				selEndX := ffi.MeasureTextToCursor(displayText, selEnd, "system", w.fontSize)
				commands = append(commands, ffi.RoundedRect(
					textX+selStartX, textY,
					selEndX-selStartX, w.fontSize*1.2,
					0x3B82F640, // blue-500 with alpha
					0,
				))
			}

			// Draw text
			if displayText != "" {
				commands = append(commands, textCommand(w,
					displayText,
					textX, textY,
					textColor,
				))
			}

			// Draw cursor if focused and cursor is in visible blink state
			// Cursor should show even when text is empty (just not when showing placeholder)
			cursorVisible := w.textBuffer != nil && w.textBuffer.CursorVisible()
			if w.focused && cursorVisible {
				// Don't show cursor when displaying placeholder text
				actualText := ""
				if w.textBuffer != nil {
					actualText = w.textBuffer.Text()
				}
				if actualText != "" || !isPlaceholder {
					cursorX := float32(0)
					if displayText != "" && !isPlaceholder {
						cursorX = ffi.MeasureTextToCursor(displayText, cursorPos, "system", w.fontSize)
					}
					// Draw cursor line
					commands = append(commands, ffi.RoundedRect(
						textX+cursorX, textY,
						2, w.fontSize*1.2,
						w.textColor,
						1,
					))
				}
			}
		}
	}

	// Render control widgets
	switch w.kind {
	case KindCheckbox:
		commands = l.renderCheckbox(commands, w, renderX, renderY, widgetWidth, widgetHeight)

	case KindToggle:
		commands = l.renderToggle(commands, w, renderX, renderY, widgetWidth, widgetHeight)

	case KindRadio:
		commands = l.renderRadio(commands, w, renderX, renderY, widgetWidth, widgetHeight)

	case KindSlider:
		commands = l.renderSlider(commands, w, renderX, renderY, widgetWidth, widgetHeight)

	case KindSelect:
		commands = l.renderSelect(commands, w, renderX, renderY, widgetWidth, widgetHeight)
	}

	// Copy children and layout info before releasing lock
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	kind := w.kind
	padding := w.padding
	gap := w.gap
	width := w.width
	height := w.height
	flexDir := w.flexDirection
	justify := w.justifyContent
	alignIt := w.alignItems
	flexWr := w.flexWrap
	isScrollView := (w.kind == KindScrollView && w.scrollEnabled) ||
		w.overflowY == "scroll" || w.overflowY == "auto" ||
		w.overflowX == "scroll" || w.overflowX == "auto"
	isClipped := w.overflowX == "hidden" || w.overflowX == "clip" ||
		w.overflowY == "hidden" || w.overflowY == "clip"

	w.mu.RUnlock()

	// Layout and render children based on parent's flex properties
	commands = l.layoutAndRenderChildrenFlex(commands, children, kind, renderX, renderY, width, height, padding, gap, flexDir, justify, alignIt, flexWr, frameNum)

	releaseWidgetSlice(children)

	// End scroll view or pop clip after children
	if isScrollView {
		commands = append(commands, ffi.EndScrollView())
		// Pop scroll context
		if len(l.scrollContextStack) > 0 {
			l.scrollContextStack = l.scrollContextStack[:len(l.scrollContextStack)-1]
		}
	} else if isClipped {
		commands = append(commands, ffi.PopClip())
	}

	return commands
}

// layoutAndRenderChildren positions and renders children using flexbox layout.
// DEPRECATED: Use renderChildren when computed layout is available.
func (l *Loop) layoutAndRenderChildren(
	commands []ffi.RenderCommand,
	children []*Widget,
	parentKind WidgetKind,
	parentX, parentY, parentWidth, parentHeight float32,
	padding [4]float32,
	gap float32,
	frameNum uint64,
) []ffi.RenderCommand {
	return l.layoutAndRenderChildrenFlex(commands, children, parentKind, parentX, parentY,
		parentWidth, parentHeight, padding, gap, FlexRow, JustifyStart, AlignStart, FlexNoWrap, frameNum)
}

// layoutAndRenderChildrenFlex renders children that have been pre-laid-out.
// When computed layout is valid, children already have their final positions.
// Falls back to on-the-fly layout calculation for backwards compatibility.
func (l *Loop) layoutAndRenderChildrenFlex(
	commands []ffi.RenderCommand,
	children []*Widget,
	parentKind WidgetKind,
	parentX, parentY, parentWidth, parentHeight float32,
	padding [4]float32,
	gap float32,
	flexDir FlexDirection,
	justify JustifyContent,
	align AlignItems,
	wrap FlexWrap,
	frameNum uint64,
) []ffi.RenderCommand {
	if len(children) == 0 {
		return commands
	}

	// Check if first child has computed layout - if so, all children should have it
	// This means layout pass already ran and we just need to render
	if len(children) > 0 {
		children[0].mu.RLock()
		hasComputedLayout := children[0].computedLayout.Valid
		children[0].mu.RUnlock()

		if hasComputedLayout {
			// Fast path: just render each child at its pre-computed position
			// Render sticky children last so they appear on top of scrolled content
			var stickyChildren []*Widget
			for _, child := range children {
				child.mu.RLock()
				isSticky := child.position == PositionSticky
				child.mu.RUnlock()
				if isSticky {
					stickyChildren = append(stickyChildren, child)
				} else {
					commands = l.renderWidgetAt(commands, child, 0, 0, frameNum)
				}
			}
			// Render sticky children on top
			for _, child := range stickyChildren {
				commands = l.renderWidgetAt(commands, child, 0, 0, frameNum)
			}
			return commands
		}
	}

	// Legacy path: compute layout on-the-fly (for backwards compatibility)
	// This code path is used when ComputeLayout hasn't been called

	// Content area after padding
	contentX := parentX + padding[3] // left padding
	contentY := parentY + padding[0] // top padding
	contentWidth := parentWidth - padding[1] - padding[3]
	contentHeight := parentHeight - padding[0] - padding[2]

	// Collect child info for layout calculations
	type childInfo struct {
		widget     *Widget
		width      float32
		height     float32
		flexGrow   float32
		flexShrink float32
		position   Position
		relOffsetX float32
		relOffsetY float32
	}

	// Separate absolute/fixed children from flow children
	var flowChildren []childInfo
	var absChildren []*Widget

	for _, child := range children {
		child.mu.RLock()
		pos := child.position
		w := child.width
		h := child.height
		grow := child.flexGrow
		shrink := child.flexShrink
		kind := child.kind
		fontSize := child.fontSize
		lineHeight := child.lineHeight
		relX := child.x
		relY := child.y
		child.mu.RUnlock()

		// Auto-size text widgets
		if (kind == KindText || kind == KindButton) && h == 0 && fontSize > 0 {
			multiplier := lineHeight
			if multiplier == 0 {
				multiplier = 1.0
			}
			h = fontSize * multiplier
		}

		if pos == PositionAbsolute || pos == PositionFixed {
			absChildren = append(absChildren, child)
		} else {
			flowChildren = append(flowChildren, childInfo{
				widget:     child,
				width:      w,
				height:     h,
				flexGrow:   grow,
				flexShrink: shrink,
				position:   pos,
				relOffsetX: relX,
				relOffsetY: relY,
			})
		}
	}

	// Determine main axis based on flex direction or parent kind
	isMainAxisHorizontal := flexDir == FlexRow || flexDir == FlexRowReverse

	// Override based on parent kind if no explicit flex direction
	// VStack = column, HStack = row, ZStack = special case
	if parentKind == KindVStack {
		isMainAxisHorizontal = false
		flexDir = FlexColumn
	} else if parentKind == KindHStack {
		isMainAxisHorizontal = true
		flexDir = FlexRow
	}

	// For ZStack, render all children at the same position
	if parentKind == KindZStack {
		for _, info := range flowChildren {
			childX := contentX
			childY := contentY
			if info.position == PositionRelative {
				childX += info.relOffsetX
				childY += info.relOffsetY
			}
			commands = l.renderWidgetAt(commands, info.widget, childX, childY, frameNum)
		}
		// Render absolute children
		for _, child := range absChildren {
			commands = l.renderWidgetAt(commands, child, 0, 0, frameNum)
		}
		return commands
	}

	// Calculate total main axis size of children and total gaps
	var totalMainSize float32
	var totalGaps float32
	if len(flowChildren) > 1 {
		totalGaps = gap * float32(len(flowChildren)-1)
	}

	for _, info := range flowChildren {
		if isMainAxisHorizontal {
			totalMainSize += info.width
		} else {
			totalMainSize += info.height
		}
	}

	// Available space in main axis
	var mainAxisSize float32
	if isMainAxisHorizontal {
		mainAxisSize = contentWidth
	} else {
		mainAxisSize = contentHeight
	}

	// Cross axis size
	var crossAxisSize float32
	if isMainAxisHorizontal {
		crossAxisSize = contentHeight
	} else {
		crossAxisSize = contentWidth
	}

	// Calculate free space (can be negative if content overflows)
	freeSpace := mainAxisSize - totalMainSize - totalGaps

	// Distribute free space based on flex-grow/shrink
	var totalGrow float32
	var totalShrink float32
	for _, info := range flowChildren {
		totalGrow += info.flexGrow
		totalShrink += info.flexShrink
	}

	// Calculate final sizes after flex distribution
	finalSizes := make([]float32, len(flowChildren))
	for i, info := range flowChildren {
		baseSize := info.width
		if !isMainAxisHorizontal {
			baseSize = info.height
		}
		finalSizes[i] = baseSize

		if freeSpace > 0 && totalGrow > 0 && info.flexGrow > 0 {
			// Distribute positive space proportionally to flex-grow
			finalSizes[i] += freeSpace * (info.flexGrow / totalGrow)
		} else if freeSpace < 0 && totalShrink > 0 && info.flexShrink > 0 {
			// Shrink proportionally to flex-shrink
			finalSizes[i] += freeSpace * (info.flexShrink / totalShrink)
		}
	}

	// Calculate actual total after flex adjustments
	var actualTotal float32
	for _, size := range finalSizes {
		actualTotal += size
	}

	// Calculate starting position based on justify-content
	var mainStart float32
	var spaceBetween float32
	remainingSpace := mainAxisSize - actualTotal - totalGaps

	switch justify {
	case JustifyStart:
		mainStart = 0
		spaceBetween = 0
	case JustifyEnd:
		mainStart = remainingSpace
		spaceBetween = 0
	case JustifyCenter:
		mainStart = remainingSpace / 2
		spaceBetween = 0
	case JustifyBetween:
		mainStart = 0
		if len(flowChildren) > 1 {
			spaceBetween = remainingSpace / float32(len(flowChildren)-1)
		}
	case JustifyAround:
		if len(flowChildren) > 0 {
			spaceBetween = remainingSpace / float32(len(flowChildren))
			mainStart = spaceBetween / 2
		}
	case JustifyEvenly:
		if len(flowChildren) > 0 {
			spaceBetween = remainingSpace / float32(len(flowChildren)+1)
			mainStart = spaceBetween
		}
	}

	// Reverse direction if needed
	isReversed := flexDir == FlexRowReverse || flexDir == FlexColumnReverse
	if isReversed {
		// Reverse the order of children for layout
		for i, j := 0, len(flowChildren)-1; i < j; i, j = i+1, j-1 {
			flowChildren[i], flowChildren[j] = flowChildren[j], flowChildren[i]
			finalSizes[i], finalSizes[j] = finalSizes[j], finalSizes[i]
		}
	}

	// Position and render each child
	cursor := mainStart
	for i, info := range flowChildren {
		var childX, childY float32
		childMainSize := finalSizes[i]
		childCrossSize := info.height
		if !isMainAxisHorizontal {
			childCrossSize = info.width
		}

		// Calculate cross axis position based on align-items
		var crossPos float32
		switch align {
		case AlignStart:
			crossPos = 0
		case AlignEnd:
			crossPos = crossAxisSize - childCrossSize
		case AlignCenter:
			crossPos = (crossAxisSize - childCrossSize) / 2
		case AlignStretch:
			crossPos = 0
			// For stretch, we'd ideally modify the child's cross-axis size
			// but we're not mutating widgets during layout here
		case AlignBaseline:
			// Baseline alignment requires text metrics - default to start
			crossPos = 0
		}

		// Set position based on axis orientation
		if isMainAxisHorizontal {
			childX = contentX + cursor
			childY = contentY + crossPos
		} else {
			childX = contentX + crossPos
			childY = contentY + cursor
		}

		// Apply relative offset
		if info.position == PositionRelative {
			childX += info.relOffsetX
			childY += info.relOffsetY
		}

		commands = l.renderWidgetAt(commands, info.widget, childX, childY, frameNum)

		// Move cursor for next child
		cursor += childMainSize + gap
		if justify == JustifyBetween || justify == JustifyAround || justify == JustifyEvenly {
			cursor += spaceBetween
		}
	}

	// Render absolute/fixed positioned children
	for _, child := range absChildren {
		commands = l.renderWidgetAt(commands, child, 0, 0, frameNum)
	}

	return commands
}

// Pause pauses the game loop (stops calling onFrame).
func (l *Loop) Pause() {
	l.paused.Store(true)
}

// Resume resumes a paused game loop.
func (l *Loop) Resume() {
	l.paused.Store(false)
}

// IsPaused returns whether the loop is paused.
func (l *Loop) IsPaused() bool {
	return l.paused.Load()
}

// IsRunning returns whether the loop is running.
func (l *Loop) IsRunning() bool {
	return l.running.Load()
}

// Stats returns loop statistics.
func (l *Loop) Stats() LoopStats {
	return LoopStats{
		FrameCount:    l.frameCount.Load(),
		DroppedFrames: l.droppedFrames.Load(),
		TargetFPS:     l.config.TargetFPS,
	}
}

// LoopStats contains performance metrics.
type LoopStats struct {
	FrameCount    uint64
	DroppedFrames uint64
	TargetFPS     int
}

// InitAnimations starts all pending animations from animate-* classes in the widget tree.
// Call this after setting the root widget to automatically start class-based animations.
// This should be called once after the tree is set up.
func (l *Loop) InitAnimations() {
	root := l.tree.Root()
	if root == nil {
		return
	}
	l.initWidgetAnimations(root)
}

// initWidgetAnimations recursively starts pending animations for a widget and its children.
func (l *Loop) initWidgetAnimations(w *Widget) {
	w.mu.Lock()
	pending := w.pendingAnimation
	hasActive := w.activeAnimation != nil
	// Capture animation config for customization
	cfg := AnimationConfig{
		Duration:   w.pendingAnimDuration,
		Easing:     w.pendingAnimEasing,
		Iterations: w.pendingAnimIterations,
	}
	w.mu.Unlock()

	// Start pending animation if there's no active animation
	if pending != "" && !hasActive {
		anim := StartPredefinedAnimationWithConfig(w, l.animations, pending, cfg)
		if anim != nil {
			w.mu.Lock()
			w.activeAnimation = anim
			w.pendingAnimation = "" // Clear pending since it's now active
			w.mu.Unlock()
		}
	}

	// Process children
	w.mu.RLock()
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()

	for _, child := range children {
		l.initWidgetAnimations(child)
	}

	releaseWidgetSlice(children)
}

// ============================================================================
// Image Widget Rendering
// ============================================================================

// calculateObjectFit computes the image position and size based on object-fit mode.
// Returns (x, y, width, height) for the image within the container bounds.
func calculateObjectFit(fit, position string, containerX, containerY, containerW, containerH, imageW, imageH float32) (float32, float32, float32, float32) {
	// Calculate aspect ratios
	containerAspect := containerW / containerH
	imageAspect := imageW / imageH

	var scaledW, scaledH float32

	switch fit {
	case "contain":
		// Scale image to fit entirely within container, maintaining aspect ratio
		if imageAspect > containerAspect {
			scaledW = containerW
			scaledH = containerW / imageAspect
		} else {
			scaledH = containerH
			scaledW = containerH * imageAspect
		}

	case "cover":
		// Scale image to cover entire container, maintaining aspect ratio
		if imageAspect > containerAspect {
			scaledH = containerH
			scaledW = containerH * imageAspect
		} else {
			scaledW = containerW
			scaledH = containerW / imageAspect
		}

	case "none":
		// Display at natural size
		scaledW = imageW
		scaledH = imageH

	case "scale-down":
		// Like "contain", but never scale up beyond natural size
		if imageW <= containerW && imageH <= containerH {
			scaledW = imageW
			scaledH = imageH
		} else {
			if imageAspect > containerAspect {
				scaledW = containerW
				scaledH = containerW / imageAspect
			} else {
				scaledH = containerH
				scaledW = containerH * imageAspect
			}
		}

	case "fill":
		fallthrough
	default:
		// Stretch to fill container bounds
		return containerX, containerY, containerW, containerH
	}

	// Apply object-position
	x, y := applyObjectPosition(position, containerX, containerY, containerW, containerH, scaledW, scaledH)
	return x, y, scaledW, scaledH
}

// applyObjectPosition positions the image within the container based on object-position.
func applyObjectPosition(position string, containerX, containerY, containerW, containerH, imageW, imageH float32) (float32, float32) {
	// Calculate available space
	spaceX := containerW - imageW
	spaceY := containerH - imageH

	// Parse horizontal and vertical alignment from position
	var hAlign, vAlign float32 = 0.5, 0.5 // default: center

	switch position {
	case "center", "":
		hAlign, vAlign = 0.5, 0.5
	case "top":
		hAlign, vAlign = 0.5, 0
	case "bottom":
		hAlign, vAlign = 0.5, 1
	case "left":
		hAlign, vAlign = 0, 0.5
	case "right":
		hAlign, vAlign = 1, 0.5
	case "left-top", "top-left":
		hAlign, vAlign = 0, 0
	case "right-top", "top-right":
		hAlign, vAlign = 1, 0
	case "left-bottom", "bottom-left":
		hAlign, vAlign = 0, 1
	case "right-bottom", "bottom-right":
		hAlign, vAlign = 1, 1
	}

	x := containerX + spaceX*hAlign
	y := containerY + spaceY*vAlign
	return x, y
}

// renderImage renders an image widget, loading the image if needed.
func (l *Loop) renderImage(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Read image state
	w.mu.RLock()
	textureID := w.imageTextureID
	source := w.imageSource
	loading := w.imageLoading
	sourceX := w.imageSourceX
	sourceY := w.imageSourceY
	sourceW := w.imageSourceW
	sourceH := w.imageSourceH
	fit := w.imageFit
	position := w.imagePosition
	imgErr := w.imageError
	cornerRadii := w.cornerRadius
	naturalW := w.imageNaturalW
	naturalH := w.imageNaturalH
	w.mu.RUnlock()

	// If no texture and we have a source, trigger async loading
	if textureID == 0 && source != "" && !loading {
		// Start async loading (both URLs and files are loaded asynchronously
		// to avoid blocking the render thread)
		l.loadImageAsync(w, source)
	}

	// If no texture yet, show placeholder
	if textureID == 0 {
		if loading || (source != "" && imgErr == nil) {
			// Show loading indicator (gray box)
			commands = append(commands, ffi.RenderCommand{
				DrawRect: &ffi.DrawRectCmd{
					X:           x,
					Y:           y,
					Width:       width,
					Height:      height,
					Color:       0x374151FF, // gray-700
					CornerRadii: cornerRadii,
				},
			})
		} else if imgErr != nil {
			// Show error state (red-tinted box)
			commands = append(commands, ffi.RenderCommand{
				DrawRect: &ffi.DrawRectCmd{
					X:           x,
					Y:           y,
					Width:       width,
					Height:      height,
					Color:       0x7F1D1DFF, // red-900
					CornerRadii: cornerRadii,
				},
			})
		}
		return commands
	}

	// Calculate image dimensions based on fit mode
	imgX, imgY, imgW, imgH := x, y, width, height

	// Apply object-fit logic using natural image dimensions
	needsClip := false
	if naturalW > 0 && naturalH > 0 {
		imgX, imgY, imgW, imgH = calculateObjectFit(fit, position, x, y, width, height, float32(naturalW), float32(naturalH))
		// Check if image extends outside container bounds (cover, none, scale-down with large image)
		needsClip = imgX < x || imgY < y || imgX+imgW > x+width || imgY+imgH > y+height
	}
	// If natural dimensions unknown, default to "fill" behavior (stretch to bounds)

	// Build source rect if specified (for sprite sheets)
	var sourceRect *[4]float32
	if sourceW > 0 && sourceH > 0 {
		sourceRect = &[4]float32{sourceX, sourceY, sourceW, sourceH}
	}

	// Check if we have rounded corners
	hasRounded := cornerRadii[0] > 0 || cornerRadii[1] > 0 || cornerRadii[2] > 0 || cornerRadii[3] > 0

	// Push clip if image extends outside container
	if needsClip {
		commands = append(commands, ffi.PushClip(x, y, width, height))
	}

	// Draw the image
	if sourceRect != nil {
		if hasRounded {
			commands = append(commands, ffi.ImageWithSourceRectAndCornerRadii(
				ffi.TextureID(textureID),
				imgX, imgY, imgW, imgH,
				*sourceRect,
				cornerRadii,
			))
		} else {
			commands = append(commands, ffi.ImageWithSourceRect(
				ffi.TextureID(textureID),
				imgX, imgY, imgW, imgH,
				*sourceRect,
			))
		}
	} else {
		if hasRounded {
			commands = append(commands, ffi.ImageWithCornerRadii(
				ffi.TextureID(textureID),
				imgX, imgY, imgW, imgH,
				cornerRadii,
			))
		} else {
			commands = append(commands, ffi.Image(
				ffi.TextureID(textureID),
				imgX, imgY, imgW, imgH,
			))
		}
	}

	// Pop clip if we pushed one
	if needsClip {
		commands = append(commands, ffi.PopClip())
	}

	return commands
}

// renderVideo renders a video widget, handling player creation, loading, and frame updates.
func (l *Loop) renderVideo(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Read video state
	w.mu.RLock()
	playerID := w.videoPlayerID
	textureID := w.videoTextureID
	source := w.videoSource
	loading := w.videoLoading
	autoplay := w.videoAutoplay
	loop := w.videoLoop
	muted := w.videoMuted
	volume := w.videoVolume
	videoErr := w.videoError
	cornerRadii := w.cornerRadius
	naturalW := w.videoNaturalW
	naturalH := w.videoNaturalH
	state := w.videoState
	onEnded := w.onVideoEnded
	imageFit := w.imageFit
	imagePosition := w.imagePosition
	w.mu.RUnlock()

	// If no player and we have a source, create player and load video
	if playerID == 0 && source != "" && !loading {
		l.loadVideoAsync(w, source, autoplay, loop, muted, volume)
	}

	// If player exists, update it to get new frames
	if playerID != 0 {
		// Update player (decodes frames as needed)
		newTextureID := ffi.VideoUpdate(ffi.VideoPlayerID(playerID))
		if newTextureID != 0 {
			w.mu.Lock()
			w.videoTextureID = uint32(newTextureID)
			textureID = uint32(newTextureID)
			w.mu.Unlock()
		}

		// Check playback state
		newState := ffi.VideoGetState(ffi.VideoPlayerID(playerID))
		if int32(newState) != state {
			w.mu.Lock()
			w.videoState = int32(newState)
			w.mu.Unlock()

			// Handle video ended callback
			if newState == ffi.VideoStateEnded && onEnded != nil {
				onEnded()
			}
		}

		// Update natural dimensions if not yet available (web: metadata loads async)
		if naturalW == 0 || naturalH == 0 {
			info, err := ffi.VideoGetInfo(ffi.VideoPlayerID(playerID))
			if err == nil && info != nil && info.Width > 0 && info.Height > 0 {
				w.mu.Lock()
				w.videoNaturalW = info.Width
				w.videoNaturalH = info.Height
				naturalW = info.Width
				naturalH = info.Height
				if info.DurationMs > 0 {
					w.videoDurationMs = info.DurationMs
				}
				w.mu.Unlock()
			}
		}
	}

	// If no texture yet, show placeholder
	if textureID == 0 {
		if loading || (source != "" && videoErr == nil) {
			// Show loading indicator (dark gray box)
			commands = append(commands, ffi.RenderCommand{
				DrawRect: &ffi.DrawRectCmd{
					X:           x,
					Y:           y,
					Width:       width,
					Height:      height,
					Color:       0x1F2937FF, // gray-800
					CornerRadii: cornerRadii,
				},
			})
		} else if videoErr != nil {
			// Show error state (red-tinted box)
			commands = append(commands, ffi.RenderCommand{
				DrawRect: &ffi.DrawRectCmd{
					X:           x,
					Y:           y,
					Width:       width,
					Height:      height,
					Color:       0x7F1D1DFF, // red-900
					CornerRadii: cornerRadii,
				},
			})
		}
		return commands
	}

	// Calculate video dimensions using object-fit (default to "contain" for video)
	imgX, imgY, imgW, imgH := x, y, width, height

	// Apply object-fit logic using natural video dimensions
	needsClip := false
	if naturalW > 0 && naturalH > 0 {
		fit := imageFit
		if fit == "" {
			fit = "contain" // Default for video
		}
		position := imagePosition
		if position == "" {
			position = "center"
		}
		imgX, imgY, imgW, imgH = calculateObjectFit(fit, position, x, y, width, height, float32(naturalW), float32(naturalH))
		needsClip = imgX < x || imgY < y || imgX+imgW > x+width || imgY+imgH > y+height
	}

	// Check if we have rounded corners
	hasRounded := cornerRadii[0] > 0 || cornerRadii[1] > 0 || cornerRadii[2] > 0 || cornerRadii[3] > 0

	// Push clip if video extends outside container
	if needsClip {
		commands = append(commands, ffi.PushClip(x, y, width, height))
	}

	// Draw the video frame as an image
	if hasRounded {
		commands = append(commands, ffi.ImageWithCornerRadii(
			ffi.TextureID(textureID),
			imgX, imgY, imgW, imgH,
			cornerRadii,
		))
	} else {
		commands = append(commands, ffi.Image(
			ffi.TextureID(textureID),
			imgX, imgY, imgW, imgH,
		))
	}

	// Pop clip if we pushed one
	if needsClip {
		commands = append(commands, ffi.PopClip())
	}

	return commands
}

// pendingVideoLoads tracks which videos are being loaded to prevent duplicate loads
var pendingVideoLoads sync.Map

// loadVideoAsync creates a video player and loads the video asynchronously.
func (l *Loop) loadVideoAsync(w *Widget, source string, autoplay, loop, muted bool, volume float32) {
	// Check if already loading this source for this widget
	widgetID := w.ID()
	loadKey := fmt.Sprintf("video:%d:%s", widgetID, source)

	// Prevent duplicate load requests
	if _, alreadyLoading := pendingVideoLoads.LoadOrStore(loadKey, true); alreadyLoading {
		return
	}

	w.mu.Lock()
	w.videoLoading = true
	w.mu.Unlock()

	// Create player and load video (this can be done on main thread - the actual
	// decoding happens asynchronously in the Rust layer)
	go func() {
		defer pendingVideoLoads.Delete(loadKey)

		// Create video player
		playerID := ffi.VideoCreate()
		if playerID == 0 {
			w.mu.Lock()
			w.videoLoading = false
			w.videoError = fmt.Errorf("failed to create video player")
			w.mu.Unlock()
			if w.onVideoError != nil {
				w.onVideoError(w.videoError)
			}
			return
		}

		// Load video from URL or file
		var err error
		if isURL(source) {
			err = ffi.VideoLoadURL(playerID, source)
		} else {
			err = ffi.VideoLoadFile(playerID, source)
		}

		if err != nil {
			ffi.VideoDestroy(playerID)
			w.mu.Lock()
			w.videoLoading = false
			w.videoError = err
			w.mu.Unlock()
			if w.onVideoError != nil {
				w.onVideoError(err)
			}
			return
		}

		// Configure player
		_ = ffi.VideoSetLooping(playerID, loop)
		_ = ffi.VideoSetMuted(playerID, muted)
		_ = ffi.VideoSetVolume(playerID, volume)

		// Get video info
		info, err := ffi.VideoGetInfo(playerID)
		if err == nil && info != nil {
			w.mu.Lock()
			w.videoNaturalW = info.Width
			w.videoNaturalH = info.Height
			w.videoDurationMs = info.DurationMs
			w.mu.Unlock()
		}

		// Store player ID and mark as loaded
		w.mu.Lock()
		w.videoPlayerID = uint32(playerID)
		w.videoLoading = false
		w.videoState = int32(ffi.VideoStatePaused)
		w.mu.Unlock()

		// Start playback if autoplay is enabled
		if autoplay {
			_ = ffi.VideoPlay(playerID)
			w.mu.Lock()
			w.videoState = int32(ffi.VideoStatePlaying)
			w.mu.Unlock()
		}
	}()
}

// isURL checks if a string is a URL (starts with http:// or https://)
func isURL(s string) bool {
	return len(s) > 7 && (s[:7] == "http://" || (len(s) > 8 && s[:8] == "https://"))
}

// ============================================================================
// Audio Support
// ============================================================================

// pendingAudioLoads tracks which audio files are being loaded to prevent duplicate loads
var pendingAudioLoads sync.Map

// updateAudio handles audio widget loading and state updates.
// Audio widgets have no visual representation - they just manage playback.
func (l *Loop) updateAudio(w *Widget) {
	// Read audio state
	w.mu.RLock()
	playerID := w.audioPlayerID
	source := w.audioSource
	loading := w.audioLoading
	autoplay := w.audioAutoplay
	loop := w.audioLoop
	volume := w.audioVolume
	w.mu.RUnlock()

	// If no player and we have a source, create player and load audio
	if playerID == 0 && source != "" && !loading {
		l.loadAudioAsync(w, source, autoplay, loop, volume)
		return
	}

	// If player exists, update it to sync state
	if playerID != 0 {
		// Update audio player state
		stateChanged := ffi.AudioUpdate(ffi.AudioPlayerID(playerID))

		// Get current state
		newState := ffi.AudioGetState(ffi.AudioPlayerID(playerID))

		w.mu.Lock()
		oldState := w.audioState
		w.audioState = int32(newState)
		w.mu.Unlock()

		// Handle state transitions
		if stateChanged || oldState != int32(newState) {
			// Check for ended state
			if newState == ffi.AudioStateEnded {
				w.mu.RLock()
				onEnded := w.onAudioEnded
				w.mu.RUnlock()
				if onEnded != nil {
					onEnded()
				}
			}

			// Check for error state
			if newState == ffi.AudioStateError {
				w.mu.RLock()
				onError := w.onAudioError
				w.mu.RUnlock()
				if onError != nil {
					onError(fmt.Errorf("audio playback error"))
				}
			}
		}

		// Call time update callback if registered
		w.mu.RLock()
		onTimeUpdate := w.onAudioTimeUpdate
		durationMs := w.audioDurationMs
		w.mu.RUnlock()

		if onTimeUpdate != nil && newState == ffi.AudioStatePlaying {
			currentMs := ffi.AudioGetTime(ffi.AudioPlayerID(playerID))
			onTimeUpdate(currentMs, durationMs)
		}
	}
}

// loadAudioAsync creates an audio player and loads the audio asynchronously.
func (l *Loop) loadAudioAsync(w *Widget, source string, autoplay, loop bool, volume float32) {
	// Check if already loading this source for this widget
	widgetID := w.ID()
	loadKey := fmt.Sprintf("audio:%d:%s", widgetID, source)

	// Prevent duplicate load requests
	if _, alreadyLoading := pendingAudioLoads.LoadOrStore(loadKey, true); alreadyLoading {
		return
	}

	w.mu.Lock()
	w.audioLoading = true
	w.mu.Unlock()

	// Create player and load audio
	go func() {
		defer pendingAudioLoads.Delete(loadKey)

		// Create audio player
		playerID := ffi.AudioCreate()
		if playerID == 0 {
			w.mu.Lock()
			w.audioLoading = false
			w.audioError = fmt.Errorf("failed to create audio player")
			w.mu.Unlock()
			if w.onAudioError != nil {
				w.onAudioError(w.audioError)
			}
			return
		}

		// Load audio from URL or file
		var err error
		if isURL(source) {
			err = ffi.AudioLoadURL(playerID, source)
		} else {
			err = ffi.AudioLoadFile(playerID, source)
		}

		if err != nil {
			ffi.AudioDestroy(playerID)
			w.mu.Lock()
			w.audioLoading = false
			w.audioError = err
			w.mu.Unlock()
			if w.onAudioError != nil {
				w.onAudioError(err)
			}
			return
		}

		// Configure player
		_ = ffi.AudioSetLooping(playerID, loop)
		_ = ffi.AudioSetVolume(playerID, volume)

		// Get audio info
		info, err := ffi.AudioGetInfo(playerID)
		if err == nil && info != nil {
			w.mu.Lock()
			w.audioDurationMs = info.DurationMs
			w.audioSampleRate = info.SampleRate
			w.audioChannels = info.Channels
			w.mu.Unlock()
		}

		// Store player ID and mark as loaded
		w.mu.Lock()
		w.audioPlayerID = uint32(playerID)
		w.audioLoading = false
		w.audioState = int32(ffi.AudioStatePaused)
		w.mu.Unlock()

		// Start playback if autoplay is enabled
		if autoplay {
			_ = ffi.AudioPlay(playerID)
			w.mu.Lock()
			w.audioState = int32(ffi.AudioStatePlaying)
			w.mu.Unlock()
		}
	}()
}

// ============================================================================
// Microphone (Audio Input) Support
// ============================================================================

// updateMicrophone handles microphone widget initialization and state updates.
// Microphone widgets have no visual representation - they capture audio
// and provide level data for visualization.
func (l *Loop) updateMicrophone(w *Widget) {
	// Read microphone state
	w.mu.RLock()
	inputID := w.micInputID
	deviceID := w.micDeviceID
	sampleRate := w.micSampleRate
	channels := w.micChannels
	autoStart := w.micAutoStart
	w.mu.RUnlock()

	// If no input created yet, create and initialize it
	if inputID == 0 {
		l.initializeMicrophone(w, deviceID, sampleRate, channels, autoStart)
		return
	}

	// Update microphone state
	newState := ffi.AudioInputGetState(ffi.AudioInputID(inputID))
	newLevel := ffi.AudioInputGetLevel(ffi.AudioInputID(inputID))

	w.mu.Lock()
	oldState := w.micState
	oldLevel := w.micLevel
	w.micState = int32(newState)
	w.micLevel = newLevel
	onStateChange := w.onMicStateChange
	onLevelChange := w.onMicLevelChange
	w.mu.Unlock()

	// Fire callbacks if state changed
	if oldState != int32(newState) && onStateChange != nil {
		onStateChange(int32(newState))
	}

	// Fire level callback if level changed significantly (avoid noise)
	if onLevelChange != nil && abs32(newLevel-oldLevel) > 0.01 {
		onLevelChange(newLevel)
	}
}

// initializeMicrophone creates and initializes a microphone input.
func (l *Loop) initializeMicrophone(w *Widget, deviceID string, sampleRate, channels uint32, autoStart bool) {
	// Create input
	inputID := ffi.AudioInputCreate()
	if inputID == 0 {
		w.mu.Lock()
		w.micState = int32(ffi.AudioInputStateError)
		onError := w.onMicError
		w.mu.Unlock()
		if onError != nil {
			onError(fmt.Errorf("failed to create audio input"))
		}
		return
	}

	// Store input ID
	w.mu.Lock()
	w.micInputID = uint32(inputID)
	w.micState = int32(ffi.AudioInputStateIdle)
	w.mu.Unlock()

	// Request permission
	err := ffi.AudioInputRequestPermission(inputID)
	if err != nil {
		w.mu.Lock()
		w.micState = int32(ffi.AudioInputStateRequestingPermission)
		onError := w.onMicError
		w.mu.Unlock()
		if onError != nil {
			onError(err)
		}
		return
	}

	// Open the device
	err = ffi.AudioInputOpen(inputID, deviceID, sampleRate, channels)
	if err != nil {
		w.mu.Lock()
		w.micState = int32(ffi.AudioInputStateError)
		onError := w.onMicError
		w.mu.Unlock()
		if onError != nil {
			onError(err)
		}
		return
	}

	w.mu.Lock()
	w.micState = int32(ffi.AudioInputStateReady)
	w.mu.Unlock()

	// Auto-start if requested
	if autoStart {
		err = ffi.AudioInputStart(inputID)
		if err == nil {
			w.mu.Lock()
			w.micState = int32(ffi.AudioInputStateCapturing)
			w.mu.Unlock()
		} else {
			w.mu.RLock()
			onError := w.onMicError
			w.mu.RUnlock()
			if onError != nil {
				onError(err)
			}
		}
	}
}

// ============================================================================
// Camera (Video Input) Support
// ============================================================================

// updateCamera handles camera widget initialization and state updates.
// Camera widgets are non-rendering data sources - they capture video and provide
// texture IDs via OnFrame callback for display elsewhere (e.g., in a Video widget).
func (l *Loop) updateCamera(w *Widget) {
	// Read camera state
	w.mu.RLock()
	inputID := w.camInputID
	deviceID := w.camDeviceID
	prefWidth := w.camWidth
	prefHeight := w.camHeight
	frameRate := w.camFrameRate
	autoStart := w.camAutoStart
	w.mu.RUnlock()

	// If no input created yet, create and initialize it
	if inputID == 0 {
		l.initializeCamera(w, deviceID, prefWidth, prefHeight, frameRate, autoStart)
		return
	}

	// Update camera state
	newState := ffi.VideoInputGetState(ffi.VideoInputID(inputID))

	w.mu.Lock()
	oldState := w.camState
	w.camState = int32(newState)
	onStateChange := w.onCamStateChange
	w.mu.Unlock()

	// Fire callback if state changed
	if oldState != int32(newState) && onStateChange != nil {
		onStateChange(int32(newState))
	}

	// Update dimensions if available
	if width, height, err := ffi.VideoInputGetDimensions(ffi.VideoInputID(inputID)); err == nil {
		w.mu.Lock()
		w.camActualWidth = width
		w.camActualHeight = height
		w.mu.Unlock()
	}

	// Get latest camera frame and upload to GPU texture when capturing
	if newState == ffi.VideoInputStateCapturing {
		// Note: We pass 0 for existing_texture_id because the old texture may still
		// be referenced by render commands generated earlier in this frame.
		// We handle cleanup of old textures after rendering is complete.
		newTextureID, err := ffi.VideoInputGetFrameTexture(ffi.VideoInputID(inputID), 0)
		if err == nil && newTextureID != 0 {
			w.mu.Lock()
			oldTextureID := w.camTextureID
			w.camTextureID = newTextureID
			onFrame := w.onCamFrame
			w.mu.Unlock()

			// Queue old texture for deferred unload (after frame is rendered)
			if oldTextureID != 0 && oldTextureID != newTextureID {
				l.pendingTextureUnloads = append(l.pendingTextureUnloads, oldTextureID)
			}

			// Fire frame callback if texture changed (new frame available)
			if newTextureID != oldTextureID && onFrame != nil {
				onFrame(newTextureID)
			}
		}
	}
}

// renderCamera renders a camera widget's preview frame.
func (l *Loop) renderCamera(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	w.mu.RLock()
	textureID := w.camTextureID
	cornerRadii := w.cornerRadius
	state := w.camState
	actualW := w.camActualWidth
	actualH := w.camActualHeight
	imageFit := w.imageFit
	imagePosition := w.imagePosition
	w.mu.RUnlock()

	// If no texture yet, show placeholder
	if textureID == 0 {
		if state == int32(ffi.VideoInputStateCapturing) || state == int32(ffi.VideoInputStateRequestingPermission) {
			// Show loading indicator (dark gray box)
			commands = append(commands, ffi.RenderCommand{
				DrawRect: &ffi.DrawRectCmd{
					X:           x,
					Y:           y,
					Width:       width,
					Height:      height,
					Color:       0x374151FF, // gray-700
					CornerRadii: cornerRadii,
				},
			})
		} else {
			// Show idle state (darker gray box)
			commands = append(commands, ffi.RenderCommand{
				DrawRect: &ffi.DrawRectCmd{
					X:           x,
					Y:           y,
					Width:       width,
					Height:      height,
					Color:       0x1F2937FF, // gray-800
					CornerRadii: cornerRadii,
				},
			})
		}
		return commands
	}

	// Calculate camera frame dimensions using object-fit
	imgX, imgY, imgW, imgH := x, y, width, height

	needsClip := false
	if actualW > 0 && actualH > 0 {
		fit := imageFit
		if fit == "" {
			fit = "cover" // Default for camera (fill the container)
		}
		position := imagePosition
		if position == "" {
			position = "center"
		}
		imgX, imgY, imgW, imgH = calculateObjectFit(fit, position, x, y, width, height, float32(actualW), float32(actualH))
		needsClip = imgX < x || imgY < y || imgX+imgW > x+width || imgY+imgH > y+height
	}

	// Check if we have rounded corners
	hasRounded := cornerRadii[0] > 0 || cornerRadii[1] > 0 || cornerRadii[2] > 0 || cornerRadii[3] > 0

	// Push clip if camera frame extends outside container
	if needsClip || hasRounded {
		commands = append(commands, ffi.PushClip(x, y, width, height))
	}

	// Draw the camera frame
	if hasRounded {
		commands = append(commands, ffi.ImageWithCornerRadii(
			ffi.TextureID(textureID),
			imgX, imgY, imgW, imgH,
			cornerRadii,
		))
	} else {
		commands = append(commands, ffi.Image(
			ffi.TextureID(textureID),
			imgX, imgY, imgW, imgH,
		))
	}

	// Pop clip if we pushed one
	if needsClip || hasRounded {
		commands = append(commands, ffi.PopClip())
	}

	return commands
}

// processPendingTextureUnloads unloads textures that were queued for deferred deletion.
// This is called at the start of each frame to clean up old camera frame textures
// that couldn't be unloaded during tree traversal because they were still referenced
// by render commands.
func (l *Loop) processPendingTextureUnloads() {
	if len(l.pendingTextureUnloads) == 0 {
		return
	}

	for _, textureID := range l.pendingTextureUnloads {
		ffi.UnloadImage(ffi.TextureID(textureID))
	}

	// Clear the slice but keep the underlying array for reuse
	l.pendingTextureUnloads = l.pendingTextureUnloads[:0]
}

// initializeCamera creates and initializes a camera input.
func (l *Loop) initializeCamera(w *Widget, deviceID string, width, height, frameRate uint32, autoStart bool) {
	// Create input
	inputID := ffi.VideoInputCreate()
	if inputID == 0 {
		w.mu.Lock()
		w.camState = int32(ffi.VideoInputStateError)
		onError := w.onCamError
		w.mu.Unlock()
		if onError != nil {
			onError(fmt.Errorf("failed to create video input"))
		}
		return
	}

	// Store input ID
	w.mu.Lock()
	w.camInputID = uint32(inputID)
	w.camState = int32(ffi.VideoInputStateIdle)
	w.mu.Unlock()

	// Request permission
	err := ffi.VideoInputRequestPermission(inputID)
	if err != nil {
		w.mu.Lock()
		w.camState = int32(ffi.VideoInputStateRequestingPermission)
		onError := w.onCamError
		w.mu.Unlock()
		if onError != nil {
			onError(err)
		}
		return
	}

	// Open the device
	err = ffi.VideoInputOpen(inputID, deviceID, width, height, frameRate)
	if err != nil {
		w.mu.Lock()
		w.camState = int32(ffi.VideoInputStateError)
		onError := w.onCamError
		w.mu.Unlock()
		if onError != nil {
			onError(err)
		}
		return
	}

	w.mu.Lock()
	w.camState = int32(ffi.VideoInputStateReady)
	w.mu.Unlock()

	// Auto-start if requested
	if autoStart {
		err = ffi.VideoInputStart(inputID)
		if err == nil {
			w.mu.Lock()
			w.camState = int32(ffi.VideoInputStateCapturing)
			w.mu.Unlock()
		} else {
			w.mu.RLock()
			onError := w.onCamError
			w.mu.RUnlock()
			if onError != nil {
				onError(err)
			}
		}
	}
}

// ============================================================================
// Clipboard Support
// ============================================================================

// updateClipboard handles clipboard widget polling for changes.
// Clipboard widgets are non-rendering data sources that can monitor clipboard changes.
func (l *Loop) updateClipboard(w *Widget) {
	w.mu.RLock()
	monitor := w.clipboardMonitor
	cachedText := w.clipboardText
	onChange := w.onClipboardChange
	w.mu.RUnlock()

	// Only poll if monitoring is enabled
	if !monitor {
		return
	}

	// Check current clipboard content
	currentText := ffi.ClipboardGetString()

	// Fire callback if content changed
	if currentText != cachedText {
		w.mu.Lock()
		w.clipboardText = currentText
		w.mu.Unlock()

		if onChange != nil {
			onChange(currentText)
		}
	}
}

// updateTrayIcon creates and manages the system tray icon.
func (l *Loop) updateTrayIcon(w *Widget) {
	w.mu.Lock()
	defer w.mu.Unlock()

	needsFullUpdate := false

	// Create tray icon if not yet created
	if !w.trayCreated {
		if err := ffi.TrayIconCreate(); err != nil {
			// Failed to create - probably not supported on this platform
			return
		}
		w.trayCreated = true
		needsFullUpdate = true // Force full update on creation

		// Set up menu callback to route to the correct MenuItem's OnClick handler
		ffi.TrayIconSetMenuCallback(func(index int) {
			w.mu.RLock()
			items := w.trayMenu
			indices := w.trayMenuIndices
			w.mu.RUnlock()

			// Find which menu item was clicked based on the native index
			for i, nativeIdx := range indices {
				if nativeIdx == index && i < len(items) && items[i].OnClick != nil {
					items[i].OnClick()
					return
				}
			}
		})
	}

	// Check if dirty and needs update
	if !w.dirty && !needsFullUpdate {
		return
	}

	// Handle visibility first - if hiding, we can skip other updates
	// If showing after being hidden, we need to reapply title/icon
	wasHidden := !ffi.TrayIconIsVisible()
	ffi.TrayIconSetVisible(w.trayVisible)

	// If now visible (either still visible or just shown), apply all settings
	if w.trayVisible {
		// Update title (always set, even if empty, to clear the default "App")
		ffi.TrayIconSetTitle(w.trayTitle)

		// Update icon (file path takes precedence)
		if w.trayIconPath != "" {
			_ = ffi.TrayIconSetIconFile(w.trayIconPath)
		} else if len(w.trayIconData) > 0 {
			_ = ffi.TrayIconSetIconData(w.trayIconData)
		}

		// Update tooltip
		if w.trayTooltip != "" {
			ffi.TrayIconSetTooltip(w.trayTooltip)
		}

		// Update menu items (always rebuild if showing after hide, or if dirty)
		if (wasHidden || needsFullUpdate || w.dirty) && len(w.trayMenu) > 0 {
			ffi.TrayIconClearMenu()
			w.trayMenuIndices = make([]int, len(w.trayMenu))

			for i, item := range w.trayMenu {
				if item.Separator {
					w.trayMenuIndices[i] = ffi.TrayIconAddSeparator()
				} else {
					enabled := item.Enabled
					w.trayMenuIndices[i] = ffi.TrayIconAddMenuItem(item.Label, enabled, item.Checked)
				}
			}
		}
	}

	w.dirty = false
}

// abs32 returns the absolute value of a float32
func abs32(x float32) float32 {
	if x < 0 {
		return -x
	}
	return x
}

// pendingImageLoads tracks which images are being loaded to prevent duplicate loads
var pendingImageLoads sync.Map

// loadImageAsync loads an image asynchronously from either a URL or file path.
// The file/URL fetch happens in a goroutine, but the GPU upload is queued
// to happen on the main thread to avoid deadlocks with the wgpu backend mutex.
func (l *Loop) loadImageAsync(w *Widget, source string) {
	// Check if already loading this source for this widget
	widgetID := w.ID()
	loadKey := fmt.Sprintf("%d:%s", widgetID, source)

	// Prevent duplicate load requests
	if _, alreadyLoading := pendingImageLoads.LoadOrStore(loadKey, true); alreadyLoading {
		return
	}

	w.mu.Lock()
	w.imageLoading = true
	w.mu.Unlock()

	// Fetch data asynchronously (file read or HTTP fetch)
	go func() {
		defer pendingImageLoads.Delete(loadKey)

		var data []byte
		var err error

		if isURL(source) {
			// Fetch from URL
			data, err = fetchURL(source)
		} else {
			// Read from file
			data, err = readImageFile(source)
		}

		// Queue the result for main thread processing
		// GPU operations must happen on the main thread
		select {
		case pendingUploads <- pendingTextureUpload{widget: w, data: data, err: err}:
			// Queued successfully
		default:
			// Channel full, mark as error
			w.mu.Lock()
			w.imageLoading = false
			w.imageError = fmt.Errorf("texture upload queue full")
			w.mu.Unlock()
		}
	}()
}

// processPendingTextureUploads processes any queued texture uploads on the main thread.
// This must be called from the main thread (during the render loop).
func processPendingTextureUploads() {
	for {
		select {
		case upload := <-pendingUploads:
			w := upload.widget
			if upload.err != nil {
				w.mu.Lock()
				w.imageLoading = false
				w.imageError = upload.err
				w.mu.Unlock()
				continue
			}

			// Now safe to call FFI from main thread
			textureID, loadErr := ffi.LoadImage(upload.data)

			w.mu.Lock()
			w.imageLoading = false
			if loadErr != nil {
				w.imageError = loadErr
			} else {
				w.imageTextureID = uint32(textureID)
				w.imageError = nil
				// Get the actual image dimensions from the loaded texture
				if texW, texH, sizeErr := ffi.GetTextureSize(textureID); sizeErr == nil {
					w.imageNaturalW = texW
					w.imageNaturalH = texH
				}
				// Mark layout dirty so parent containers can resize
				// The image now has content that affects layout
				w.markDirty(DirtySize | DirtyLayout)
			}
			w.mu.Unlock()
		default:
			// No more pending uploads
			return
		}
	}
}

// readImageFile reads image data from a file path.
func readImageFile(path string) ([]byte, error) {
	// Try the path as-is first
	data, err := os.ReadFile(path)
	if err == nil {
		return data, nil
	}

	// If relative path, try relative to current working directory
	if !filepath.IsAbs(path) {
		if cwd, cwdErr := os.Getwd(); cwdErr == nil {
			fullPath := filepath.Join(cwd, path)
			data, err = os.ReadFile(fullPath)
			if err == nil {
				return data, nil
			}
		}

		// Try relative to executable directory
		if exePath, exeErr := os.Executable(); exeErr == nil {
			exeDir := filepath.Dir(exePath)
			fullPath := filepath.Join(exeDir, path)
			data, err = os.ReadFile(fullPath)
			if err == nil {
				return data, nil
			}
		}
	}

	return nil, fmt.Errorf("image file not found: %s", path)
}

// fetchURL fetches data from a URL.
func fetchURL(url string) ([]byte, error) {
	resp, err := httpClient.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("HTTP %d: %s", resp.StatusCode, resp.Status)
	}

	return io.ReadAll(resp.Body)
}

// ============================================================================
// Control Widget Rendering
// ============================================================================

// renderCheckbox renders a checkbox widget with a box and optional label
func (l *Loop) renderCheckbox(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Checkbox dimensions
	boxSize := float32(18)
	boxPadding := float32(2)

	// Center checkbox vertically
	boxY := y + (height-boxSize)/2

	// Draw checkbox box (border)
	borderColor := uint32(0x6B7280FF) // gray-500
	if w.focused {
		borderColor = 0x3B82F6FF // blue-500
	}
	if w.disabled {
		borderColor = 0x9CA3AFFF // gray-400
	}

	commands = append(commands, ffi.RenderCommand{
		DrawRect: &ffi.DrawRectCmd{
			X:           x,
			Y:           boxY,
			Width:       boxSize,
			Height:      boxSize,
			Color:       0x1F2937FF, // gray-800 background
			CornerRadii: [4]float32{4, 4, 4, 4},
			Border: &ffi.Border{
				Width: 2,
				Color: borderColor,
				Style: "Solid",
			},
		},
	})

	// Draw checkmark if checked
	if w.checked {
		checkColor := uint32(0x3B82F6FF) // blue-500
		if w.disabled {
			checkColor = 0x6B7280FF // gray-500
		}
		// Draw a filled inner box as checkmark
		commands = append(commands, ffi.RoundedRect(
			x+boxPadding+2,
			boxY+boxPadding+2,
			boxSize-boxPadding*2-4,
			boxSize-boxPadding*2-4,
			checkColor,
			2,
		))
	}

	// Draw label if present
	if w.text != "" {
		textX := x + boxSize + 8
		// Align text top with checkbox top - simple and effective
		// Both are similar heights, so top-alignment looks correct
		textY := boxY
		textColor := w.textColor
		if w.disabled {
			textColor = 0x6B7280FF // gray-500
		}
		commands = append(commands, textCommand(w, w.text, textX, textY, textColor))
	}

	return commands
}

// renderToggle renders an iOS-style toggle switch
func (l *Loop) renderToggle(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Toggle dimensions
	toggleWidth := float32(44)
	toggleHeight := float32(24)
	knobSize := float32(20)
	knobPadding := float32(2)

	// Center toggle vertically
	toggleY := y + (height-toggleHeight)/2

	// Track color
	trackColor := uint32(0x374151FF) // gray-700 (off)
	if w.checked {
		trackColor = 0x22C55EFF // green-500 (on)
	}
	if w.disabled {
		trackColor = 0x4B5563FF // gray-600
	}

	// Draw track (rounded pill shape)
	commands = append(commands, ffi.RoundedRect(
		x, toggleY, toggleWidth, toggleHeight, trackColor, toggleHeight/2,
	))

	// Knob position
	knobX := x + knobPadding
	if w.checked {
		knobX = x + toggleWidth - knobSize - knobPadding
	}
	knobY := toggleY + knobPadding

	// Knob color
	knobColor := uint32(0xFFFFFFFF) // white
	if w.disabled {
		knobColor = 0xD1D5DBFF // gray-300
	}

	// Draw knob
	commands = append(commands, ffi.RoundedRect(
		knobX, knobY, knobSize, knobSize, knobColor, knobSize/2,
	))

	// Draw focus ring if focused
	if w.focused {
		commands = append(commands, ffi.RenderCommand{
			DrawRect: &ffi.DrawRectCmd{
				X:           x - 2,
				Y:           toggleY - 2,
				Width:       toggleWidth + 4,
				Height:      toggleHeight + 4,
				Color:       0x00000000, // transparent
				CornerRadii: [4]float32{(toggleHeight + 4) / 2, (toggleHeight + 4) / 2, (toggleHeight + 4) / 2, (toggleHeight + 4) / 2},
				Border: &ffi.Border{
					Width: 2,
					Color: 0x3B82F680, // blue-500 with alpha
					Style: "Solid",
				},
			},
		})
	}

	return commands
}

// renderRadio renders a radio button widget
func (l *Loop) renderRadio(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Radio dimensions
	radioSize := float32(18)

	// Center radio vertically
	radioY := y + (height-radioSize)/2

	// Border color
	borderColor := uint32(0x6B7280FF) // gray-500
	if w.focused {
		borderColor = 0x3B82F6FF // blue-500
	}
	if w.disabled {
		borderColor = 0x9CA3AFFF // gray-400
	}

	// Draw outer circle (using rounded rect with full radius)
	commands = append(commands, ffi.RenderCommand{
		DrawRect: &ffi.DrawRectCmd{
			X:           x,
			Y:           radioY,
			Width:       radioSize,
			Height:      radioSize,
			Color:       0x1F2937FF, // gray-800 background
			CornerRadii: [4]float32{radioSize / 2, radioSize / 2, radioSize / 2, radioSize / 2},
			Border: &ffi.Border{
				Width: 2,
				Color: borderColor,
				Style: "Solid",
			},
		},
	})

	// Draw inner dot if checked
	if w.checked {
		dotColor := uint32(0x3B82F6FF) // blue-500
		if w.disabled {
			dotColor = 0x6B7280FF // gray-500
		}
		dotSize := radioSize - 8
		dotX := x + 4
		dotY := radioY + 4
		commands = append(commands, ffi.RoundedRect(
			dotX, dotY, dotSize, dotSize, dotColor, dotSize/2,
		))
	}

	// Draw label if present
	if w.text != "" {
		textX := x + radioSize + 8
		// Align text top with radio button top - simple and effective
		// Both are similar heights, so top-alignment looks correct
		textY := radioY
		textColor := w.textColor
		if w.disabled {
			textColor = 0x6B7280FF // gray-500
		}
		commands = append(commands, textCommand(w, w.text, textX, textY, textColor))
	}

	return commands
}

// renderSlider renders a slider widget with track and thumb
func (l *Loop) renderSlider(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Slider dimensions
	trackHeight := float32(6)
	thumbSize := float32(16)
	trackPadding := thumbSize / 2 // So thumb doesn't go past edges

	// Track area
	trackX := x + trackPadding
	trackY := y + (height-trackHeight)/2
	trackWidth := width - thumbSize

	// Track background color
	trackBgColor := uint32(0x374151FF) // gray-700
	if w.disabled {
		trackBgColor = 0x4B5563FF // gray-600
	}

	// Draw track background
	commands = append(commands, ffi.RoundedRect(
		trackX, trackY, trackWidth, trackHeight, trackBgColor, trackHeight/2,
	))

	// Calculate thumb position based on value
	ratio := float32(0)
	if w.sliderMax > w.sliderMin {
		ratio = (w.sliderValue - w.sliderMin) / (w.sliderMax - w.sliderMin)
	}
	if ratio < 0 {
		ratio = 0
	}
	if ratio > 1 {
		ratio = 1
	}

	// Draw filled portion of track
	fillColor := uint32(0x3B82F6FF) // blue-500
	if w.disabled {
		fillColor = 0x6B7280FF // gray-500
	}
	fillWidth := trackWidth * ratio
	if fillWidth > 0 {
		commands = append(commands, ffi.RoundedRect(
			trackX, trackY, fillWidth, trackHeight, fillColor, trackHeight/2,
		))
	}

	// Thumb position
	thumbX := trackX + trackWidth*ratio - thumbSize/2
	thumbY := y + (height-thumbSize)/2

	// Thumb color
	thumbColor := uint32(0xFFFFFFFF) // white
	if w.disabled {
		thumbColor = 0xD1D5DBFF // gray-300
	}

	// Draw thumb
	commands = append(commands, ffi.RoundedRect(
		thumbX, thumbY, thumbSize, thumbSize, thumbColor, thumbSize/2,
	))

	// Draw focus ring if focused
	if w.focused {
		commands = append(commands, ffi.RenderCommand{
			DrawRect: &ffi.DrawRectCmd{
				X:           thumbX - 2,
				Y:           thumbY - 2,
				Width:       thumbSize + 4,
				Height:      thumbSize + 4,
				Color:       0x00000000, // transparent
				CornerRadii: [4]float32{(thumbSize + 4) / 2, (thumbSize + 4) / 2, (thumbSize + 4) / 2, (thumbSize + 4) / 2},
				Border: &ffi.Border{
					Width: 2,
					Color: 0x3B82F680, // blue-500 with alpha
					Style: "Solid",
				},
			},
		})
	}

	return commands
}

// renderSelect renders a dropdown select widget
func (l *Loop) renderSelect(commands []ffi.RenderCommand, w *Widget, x, y, width, height float32) []ffi.RenderCommand {
	// Border color
	borderColor := uint32(0x4B5563FF) // gray-600
	if w.focused {
		borderColor = 0x3B82F6FF // blue-500
	}
	if w.disabled {
		borderColor = 0x374151FF // gray-700
	}

	// Background already drawn by parent render

	// Draw border if no background was set
	if w.backgroundColor == nil {
		commands = append(commands, ffi.RenderCommand{
			DrawRect: &ffi.DrawRectCmd{
				X:           x,
				Y:           y,
				Width:       width,
				Height:      height,
				Color:       0x1F2937FF, // gray-800
				CornerRadii: [4]float32{6, 6, 6, 6},
				Border: &ffi.Border{
					Width: 1,
					Color: borderColor,
					Style: "Solid",
				},
			},
		})
	}

	// Determine displayed text
	displayText := w.text           // placeholder
	textColor := uint32(0x9CA3AFFF) // gray-400 for placeholder
	if w.selectIndex >= 0 && w.selectIndex < len(w.selectOptions) {
		displayText = w.selectOptions[w.selectIndex].Label
		textColor = w.textColor
	}
	if w.disabled {
		textColor = 0x6B7280FF // gray-500
	}

	// Draw text - use padding from top for consistent positioning
	textX := x + 12
	textY := y + 10 // Fixed padding from top, similar to button styling
	commands = append(commands, textCommand(w, displayText, textX, textY, textColor))

	// Draw dropdown arrow
	arrowSize := float32(8)
	arrowX := x + width - arrowSize - 12
	arrowY := y + (height-arrowSize)/2

	arrowColor := uint32(0x9CA3AFFF) // gray-400
	if w.disabled {
		arrowColor = 0x6B7280FF // gray-500
	}

	// Simple down arrow (triangle made of small rects)
	commands = append(commands, ffi.RoundedRect(arrowX, arrowY, arrowSize, 2, arrowColor, 1))
	commands = append(commands, ffi.RoundedRect(arrowX+2, arrowY+3, arrowSize-4, 2, arrowColor, 1))
	commands = append(commands, ffi.RoundedRect(arrowX+4, arrowY+6, arrowSize-8, 2, arrowColor, 1))

	// Draw dropdown list if open - DEFERRED to render on top of everything
	if w.selectOpen && len(w.selectOptions) > 0 {
		optionHeight := float32(32)
		listHeight := float32(len(w.selectOptions)) * optionHeight
		listY := y + height + 4

		// Dropdown background - deferred
		l.deferredOverlays = append(l.deferredOverlays, ffi.RenderCommand{
			DrawRect: &ffi.DrawRectCmd{
				X:           x,
				Y:           listY,
				Width:       width,
				Height:      listHeight,
				Color:       0x1F2937FF, // gray-800
				CornerRadii: [4]float32{6, 6, 6, 6},
				Border: &ffi.Border{
					Width: 1,
					Color: 0x374151FF, // gray-700
					Style: "Solid",
				},
			},
		})

		// Draw options - deferred
		for i, opt := range w.selectOptions {
			optY := listY + float32(i)*optionHeight

			// Highlight selected option
			if i == w.selectIndex {
				l.deferredOverlays = append(l.deferredOverlays, ffi.RoundedRect(
					x+2, optY+2, width-4, optionHeight-4, 0x374151FF, 4, // gray-700
				))
			}

			// Option text
			optTextColor := uint32(0xE5E7EBFF) // gray-200
			if opt.Disabled {
				optTextColor = 0x6B7280FF // gray-500
			}
			l.deferredOverlays = append(l.deferredOverlays, textCommand(w,
				opt.Label,
				x+12, optY+(optionHeight-w.fontSize)/2,
				optTextColor,
			))
		}
	}

	return commands
}
