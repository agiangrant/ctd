// Package retained provides a retained-mode widget system with efficient
// batched updates to the Rust rendering engine.
//
// The architecture supports two modes:
//   - Retained: Widget tree with delta updates only on state changes
//   - Retained+Immediate: Game loop at target FPS with interleaved immediate draws
//
// Updates flow through a sharded channel pool for high-throughput concurrent access.
package retained

import (
	"fmt"
	"sync"
	"sync/atomic"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/tw"
)

// WidgetID uniquely identifies a widget in the tree.
// IDs are stable across updates and used for delta tracking.
type WidgetID uint64

var nextWidgetID atomic.Uint64

func newWidgetID() WidgetID {
	return WidgetID(nextWidgetID.Add(1))
}

// WidgetKind identifies the type of widget for rendering.
type WidgetKind string

const (
	KindContainer  WidgetKind = "container"
	KindVStack     WidgetKind = "vstack"
	KindHStack     WidgetKind = "hstack"
	KindZStack     WidgetKind = "zstack"
	KindScrollView WidgetKind = "scroll_view"
	KindText       WidgetKind = "text"
	KindButton     WidgetKind = "button"
	KindImage      WidgetKind = "image"
	KindTextField  WidgetKind = "text_field"
	KindTextArea   WidgetKind = "text_area"
	KindCheckbox   WidgetKind = "checkbox"
	KindToggle     WidgetKind = "toggle"
	KindRadio      WidgetKind = "radio"
	KindSlider     WidgetKind = "slider"
	KindSelect     WidgetKind = "select"
	KindVideo       WidgetKind = "video"
	KindAudio       WidgetKind = "audio"
	KindMicrophone  WidgetKind = "microphone"
	KindCamera      WidgetKind = "camera"
	KindClipboard   WidgetKind = "clipboard"
	KindFilePicker  WidgetKind = "filepicker"
	KindTrayIcon    WidgetKind = "trayicon"
	KindCustom      WidgetKind = "custom"
)

// FileFilter represents a file type filter for file picker dialogs.
type FileFilter struct {
	Name       string   // Display name (e.g., "Images")
	Extensions []string // File extensions without dots (e.g., []string{"png", "jpg", "jpeg"})
}

// MenuItem represents an item in a tray icon menu.
type MenuItem struct {
	Label     string     // Display text
	Enabled   bool       // Whether the item is clickable (default true)
	Checked   bool       // Shows checkmark if true
	Separator bool       // If true, renders as separator line (ignores other fields)
	OnClick   func()     // Called when item is clicked
}

// Position determines how a widget is positioned relative to its parent.
// Similar to CSS position property.
type Position int

const (
	// PositionStatic is the default - widget flows in parent's layout (VStack/HStack).
	// The x/y values are ignored; position is determined by layout.
	PositionStatic Position = iota

	// PositionRelative positions widget relative to where it would be in normal flow.
	// x/y are offsets from the normal position.
	PositionRelative

	// PositionAbsolute positions widget relative to its nearest positioned ancestor.
	// x/y are coordinates within that ancestor's bounds.
	PositionAbsolute

	// PositionFixed positions widget relative to the viewport/window.
	// x/y are screen coordinates. Widget is removed from normal flow.
	PositionFixed

	// PositionSticky positions widget relative to its scroll container.
	// It acts like relative until a scroll threshold is crossed, then acts like fixed.
	PositionSticky
)

// FlexDirection determines the main axis for flex layout.
type FlexDirection int

const (
	FlexRow FlexDirection = iota
	FlexColumn
	FlexRowReverse
	FlexColumnReverse
)

// JustifyContent controls alignment along the main axis.
type JustifyContent int

const (
	JustifyStart JustifyContent = iota
	JustifyEnd
	JustifyCenter
	JustifyBetween
	JustifyAround
	JustifyEvenly
)

// AlignItems controls alignment along the cross axis.
type AlignItems int

const (
	AlignStart AlignItems = iota
	AlignEnd
	AlignCenter
	AlignStretch
	AlignBaseline
)

// AlignSelf controls alignment of an individual flex item (overrides parent's alignItems).
type AlignSelf int

const (
	AlignSelfAuto AlignSelf = iota // Use parent's alignItems
	AlignSelfStart
	AlignSelfEnd
	AlignSelfCenter
	AlignSelfStretch
	AlignSelfBaseline
)

// FlexBasisMode determines how flex basis is calculated.
type FlexBasisMode int

const (
	FlexBasisAuto    FlexBasisMode = iota // Auto (use width/height)
	FlexBasisFixed                        // Fixed pixel value
	FlexBasisPercent                      // Percentage of parent
	FlexBasisFull                         // 100% of parent
)

// FlexWrap controls whether items wrap to new lines.
type FlexWrap int

const (
	FlexNoWrap FlexWrap = iota
	FlexWrapWrap
	FlexWrapReverse
)

// SelectOption represents an option in a Select/Dropdown widget.
type SelectOption struct {
	Label    string // Display text
	Value    any    // Associated value
	Disabled bool   // Option is disabled/unselectable
}

// Widget represents a UI element in the retained tree.
// Widgets are thread-safe for concurrent property updates.
type Widget struct {
	mu sync.RWMutex

	id       WidgetID
	kind     WidgetKind
	parent   *Widget
	children []*Widget

	// Layout properties
	position      Position // How this widget is positioned (static, relative, absolute, fixed, sticky)
	x, y          float32  // Legacy: For relative offset. Use posTop/posLeft for absolute/fixed.
	width, height float32

	// Position offsets (CSS top/right/bottom/left)
	// nil means "auto" (unset), pointer to value means explicitly set
	posTop    *float32
	posRight  *float32
	posBottom *float32
	posLeft   *float32
	minWidth      *float32
	minHeight     *float32
	maxWidth      *float32
	maxHeight     *float32

	// Size modes for responsive/flex sizing
	widthMode     SizeMode // How width is calculated (fixed, auto, full, percent, flex)
	heightMode    SizeMode // How height is calculated
	widthPercent  float32  // Percentage value when widthMode is SizePercent
	heightPercent float32  // Percentage value when heightMode is SizePercent

	// Computed layout (cached, recomputed when layoutDirty)
	computedLayout ComputedLayout
	layoutDirty    bool // True when layout needs recomputation

	// Spacing
	padding [4]float32 // [top, right, bottom, left]
	gap     float32    // Space between children (for VStack/HStack)

	// Flex properties (for children layout)
	flexDirection  FlexDirection
	justifyContent JustifyContent
	alignItems     AlignItems
	flexWrap       FlexWrap

	// Flex properties (for self within parent)
	flexGrow         float32
	flexShrink       float32
	flexBasis        *float32
	flexBasisMode    FlexBasisMode
	flexBasisPercent float32
	alignSelf        AlignSelf
	order            int

	// Visual properties
	backgroundColor *uint32
	borderColor     *uint32
	borderWidth     float32
	cornerRadius    [4]float32
	opacity         float32
	rotation        float32 // Rotation angle in radians (around center)
	visible         bool
	zIndex          int

	// Content
	text       string
	textColor  uint32
	fontSize   float32
	lineHeight float32 // Multiplier for line height (default 1.2, 0 means use default)
	fontName   string  // Resolved font name for rendering (system font name or "bundled:path")
	fontFamily string  // Theme font family key (sans, serif, mono, custom) from Tailwind classes
	textAlign  string  // "left", "center", "right", "justify", "start", "end" (default "left")

	// Cached text metrics for layout (measured via FFI, cached to avoid repeated calls)
	textWidth      float32 // Measured text width in pixels (0 if not yet measured)
	textWidthDirty bool    // True when text width needs remeasurement

	// Scroll state
	scrollX, scrollY       float32
	contentWidth           float32
	contentHeight          float32
	scrollEnabled          bool
	showScrollIndicators   bool
	lastCursorPos          int // Track cursor position to detect when to auto-scroll

	// Overflow behavior (from overflow-* classes)
	// Values: "visible" (default), "hidden", "clip", "scroll", "auto"
	overflowX string
	overflowY string

	// Scrollbar drag state
	scrollbarDragging   bool
	scrollbarDragStartY float32 // Y position where drag started
	scrollbarDragStartScrollY float32 // scrollY value when drag started

	// Tailwind classes (parsed at creation, applied to properties)
	classes        string
	computedStyles *tw.ComputedStyles // Cached parsed styles for state changes

	// Animation state (from animate-* classes)
	pendingAnimation      string     // Animation type to start (e.g., "pulse", "bounce")
	pendingAnimDuration   float32    // Custom duration in ms (0 = use default)
	pendingAnimEasing     string     // Custom easing function (empty = use default)
	pendingAnimIterations int        // Custom iteration count (-1 = infinite, 0 = use default, >0 = N times)
	activeAnimation       *Animation // Currently running animation from class

	// Image properties (for KindImage widgets)
	imageSource    string  // Source path or URL (e.g., "assets/icon.png" or "https://...")
	imageTextureID uint32  // Loaded GPU texture ID (0 = not loaded)
	imageLoading   bool    // True while image is loading asynchronously
	imageError     error   // Error if image failed to load
	imageFit       string  // How image fits in bounds: "contain", "cover", "fill", "none" (default "contain")
	imagePosition  string  // Where image is positioned: "center", "top", "bottom", etc. (default "center")
	imageSourceX      float32 // Source rect for sprite sheets (optional)
	imageSourceY      float32
	imageSourceW      float32
	imageSourceH      float32
	imageNaturalW     uint32  // Actual image width in pixels (set when loaded)
	imageNaturalH     uint32  // Actual image height in pixels (set when loaded)

	// Video properties (for KindVideo widgets)
	videoSource    string              // Source path or URL
	videoPlayerID  uint32              // Video player ID (0 = not created)
	videoTextureID uint32              // Current video texture ID
	videoLoading   bool                // True while video is loading
	videoError     error               // Error if video failed to load
	videoAutoplay  bool                // Start playing automatically when loaded
	videoLoop      bool                // Loop video playback
	videoMuted     bool                // Mute audio
	videoVolume    float32             // Audio volume (0.0 - 1.0)
	videoState     int32               // Current playback state
	videoNaturalW  uint32              // Video width in pixels
	videoNaturalH  uint32              // Video height in pixels
	videoDurationMs uint64             // Video duration in milliseconds
	onVideoEnded   func()              // Called when video playback ends
	onVideoError   func(error)         // Called on video error
	onVideoTimeUpdate func(uint64, uint64) // Called with (currentMs, durationMs)

	// Audio properties (for KindAudio widgets)
	audioSource    string              // Source path or URL
	audioPlayerID  uint32              // Audio player ID (0 = not created)
	audioLoading   bool                // True while audio is loading
	audioError     error               // Error if audio failed to load
	audioAutoplay  bool                // Start playing automatically when loaded
	audioLoop      bool                // Loop audio playback
	audioVolume    float32             // Audio volume (0.0 - 1.0)
	audioState     int32               // Current playback state
	audioDurationMs uint64             // Audio duration in milliseconds
	audioSampleRate uint32             // Sample rate in Hz
	audioChannels   uint32             // Number of channels
	onAudioEnded   func()              // Called when audio playback ends
	onAudioError   func(error)         // Called on audio error
	onAudioTimeUpdate func(uint64, uint64) // Called with (currentMs, durationMs)

	// Microphone (audio input) properties
	micInputID       uint32               // Microphone input ID (0 = not created)
	micDeviceID      string               // Specific device ID to use (empty = default)
	micSampleRate    uint32               // Sample rate in Hz (0 = default 44100)
	micChannels      uint32               // Number of channels (0 = default 1)
	micState         int32                // Current input state
	micLevel         float32              // Current audio level (0.0 - 1.0 RMS)
	micAutoStart     bool                 // Auto-start capturing when ready
	onMicError       func(error)          // Called on microphone error
	onMicLevelChange func(float32)        // Called when audio level changes
	onMicStateChange func(int32)          // Called when input state changes

	// Camera (video input) properties
	camInputID       uint32               // Camera input ID (0 = not created)
	camDeviceID      string               // Specific device ID to use (empty = default)
	camWidth         uint32               // Preferred width (0 = default 1280)
	camHeight        uint32               // Preferred height (0 = default 720)
	camFrameRate     uint32               // Preferred frame rate (0 = default 30)
	camState         int32                // Current input state
	camTextureID     uint32               // Current camera frame texture ID
	camActualWidth   uint32               // Actual capture width
	camActualHeight  uint32               // Actual capture height
	camAutoStart     bool                 // Auto-start capturing when ready
	onCamError       func(error)          // Called on camera error
	onCamStateChange func(int32)          // Called when input state changes
	onCamFrame       func(textureID uint32) // Called when new frame is available with texture ID

	// =========================================================================
	// Clipboard Widget Fields
	// =========================================================================
	clipboardText     string            // Current clipboard text (cached)
	clipboardMonitor  bool              // Whether to monitor clipboard changes
	onClipboardChange func(text string) // Called when clipboard content changes

	// =========================================================================
	// FilePicker Widget Fields
	// =========================================================================
	filePickerTitle       string           // Dialog title
	filePickerFilters     []FileFilter     // File type filters
	filePickerMultiple    bool             // Allow multiple file selection
	filePickerDirectory   string           // Initial directory
	filePickerDialogOpen  bool             // Whether dialog is currently open
	onFileSelect          func(paths []string) // Called when files are selected
	onFileCancel          func()           // Called when dialog is cancelled

	// =========================================================================
	// TrayIcon Widget Fields
	// =========================================================================
	trayIconPath      string       // Icon file path
	trayIconData      []byte       // Icon image data (alternative to path)
	trayTooltip       string       // Tooltip text
	trayTitle         string       // Title text (alternative to icon)
	trayMenu          []MenuItem   // Menu items
	trayMenuIndices   []int        // Maps menu item index to callback index
	trayCreated       bool         // Whether tray icon has been created
	trayVisible       bool         // Whether tray icon is visible
	onTrayClick       func()       // Called on tray icon click

	// Custom data for application use
	data any

	// Dirty tracking for delta updates
	dirty     bool
	dirtyMask uint64 // Bitmask of which properties changed

	// Reference to tree for update dispatch
	tree *Tree

	// =========================================================================
	// Event System
	// =========================================================================

	// Computed bounds (cached during render for O(1) hit testing)
	// These are screen-space coordinates updated every frame during rendering.
	computedBounds Bounds
	boundsFrame    uint64 // Frame number when bounds were last updated

	// Interactive state
	hovered  bool // Mouse is over this widget
	focused  bool // Widget has keyboard focus
	pressed  bool // Mouse button is down on this widget
	disabled bool // Widget is disabled (doesn't receive events)

	// Event handlers (simple callback API)
	onClick       MouseHandler
	onDoubleClick MouseHandler
	onTripleClick MouseHandler
	onMouseDown   MouseHandler
	onMouseUp     MouseHandler
	onMouseEnter  MouseHandler
	onMouseLeave  MouseHandler
	onMouseMove   MouseHandler
	onMouseWheel  MouseHandler
	onKeyDown     KeyHandler
	onKeyUp       KeyHandler
	onKeyPress    KeyHandler
	onFocus       FocusHandler
	onBlur        FocusHandler

	// Custom responder for advanced event handling (embedding/composition)
	// If set, this takes precedence over the callback handlers.
	responder Responder

	// Behaviors attached to this widget (composable event modifiers)
	behaviors []Behavior

	// Text input buffer (for TextField and TextArea)
	textBuffer *TextBuffer

	// Control widget state
	checked       bool            // For Checkbox, Toggle, Radio
	radioGroup    string          // For Radio - groups radios together
	sliderValue   float32         // For Slider - current value (0-1 normalized)
	sliderMin     float32         // For Slider - minimum value
	sliderMax     float32         // For Slider - maximum value
	sliderStep    float32         // For Slider - step increment (0 = continuous)
	selectOptions []SelectOption  // For Select - available options
	selectIndex   int             // For Select - currently selected index (-1 = none)
	selectOpen    bool            // For Select - dropdown is open
	onChangeValue func(value any) // Generic value change callback
}

// Property change flags for dirty tracking
const (
	DirtyPosition uint64 = 1 << iota
	DirtySize
	DirtyBackground
	DirtyBorder
	DirtyOpacity
	DirtyRotation
	DirtyVisible
	DirtyText
	DirtyScroll
	DirtyChildren
	DirtyZIndex
	DirtyLayout // Layout needs recomputation
)

// NewWidget creates a widget with default values.
// The widget is not attached to any tree until added as a child.
func NewWidget(kind WidgetKind) *Widget {
	return &Widget{
		id:        newWidgetID(),
		kind:      kind,
		opacity:   1.0,
		visible:   true,
		fontSize:  14,
		fontName:  "system",
		textColor: 0xFFFFFFFF,
	}
}

// ID returns the widget's unique identifier.
func (w *Widget) ID() WidgetID {
	return w.id
}

// Kind returns the widget type.
func (w *Widget) Kind() WidgetKind {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.kind
}

// ============================================================================
// Tree Structure
// ============================================================================

// Parent returns the widget's parent, or nil if it's the root.
func (w *Widget) Parent() *Widget {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.parent
}

// Children returns a copy of the widget's children slice.
func (w *Widget) Children() []*Widget {
	w.mu.RLock()
	defer w.mu.RUnlock()
	result := make([]*Widget, len(w.children))
	copy(result, w.children)
	return result
}

// AddChild appends a child widget.
func (w *Widget) AddChild(child *Widget) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()

	child.mu.Lock()
	child.parent = w
	child.tree = w.tree
	child.mu.Unlock()

	w.children = append(w.children, child)
	w.markDirty(DirtyChildren)
	return w
}

// InsertChild inserts a child at the specified index.
func (w *Widget) InsertChild(index int, child *Widget) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()

	child.mu.Lock()
	child.parent = w
	child.tree = w.tree
	child.mu.Unlock()

	if index >= len(w.children) {
		w.children = append(w.children, child)
	} else {
		w.children = append(w.children[:index+1], w.children[index:]...)
		w.children[index] = child
	}
	w.markDirty(DirtyChildren)
	return w
}

// RemoveChild removes a child by reference.
func (w *Widget) RemoveChild(child *Widget) bool {
	w.mu.Lock()
	defer w.mu.Unlock()

	for i, c := range w.children {
		if c == child {
			w.children = append(w.children[:i], w.children[i+1:]...)
			child.mu.Lock()
			child.parent = nil
			child.tree = nil
			child.mu.Unlock()
			w.markDirty(DirtyChildren)
			return true
		}
	}
	return false
}

// RemoveFromParent removes this widget from its parent.
func (w *Widget) RemoveFromParent() {
	w.mu.RLock()
	parent := w.parent
	w.mu.RUnlock()

	if parent != nil {
		parent.RemoveChild(w)
	}
}

// ============================================================================
// Property Setters (all thread-safe, trigger dirty tracking)
// ============================================================================

func (w *Widget) markDirty(flags uint64) {
	w.dirty = true
	w.dirtyMask |= flags

	// Mark layout dirty if size or position affecting properties changed
	if flags&(DirtySize|DirtyPosition|DirtyChildren|DirtyLayout) != 0 {
		w.layoutDirty = true
		w.computedLayout.Valid = false
	}

	// Dispatch update to tree if attached
	if w.tree != nil {
		w.tree.notifyUpdate(w, flags)
	}
}

// SetPosition sets x and y coordinates.
func (w *Widget) SetPosition(x, y float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.x != x || w.y != y {
		w.x, w.y = x, y
		w.markDirty(DirtyPosition)
	}
	return w
}

// SetSize sets width and height.
func (w *Widget) SetSize(width, height float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.width != width || w.height != height {
		w.width, w.height = width, height
		w.markDirty(DirtySize)
	}
	return w
}

// SetFrame sets position and size in one call.
// Note: For PositionStatic widgets, x/y are ignored during layout.
func (w *Widget) SetFrame(x, y, width, height float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	changed := false
	if w.x != x || w.y != y {
		w.x, w.y = x, y
		changed = true
	}
	if w.width != width || w.height != height {
		w.width, w.height = width, height
		changed = true
	}
	// SetFrame implies absolute positioning since we're specifying exact coordinates
	if w.position != PositionAbsolute {
		w.position = PositionAbsolute
		changed = true
	}
	if changed {
		w.markDirty(DirtyPosition | DirtySize)
	}
	return w
}

// SetPositionMode sets how the widget is positioned.
func (w *Widget) SetPositionMode(pos Position) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.position != pos {
		w.position = pos
		w.markDirty(DirtyPosition)
	}
	return w
}

// SetWidthMode sets how the widget's width is calculated.
func (w *Widget) SetWidthMode(mode SizeMode) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.widthMode != mode {
		w.widthMode = mode
		w.markDirty(DirtySize | DirtyLayout)
	}
	return w
}

// SetHeightMode sets how the widget's height is calculated.
func (w *Widget) SetHeightMode(mode SizeMode) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.heightMode != mode {
		w.heightMode = mode
		w.markDirty(DirtySize | DirtyLayout)
	}
	return w
}

// SetWidthPercent sets the width as a percentage of parent (0-100).
func (w *Widget) SetWidthPercent(percent float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.widthMode = SizePercent
	w.widthPercent = percent
	w.markDirty(DirtySize | DirtyLayout)
	return w
}

// SetHeightPercent sets the height as a percentage of parent (0-100).
func (w *Widget) SetHeightPercent(percent float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.heightMode = SizePercent
	w.heightPercent = percent
	w.markDirty(DirtySize | DirtyLayout)
	return w
}

// SetWidthFull makes the widget fill its parent's width (equivalent to w-full).
func (w *Widget) SetWidthFull() *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.widthMode = SizeFull
	w.markDirty(DirtySize | DirtyLayout)
	return w
}

// SetHeightFull makes the widget fill its parent's height (equivalent to h-full).
func (w *Widget) SetHeightFull() *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.heightMode = SizeFull
	w.markDirty(DirtySize | DirtyLayout)
	return w
}

// SetFlexGrow sets the flex-grow value for distributing extra space.
// flex-1 = SetFlexGrow(1), flex-0 = SetFlexGrow(0)
func (w *Widget) SetFlexGrow(grow float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.flexGrow != grow {
		w.flexGrow = grow
		w.markDirty(DirtySize | DirtyLayout)
	}
	return w
}

// SetPadding sets uniform padding on all sides.
func (w *Widget) SetPadding(padding float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	newPadding := [4]float32{padding, padding, padding, padding}
	if w.padding != newPadding {
		w.padding = newPadding
		w.markDirty(DirtySize)
	}
	return w
}

// SetPaddingXY sets horizontal and vertical padding.
func (w *Widget) SetPaddingXY(horizontal, vertical float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	newPadding := [4]float32{vertical, horizontal, vertical, horizontal}
	if w.padding != newPadding {
		w.padding = newPadding
		w.markDirty(DirtySize)
	}
	return w
}

// SetPaddingAll sets padding for each side individually [top, right, bottom, left].
func (w *Widget) SetPaddingAll(top, right, bottom, left float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	newPadding := [4]float32{top, right, bottom, left}
	if w.padding != newPadding {
		w.padding = newPadding
		w.markDirty(DirtySize)
	}
	return w
}

// SetGap sets the space between children for VStack/HStack.
func (w *Widget) SetGap(gap float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.gap != gap {
		w.gap = gap
		w.markDirty(DirtySize)
	}
	return w
}

// SetBackgroundColor sets the fill color (0xRRGGBBAA).
func (w *Widget) SetBackgroundColor(color uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.backgroundColor == nil || *w.backgroundColor != color {
		w.backgroundColor = &color
		w.markDirty(DirtyBackground)
	}
	return w
}

// ClearBackgroundColor removes the background color.
func (w *Widget) ClearBackgroundColor() *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.backgroundColor != nil {
		w.backgroundColor = nil
		w.markDirty(DirtyBackground)
	}
	return w
}

// SetBorder sets border width and color.
func (w *Widget) SetBorder(width float32, color uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.borderWidth != width || w.borderColor == nil || *w.borderColor != color {
		w.borderWidth = width
		w.borderColor = &color
		w.markDirty(DirtyBorder)
	}
	return w
}

// SetCornerRadius sets uniform corner radius.
func (w *Widget) SetCornerRadius(radius float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	newRadii := [4]float32{radius, radius, radius, radius}
	if w.cornerRadius != newRadii {
		w.cornerRadius = newRadii
		w.markDirty(DirtyBackground)
	}
	return w
}

// SetCornerRadii sets per-corner radii [topLeft, topRight, bottomRight, bottomLeft].
func (w *Widget) SetCornerRadii(radii [4]float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.cornerRadius != radii {
		w.cornerRadius = radii
		w.markDirty(DirtyBackground)
	}
	return w
}

// SetOpacity sets the opacity (0.0 to 1.0).
func (w *Widget) SetOpacity(opacity float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.opacity != opacity {
		w.opacity = opacity
		w.markDirty(DirtyOpacity)
	}
	return w
}

// SetRotation sets the rotation angle in radians (around center).
func (w *Widget) SetRotation(radians float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.rotation != radians {
		w.rotation = radians
		w.markDirty(DirtyRotation)
	}
	return w
}

// Rotation returns the current rotation angle in radians.
func (w *Widget) Rotation() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.rotation
}

// SetVisible sets whether the widget is rendered.
func (w *Widget) SetVisible(visible bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.visible != visible {
		w.visible = visible
		w.markDirty(DirtyVisible)
	}
	return w
}

// SetZIndex sets the z-order for layering.
func (w *Widget) SetZIndex(z int) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.zIndex != z {
		w.zIndex = z
		w.markDirty(DirtyZIndex)
	}
	return w
}

// SetText sets the text content.
func (w *Widget) SetText(text string) *Widget {
	w.mu.Lock()
	changed := w.text != text
	if changed {
		w.text = text
		w.textWidthDirty = true
		// Mark both text and layout dirty - text content affects widget sizing
		// (especially for multi-line text with newlines)
		w.markDirty(DirtyText | DirtyLayout)
	}
	w.mu.Unlock()

	// Request redraw AFTER releasing lock to avoid race condition where
	// render thread tries to read while we still hold the write lock
	if changed {
		ffi.RequestRedraw()
	}
	return w
}

// SetTextColor sets the text color.
func (w *Widget) SetTextColor(color uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.textColor != color {
		w.textColor = color
		w.markDirty(DirtyText)
	}
	return w
}

// SetFontSize sets the font size in points.
func (w *Widget) SetFontSize(size float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.fontSize != size {
		w.fontSize = size
		w.textWidthDirty = true
		w.markDirty(DirtyText)
	}
	return w
}

// SetLineHeight sets the line height multiplier (e.g., 1.2 for 120%).
// A value of 0 means use the default (1.2).
func (w *Widget) SetLineHeight(multiplier float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.lineHeight != multiplier {
		w.lineHeight = multiplier
		w.markDirty(DirtyText)
	}
	return w
}

// EffectiveLineHeight returns the computed line height in pixels.
// If lineHeight is 0, uses 1.2 as default multiplier.
func (w *Widget) EffectiveLineHeight() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	multiplier := w.lineHeight
	if multiplier == 0 {
		multiplier = 1.2 // Default line height
	}
	return w.fontSize * multiplier
}

// SetFontName sets the font family name directly (bypasses theme resolution).
// FontName returns the resolved font name for rendering.
func (w *Widget) FontName() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.fontName
}

// SetFontName sets the direct font name for rendering (system font name).
// For theme-based fonts, use SetFontFamily with font-sans, font-serif, etc. classes.
func (w *Widget) SetFontName(name string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.fontName != name {
		w.fontName = name
		w.textWidthDirty = true
		w.markDirty(DirtyText)
	}
	return w
}

// FontFamily returns the theme font family key (sans, serif, mono, custom).
func (w *Widget) FontFamily() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.fontFamily
}

// SetFontFamily sets the theme font family key.
// The actual font name/path is resolved at render time via ThemeFonts().
// Example: "sans", "serif", "mono", or custom names from theme.toml.
func (w *Widget) SetFontFamily(family string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.fontFamily != family {
		w.fontFamily = family
		w.textWidthDirty = true
		w.markDirty(DirtyText)
	}
	return w
}

// TextWidth returns the cached text width in pixels, measuring if needed.
// This method calls FFI to measure text width if the cache is dirty.
// The result is cached and only re-measured when text or font properties change.
func (w *Widget) TextWidth() float32 {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.textWidthLocked()
}

// textWidthLocked returns the cached text width, measuring if needed.
// Must be called with w.mu held.
func (w *Widget) textWidthLocked() float32 {
	if w.text == "" || w.fontSize == 0 {
		return 0
	}

	if w.textWidthDirty || w.textWidth == 0 {
		w.measureTextWidthLocked()
	}
	return w.textWidth
}

// measureTextWidthLocked performs the actual text measurement.
// Must be called with w.mu held.
func (w *Widget) measureTextWidthLocked() {
	// Use the extended measurement function that supports font families
	w.textWidth = measureTextWidthExtFunc(w.text, w.fontName, w.fontFamily, w.fontSize)
	w.textWidthDirty = false
}

// measureTextWidthFunc is the function used to measure text width (legacy, system fonts only).
// This can be swapped out for testing or for a shared memory implementation.
var measureTextWidthFunc = defaultMeasureTextWidth

// measureTextWidthExtFunc is the extended measurement function that supports font families.
// It resolves fontFamily through ThemeFonts() to get the actual font source.
var measureTextWidthExtFunc = defaultMeasureTextWidthExt

// defaultMeasureTextWidth uses the FFI to measure text width (legacy).
func defaultMeasureTextWidth(text string, fontName string, fontSize float32) float32 {
	// This will be implemented in layout.go which imports ffi
	// For now, return 0 - the actual implementation will be provided by the ffi package
	return 0
}

// defaultMeasureTextWidthExt measures text with font family support.
func defaultMeasureTextWidthExt(text string, fontName string, fontFamily string, fontSize float32) float32 {
	// Default implementation falls back to basic measurement
	return measureTextWidthFunc(text, fontName, fontSize)
}

// SetMeasureTextWidthFunc allows setting a custom text measurement function.
// This is useful for testing or for transitioning to a shared memory approach.
func SetMeasureTextWidthFunc(fn func(text string, fontName string, fontSize float32) float32) {
	measureTextWidthFunc = fn
}

// SetMeasureTextWidthExtFunc allows setting the extended text measurement function
// that supports font families (bundled fonts, theme fonts).
func SetMeasureTextWidthExtFunc(fn func(text string, fontName string, fontFamily string, fontSize float32) float32) {
	measureTextWidthExtFunc = fn
}

// SetScroll sets the scroll offset.
func (w *Widget) SetScroll(x, y float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.scrollX != x || w.scrollY != y {
		w.scrollX, w.scrollY = x, y
		w.markDirty(DirtyScroll)
	}
	return w
}

// SetContentSize sets the total content dimensions for scroll calculations.
func (w *Widget) SetContentSize(width, height float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.contentWidth != width || w.contentHeight != height {
		w.contentWidth, w.contentHeight = width, height
		w.markDirty(DirtyScroll)
	}
	return w
}

// SetData sets custom application data.
func (w *Widget) SetData(data any) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.data = data
	return w
}

// ============================================================================
// Image Properties
// ============================================================================

// SetImageSource sets the image source path or URL.
// The image will be loaded automatically when rendered.
// For bundled files: "assets/icon.png"
// For URLs: "https://example.com/image.png"
func (w *Widget) SetImageSource(source string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.imageSource != source {
		w.imageSource = source
		// Reset loading state when source changes
		w.imageTextureID = 0
		w.imageLoading = false
		w.imageError = nil
		w.markDirty(DirtyVisible)
	}
	return w
}

// SetImageTextureID sets the GPU texture ID directly (for pre-loaded images).
func (w *Widget) SetImageTextureID(textureID uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.imageTextureID = textureID
	w.imageSource = "" // Clear source since we have a direct texture
	w.imageLoading = false
	w.imageError = nil
	w.markDirty(DirtyVisible)
	return w
}

// SetImageFit sets how the image fits within its bounds.
// Values: "contain" (default), "cover", "fill", "none"
func (w *Widget) SetImageFit(fit string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.imageFit = fit
	return w
}

// SetImageSourceRect sets the source rectangle for sprite sheet rendering.
// Pass (0,0,0,0) to use the full image.
func (w *Widget) SetImageSourceRect(x, y, w2, h float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.imageSourceX = x
	w.imageSourceY = y
	w.imageSourceW = w2
	w.imageSourceH = h
	return w
}

// ImageSource returns the image source path or URL.
func (w *Widget) ImageSource() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.imageSource
}

// ImageTextureID returns the loaded GPU texture ID, or 0 if not loaded.
func (w *Widget) ImageTextureID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.imageTextureID
}

// ImageLoading returns true if the image is currently loading.
func (w *Widget) ImageLoading() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.imageLoading
}

// ImageError returns the error if the image failed to load.
func (w *Widget) ImageError() error {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.imageError
}

// ImageFit returns how the image fits in its bounds.
func (w *Widget) ImageFit() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.imageFit == "" {
		return "contain"
	}
	return w.imageFit
}

// SetImagePosition sets where the image is positioned within its container.
// Values: "center" (default), "top", "bottom", "left", "right", "left-top", etc.
func (w *Widget) SetImagePosition(pos string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.imagePosition = pos
	return w
}

// ImagePosition returns where the image is positioned.
func (w *Widget) ImagePosition() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.imagePosition == "" {
		return "center"
	}
	return w.imagePosition
}

// ============================================================================
// Video Properties
// ============================================================================

// SetVideoSource sets the video source URL or file path.
func (w *Widget) SetVideoSource(source string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.videoSource = source
	w.dirty = true
	return w
}

// VideoSource returns the video source URL or file path.
func (w *Widget) VideoSource() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoSource
}

// VideoPlayerID returns the video player ID (0 if not created yet).
func (w *Widget) VideoPlayerID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoPlayerID
}

// VideoTextureID returns the current video texture ID for rendering.
func (w *Widget) VideoTextureID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoTextureID
}

// SetTextureID sets a texture ID directly for rendering.
// This allows displaying frames from external sources like Camera widgets.
// When a texture ID is set this way, video file playback is bypassed.
func (w *Widget) SetTextureID(textureID uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.videoTextureID = textureID
	return w
}

// VideoState returns the current playback state (0=Idle, 1=Loading, 2=Playing, 3=Paused, 4=Ended, 5=Error).
func (w *Widget) VideoState() int32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoState
}

// VideoLoading returns true if the video is still loading.
func (w *Widget) VideoLoading() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoLoading
}

// VideoError returns the error if video failed to load.
func (w *Widget) VideoError() error {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoError
}

// VideoDuration returns the video duration in milliseconds.
func (w *Widget) VideoDuration() uint64 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoDurationMs
}

// VideoNaturalSize returns the video's native width and height.
func (w *Widget) VideoNaturalSize() (width, height uint32) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoNaturalW, w.videoNaturalH
}

// SetVideoAutoplay sets whether the video should autoplay when loaded.
func (w *Widget) SetVideoAutoplay(autoplay bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.videoAutoplay = autoplay
	return w
}

// SetVideoLoop sets whether the video should loop.
func (w *Widget) SetVideoLoop(loop bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.videoLoop = loop
	return w
}

// SetVideoMuted sets whether the video is muted.
func (w *Widget) SetVideoMuted(muted bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.videoMuted = muted
	return w
}

// SetVideoVolume sets the audio volume (0.0 - 1.0).
func (w *Widget) SetVideoVolume(volume float32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.videoVolume = volume
	return w
}

// VideoVolume returns the current audio volume.
func (w *Widget) VideoVolume() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoVolume
}

// IsVideoAutoplay returns true if autoplay is enabled.
func (w *Widget) IsVideoAutoplay() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoAutoplay
}

// IsVideoLoop returns true if looping is enabled.
func (w *Widget) IsVideoLoop() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoLoop
}

// IsVideoMuted returns true if video is muted.
func (w *Widget) IsVideoMuted() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.videoMuted
}

// ============================================================================
// Video Playback Control
// ============================================================================

// VideoPlay starts or resumes video playback.
func (w *Widget) VideoPlay() error {
	w.mu.Lock()
	playerID := w.videoPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("video not loaded")
	}
	return ffi.VideoPlay(ffi.VideoPlayerID(playerID))
}

// VideoPause pauses video playback.
func (w *Widget) VideoPause() error {
	w.mu.Lock()
	playerID := w.videoPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("video not loaded")
	}
	return ffi.VideoPause(ffi.VideoPlayerID(playerID))
}

// VideoSeek seeks to a specific position in milliseconds.
func (w *Widget) VideoSeek(timestampMs uint64) error {
	w.mu.Lock()
	playerID := w.videoPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("video not loaded")
	}
	return ffi.VideoSeek(ffi.VideoPlayerID(playerID), timestampMs)
}

// VideoCurrentTime returns the current playback position in milliseconds.
func (w *Widget) VideoCurrentTime() uint64 {
	w.mu.RLock()
	playerID := w.videoPlayerID
	w.mu.RUnlock()

	if playerID == 0 {
		return 0
	}
	return ffi.VideoGetTime(ffi.VideoPlayerID(playerID))
}

// VideoIsPlaying returns true if video is currently playing.
func (w *Widget) VideoIsPlaying() bool {
	w.mu.RLock()
	state := w.videoState
	w.mu.RUnlock()
	return state == 2 // PlaybackState::Playing
}

// VideoIsPaused returns true if video is paused.
func (w *Widget) VideoIsPaused() bool {
	w.mu.RLock()
	state := w.videoState
	w.mu.RUnlock()
	return state == 3 // PlaybackState::Paused
}

// VideoHasEnded returns true if video has finished playing.
func (w *Widget) VideoHasEnded() bool {
	w.mu.RLock()
	state := w.videoState
	w.mu.RUnlock()
	return state == 4 // PlaybackState::Ended
}

// VideoPushFrame pushes a raw video frame for streaming video.
// Used for WebRTC, camera input, or other live sources.
func (w *Widget) VideoPushFrame(width, height uint32, data []byte, timestampMs uint64) error {
	w.mu.Lock()
	playerID := w.videoPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("video stream not initialized")
	}
	return ffi.VideoPushFrame(ffi.VideoPlayerID(playerID), width, height, data, timestampMs)
}

// ============================================================================
// Audio Properties
// ============================================================================

// SetAudioSource sets the audio source URL or file path.
func (w *Widget) SetAudioSource(source string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.audioSource = source
	w.dirty = true
	return w
}

// AudioSource returns the audio source URL or file path.
func (w *Widget) AudioSource() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioSource
}

// AudioPlayerID returns the audio player ID (0 if not created yet).
func (w *Widget) AudioPlayerID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioPlayerID
}

// AudioState returns the current playback state (0=Idle, 1=Loading, 2=Playing, 3=Paused, 4=Ended, 5=Error).
func (w *Widget) AudioState() int32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioState
}

// AudioLoading returns true if the audio is still loading.
func (w *Widget) AudioLoading() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioLoading
}

// AudioError returns the error if audio failed to load.
func (w *Widget) AudioError() error {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioError
}

// AudioDuration returns the audio duration in milliseconds.
func (w *Widget) AudioDuration() uint64 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioDurationMs
}

// AudioSampleRate returns the sample rate in Hz.
func (w *Widget) AudioSampleRate() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioSampleRate
}

// AudioChannels returns the number of audio channels.
func (w *Widget) AudioChannels() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioChannels
}

// SetAudioAutoplay sets whether the audio should autoplay when loaded.
func (w *Widget) SetAudioAutoplay(autoplay bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.audioAutoplay = autoplay
	return w
}

// SetAudioLoop sets whether the audio should loop.
// If audio is already loaded, the loop setting takes effect immediately.
func (w *Widget) SetAudioLoop(loop bool) *Widget {
	w.mu.Lock()
	w.audioLoop = loop
	playerID := w.audioPlayerID
	w.mu.Unlock()

	// If player is already loaded, update looping immediately
	if playerID != 0 {
		ffi.AudioSetLooping(ffi.AudioPlayerID(playerID), loop)
	}
	return w
}

// SetAudioVolume sets the audio volume (0.0 - 1.0).
// If audio is already loaded, the volume change takes effect immediately.
func (w *Widget) SetAudioVolume(volume float32) *Widget {
	w.mu.Lock()
	w.audioVolume = volume
	playerID := w.audioPlayerID
	w.mu.Unlock()

	// If player is already loaded, update volume immediately
	if playerID != 0 {
		ffi.AudioSetVolume(ffi.AudioPlayerID(playerID), volume)
	}
	return w
}

// AudioVolume returns the current audio volume.
func (w *Widget) AudioVolume() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioVolume
}

// IsAudioAutoplay returns true if autoplay is enabled.
func (w *Widget) IsAudioAutoplay() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioAutoplay
}

// IsAudioLoop returns true if looping is enabled.
func (w *Widget) IsAudioLoop() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.audioLoop
}

// ============================================================================
// Audio Playback Control
// ============================================================================

// AudioPlay starts or resumes audio playback.
func (w *Widget) AudioPlay() error {
	w.mu.Lock()
	playerID := w.audioPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("audio not loaded")
	}
	return ffi.AudioPlay(ffi.AudioPlayerID(playerID))
}

// AudioPause pauses audio playback.
func (w *Widget) AudioPause() error {
	w.mu.Lock()
	playerID := w.audioPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("audio not loaded")
	}
	return ffi.AudioPause(ffi.AudioPlayerID(playerID))
}

// AudioStop stops audio playback and resets to beginning.
func (w *Widget) AudioStop() error {
	w.mu.Lock()
	playerID := w.audioPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("audio not loaded")
	}
	return ffi.AudioStop(ffi.AudioPlayerID(playerID))
}

// AudioSeek seeks to a specific position in milliseconds.
func (w *Widget) AudioSeek(timestampMs uint64) error {
	w.mu.Lock()
	playerID := w.audioPlayerID
	w.mu.Unlock()

	if playerID == 0 {
		return fmt.Errorf("audio not loaded")
	}
	return ffi.AudioSeek(ffi.AudioPlayerID(playerID), timestampMs)
}

// AudioCurrentTime returns the current playback position in milliseconds.
func (w *Widget) AudioCurrentTime() uint64 {
	w.mu.RLock()
	playerID := w.audioPlayerID
	w.mu.RUnlock()

	if playerID == 0 {
		return 0
	}
	return ffi.AudioGetTime(ffi.AudioPlayerID(playerID))
}

// AudioIsPlaying returns true if audio is currently playing.
func (w *Widget) AudioIsPlaying() bool {
	w.mu.RLock()
	state := w.audioState
	w.mu.RUnlock()
	return state == 2 // PlaybackState::Playing
}

// AudioIsPaused returns true if audio is paused.
func (w *Widget) AudioIsPaused() bool {
	w.mu.RLock()
	state := w.audioState
	w.mu.RUnlock()
	return state == 3 // PlaybackState::Paused
}

// AudioHasEnded returns true if audio has finished playing.
func (w *Widget) AudioHasEnded() bool {
	w.mu.RLock()
	state := w.audioState
	w.mu.RUnlock()
	return state == 4 // PlaybackState::Ended
}

// ============================================================================
// Microphone Properties
// ============================================================================

// MicrophoneInputID returns the microphone input ID (0 if not created yet).
func (w *Widget) MicrophoneInputID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.micInputID
}

// MicrophoneState returns the current input state (0=Idle, 1=RequestingPermission, 2=Ready, 3=Capturing, 4=Stopped, 5=Error).
func (w *Widget) MicrophoneState() int32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.micState
}

// MicrophoneLevel returns the current audio input level (0.0 - 1.0 RMS).
func (w *Widget) MicrophoneLevel() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.micLevel
}

// SetMicrophoneDevice sets the device ID to use (empty string for default).
func (w *Widget) SetMicrophoneDevice(deviceID string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.micDeviceID = deviceID
	w.dirty = true
	return w
}

// MicrophoneDevice returns the configured device ID.
func (w *Widget) MicrophoneDevice() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.micDeviceID
}

// SetMicrophoneSampleRate sets the sample rate in Hz (0 for default 44100).
func (w *Widget) SetMicrophoneSampleRate(rate uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.micSampleRate = rate
	return w
}

// SetMicrophoneChannels sets the number of channels (0 for default 1 = mono).
func (w *Widget) SetMicrophoneChannels(channels uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.micChannels = channels
	return w
}

// SetMicrophoneAutoStart sets whether to auto-start capturing when ready.
func (w *Widget) SetMicrophoneAutoStart(autoStart bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.micAutoStart = autoStart
	return w
}

// IsMicrophoneAutoStart returns true if auto-start is enabled.
func (w *Widget) IsMicrophoneAutoStart() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.micAutoStart
}

// OnMicrophoneError sets the callback for microphone errors.
func (w *Widget) OnMicrophoneError(handler func(error)) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMicError = handler
	return w
}

// OnMicrophoneLevelChange sets the callback for audio level changes.
// The handler receives the current RMS level (0.0 - 1.0).
func (w *Widget) OnMicrophoneLevelChange(handler func(float32)) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMicLevelChange = handler
	return w
}

// OnMicrophoneStateChange sets the callback for state changes.
func (w *Widget) OnMicrophoneStateChange(handler func(int32)) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMicStateChange = handler
	return w
}

// ============================================================================
// Microphone Control
// ============================================================================

// MicrophoneStart starts capturing audio from the microphone.
func (w *Widget) MicrophoneStart() error {
	w.mu.Lock()
	inputID := w.micInputID
	w.mu.Unlock()

	if inputID == 0 {
		return fmt.Errorf("microphone not initialized")
	}
	return ffi.AudioInputStart(ffi.AudioInputID(inputID))
}

// MicrophoneStop stops capturing audio.
func (w *Widget) MicrophoneStop() error {
	w.mu.Lock()
	inputID := w.micInputID
	w.mu.Unlock()

	if inputID == 0 {
		return fmt.Errorf("microphone not initialized")
	}
	return ffi.AudioInputStop(ffi.AudioInputID(inputID))
}

// MicrophoneIsCapturing returns true if currently capturing audio.
func (w *Widget) MicrophoneIsCapturing() bool {
	w.mu.RLock()
	state := w.micState
	w.mu.RUnlock()
	return state == 3 // AudioInputState::Capturing
}

// MicrophoneHasPermission returns true if microphone permission is granted.
func (w *Widget) MicrophoneHasPermission() bool {
	w.mu.RLock()
	inputID := w.micInputID
	w.mu.RUnlock()

	if inputID == 0 {
		return false
	}
	return ffi.AudioInputHasPermission(ffi.AudioInputID(inputID))
}

// ============================================================================
// Camera Properties
// ============================================================================

// CameraInputID returns the camera input ID (0 if not created yet).
func (w *Widget) CameraInputID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.camInputID
}

// CameraState returns the current input state (0=Idle, 1=RequestingPermission, 2=Ready, 3=Capturing, 4=Stopped, 5=Error).
func (w *Widget) CameraState() int32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.camState
}

// CameraTextureID returns the current camera frame texture ID for rendering.
func (w *Widget) CameraTextureID() uint32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.camTextureID
}

// CameraDimensions returns the actual capture width and height.
func (w *Widget) CameraDimensions() (width, height uint32) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.camActualWidth, w.camActualHeight
}

// SetCameraDevice sets the device ID to use (empty string for default).
func (w *Widget) SetCameraDevice(deviceID string) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.camDeviceID = deviceID
	w.dirty = true
	return w
}

// CameraDevice returns the configured device ID.
func (w *Widget) CameraDevice() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.camDeviceID
}

// SetCameraResolution sets the preferred resolution.
func (w *Widget) SetCameraResolution(width, height uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.camWidth = width
	w.camHeight = height
	return w
}

// SetCameraFrameRate sets the preferred frame rate.
func (w *Widget) SetCameraFrameRate(frameRate uint32) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.camFrameRate = frameRate
	return w
}

// SetCameraAutoStart sets whether to auto-start capturing when ready.
func (w *Widget) SetCameraAutoStart(autoStart bool) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.camAutoStart = autoStart
	return w
}

// IsCameraAutoStart returns true if auto-start is enabled.
func (w *Widget) IsCameraAutoStart() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.camAutoStart
}

// OnCameraError sets the callback for camera errors.
func (w *Widget) OnCameraError(handler func(error)) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onCamError = handler
	return w
}

// OnCameraStateChange sets the callback for state changes.
func (w *Widget) OnCameraStateChange(handler func(int32)) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onCamStateChange = handler
	return w
}

// OnCameraFrame sets the callback for when a new frame is available.
// The callback receives the texture ID which can be used with Video.SetTextureID().
func (w *Widget) OnCameraFrame(handler func(textureID uint32)) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onCamFrame = handler
	return w
}

// ============================================================================
// Camera Control
// ============================================================================

// CameraStart starts capturing video from the camera.
func (w *Widget) CameraStart() error {
	w.mu.Lock()
	inputID := w.camInputID
	w.mu.Unlock()

	if inputID == 0 {
		return fmt.Errorf("camera not initialized")
	}
	return ffi.VideoInputStart(ffi.VideoInputID(inputID))
}

// CameraStop stops capturing video.
func (w *Widget) CameraStop() error {
	w.mu.Lock()
	inputID := w.camInputID
	w.mu.Unlock()

	if inputID == 0 {
		return fmt.Errorf("camera not initialized")
	}
	return ffi.VideoInputStop(ffi.VideoInputID(inputID))
}

// CameraIsCapturing returns true if currently capturing video.
func (w *Widget) CameraIsCapturing() bool {
	w.mu.RLock()
	state := w.camState
	w.mu.RUnlock()
	return state == 3 // VideoInputState::Capturing
}

// CameraHasPermission returns true if camera permission is granted.
func (w *Widget) CameraHasPermission() bool {
	w.mu.RLock()
	inputID := w.camInputID
	w.mu.RUnlock()

	if inputID == 0 {
		return false
	}
	return ffi.VideoInputHasPermission(ffi.VideoInputID(inputID))
}

// ============================================================================
// Device Enumeration (Package-level functions)
// ============================================================================

// ListMicrophoneDevices returns a list of available microphone devices.
func ListMicrophoneDevices() ([]ffi.AudioInputDevice, error) {
	// Create temporary input to enumerate devices
	inputID := ffi.AudioInputCreate()
	if inputID == 0 {
		return nil, fmt.Errorf("failed to create audio input")
	}
	defer ffi.AudioInputDestroy(inputID)

	return ffi.AudioInputListDevices(inputID)
}

// ListCameraDevices returns a list of available camera devices.
func ListCameraDevices() ([]ffi.VideoInputDevice, error) {
	// Create temporary input to enumerate devices
	inputID := ffi.VideoInputCreate()
	if inputID == 0 {
		return nil, fmt.Errorf("failed to create video input")
	}
	defer ffi.VideoInputDestroy(inputID)

	return ffi.VideoInputListDevices(inputID)
}

// ============================================================================
// Property Getters
// ============================================================================

// Position returns x and y coordinates.
func (w *Widget) Position() (x, y float32) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.x, w.y
}

// Size returns width and height.
func (w *Widget) Size() (width, height float32) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.width, w.height
}

// Frame returns position and size.
func (w *Widget) Frame() (x, y, width, height float32) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.x, w.y, w.width, w.height
}

// BackgroundColor returns the background color, or 0 if not set.
func (w *Widget) BackgroundColor() (color uint32, ok bool) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.backgroundColor != nil {
		return *w.backgroundColor, true
	}
	return 0, false
}

// Text returns the text content.
func (w *Widget) Text() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.text
}

// Scroll returns the scroll offset.
func (w *Widget) Scroll() (x, y float32) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.scrollX, w.scrollY
}

// Data returns the custom application data.
func (w *Widget) Data() any {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.data
}

// TextBuffer returns the text buffer for text input widgets.
// Returns nil for non-text-input widgets.
func (w *Widget) TextBuffer() *TextBuffer {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.textBuffer
}

// IsDirty returns whether the widget has pending changes.
func (w *Widget) IsDirty() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.dirty
}

// DirtyMask returns the bitmask of changed properties.
func (w *Widget) DirtyMask() uint64 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.dirtyMask
}

// ClearDirty resets the dirty state (called after sync to renderer).
func (w *Widget) ClearDirty() {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.dirty = false
	w.dirtyMask = 0
}

// ============================================================================
// Event Handler Setters (Simple Callback API)
// ============================================================================

// OnClick sets the click handler.
func (w *Widget) OnClick(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onClick = handler
	return w
}

// OnDoubleClick sets the double-click handler.
func (w *Widget) OnDoubleClick(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onDoubleClick = handler
	return w
}

// OnTripleClick sets the triple-click handler.
func (w *Widget) OnTripleClick(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onTripleClick = handler
	return w
}

// OnMouseDown sets the mouse down handler.
func (w *Widget) OnMouseDown(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMouseDown = handler
	return w
}

// OnMouseUp sets the mouse up handler.
func (w *Widget) OnMouseUp(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMouseUp = handler
	return w
}

// OnMouseEnter sets the mouse enter handler.
func (w *Widget) OnMouseEnter(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMouseEnter = handler
	return w
}

// OnMouseLeave sets the mouse leave handler.
func (w *Widget) OnMouseLeave(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMouseLeave = handler
	return w
}

// OnMouseMove sets the mouse move handler.
func (w *Widget) OnMouseMove(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMouseMove = handler
	return w
}

// OnMouseWheel sets the mouse wheel handler.
func (w *Widget) OnMouseWheel(handler MouseHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onMouseWheel = handler
	return w
}

// OnKeyDown sets the key down handler.
func (w *Widget) OnKeyDown(handler KeyHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onKeyDown = handler
	return w
}

// OnKeyUp sets the key up handler.
func (w *Widget) OnKeyUp(handler KeyHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onKeyUp = handler
	return w
}

// OnKeyPress sets the key press (character input) handler.
func (w *Widget) OnKeyPress(handler KeyHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onKeyPress = handler
	return w
}

// OnFocus sets the focus handler.
func (w *Widget) OnFocus(handler FocusHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onFocus = handler
	return w
}

// OnBlur sets the blur (focus lost) handler.
func (w *Widget) OnBlur(handler FocusHandler) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.onBlur = handler
	return w
}

// ============================================================================
// Interactive State
// ============================================================================

// IsHovered returns true if the mouse is over this widget.
func (w *Widget) IsHovered() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.hovered
}

// IsFocused returns true if this widget has keyboard focus.
func (w *Widget) IsFocused() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.focused
}

// IsPressed returns true if the mouse button is down on this widget.
func (w *Widget) IsPressed() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.pressed
}

// IsDisabled returns true if this widget is disabled.
func (w *Widget) IsDisabled() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.disabled
}

// SetDisabled sets whether the widget is disabled.
func (w *Widget) SetDisabled(disabled bool) *Widget {
	w.mu.Lock()
	changed := w.disabled != disabled
	w.disabled = disabled
	w.mu.Unlock()
	if changed {
		w.updateStyleState()
	}
	return w
}

// setHovered sets the hover state (called by event dispatcher).
func (w *Widget) setHovered(hovered bool) {
	w.mu.Lock()
	changed := w.hovered != hovered
	w.hovered = hovered
	w.mu.Unlock()
	if changed {
		w.updateStyleState()
	}
}

// setFocused sets the focus state (called by event dispatcher).
func (w *Widget) setFocused(focused bool) {
	w.mu.Lock()
	changed := w.focused != focused
	w.focused = focused
	w.mu.Unlock()
	if changed {
		w.updateStyleState()
	}
}

// setPressed sets the pressed state (called by event dispatcher).
func (w *Widget) setPressed(pressed bool) {
	w.mu.Lock()
	changed := w.pressed != pressed
	w.pressed = pressed
	w.mu.Unlock()
	if changed {
		w.updateStyleState()
	}
}

// updateStyleState applies the appropriate styles based on current interaction state.
// Priority: disabled > active (pressed) > focus > hover > default
func (w *Widget) updateStyleState() {
	w.mu.RLock()
	styles := w.computedStyles
	disabled := w.disabled
	pressed := w.pressed
	focused := w.focused
	hovered := w.hovered
	w.mu.RUnlock()

	// No styles to apply
	if styles == nil {
		return
	}

	// Determine the effective state based on priority
	var state tw.State
	switch {
	case disabled:
		state = tw.StateDisabled
	case pressed:
		state = tw.StateActive
	case focused:
		state = tw.StateFocus
	case hovered:
		state = tw.StateHover
	default:
		state = tw.StateDefault
	}

	// Apply styles for this state (UpdateState handles lock internally)
	w.UpdateState(state)
}

// ============================================================================
// Computed Bounds (for hit testing)
// ============================================================================

// ComputedBounds returns the cached screen-space bounds.
func (w *Widget) ComputedBounds() Bounds {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.computedBounds
}

// updateBounds updates the cached screen-space bounds during rendering.
// Called internally by the render loop.
func (w *Widget) updateBounds(x, y, width, height float32, frame uint64) {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.computedBounds = Bounds{X: x, Y: y, Width: width, Height: height}
	w.boundsFrame = frame
}

// ============================================================================
// Responder Interface Implementation
// ============================================================================

// HandleEvent processes an event during the given phase.
// This is the default implementation that dispatches to callback handlers.
// Custom components can embed Widget and override this method.
func (w *Widget) HandleEvent(event Event, phase EventPhase) bool {
	// Only handle events during bubble phase by default
	// (capture phase is for parent interception)
	if phase != PhaseBubble && phase != PhaseTarget {
		return false
	}

	// First, let behaviors handle the event
	w.mu.RLock()
	behaviors := w.behaviors
	w.mu.RUnlock()

	for _, b := range behaviors {
		if b.HandleEvent(w, event, phase) {
			return true
		}
	}

	// Then dispatch to callback handlers based on event type
	switch e := event.(type) {
	case *MouseEvent:
		return w.handleMouseEvent(e)
	case *KeyEvent:
		return w.handleKeyEvent(e)
	case *FocusEvent:
		return w.handleFocusEvent(e)
	}

	return false
}

// handleMouseEvent dispatches to the appropriate mouse handler.
func (w *Widget) handleMouseEvent(e *MouseEvent) bool {
	w.mu.RLock()
	var handler MouseHandler
	switch e.Type() {
	case EventClick:
		handler = w.onClick
	case EventDoubleClick:
		handler = w.onDoubleClick
	case EventTripleClick:
		handler = w.onTripleClick
	case EventMouseDown:
		handler = w.onMouseDown
	case EventMouseUp:
		handler = w.onMouseUp
	case EventMouseEnter:
		handler = w.onMouseEnter
	case EventMouseLeave:
		handler = w.onMouseLeave
	case EventMouseMove:
		handler = w.onMouseMove
	case EventMouseWheel:
		handler = w.onMouseWheel
	}
	w.mu.RUnlock()

	if handler != nil {
		handler(e)
		return true
	}
	return false
}

// handleKeyEvent dispatches to the appropriate key handler.
func (w *Widget) handleKeyEvent(e *KeyEvent) bool {
	w.mu.RLock()
	var handler KeyHandler
	switch e.Type() {
	case EventKeyDown:
		handler = w.onKeyDown
	case EventKeyUp:
		handler = w.onKeyUp
	case EventKeyPress:
		handler = w.onKeyPress
	}
	w.mu.RUnlock()

	if handler != nil {
		handler(e)
		return true
	}
	return false
}

// handleFocusEvent dispatches to the appropriate focus handler.
func (w *Widget) handleFocusEvent(e *FocusEvent) bool {
	w.mu.RLock()
	var handler FocusHandler
	switch e.Type() {
	case EventFocus:
		handler = w.onFocus
	case EventBlur:
		handler = w.onBlur
	}
	w.mu.RUnlock()

	if handler != nil {
		handler(e)
		return true
	}
	return false
}

// HitTest returns true if this widget should receive events at the given
// local coordinates. Default implementation checks rectangular bounds.
func (w *Widget) HitTest(localX, localY float32) bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	// Use computed layout dimensions if available, otherwise use raw dimensions
	width := w.width
	height := w.height
	if w.computedLayout.Valid {
		width = w.computedLayout.Width
		height = w.computedLayout.Height
	}

	// Check main widget bounds
	inMainBounds := localX >= 0 && localX < width && localY >= 0 && localY < height

	// For Select widgets with open dropdown, also check dropdown area
	if w.kind == KindSelect && w.selectOpen && len(w.selectOptions) > 0 {
		optionHeight := float32(32)
		dropdownHeight := float32(len(w.selectOptions)) * optionHeight
		dropdownTop := height + 4 // Gap between trigger and dropdown
		dropdownBottom := dropdownTop + dropdownHeight

		// Check if in dropdown bounds
		inDropdown := localX >= 0 && localX < width &&
			localY >= dropdownTop && localY < dropdownBottom
		return inMainBounds || inDropdown
	}

	return inMainBounds
}

// CanReceiveEvents returns true if this widget can receive events.
func (w *Widget) CanReceiveEvents() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.visible && !w.disabled
}

// ============================================================================
// Behavior Management
// ============================================================================

// AddBehavior attaches a behavior to this widget.
func (w *Widget) AddBehavior(b Behavior) *Widget {
	w.mu.Lock()
	w.behaviors = append(w.behaviors, b)
	w.mu.Unlock()
	b.Attach(w)
	return w
}

// RemoveBehavior removes a behavior from this widget.
func (w *Widget) RemoveBehavior(b Behavior) *Widget {
	w.mu.Lock()
	for i, existing := range w.behaviors {
		if existing == b {
			w.behaviors = append(w.behaviors[:i], w.behaviors[i+1:]...)
			break
		}
	}
	w.mu.Unlock()
	b.Detach(w)
	return w
}

// SetResponder sets a custom responder for advanced event handling.
// If set, this takes precedence over the default HandleEvent implementation.
func (w *Widget) SetResponder(r Responder) *Widget {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.responder = r
	return w
}

// GetResponder returns the custom responder, or nil if not set.
func (w *Widget) GetResponder() Responder {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.responder
}

// ============================================================================
// FormControl Interface Implementation
// ============================================================================

// FormField registers this widget with a form under the given name.
// Validators are optional and run when form.Validate() is called.
// Returns the widget for chaining.
func (w *Widget) FormField(form *Form, name string, validators ...Validator) *Widget {
	form.RegisterField(name, w, validators...)
	return w
}

// FormValue returns the current value of this widget for form purposes.
// Implements FormControl interface.
func (w *Widget) FormValue() any {
	w.mu.RLock()
	defer w.mu.RUnlock()

	switch w.kind {
	case KindTextField, KindTextArea:
		// Text fields store their content in textBuffer, not w.text
		if w.textBuffer != nil {
			return w.textBuffer.Text()
		}
		return ""
	case KindCheckbox, KindToggle:
		// Both Checkbox and Toggle use the 'checked' field
		return w.checked
	case KindRadio:
		if w.checked {
			// Radio stores its value in the 'data' field
			return w.data
		}
		return nil
	case KindSlider:
		return w.sliderValue
	case KindSelect:
		if w.selectIndex >= 0 && w.selectIndex < len(w.selectOptions) {
			return w.selectOptions[w.selectIndex].Value
		}
		return nil
	default:
		return nil
	}
}

// SetFormValue sets the value programmatically for form purposes.
// Implements FormControl interface.
func (w *Widget) SetFormValue(value any) {
	switch w.kind {
	case KindTextField, KindTextArea:
		if s, ok := value.(string); ok {
			// For text fields, set the text buffer content
			w.mu.Lock()
			if w.textBuffer != nil {
				w.textBuffer.SetText(s)
			}
			w.mu.Unlock()
		}
	case KindCheckbox, KindToggle:
		// Both Checkbox and Toggle use SetChecked/SetOn (same underlying field)
		if b, ok := value.(bool); ok {
			w.SetChecked(b)
		}
	case KindSlider:
		switch v := value.(type) {
		case float32:
			w.SetSliderValue(v)
		case float64:
			w.SetSliderValue(float32(v))
		case int:
			w.SetSliderValue(float32(v))
		}
	case KindSelect:
		// Find the option with matching value and select it
		w.mu.Lock()
		for i, opt := range w.selectOptions {
			if opt.Value == value {
				w.selectIndex = i
				w.dirtyMask |= DirtyText
				break
			}
		}
		w.mu.Unlock()
	}
}

// OnFormChange registers a callback for value changes.
// Implements FormControl interface.
func (w *Widget) OnFormChange(callback func(value any)) {
	w.mu.RLock()
	kind := w.kind
	w.mu.RUnlock()

	// TextField and TextArea use TextBuffer's OnChange which receives a string
	if kind == KindTextField || kind == KindTextArea {
		w.OnInputChange(func(text string) {
			callback(text)
		})
		return
	}

	// Other controls use the widget's onChangeValue callback
	w.OnChange(callback)
}

// FormReset resets the widget to its default/zero value.
// Implements FormControl interface.
func (w *Widget) FormReset() {
	switch w.kind {
	case KindTextField, KindTextArea:
		// Clear the text buffer content
		w.mu.Lock()
		if w.textBuffer != nil {
			w.textBuffer.SetText("")
		}
		w.mu.Unlock()
	case KindCheckbox, KindToggle:
		// Both use SetChecked (Toggle's SetOn is an alias)
		w.SetChecked(false)
	case KindSlider:
		w.SetSliderValue(w.SliderMin())
	case KindSelect:
		w.SetSelectedIndex(-1)
	case KindRadio:
		w.SetChecked(false)
	}
}

// Ensure Widget implements FormControl at compile time
var _ FormControl = (*Widget)(nil)

// ============================================================================
// Clipboard Widget Methods
// ============================================================================

// ClipboardText returns the current text content of the system clipboard.
func (w *Widget) ClipboardText() string {
	return ffi.ClipboardGetString()
}

// SetClipboardText sets the text content of the system clipboard.
func (w *Widget) SetClipboardText(text string) {
	ffi.ClipboardSetString(text)
	w.mu.Lock()
	w.clipboardText = text
	w.mu.Unlock()
}

// ClipboardMonitor returns whether this widget monitors clipboard changes.
func (w *Widget) ClipboardMonitor() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.clipboardMonitor
}

// SetClipboardMonitor enables or disables clipboard change monitoring.
// When enabled, the OnClipboardChange callback will be called when
// the clipboard content changes.
func (w *Widget) SetClipboardMonitor(monitor bool) *Widget {
	w.mu.Lock()
	w.clipboardMonitor = monitor
	if monitor {
		// Cache current clipboard content
		w.clipboardText = ffi.ClipboardGetString()
	}
	w.mu.Unlock()
	return w
}

// OnClipboardChange sets the callback for clipboard content changes.
// The callback receives the new clipboard text content.
func (w *Widget) OnClipboardChange(cb func(text string)) *Widget {
	w.mu.Lock()
	w.onClipboardChange = cb
	w.mu.Unlock()
	return w
}

// ============================================================================
// FilePicker Widget Methods
// ============================================================================

// FilePickerTitle returns the dialog title.
func (w *Widget) FilePickerTitle() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.filePickerTitle
}

// SetFilePickerTitle sets the dialog title.
func (w *Widget) SetFilePickerTitle(title string) *Widget {
	w.mu.Lock()
	w.filePickerTitle = title
	w.mu.Unlock()
	return w
}

// FilePickerFilters returns the file type filters.
func (w *Widget) FilePickerFilters() []FileFilter {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.filePickerFilters
}

// SetFilePickerFilters sets the file type filters.
func (w *Widget) SetFilePickerFilters(filters []FileFilter) *Widget {
	w.mu.Lock()
	w.filePickerFilters = filters
	w.mu.Unlock()
	return w
}

// FilePickerMultiple returns whether multiple file selection is allowed.
func (w *Widget) FilePickerMultiple() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.filePickerMultiple
}

// SetFilePickerMultiple enables or disables multiple file selection.
func (w *Widget) SetFilePickerMultiple(multiple bool) *Widget {
	w.mu.Lock()
	w.filePickerMultiple = multiple
	w.mu.Unlock()
	return w
}

// FilePickerDirectory returns the initial directory.
func (w *Widget) FilePickerDirectory() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.filePickerDirectory
}

// SetFilePickerDirectory sets the initial directory for the dialog.
func (w *Widget) SetFilePickerDirectory(dir string) *Widget {
	w.mu.Lock()
	w.filePickerDirectory = dir
	w.mu.Unlock()
	return w
}

// FilePickerIsOpen returns whether the file picker dialog is currently open.
func (w *Widget) FilePickerIsOpen() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.filePickerDialogOpen
}

// OnFileSelect sets the callback for when files are selected.
func (w *Widget) OnFileSelect(cb func(paths []string)) *Widget {
	w.mu.Lock()
	w.onFileSelect = cb
	w.mu.Unlock()
	return w
}

// OnFileCancel sets the callback for when the dialog is cancelled.
func (w *Widget) OnFileCancel(cb func()) *Widget {
	w.mu.Lock()
	w.onFileCancel = cb
	w.mu.Unlock()
	return w
}

// convertFilters converts retained.FileFilter to ffi.FileFilter
func convertFilters(filters []FileFilter) []ffi.FileFilter {
	if filters == nil {
		return nil
	}
	result := make([]ffi.FileFilter, len(filters))
	for i, f := range filters {
		result[i] = ffi.FileFilter{
			Name:       f.Name,
			Extensions: f.Extensions,
		}
	}
	return result
}

// OpenFile opens the file picker dialog for selecting files.
// This blocks until the user makes a selection or cancels.
// Results are delivered via OnFileSelect/OnFileCancel callbacks.
// NOTE: Must be called from the main thread (e.g., from an OnClick handler).
func (w *Widget) OpenFile() error {
	w.mu.Lock()
	if w.filePickerDialogOpen {
		w.mu.Unlock()
		return fmt.Errorf("file picker dialog is already open")
	}
	w.filePickerDialogOpen = true
	title := w.filePickerTitle
	filters := w.filePickerFilters
	multiple := w.filePickerMultiple
	directory := w.filePickerDirectory
	onSelect := w.onFileSelect
	onCancel := w.onFileCancel
	w.mu.Unlock()

	// Convert filters to ffi type
	ffiFilters := convertFilters(filters)

	// Run dialog synchronously (must be on main thread for macOS)
	paths, ok := ffi.OpenFileDialog(title, directory, ffiFilters, multiple)

	w.mu.Lock()
	w.filePickerDialogOpen = false
	w.mu.Unlock()

	if ok && len(paths) > 0 {
		if onSelect != nil {
			onSelect(paths)
		}
	} else {
		if onCancel != nil {
			onCancel()
		}
	}

	return nil
}

// SaveFile opens the save file dialog.
// This blocks until the user makes a selection or cancels.
// Results are delivered via OnFileSelect/OnFileCancel callbacks.
// NOTE: Must be called from the main thread (e.g., from an OnClick handler).
func (w *Widget) SaveFile() error {
	w.mu.Lock()
	if w.filePickerDialogOpen {
		w.mu.Unlock()
		return fmt.Errorf("file picker dialog is already open")
	}
	w.filePickerDialogOpen = true
	title := w.filePickerTitle
	filters := w.filePickerFilters
	directory := w.filePickerDirectory
	onSelect := w.onFileSelect
	onCancel := w.onFileCancel
	w.mu.Unlock()

	// Convert filters to ffi type
	ffiFilters := convertFilters(filters)

	// Run dialog synchronously (must be on main thread for macOS)
	path, ok := ffi.SaveFileDialog(title, directory, ffiFilters)

	w.mu.Lock()
	w.filePickerDialogOpen = false
	w.mu.Unlock()

	if ok && path != "" {
		if onSelect != nil {
			onSelect([]string{path})
		}
	} else {
		if onCancel != nil {
			onCancel()
		}
	}

	return nil
}

// =============================================================================
// TrayIcon Widget Methods
// =============================================================================

// SetTrayIconFile sets the tray icon from a file path.
// The icon should be a template image (monochrome) for proper dark/light mode support.
func (w *Widget) SetTrayIconFile(path string) *Widget {
	w.mu.Lock()
	w.trayIconPath = path
	w.trayIconData = nil
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// SetTrayIconData sets the tray icon from raw image data (PNG bytes).
// The icon should be a template image (monochrome) for proper dark/light mode support.
func (w *Widget) SetTrayIconData(data []byte) *Widget {
	w.mu.Lock()
	w.trayIconData = data
	w.trayIconPath = ""
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// SetTrayTooltip sets the tooltip shown when hovering over the tray icon.
func (w *Widget) SetTrayTooltip(tooltip string) *Widget {
	w.mu.Lock()
	w.trayTooltip = tooltip
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// SetTrayTitle sets the title text shown in the menu bar (alternative to icon).
func (w *Widget) SetTrayTitle(title string) *Widget {
	w.mu.Lock()
	w.trayTitle = title
	w.markDirty(DirtyVisible)
	created := w.trayCreated
	w.mu.Unlock()

	// Apply title immediately if tray is already created
	if created {
		ffi.TrayIconSetTitle(title)
	}
	ffi.RequestRedraw()
	return w
}

// SetTrayMenu sets the menu items for the tray icon dropdown.
func (w *Widget) SetTrayMenu(items []MenuItem) *Widget {
	w.mu.Lock()
	w.trayMenu = items
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// AddTrayMenuItem adds a menu item to the tray icon dropdown.
func (w *Widget) AddTrayMenuItem(item MenuItem) *Widget {
	w.mu.Lock()
	w.trayMenu = append(w.trayMenu, item)
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// AddTraySeparator adds a separator line to the tray menu.
func (w *Widget) AddTraySeparator() *Widget {
	w.mu.Lock()
	w.trayMenu = append(w.trayMenu, MenuItem{Separator: true})
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// ClearTrayMenu removes all menu items from the tray icon.
func (w *Widget) ClearTrayMenu() *Widget {
	w.mu.Lock()
	w.trayMenu = nil
	w.trayMenuIndices = nil
	w.markDirty(DirtyVisible)
	w.mu.Unlock()
	ffi.RequestRedraw()
	return w
}

// SetTrayVisible shows or hides the tray icon.
func (w *Widget) SetTrayVisible(visible bool) *Widget {
	w.mu.Lock()
	w.trayVisible = visible
	w.markDirty(DirtyVisible)
	created := w.trayCreated
	w.mu.Unlock()

	// Apply visibility immediately if tray is already created
	if created {
		ffi.TrayIconSetVisible(visible)
		// If showing, reapply title since status item was recreated
		if visible {
			w.mu.RLock()
			title := w.trayTitle
			w.mu.RUnlock()
			ffi.TrayIconSetTitle(title)
		}
	}
	ffi.RequestRedraw()
	return w
}

// OnTrayClick sets the callback for when the tray icon is clicked.
func (w *Widget) OnTrayClick(cb func()) *Widget {
	w.mu.Lock()
	w.onTrayClick = cb
	w.mu.Unlock()
	return w
}

// TrayMenu returns the current menu items.
func (w *Widget) TrayMenu() []MenuItem {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.trayMenu
}

// IsTrayCreated returns whether the tray icon has been created.
func (w *Widget) IsTrayCreated() bool {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.trayCreated
}

// IsTrayVisible returns whether the tray icon is currently visible.
func (w *Widget) IsTrayVisible() bool {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.trayVisible
}
