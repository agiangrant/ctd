//go:build js && wasm

// Package ffi provides Go bindings to the Centered Rust engine for WebAssembly.
// This implementation uses syscall/js to communicate with the Rust WASM module
// via JavaScript bridge functions.
package ffi

import (
	"fmt"
	"syscall/js"
)

// ============================================================================
// JavaScript Bridge
// ============================================================================

var (
	jsGlobal     js.Value
	jsRustEngine js.Value
	jsDocument   js.Value
	jsWindow     js.Value

	// Current event handler
	currentHandler EventHandler

	// Dimensions
	currentWidth  float64
	currentHeight float64
	scaleFactor   float64 = 1.0

	// Animation loop control
	animationLoopRunning bool
	animationFrame       js.Func
)

func init() {
	jsGlobal = js.Global()
	jsWindow = jsGlobal.Get("window")
	jsDocument = jsGlobal.Get("document")
}

// initRustEngine initializes connection to the Rust WASM module
func initRustEngine() error {
	jsRustEngine = jsGlobal.Get("centeredEngine")
	if jsRustEngine.IsUndefined() {
		return fmt.Errorf("centeredEngine not found in global scope - ensure Rust WASM is loaded")
	}
	return nil
}

// ============================================================================
// Event Types
// ============================================================================

type EventType int32

const (
	EventReady                 EventType = 0
	EventRedrawRequested       EventType = 1
	EventResized               EventType = 2
	EventCloseRequested        EventType = 3
	EventMouseMoved            EventType = 4
	EventMousePressed          EventType = 5
	EventMouseReleased         EventType = 6
	EventMouseWheel            EventType = 7
	EventKeyPressed            EventType = 8
	EventKeyReleased           EventType = 9
	EventCharInput             EventType = 10
	EventTouchStart            EventType = 11
	EventTouchMove             EventType = 12
	EventTouchEnd              EventType = 13
	EventTouchCancel           EventType = 14
	EventKeyboardFrameChanged  EventType = 15
)

// Event represents a platform event
type Event struct {
	Type      EventType
	Data1     float64 // x, width, keycode
	Data2     float64 // y, height, modifiers
	Data3     float64 // button, scroll delta x
	Data4     float64 // scroll delta y
	Text      string  // for text input
	Timestamp float64
}

// Event accessor methods
func (e Event) Keycode() uint32         { return uint32(e.Data1) }
func (e Event) Modifiers() Modifiers    { return Modifiers(uint32(e.Data2)) }
func (e Event) HasShift() bool          { return e.Modifiers()&ModShift != 0 }
func (e Event) HasCtrl() bool           { return e.Modifiers()&ModCtrl != 0 }
func (e Event) HasAlt() bool            { return e.Modifiers()&ModAlt != 0 }
func (e Event) HasSuper() bool          { return e.Modifiers()&ModSuper != 0 }
func (e Event) Char() rune {
	if len(e.Text) > 0 {
		return []rune(e.Text)[0]
	}
	return 0
}
func (e Event) MouseX() float64         { return e.Data1 }
func (e Event) MouseY() float64         { return e.Data2 }
func (e Event) MouseButton() int        { return int(e.Data3) }
func (e Event) Width() float64          { return e.Data1 }
func (e Event) Height() float64         { return e.Data2 }
func (e Event) ScrollDelta() (float64, float64) { return e.Data3, e.Data4 }

// Modifiers for keyboard events
type Modifiers uint32

const (
	ModShift Modifiers = 1 << 0
	ModCtrl  Modifiers = 1 << 1
	ModAlt   Modifiers = 1 << 2
	ModSuper Modifiers = 1 << 3
)

// ============================================================================
// Keycodes
// ============================================================================

type Keycode uint32

const (
	KeyA         Keycode = 65
	KeyB         Keycode = 66
	KeyC         Keycode = 67
	KeyD         Keycode = 68
	KeyE         Keycode = 69
	KeyF         Keycode = 70
	KeyG         Keycode = 71
	KeyH         Keycode = 72
	KeyI         Keycode = 73
	KeyJ         Keycode = 74
	KeyK         Keycode = 75
	KeyL         Keycode = 76
	KeyM         Keycode = 77
	KeyN         Keycode = 78
	KeyO         Keycode = 79
	KeyP         Keycode = 80
	KeyQ         Keycode = 81
	KeyR         Keycode = 82
	KeyS         Keycode = 83
	KeyT         Keycode = 84
	KeyU         Keycode = 85
	KeyV         Keycode = 86
	KeyW         Keycode = 87
	KeyX         Keycode = 88
	KeyY         Keycode = 89
	KeyZ         Keycode = 90
	Key0         Keycode = 48
	Key1         Keycode = 49
	Key2         Keycode = 50
	Key3         Keycode = 51
	Key4         Keycode = 52
	Key5         Keycode = 53
	Key6         Keycode = 54
	Key7         Keycode = 55
	Key8         Keycode = 56
	Key9         Keycode = 57
	KeySpace     Keycode = 32
	KeyEnter     Keycode = 13
	KeyTab       Keycode = 9
	KeyBackspace Keycode = 8
	KeyDelete    Keycode = 46
	KeyEscape    Keycode = 27
	KeyLeft      Keycode = 37
	KeyUp        Keycode = 38
	KeyRight     Keycode = 39
	KeyDown      Keycode = 40
	KeyHome      Keycode = 36
	KeyEnd       Keycode = 35
	KeyPageUp    Keycode = 33
	KeyPageDown  Keycode = 34
	KeyF1        Keycode = 112
	KeyF2        Keycode = 113
	KeyF3        Keycode = 114
	KeyF4        Keycode = 115
	KeyF5        Keycode = 116
	KeyF6        Keycode = 117
	KeyF7        Keycode = 118
	KeyF8        Keycode = 119
	KeyF9        Keycode = 120
	KeyF10       Keycode = 121
	KeyF11       Keycode = 122
	KeyF12       Keycode = 123
)

// Key is an alias for Keycode
type Key = Keycode

// ============================================================================
// App Configuration
// ============================================================================

type AppConfig struct {
	Title     string
	Width     int
	Height    int
	Resizable bool
	TargetFPS int
}

func DefaultAppConfig() AppConfig {
	return AppConfig{
		Title:     "Centered",
		Width:     800,
		Height:    600,
		Resizable: true,
		TargetFPS: 60,
	}
}

// ============================================================================
// Frame Response
// ============================================================================

type FrameResponse struct {
	RequestRedraw    bool
	ImmediateCommands []RenderCommand
	Exit             bool
	RedrawAfterMs    uint32
}

// EventHandler is the callback type for event handling
type EventHandler func(event Event) FrameResponse

// ============================================================================
// Render Commands - Must match native FFI exactly for retained package
// ============================================================================

// RenderCommand represents a single rendering operation (tagged union style)
type RenderCommand struct {
	DrawRect        *DrawRectCmd        `json:"DrawRect,omitempty"`
	DrawText        *DrawTextCmd        `json:"DrawText,omitempty"`
	DrawImage       *DrawImageCmd       `json:"DrawImage,omitempty"`
	DrawShadow      *DrawShadowCmd      `json:"DrawShadow,omitempty"`
	Clear           *ClearCmd           `json:"Clear,omitempty"`
	PushClip        *PushClipCmd        `json:"PushClip,omitempty"`
	PopClip         *struct{}           `json:"PopClip,omitempty"`
	BeginScrollView *BeginScrollViewCmd `json:"BeginScrollView,omitempty"`
	EndScrollView   *struct{}           `json:"EndScrollView,omitempty"`
	SetOpacity      *float32            `json:"SetOpacity,omitempty"`
	// Web-specific extensions
	DrawVideo      *DrawVideoCmd      `json:"-"`
	DrawVideoInput *DrawVideoInputCmd `json:"-"`
}

type BeginScrollViewCmd struct {
	X             float32  `json:"x"`
	Y             float32  `json:"y"`
	Width         float32  `json:"width"`
	Height        float32  `json:"height"`
	ScrollX       float32  `json:"scroll_x"`
	ScrollY       float32  `json:"scroll_y"`
	ContentWidth  *float32 `json:"content_width"`
	ContentHeight *float32 `json:"content_height"`
}

type DrawRectCmd struct {
	X           float32    `json:"x"`
	Y           float32    `json:"y"`
	Width       float32    `json:"width"`
	Height      float32    `json:"height"`
	Color       uint32     `json:"color"`
	CornerRadii [4]float32 `json:"corner_radii"`
	Rotation    float32    `json:"rotation,omitempty"`
	Border      *Border    `json:"border,omitempty"`
	Gradient    *Gradient  `json:"gradient,omitempty"`
}

type DrawImageCmd struct {
	X           float32     `json:"x"`
	Y           float32     `json:"y"`
	Width       float32     `json:"width"`
	Height      float32     `json:"height"`
	TextureID   uint32      `json:"texture_id"`
	SourceRect  *[4]float32 `json:"source_rect,omitempty"`
	CornerRadii [4]float32  `json:"corner_radii,omitempty"`
}

type DrawVideoCmd struct {
	X           float32
	Y           float32
	Width       float32
	Height      float32
	VideoID     VideoPlayerID
	CornerRadii [4]float32
}

type DrawVideoInputCmd struct {
	X            float32
	Y            float32
	Width        float32
	Height       float32
	VideoInputID VideoInputID
	CornerRadii  [4]float32
}

type Border struct {
	Width float32 `json:"width"`
	Color uint32  `json:"color"`
	Style string  `json:"style"`
}

type Gradient struct {
	Linear *LinearGradient `json:"Linear,omitempty"`
	Radial *RadialGradient `json:"Radial,omitempty"`
}

type LinearGradient struct {
	Angle float32        `json:"angle"`
	Stops []GradientStop `json:"stops"`
}

type RadialGradient struct {
	CenterX float32        `json:"center_x"`
	CenterY float32        `json:"center_y"`
	Stops   []GradientStop `json:"stops"`
}

type GradientStop struct {
	Position float32 `json:"position"`
	Color    uint32  `json:"color"`
}

type DrawTextCmd struct {
	X      float32          `json:"x"`
	Y      float32          `json:"y"`
	Text   string           `json:"text"`
	Font   FontDescriptor   `json:"font"`
	Color  uint32           `json:"color"`
	Layout TextLayoutConfig `json:"layout"`
}

type DrawShadowCmd struct {
	X           float32    `json:"x"`
	Y           float32    `json:"y"`
	Width       float32    `json:"width"`
	Height      float32    `json:"height"`
	Blur        float32    `json:"blur"`
	Color       uint32     `json:"color"`
	OffsetX     float32    `json:"offset_x"`
	OffsetY     float32    `json:"offset_y"`
	CornerRadii [4]float32 `json:"corner_radii"`
}

type ClearCmd struct {
	R uint8 `json:"r"`
	G uint8 `json:"g"`
	B uint8 `json:"b"`
	A uint8 `json:"a"`
}

type PushClipCmd struct {
	X      float32 `json:"x"`
	Y      float32 `json:"y"`
	Width  float32 `json:"width"`
	Height float32 `json:"height"`
}

type TextureID = int32

type TextAlign string

const (
	TextAlignLeft    TextAlign = "Left"
	TextAlignCenter  TextAlign = "Center"
	TextAlignRight   TextAlign = "Right"
	TextAlignJustify TextAlign = "Justify"
)

type VerticalAlign string

const (
	VerticalAlignTop      VerticalAlign = "Top"
	VerticalAlignMiddle   VerticalAlign = "Middle"
	VerticalAlignBottom   VerticalAlign = "Bottom"
	VerticalAlignBaseline VerticalAlign = "Baseline"
)

type WordBreak string

const (
	WordBreakNormal    WordBreak = "Normal"
	WordBreakBreakAll  WordBreak = "BreakAll"
	WordBreakKeepAll   WordBreak = "KeepAll"
	WordBreakBreakWord WordBreak = "BreakWord"
)

type TextOverflow string

const (
	TextOverflowClip     TextOverflow = "Clip"
	TextOverflowEllipsis TextOverflow = "Ellipsis"
	TextOverflowWrap     TextOverflow = "Wrap"
)

type WhiteSpace string

const (
	WhiteSpaceNormal  WhiteSpace = "Normal"
	WhiteSpaceNoWrap  WhiteSpace = "NoWrap"
	WhiteSpacePre     WhiteSpace = "Pre"
	WhiteSpacePreWrap WhiteSpace = "PreWrap"
)

// ============================================================================
// Font Configuration
// ============================================================================

type FontDescriptor struct {
	Source FontSource `json:"source"`
	Weight uint16     `json:"weight"`
	Style  FontStyle  `json:"style"`
	Size   float32    `json:"size"`
}

type FontSource struct {
	System  *string `json:"System,omitempty"`
	Bundled *string `json:"Bundled,omitempty"`
}

type FontStyle string

const (
	FontStyleNormal FontStyle = "Normal"
	FontStyleItalic FontStyle = "Italic"
)

func SystemFont(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 400, Style: FontStyleNormal, Size: size}
}

func SystemFontBold(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 700, Style: FontStyleNormal, Size: size}
}

func SystemFontItalic(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 400, Style: FontStyleItalic, Size: size}
}

func SystemFontBoldItalic(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 700, Style: FontStyleItalic, Size: size}
}

func SystemFontWithWeight(name string, size float32, weight uint16) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: weight, Style: FontStyleNormal, Size: size}
}

func SystemFontLight(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 300, Style: FontStyleNormal, Size: size}
}

func SystemFontMedium(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 500, Style: FontStyleNormal, Size: size}
}

func SystemFontSemiBold(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 600, Style: FontStyleNormal, Size: size}
}

func SystemFontHeavy(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 800, Style: FontStyleNormal, Size: size}
}

func SystemFontBlack(name string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{System: &name}, Weight: 900, Style: FontStyleNormal, Size: size}
}

func BundledFont(path string, size float32) FontDescriptor {
	return FontDescriptor{Source: FontSource{Bundled: &path}, Weight: 400, Style: FontStyleNormal, Size: size}
}

func BundledFontWithWeight(path string, size float32, weight uint16) FontDescriptor {
	return FontDescriptor{Source: FontSource{Bundled: &path}, Weight: weight, Style: FontStyleNormal, Size: size}
}

// ============================================================================
// Text Layout Configuration
// ============================================================================

type TextLayoutConfig struct {
	MaxWidth      *float32      `json:"max_width,omitempty"`
	MaxHeight     *float32      `json:"max_height,omitempty"`
	MaxLines      *int          `json:"max_lines,omitempty"`
	LineHeight    float32       `json:"line_height"`
	LetterSpacing float32       `json:"letter_spacing"`
	WordSpacing   float32       `json:"word_spacing"`
	Alignment     TextAlign     `json:"alignment"`
	VerticalAlign VerticalAlign `json:"vertical_align"`
	WordBreak     WordBreak     `json:"word_break"`
	Overflow      TextOverflow  `json:"overflow"`
	WhiteSpace    WhiteSpace    `json:"white_space"`
}

func DefaultTextLayout() TextLayoutConfig {
	return TextLayoutConfig{
		LineHeight: 1.5, LetterSpacing: 0.0, WordSpacing: 0.0,
		Alignment: TextAlignLeft, VerticalAlign: VerticalAlignTop,
		WordBreak: WordBreakNormal, Overflow: TextOverflowWrap,
		WhiteSpace: WhiteSpaceNormal,
	}
}

func CenteredTextLayout() TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.Alignment = TextAlignCenter
	return layout
}

func WrappedTextLayout(maxWidth float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	return layout
}

func WrappedCenteredTextLayout(maxWidth float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.Alignment = TextAlignCenter
	return layout
}

func EllipsisTextLayout(maxWidth float32, maxLines int) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxLines = &maxLines
	layout.Overflow = TextOverflowEllipsis
	return layout
}

func SingleLineEllipsisLayout(maxWidth float32) TextLayoutConfig {
	maxLines := 1
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxLines = &maxLines
	layout.Overflow = TextOverflowEllipsis
	return layout
}

func EllipsisHeightLayout(maxWidth, maxHeight float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxHeight = &maxHeight
	layout.Overflow = TextOverflowEllipsis
	return layout
}

func ClippedTextLayout(maxWidth, maxHeight float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.MaxWidth = &maxWidth
	layout.MaxHeight = &maxHeight
	layout.Overflow = TextOverflowClip
	return layout
}

func SpacedTextLayout(letterSpacing, wordSpacing float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.LetterSpacing = letterSpacing
	layout.WordSpacing = wordSpacing
	return layout
}

func TrackingLayout(letterSpacing float32) TextLayoutConfig {
	layout := DefaultTextLayout()
	layout.LetterSpacing = letterSpacing
	return layout
}

// ============================================================================
// Render Command Builders (must match native ffi.go exactly)
// ============================================================================

func Clear(r, g, b, a uint8) RenderCommand {
	return RenderCommand{
		Clear: &ClearCmd{R: r, G: g, B: b, A: a},
	}
}

func Rect(x, y, width, height float32, color uint32) RenderCommand {
	return RenderCommand{
		DrawRect: &DrawRectCmd{
			X: x, Y: y, Width: width, Height: height,
			Color:       color,
			CornerRadii: [4]float32{0, 0, 0, 0},
		},
	}
}

func RoundedRect(x, y, width, height float32, color uint32, radius float32) RenderCommand {
	return RenderCommand{
		DrawRect: &DrawRectCmd{
			X: x, Y: y, Width: width, Height: height,
			Color:       color,
			CornerRadii: [4]float32{radius, radius, radius, radius},
		},
	}
}

func RoundedRectWithRadii(x, y, width, height float32, color uint32, radii [4]float32) RenderCommand {
	return RenderCommand{
		DrawRect: &DrawRectCmd{
			X: x, Y: y, Width: width, Height: height,
			Color: color, CornerRadii: radii,
		},
	}
}

func RoundedRectWithBorder(x, y, width, height float32, color uint32, radii [4]float32, border Border) RenderCommand {
	return RenderCommand{
		DrawRect: &DrawRectCmd{
			X: x, Y: y, Width: width, Height: height,
			Color: color, CornerRadii: radii, Border: &border,
		},
	}
}

func Shadow(x, y, width, height, blur float32, color uint32, offsetX, offsetY float32, radii [4]float32) RenderCommand {
	return RenderCommand{
		DrawShadow: &DrawShadowCmd{
			X: x, Y: y, Width: width, Height: height,
			Blur: blur, Color: color,
			OffsetX: offsetX, OffsetY: offsetY,
			CornerRadii: radii,
		},
	}
}

func Text(text string, x, y float32, size float32, color uint32) RenderCommand {
	fontName := "system"
	return RenderCommand{
		DrawText: &DrawTextCmd{
			X: x, Y: y, Text: text, Color: color,
			Font: FontDescriptor{
				Source: FontSource{System: &fontName},
				Weight: 400, Style: FontStyleNormal, Size: size,
			},
			Layout: DefaultTextLayout(),
		},
	}
}

func TextWithFont(text string, x, y float32, font FontDescriptor, color uint32) RenderCommand {
	return RenderCommand{
		DrawText: &DrawTextCmd{
			X: x, Y: y, Text: text, Color: color,
			Font: font, Layout: DefaultTextLayout(),
		},
	}
}

func TextWithLayout(text string, x, y float32, font FontDescriptor, color uint32, layout TextLayoutConfig) RenderCommand {
	return RenderCommand{
		DrawText: &DrawTextCmd{
			X: x, Y: y, Text: text, Color: color,
			Font: font, Layout: layout,
		},
	}
}

func Image(textureID TextureID, x, y, width, height float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID),
		},
	}
}

func ImageWithSourceRect(textureID TextureID, x, y, width, height float32, sourceRect [4]float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID), SourceRect: &sourceRect,
		},
	}
}

func ImageWithCornerRadii(textureID TextureID, x, y, width, height float32, cornerRadii [4]float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID), CornerRadii: cornerRadii,
		},
	}
}

func ImageWithSourceRectAndCornerRadii(textureID TextureID, x, y, width, height float32, sourceRect [4]float32, cornerRadii [4]float32) RenderCommand {
	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID: uint32(textureID), SourceRect: &sourceRect, CornerRadii: cornerRadii,
		},
	}
}

func Sprite(textureID TextureID, x, y, width, height float32, spriteX, spriteY, cols, rows int) RenderCommand {
	spriteWidth := 1.0 / float32(cols)
	spriteHeight := 1.0 / float32(rows)
	srcX := float32(spriteX) * spriteWidth
	srcY := float32(spriteY) * spriteHeight

	return RenderCommand{
		DrawImage: &DrawImageCmd{
			X: x, Y: y, Width: width, Height: height,
			TextureID:  uint32(textureID),
			SourceRect: &[4]float32{srcX, srcY, spriteWidth, spriteHeight},
		},
	}
}

func PushClip(x, y, width, height float32) RenderCommand {
	return RenderCommand{
		PushClip: &PushClipCmd{X: x, Y: y, Width: width, Height: height},
	}
}

func PopClip() RenderCommand {
	return RenderCommand{
		PopClip: &struct{}{},
	}
}

func BeginScrollView(x, y, width, height, scrollX, scrollY float32) RenderCommand {
	return RenderCommand{
		BeginScrollView: &BeginScrollViewCmd{
			X: x, Y: y, Width: width, Height: height,
			ScrollX: scrollX, ScrollY: scrollY,
		},
	}
}

func BeginScrollViewWithContent(x, y, width, height, scrollX, scrollY, contentWidth, contentHeight float32) RenderCommand {
	return RenderCommand{
		BeginScrollView: &BeginScrollViewCmd{
			X: x, Y: y, Width: width, Height: height,
			ScrollX: scrollX, ScrollY: scrollY,
			ContentWidth: &contentWidth, ContentHeight: &contentHeight,
		},
	}
}

func EndScrollView() RenderCommand {
	return RenderCommand{
		EndScrollView: &struct{}{},
	}
}

// Video draws a video player frame to the canvas (web-specific)
func Video(videoID VideoPlayerID, x, y, width, height float32) RenderCommand {
	return RenderCommand{
		DrawVideo: &DrawVideoCmd{
			X: x, Y: y, Width: width, Height: height,
			VideoID: videoID,
		},
	}
}

// VideoWithRadii draws a video player frame with rounded corners (web-specific)
func VideoWithRadii(videoID VideoPlayerID, x, y, width, height float32, radii [4]float32) RenderCommand {
	return RenderCommand{
		DrawVideo: &DrawVideoCmd{
			X: x, Y: y, Width: width, Height: height,
			VideoID: videoID, CornerRadii: radii,
		},
	}
}

// Camera draws a camera input frame to the canvas (web-specific)
func Camera(cameraID VideoInputID, x, y, width, height float32) RenderCommand {
	return RenderCommand{
		DrawVideoInput: &DrawVideoInputCmd{
			X: x, Y: y, Width: width, Height: height,
			VideoInputID: cameraID,
		},
	}
}

// CameraWithRadii draws a camera input frame with rounded corners (web-specific)
func CameraWithRadii(cameraID VideoInputID, x, y, width, height float32, radii [4]float32) RenderCommand {
	return RenderCommand{
		DrawVideoInput: &DrawVideoInputCmd{
			X: x, Y: y, Width: width, Height: height,
			VideoInputID: cameraID, CornerRadii: radii,
		},
	}
}

func RGBA(r, g, b, a uint8) uint32 {
	return uint32(r)<<24 | uint32(g)<<16 | uint32(b)<<8 | uint32(a)
}

func RGB(r, g, b uint8) uint32 {
	return RGBA(r, g, b, 255)
}

// ============================================================================
// Safe Area Insets
// ============================================================================

type SafeAreaInsets struct {
	Top    float64
	Left   float64
	Bottom float64
	Right  float64
}

func GetSafeAreaInsets() SafeAreaInsets {
	// Web doesn't have hardware safe areas, but we could query CSS env() values
	return SafeAreaInsets{}
}

// ============================================================================
// Core Functions
// ============================================================================

// Run starts the application event loop
func Run(config AppConfig, handler EventHandler) error {
	currentHandler = handler

	// Set up canvas
	canvas := jsDocument.Call("getElementById", "centered-canvas")
	if canvas.IsUndefined() || canvas.IsNull() {
		return fmt.Errorf("canvas element 'centered-canvas' not found")
	}

	// Get container size (use parent element or window)
	container := canvas.Get("parentElement")
	var logicalWidth, logicalHeight float64
	if !container.IsUndefined() && !container.IsNull() {
		logicalWidth = container.Get("clientWidth").Float()
		logicalHeight = container.Get("clientHeight").Float()
	} else {
		logicalWidth = jsWindow.Get("innerWidth").Float()
		logicalHeight = jsWindow.Get("innerHeight").Float()
	}

	// Handle HiDPI displays
	dpr := jsWindow.Get("devicePixelRatio").Float()
	if dpr < 1 {
		dpr = 1
	}

	// Set physical canvas size (actual pixels)
	canvas.Set("width", int(logicalWidth*dpr))
	canvas.Set("height", int(logicalHeight*dpr))

	// Set CSS size (logical pixels)
	canvas.Get("style").Set("width", fmt.Sprintf("%dpx", int(logicalWidth)))
	canvas.Get("style").Set("height", fmt.Sprintf("%dpx", int(logicalHeight)))

	currentWidth = logicalWidth
	currentHeight = logicalHeight

	// Get 2D context for basic rendering
	ctx := canvas.Call("getContext", "2d")
	if ctx.IsUndefined() {
		return fmt.Errorf("failed to get 2D context")
	}

	// Scale context for HiDPI
	ctx.Call("scale", dpr, dpr)

	// Store context globally for render commands
	jsGlobal.Set("centeredCtx", ctx)
	jsGlobal.Set("centeredCanvas", canvas)
	jsGlobal.Set("centeredDPR", dpr)

	// Set up event listeners
	setupEventListeners(canvas)

	// Send ready event
	readyEvent := Event{
		Type:  EventReady,
		Data1: currentWidth,
		Data2: currentHeight,
	}
	resp := handler(readyEvent)
	if resp.Exit {
		return nil
	}

	// Render initial frame
	renderFrame(resp.ImmediateCommands)

	// Set up animation loop - always keeps running for retained mode animations
	animationFrame = js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		if !animationLoopRunning {
			return nil
		}

		// Request redraw event
		event := Event{Type: EventRedrawRequested}
		resp := currentHandler(event)

		if resp.Exit {
			animationLoopRunning = false
			animationFrame.Release()
			return nil
		}

		if len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}

		// Always continue animation loop for retained mode (animations need continuous ticks)
		jsWindow.Call("requestAnimationFrame", animationFrame)

		return nil
	})

	// Start animation loop
	animationLoopRunning = true
	jsWindow.Call("requestAnimationFrame", animationFrame)

	// Block forever (Go WASM programs run until explicitly stopped)
	select {}
}

func setupEventListeners(canvas js.Value) {
	// Mouse move
	canvas.Call("addEventListener", "mousemove", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		rect := canvas.Call("getBoundingClientRect")
		x := e.Get("clientX").Float() - rect.Get("left").Float()
		y := e.Get("clientY").Float() - rect.Get("top").Float()

		event := Event{Type: EventMouseMoved, Data1: x, Data2: y}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))

	// Mouse down
	canvas.Call("addEventListener", "mousedown", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		rect := canvas.Call("getBoundingClientRect")
		x := e.Get("clientX").Float() - rect.Get("left").Float()
		y := e.Get("clientY").Float() - rect.Get("top").Float()
		button := e.Get("button").Int()

		event := Event{Type: EventMousePressed, Data1: x, Data2: y, Data3: float64(button)}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))

	// Mouse up
	canvas.Call("addEventListener", "mouseup", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		rect := canvas.Call("getBoundingClientRect")
		x := e.Get("clientX").Float() - rect.Get("left").Float()
		y := e.Get("clientY").Float() - rect.Get("top").Float()
		button := e.Get("button").Int()

		event := Event{Type: EventMouseReleased, Data1: x, Data2: y, Data3: float64(button)}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))

	// Keyboard events (on document for global capture)
	jsDocument.Call("addEventListener", "keydown", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		keyCode := e.Get("keyCode").Int()
		mods := getModifiers(e)

		event := Event{Type: EventKeyPressed, Data1: float64(keyCode), Data2: float64(mods)}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}

		// Handle text input separately
		key := e.Get("key").String()
		if len(key) == 1 {
			textEvent := Event{Type: EventCharInput, Text: key}
			resp = currentHandler(textEvent)
			if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
				renderFrame(resp.ImmediateCommands)
			}
		}

		return nil
	}))

	jsDocument.Call("addEventListener", "keyup", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		keyCode := e.Get("keyCode").Int()
		mods := getModifiers(e)

		event := Event{Type: EventKeyReleased, Data1: float64(keyCode), Data2: float64(mods)}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))

	// Wheel events
	canvas.Call("addEventListener", "wheel", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		e.Call("preventDefault")

		rect := canvas.Call("getBoundingClientRect")
		x := e.Get("clientX").Float() - rect.Get("left").Float()
		y := e.Get("clientY").Float() - rect.Get("top").Float()
		dx := e.Get("deltaX").Float()
		dy := e.Get("deltaY").Float()

		// Data1/Data2 = mouse position, Data3/Data4 = scroll delta
		event := Event{Type: EventMouseWheel, Data1: x, Data2: y, Data3: dx, Data4: dy}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))

	// Touch events
	canvas.Call("addEventListener", "touchstart", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		e.Call("preventDefault")
		touches := e.Get("touches")
		if touches.Length() > 0 {
			touch := touches.Index(0)
			rect := canvas.Call("getBoundingClientRect")
			x := touch.Get("clientX").Float() - rect.Get("left").Float()
			y := touch.Get("clientY").Float() - rect.Get("top").Float()

			event := Event{Type: EventTouchStart, Data1: x, Data2: y}
			resp := currentHandler(event)
			if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
				renderFrame(resp.ImmediateCommands)
			}
		}
		return nil
	}))

	canvas.Call("addEventListener", "touchmove", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		e := args[0]
		e.Call("preventDefault")
		touches := e.Get("touches")
		if touches.Length() > 0 {
			touch := touches.Index(0)
			rect := canvas.Call("getBoundingClientRect")
			x := touch.Get("clientX").Float() - rect.Get("left").Float()
			y := touch.Get("clientY").Float() - rect.Get("top").Float()

			event := Event{Type: EventTouchMove, Data1: x, Data2: y}
			resp := currentHandler(event)
			if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
				renderFrame(resp.ImmediateCommands)
			}
		}
		return nil
	}))

	canvas.Call("addEventListener", "touchend", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		_ = args[0] // Touch event data available if needed
		event := Event{Type: EventTouchEnd}
		resp := currentHandler(event)
		if resp.RequestRedraw && len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))

	// Resize
	jsWindow.Call("addEventListener", "resize", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		// Update canvas size to match container
		container := canvas.Get("parentElement")
		var logicalWidth, logicalHeight float64
		if !container.IsUndefined() && !container.IsNull() {
			logicalWidth = container.Get("clientWidth").Float()
			logicalHeight = container.Get("clientHeight").Float()
		} else {
			logicalWidth = jsWindow.Get("innerWidth").Float()
			logicalHeight = jsWindow.Get("innerHeight").Float()
		}

		if logicalWidth > 0 && logicalHeight > 0 {
			dpr := jsWindow.Get("devicePixelRatio").Float()
			if dpr < 1 {
				dpr = 1
			}

			// Set physical canvas size
			canvas.Set("width", int(logicalWidth*dpr))
			canvas.Set("height", int(logicalHeight*dpr))

			// Set CSS size
			canvas.Get("style").Set("width", fmt.Sprintf("%dpx", int(logicalWidth)))
			canvas.Get("style").Set("height", fmt.Sprintf("%dpx", int(logicalHeight)))

			currentWidth = logicalWidth
			currentHeight = logicalHeight

			// Reset context and scale for HiDPI
			ctx := canvas.Call("getContext", "2d")
			ctx.Call("setTransform", 1, 0, 0, 1, 0, 0) // Reset transform
			ctx.Call("scale", dpr, dpr)
			jsGlobal.Set("centeredCtx", ctx)
			jsGlobal.Set("centeredDPR", dpr)

			// Send resize event
			event := Event{Type: EventResized, Data1: currentWidth, Data2: currentHeight}
			resp := currentHandler(event)
			if len(resp.ImmediateCommands) > 0 {
				renderFrame(resp.ImmediateCommands)
			}
		}
		return nil
	}))
}

func getModifiers(e js.Value) Modifiers {
	var mods Modifiers
	if e.Get("shiftKey").Bool() {
		mods |= ModShift
	}
	if e.Get("ctrlKey").Bool() {
		mods |= ModCtrl
	}
	if e.Get("altKey").Bool() {
		mods |= ModAlt
	}
	if e.Get("metaKey").Bool() {
		mods |= ModSuper
	}
	return mods
}

// renderFrame renders a list of commands using Canvas 2D API
func renderFrame(commands []RenderCommand) {
	ctx := jsGlobal.Get("centeredCtx")
	if ctx.IsUndefined() {
		return
	}

	for _, cmd := range commands {
		switch {
		case cmd.Clear != nil:
			// Use logical dimensions (context is already scaled for HiDPI)
			w := currentWidth
			h := currentHeight
			color := RGBA(cmd.Clear.R, cmd.Clear.G, cmd.Clear.B, cmd.Clear.A)
			ctx.Set("fillStyle", colorToCSS(color))
			ctx.Call("fillRect", 0, 0, w, h)

		case cmd.DrawRect != nil:
			drawRect(ctx, cmd.DrawRect)

		case cmd.DrawText != nil:
			drawText(ctx, cmd.DrawText)

		case cmd.DrawShadow != nil:
			drawShadow(ctx, cmd.DrawShadow)

		case cmd.DrawImage != nil:
			drawImage(ctx, cmd.DrawImage)

		case cmd.PushClip != nil:
			ctx.Call("save")
			ctx.Call("beginPath")
			ctx.Call("rect", cmd.PushClip.X, cmd.PushClip.Y, cmd.PushClip.Width, cmd.PushClip.Height)
			ctx.Call("clip")

		case cmd.PopClip != nil:
			ctx.Call("restore")

		case cmd.BeginScrollView != nil:
			sv := cmd.BeginScrollView
			ctx.Call("save")
			ctx.Call("beginPath")
			ctx.Call("rect", sv.X, sv.Y, sv.Width, sv.Height)
			ctx.Call("clip")
			ctx.Call("translate", -sv.ScrollX, -sv.ScrollY)

		case cmd.EndScrollView != nil:
			ctx.Call("restore")

		case cmd.DrawVideo != nil:
			drawVideo(ctx, cmd.DrawVideo)

		case cmd.DrawVideoInput != nil:
			drawVideoInput(ctx, cmd.DrawVideoInput)
		}
	}
}

func drawVideo(ctx js.Value, cmd *DrawVideoCmd) {
	videoEl := VideoGetElement(cmd.VideoID)
	if videoEl.IsUndefined() || videoEl.IsNull() {
		return
	}

	x, y, w, h := float64(cmd.X), float64(cmd.Y), float64(cmd.Width), float64(cmd.Height)
	radii := cmd.CornerRadii
	hasRadius := radii[0] > 0 || radii[1] > 0 || radii[2] > 0 || radii[3] > 0

	if hasRadius {
		ctx.Call("save")
		ctx.Call("beginPath")
		ctx.Call("moveTo", x+float64(radii[0]), y)
		ctx.Call("lineTo", x+w-float64(radii[1]), y)
		ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
		ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
		ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
		ctx.Call("lineTo", x+float64(radii[3]), y+h)
		ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
		ctx.Call("lineTo", x, y+float64(radii[0]))
		ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
		ctx.Call("closePath")
		ctx.Call("clip")
	}

	ctx.Call("drawImage", videoEl, x, y, w, h)

	if hasRadius {
		ctx.Call("restore")
	}
}

func drawVideoInput(ctx js.Value, cmd *DrawVideoInputCmd) {
	videoEl := VideoInputGetElement(cmd.VideoInputID)
	if videoEl.IsUndefined() || videoEl.IsNull() {
		return
	}

	x, y, w, h := float64(cmd.X), float64(cmd.Y), float64(cmd.Width), float64(cmd.Height)
	radii := cmd.CornerRadii
	hasRadius := radii[0] > 0 || radii[1] > 0 || radii[2] > 0 || radii[3] > 0

	if hasRadius {
		ctx.Call("save")
		ctx.Call("beginPath")
		ctx.Call("moveTo", x+float64(radii[0]), y)
		ctx.Call("lineTo", x+w-float64(radii[1]), y)
		ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
		ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
		ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
		ctx.Call("lineTo", x+float64(radii[3]), y+h)
		ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
		ctx.Call("lineTo", x, y+float64(radii[0]))
		ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
		ctx.Call("closePath")
		ctx.Call("clip")
	}

	ctx.Call("drawImage", videoEl, x, y, w, h)

	if hasRadius {
		ctx.Call("restore")
	}
}

func drawRect(ctx js.Value, cmd *DrawRectCmd) {
	x, y, w, h := float64(cmd.X), float64(cmd.Y), float64(cmd.Width), float64(cmd.Height)
	radii := cmd.CornerRadii

	ctx.Call("beginPath")

	// Check if we have rounded corners
	hasRadius := radii[0] > 0 || radii[1] > 0 || radii[2] > 0 || radii[3] > 0

	if hasRadius {
		// Draw rounded rectangle
		ctx.Call("moveTo", x+float64(radii[0]), y)
		ctx.Call("lineTo", x+w-float64(radii[1]), y)
		ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
		ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
		ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
		ctx.Call("lineTo", x+float64(radii[3]), y+h)
		ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
		ctx.Call("lineTo", x, y+float64(radii[0]))
		ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
	} else {
		ctx.Call("rect", x, y, w, h)
	}

	ctx.Call("closePath")
	ctx.Set("fillStyle", colorToCSS(cmd.Color))
	ctx.Call("fill")

	// Draw border if present
	if cmd.Border != nil && cmd.Border.Width > 0 {
		ctx.Set("strokeStyle", colorToCSS(cmd.Border.Color))
		ctx.Set("lineWidth", cmd.Border.Width)
		ctx.Call("stroke")
	}
}

func drawText(ctx js.Value, cmd *DrawTextCmd) {
	fontSize := cmd.Font.Size
	fontWeight := cmd.Font.Weight
	if fontWeight == 0 {
		fontWeight = 400
	}

	// Map font names to proper CSS font families
	fontFamily := "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
	if cmd.Font.Source.System != nil {
		name := *cmd.Font.Source.System
		switch name {
		case "", "system", "system-ui":
			// Use default system font stack
		case "monospace", "mono":
			fontFamily = "'SF Mono', 'Fira Code', 'Consolas', monospace"
		case "serif":
			fontFamily = "Georgia, 'Times New Roman', serif"
		default:
			// Use the specified font name, falling back to system
			fontFamily = fmt.Sprintf("'%s', system-ui, sans-serif", name)
		}
	}

	// Build font style string (italic if needed)
	fontStyle := ""
	if cmd.Font.Style == FontStyleItalic {
		fontStyle = "italic "
	}

	// CSS font format: [style] weight size family
	fontString := fmt.Sprintf("%s%d %.0fpx %s", fontStyle, fontWeight, fontSize, fontFamily)
	ctx.Set("font", fontString)
	ctx.Set("fillStyle", colorToCSS(cmd.Color))
	ctx.Set("textBaseline", "top")

	// Adjust Y position to account for line height centering
	// Web fonts with textBaseline="top" draw at the top of the em box,
	// but native platforms center text within the line height.
	lineHeight := cmd.Layout.LineHeight
	if lineHeight == 0 {
		lineHeight = 1.4
	}
	// Center within line height: offset = (lineHeight - 1) * fontSize / 2
	yOffset := (lineHeight - 1.0) * fontSize * 0.5
	adjustedY := float64(cmd.Y) + float64(yOffset)

	// Handle text alignment
	// Canvas textAlign changes what the X coordinate means:
	// - "left": X is the left edge of the text
	// - "center": X is the center of the text
	// - "right": X is the right edge of the text
	// Our cmd.X is always the left edge, so we need to adjust for center/right
	adjustedX := float64(cmd.X)
	var maxWidth float64
	if cmd.Layout.MaxWidth != nil {
		maxWidth = float64(*cmd.Layout.MaxWidth)
	}

	switch cmd.Layout.Alignment {
	case TextAlignCenter:
		ctx.Set("textAlign", "center")
		if maxWidth > 0 {
			adjustedX = float64(cmd.X) + maxWidth/2
		}
	case TextAlignRight:
		ctx.Set("textAlign", "right")
		if maxWidth > 0 {
			adjustedX = float64(cmd.X) + maxWidth
		}
	default:
		ctx.Set("textAlign", "left")
	}

	ctx.Call("fillText", cmd.Text, adjustedX, adjustedY)
}

func drawShadow(ctx js.Value, cmd *DrawShadowCmd) {
	// Save current state
	ctx.Call("save")

	// Set shadow properties
	ctx.Set("shadowColor", colorToCSS(cmd.Color))
	ctx.Set("shadowBlur", cmd.Blur)
	ctx.Set("shadowOffsetX", cmd.OffsetX)
	ctx.Set("shadowOffsetY", cmd.OffsetY)

	// Draw the shape that casts the shadow
	x, y, w, h := float64(cmd.X), float64(cmd.Y), float64(cmd.Width), float64(cmd.Height)
	radii := cmd.CornerRadii

	ctx.Call("beginPath")
	if radii[0] > 0 || radii[1] > 0 || radii[2] > 0 || radii[3] > 0 {
		ctx.Call("moveTo", x+float64(radii[0]), y)
		ctx.Call("lineTo", x+w-float64(radii[1]), y)
		ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
		ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
		ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
		ctx.Call("lineTo", x+float64(radii[3]), y+h)
		ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
		ctx.Call("lineTo", x, y+float64(radii[0]))
		ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
	} else {
		ctx.Call("rect", x, y, w, h)
	}
	ctx.Call("closePath")

	// Use transparent fill - we only want the shadow
	ctx.Set("fillStyle", "rgba(0,0,0,0)")
	ctx.Call("fill")

	// Restore state
	ctx.Call("restore")
}

func drawImage(ctx js.Value, cmd *DrawImageCmd) {
	x, y, w, h := float64(cmd.X), float64(cmd.Y), float64(cmd.Width), float64(cmd.Height)
	radii := cmd.CornerRadii
	hasRadius := radii[0] > 0 || radii[1] > 0 || radii[2] > 0 || radii[3] > 0
	textureID := cmd.TextureID

	// Check if this is a camera input (has offset bit set)
	if textureID >= cameraTextureOffset {
		cameraID := VideoInputID(textureID - cameraTextureOffset)
		cameraEl := VideoInputGetElement(cameraID)

		if !cameraEl.IsUndefined() && !cameraEl.IsNull() {
			// This is a camera frame - draw the video input element
			if hasRadius {
				ctx.Call("save")
				ctx.Call("beginPath")
				ctx.Call("moveTo", x+float64(radii[0]), y)
				ctx.Call("lineTo", x+w-float64(radii[1]), y)
				ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
				ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
				ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
				ctx.Call("lineTo", x+float64(radii[3]), y+h)
				ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
				ctx.Call("lineTo", x, y+float64(radii[0]))
				ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
				ctx.Call("closePath")
				ctx.Call("clip")
			}

			ctx.Call("drawImage", cameraEl, x, y, w, h)

			if hasRadius {
				ctx.Call("restore")
			}
			return
		}
	}

	// Check if this texture ID corresponds to a video player
	// (VideoUpdate returns the player ID as the texture ID on web)
	videoID := VideoPlayerID(textureID)
	videoEl := VideoGetElement(videoID)

	if !videoEl.IsUndefined() && !videoEl.IsNull() {
		// This is a video frame - draw the video element
		if hasRadius {
			ctx.Call("save")
			ctx.Call("beginPath")
			ctx.Call("moveTo", x+float64(radii[0]), y)
			ctx.Call("lineTo", x+w-float64(radii[1]), y)
			ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
			ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
			ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
			ctx.Call("lineTo", x+float64(radii[3]), y+h)
			ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
			ctx.Call("lineTo", x, y+float64(radii[0]))
			ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
			ctx.Call("closePath")
			ctx.Call("clip")
		}

		ctx.Call("drawImage", videoEl, x, y, w, h)

		if hasRadius {
			ctx.Call("restore")
		}
		return
	}

	// Regular image texture - draw placeholder for now
	// TODO: Implement proper image texture management
	if hasRadius {
		ctx.Call("save")
		ctx.Call("beginPath")
		ctx.Call("moveTo", x+float64(radii[0]), y)
		ctx.Call("lineTo", x+w-float64(radii[1]), y)
		ctx.Call("quadraticCurveTo", x+w, y, x+w, y+float64(radii[1]))
		ctx.Call("lineTo", x+w, y+h-float64(radii[2]))
		ctx.Call("quadraticCurveTo", x+w, y+h, x+w-float64(radii[2]), y+h)
		ctx.Call("lineTo", x+float64(radii[3]), y+h)
		ctx.Call("quadraticCurveTo", x, y+h, x, y+h-float64(radii[3]))
		ctx.Call("lineTo", x, y+float64(radii[0]))
		ctx.Call("quadraticCurveTo", x, y, x+float64(radii[0]), y)
		ctx.Call("closePath")
		ctx.Call("clip")
	}

	// Draw placeholder for images (gray rectangle)
	ctx.Set("fillStyle", "rgba(128,128,128,0.5)")
	ctx.Call("fillRect", x, y, w, h)

	if hasRadius {
		ctx.Call("restore")
	}
}

func colorToCSS(color uint32) string {
	r := (color >> 24) & 0xFF
	g := (color >> 16) & 0xFF
	b := (color >> 8) & 0xFF
	a := color & 0xFF
	return fmt.Sprintf("rgba(%d,%d,%d,%.3f)", r, g, b, float64(a)/255.0)
}

// ============================================================================
// Window Control Functions (stubs for web)
// ============================================================================

func RequestExit() {
	// Can't really exit a web page, but could close tab if allowed
}

func RequestRedraw() {
	// Trigger animation frame
	jsWindow.Call("requestAnimationFrame", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		event := Event{Type: EventRedrawRequested}
		resp := currentHandler(event)
		if len(resp.ImmediateCommands) > 0 {
			renderFrame(resp.ImmediateCommands)
		}
		return nil
	}))
}

func WindowMinimize()          {} // Not applicable for web
func WindowToggleMaximize()    {} // Could use Fullscreen API
func WindowEnterFullscreen()   { jsDocument.Get("documentElement").Call("requestFullscreen") }
func WindowExitFullscreen()    { jsDocument.Call("exitFullscreen") }
func WindowToggleFullscreen()  {} // Toggle based on current state
func WindowClose()             {} // Not applicable for web
func WindowSetTitle(title string) { jsDocument.Set("title", title) }

// ============================================================================
// Text Measurement
// ============================================================================

func MeasureTextWidth(text, fontName string, fontSize float32) float32 {
	ctx := jsGlobal.Get("centeredCtx")
	if ctx.IsUndefined() {
		// Create temporary canvas for measurement
		tempCanvas := jsDocument.Call("createElement", "canvas")
		ctx = tempCanvas.Call("getContext", "2d")
	}

	// Map font names to proper CSS font families
	fontFamily := "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
	switch fontName {
	case "", "system", "system-ui":
		// Use default
	case "monospace", "mono":
		fontFamily = "'SF Mono', 'Fira Code', 'Consolas', monospace"
	case "serif":
		fontFamily = "Georgia, 'Times New Roman', serif"
	default:
		fontFamily = fmt.Sprintf("'%s', system-ui, sans-serif", fontName)
	}

	font := fmt.Sprintf("400 %.0fpx %s", fontSize, fontFamily)
	ctx.Set("font", font)

	metrics := ctx.Call("measureText", text)
	return float32(metrics.Get("width").Float())
}

type TextMeasurementRequest struct {
	Text     string
	FontName string
	FontSize float32
}

func MeasureTextWidthBatch(requests []TextMeasurementRequest) []float32 {
	results := make([]float32, len(requests))
	for i, req := range requests {
		results[i] = MeasureTextWidth(req.Text, req.FontName, req.FontSize)
	}
	return results
}

func MeasureTextWithFont(text string, font FontDescriptor) float32 {
	fontName := ""
	if font.Source.System != nil {
		fontName = *font.Source.System
	}
	return MeasureTextWidth(text, fontName, font.Size)
}

func MeasureTextToCursor(text string, charIndex int, fontName string, fontSize float32) float32 {
	if charIndex >= len(text) {
		return MeasureTextWidth(text, fontName, fontSize)
	}
	return MeasureTextWidth(text[:charIndex], fontName, fontSize)
}

func GetScaleFactor() float64 {
	return jsWindow.Get("devicePixelRatio").Float()
}

// ============================================================================
// Image/Texture Functions (stubs - would need WebGL for proper implementation)
// ============================================================================

var imageCache = make(map[TextureID]js.Value)
var nextTextureID TextureID = 1

func LoadImage(data []byte) (TextureID, error) {
	// For proper implementation, would create ImageBitmap or WebGL texture
	// This is a stub that returns a placeholder ID
	id := nextTextureID
	nextTextureID++
	return id, nil
}

func LoadImageFile(path string) (TextureID, error) {
	// Load image from URL
	id := nextTextureID
	nextTextureID++
	return id, nil
}

func UnloadImage(textureID TextureID) error {
	delete(imageCache, textureID)
	return nil
}

func GetTextureSize(textureID TextureID) (uint32, uint32, error) {
	return 0, 0, fmt.Errorf("texture not found")
}

// ============================================================================
// System Functions
// ============================================================================

func SystemDarkMode() bool {
	return jsWindow.Call("matchMedia", "(prefers-color-scheme: dark)").Get("matches").Bool()
}

func GetNaturalScrolling() bool {
	return true // Web always uses natural scrolling
}

// ============================================================================
// Clipboard Functions
// ============================================================================

func ClipboardGetString() string {
	// Clipboard API is async, this is a sync stub
	return ""
}

func ClipboardSetString(text string) {
	jsGlobal.Get("navigator").Get("clipboard").Call("writeText", text)
}

// ============================================================================
// Keyboard Functions (stubs - web keyboards are automatic)
// ============================================================================

func KeyboardShow()            {} // Mobile browsers show keyboard on input focus
func KeyboardHide()            {} // Blur the input to hide keyboard
func KeyboardIsVisible() bool  { return false }

// ============================================================================
// Haptic Feedback (stub - Web Vibration API)
// ============================================================================

type HapticType int

const (
	HapticImpactLight HapticType = iota
	HapticImpactMedium
	HapticImpactHeavy
	HapticSelection
	HapticNotificationSuccess
	HapticNotificationWarning
	HapticNotificationError
)

func HapticFeedback(hapticType HapticType) {
	navigator := jsGlobal.Get("navigator")
	if !navigator.Get("vibrate").IsUndefined() {
		// Different durations for different haptic types
		var duration int
		switch hapticType {
		case HapticImpactLight:
			duration = 10
		case HapticImpactMedium:
			duration = 25
		case HapticImpactHeavy:
			duration = 50
		default:
			duration = 15
		}
		navigator.Call("vibrate", duration)
	}
}

// ============================================================================
// Audio/Video Implementation using Web APIs
// ============================================================================

type AudioPlayerID uint32
type AudioInputID uint32
type VideoPlayerID uint32
type VideoInputID uint32

// Audio state constants
const (
	AudioStateIdle    = 0
	AudioStatePlaying = 1
	AudioStatePaused  = 2
	AudioStateEnded   = 3
	AudioStateError   = 4
)

// Video state constants
const (
	VideoStateIdle    = 0
	VideoStatePlaying = 1
	VideoStatePaused  = 2
	VideoStateEnded   = 3
	VideoStateError   = 4
)

// AudioInfo contains metadata about an audio track
type AudioInfo struct {
	DurationMs uint64
	SampleRate uint32
	Channels   uint32
}

// VideoInfo contains metadata about a video
type VideoInfo struct {
	Width      uint32
	Height     uint32
	DurationMs uint64
}

// Audio input state constants
const (
	AudioInputStateIdle                 = 0
	AudioInputStateReady                = 1
	AudioInputStateCapturing            = 2
	AudioInputStateRequestingPermission = 3
	AudioInputStateError                = 4
)

// Video input state constants
const (
	VideoInputStateIdle                 = 0
	VideoInputStateReady                = 1
	VideoInputStateCapturing            = 2
	VideoInputStateRequestingPermission = 3
	VideoInputStateError                = 4
)

// Audio player storage
var (
	audioPlayers   = make(map[AudioPlayerID]js.Value)
	nextAudioID    AudioPlayerID = 1
	audioStates    = make(map[AudioPlayerID]int)
	audioCallbacks = make(map[AudioPlayerID]func(state int))
)

// Audio player functions using HTML5 Audio
func AudioCreate() AudioPlayerID {
	audio := jsDocument.Call("createElement", "audio")
	id := nextAudioID
	nextAudioID++
	audioPlayers[id] = audio
	audioStates[id] = AudioStateIdle

	// Set up event listeners
	audio.Call("addEventListener", "ended", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		audioStates[id] = AudioStateEnded
		if cb, ok := audioCallbacks[id]; ok {
			cb(AudioStateEnded)
		}
		return nil
	}))
	audio.Call("addEventListener", "error", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		audioStates[id] = AudioStateError
		if cb, ok := audioCallbacks[id]; ok {
			cb(AudioStateError)
		}
		return nil
	}))

	return id
}

func AudioDestroy(id AudioPlayerID) {
	if audio, ok := audioPlayers[id]; ok {
		audio.Call("pause")
		audio.Set("src", "")
		delete(audioPlayers, id)
		delete(audioStates, id)
		delete(audioCallbacks, id)
	}
}

func AudioLoadURL(id AudioPlayerID, url string) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Set("src", url)
		audio.Set("preload", "auto")
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioLoadFile(id AudioPlayerID, path string) error {
	// For web, treat file path as URL
	return AudioLoadURL(id, path)
}

func AudioPlay(id AudioPlayerID) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Call("play")
		audioStates[id] = AudioStatePlaying
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioPause(id AudioPlayerID) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Call("pause")
		audioStates[id] = AudioStatePaused
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioStop(id AudioPlayerID) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Call("pause")
		audio.Set("currentTime", 0)
		audioStates[id] = AudioStateIdle
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioSeek(id AudioPlayerID, timestampMs uint64) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Set("currentTime", float64(timestampMs)/1000.0)
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioSetLooping(id AudioPlayerID, loop bool) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Set("loop", loop)
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioSetVolume(id AudioPlayerID, vol float32) error {
	if audio, ok := audioPlayers[id]; ok {
		audio.Set("volume", vol)
		return nil
	}
	return fmt.Errorf("audio player %d not found", id)
}

func AudioGetVolume(id AudioPlayerID) float32 {
	if audio, ok := audioPlayers[id]; ok {
		return float32(audio.Get("volume").Float())
	}
	return 0
}

func AudioGetState(id AudioPlayerID) int {
	if state, ok := audioStates[id]; ok {
		return state
	}
	return AudioStateIdle
}

func AudioGetTime(id AudioPlayerID) uint64 {
	if audio, ok := audioPlayers[id]; ok {
		return uint64(audio.Get("currentTime").Float() * 1000)
	}
	return 0
}

func AudioGetInfo(id AudioPlayerID) (*AudioInfo, error) {
	if audio, ok := audioPlayers[id]; ok {
		durationSec := audio.Get("duration").Float()
		return &AudioInfo{
			DurationMs: uint64(durationSec * 1000),
			SampleRate: 44100, // Default - Web Audio API doesn't expose this from Audio element
			Channels:   2,     // Default
		}, nil
	}
	return nil, fmt.Errorf("audio player %d not found", id)
}

func AudioIsLooping(id AudioPlayerID) bool {
	if audio, ok := audioPlayers[id]; ok {
		return audio.Get("loop").Bool()
	}
	return false
}

func AudioUpdate(id AudioPlayerID) bool {
	// For web, audio updates automatically - just check if playing
	if state, ok := audioStates[id]; ok {
		return state == AudioStatePlaying
	}
	return false
}

func AudioSetStateCallback(id AudioPlayerID, cb func(state int)) {
	audioCallbacks[id] = cb
}

// Audio input storage
type AudioInputDevice struct {
	ID   string
	Name string
}

var (
	audioInputs       = make(map[AudioInputID]*audioInputState)
	nextAudioInputID  AudioInputID = 1
)

type audioInputState struct {
	stream        js.Value
	audioContext  js.Value
	analyser      js.Value
	dataArray     js.Value
	state         int
	hasPermission bool
	level         float32
}

func AudioInputCreate() AudioInputID {
	id := nextAudioInputID
	nextAudioInputID++
	audioInputs[id] = &audioInputState{
		state: AudioInputStateIdle,
	}
	return id
}

func AudioInputDestroy(id AudioInputID) {
	if input, ok := audioInputs[id]; ok {
		if !input.stream.IsUndefined() && !input.stream.IsNull() {
			tracks := input.stream.Call("getTracks")
			for i := 0; i < tracks.Length(); i++ {
				tracks.Index(i).Call("stop")
			}
		}
		delete(audioInputs, id)
	}
}

func AudioInputRequestPermission(id AudioInputID) error {
	input, ok := audioInputs[id]
	if !ok {
		return fmt.Errorf("audio input %d not found", id)
	}

	input.state = AudioInputStateRequestingPermission

	// Request microphone permission
	navigator := jsGlobal.Get("navigator")
	mediaDevices := navigator.Get("mediaDevices")
	if mediaDevices.IsUndefined() {
		input.state = AudioInputStateError
		return fmt.Errorf("mediaDevices not supported")
	}

	constraints := map[string]interface{}{
		"audio": true,
		"video": false,
	}
	constraintsJS := js.ValueOf(constraints)

	promise := mediaDevices.Call("getUserMedia", constraintsJS)

	// Handle promise with callbacks
	promise.Call("then", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		stream := args[0]
		input.stream = stream
		input.hasPermission = true
		input.state = AudioInputStateReady

		// Set up audio analysis
		audioCtx := jsGlobal.Get("AudioContext").New()
		input.audioContext = audioCtx
		source := audioCtx.Call("createMediaStreamSource", stream)
		analyser := audioCtx.Call("createAnalyser")
		analyser.Set("fftSize", 256)
		source.Call("connect", analyser)
		input.analyser = analyser

		bufferLength := analyser.Get("frequencyBinCount").Int()
		input.dataArray = jsGlobal.Get("Uint8Array").New(bufferLength)

		return nil
	}))

	promise.Call("catch", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		input.state = AudioInputStateError
		input.hasPermission = false
		return nil
	}))

	return nil
}

func AudioInputHasPermission(id AudioInputID) bool {
	if input, ok := audioInputs[id]; ok {
		return input.hasPermission
	}
	return false
}

func AudioInputListDevices(id AudioInputID) ([]AudioInputDevice, error) {
	var devices []AudioInputDevice

	navigator := jsGlobal.Get("navigator")
	mediaDevices := navigator.Get("mediaDevices")
	if mediaDevices.IsUndefined() {
		return devices, nil
	}

	// Note: This is async in JS, returning empty for now
	// Would need proper async handling for full implementation
	return devices, nil
}

func AudioInputOpen(id AudioInputID, device string, rate, ch uint32) error {
	// Already opened during permission request
	return nil
}

func AudioInputStart(id AudioInputID) error {
	if input, ok := audioInputs[id]; ok {
		if input.hasPermission {
			input.state = AudioInputStateCapturing
			return nil
		}
		return fmt.Errorf("no permission")
	}
	return fmt.Errorf("audio input %d not found", id)
}

func AudioInputStop(id AudioInputID) error {
	if input, ok := audioInputs[id]; ok {
		input.state = AudioInputStateReady
		return nil
	}
	return fmt.Errorf("audio input %d not found", id)
}

func AudioInputClose(id AudioInputID) {
	if input, ok := audioInputs[id]; ok {
		if !input.stream.IsUndefined() && !input.stream.IsNull() {
			tracks := input.stream.Call("getTracks")
			for i := 0; i < tracks.Length(); i++ {
				tracks.Index(i).Call("stop")
			}
		}
		input.state = AudioInputStateIdle
	}
}

func AudioInputGetLevel(id AudioInputID) float32 {
	input, ok := audioInputs[id]
	if !ok || input.analyser.IsUndefined() {
		return 0
	}

	input.analyser.Call("getByteFrequencyData", input.dataArray)
	bufferLength := input.dataArray.Get("length").Int()

	var sum float64
	for i := 0; i < bufferLength; i++ {
		sum += float64(input.dataArray.Index(i).Int())
	}
	average := sum / float64(bufferLength)

	return float32(average / 255.0)
}

func AudioInputGetState(id AudioInputID) int {
	if input, ok := audioInputs[id]; ok {
		return input.state
	}
	return AudioInputStateIdle
}

// Video player storage
var (
	videoPlayers   = make(map[VideoPlayerID]js.Value)
	nextVideoID    VideoPlayerID = 1
	videoStates    = make(map[VideoPlayerID]int)
	videoCallbacks = make(map[VideoPlayerID]func(state int))
	videoInfoCache = make(map[VideoPlayerID]*VideoInfo) // Cache dimensions after metadata loads
)

// Video player functions using HTML5 Video
func VideoCreate() VideoPlayerID {
	video := jsDocument.Call("createElement", "video")
	video.Set("playsInline", true)
	video.Set("crossOrigin", "anonymous")
	id := nextVideoID
	nextVideoID++
	videoPlayers[id] = video
	videoStates[id] = VideoStateIdle

	// Set up event listeners
	video.Call("addEventListener", "ended", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		videoStates[id] = VideoStateEnded
		if cb, ok := videoCallbacks[id]; ok {
			cb(VideoStateEnded)
		}
		return nil
	}))

	// Handle errors (including CORS issues)
	video.Call("addEventListener", "error", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		videoStates[id] = VideoStateError
		// Log the error for debugging
		errorObj := video.Get("error")
		if !errorObj.IsUndefined() && !errorObj.IsNull() {
			code := errorObj.Get("code").Int()
			message := errorObj.Get("message").String()
			jsGlobal.Get("console").Call("error", fmt.Sprintf("Video %d error: code=%d, message=%s", id, code, message))
		}
		if cb, ok := videoCallbacks[id]; ok {
			cb(VideoStateError)
		}
		return nil
	}))

	// Cache dimensions when metadata loads
	video.Call("addEventListener", "loadedmetadata", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		w := video.Get("videoWidth").Int()
		h := video.Get("videoHeight").Int()
		durationSec := video.Get("duration").Float()
		videoInfoCache[id] = &VideoInfo{
			Width:      uint32(w),
			Height:     uint32(h),
			DurationMs: uint64(durationSec * 1000),
		}
		jsGlobal.Get("console").Call("log", fmt.Sprintf("Video %d metadata loaded: %dx%d, duration=%.2fs", id, w, h, durationSec))
		return nil
	}))

	// Log when video can play
	video.Call("addEventListener", "canplay", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		jsGlobal.Get("console").Call("log", fmt.Sprintf("Video %d ready to play", id))
		return nil
	}))

	return id
}

func VideoDestroy(id VideoPlayerID) {
	if video, ok := videoPlayers[id]; ok {
		video.Call("pause")
		video.Set("src", "")
		delete(videoPlayers, id)
		delete(videoStates, id)
		delete(videoCallbacks, id)
		delete(videoInfoCache, id)
	}
}

func VideoLoadURL(id VideoPlayerID, url string) error {
	if video, ok := videoPlayers[id]; ok {
		video.Set("src", url)
		video.Set("preload", "auto")
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoLoadFile(id VideoPlayerID, path string) error {
	return VideoLoadURL(id, path)
}

func VideoPushFrame(id VideoPlayerID, width, height uint32, data []byte, timestampMs uint64) error {
	// For web, video frames are typically handled via MediaSource Extensions or WebCodecs
	// This is a stub - custom video frame pushing is not supported in the web build
	return fmt.Errorf("video frame pushing not supported on web")
}

func VideoPlay(id VideoPlayerID) error {
	if video, ok := videoPlayers[id]; ok {
		video.Call("play")
		videoStates[id] = VideoStatePlaying
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoPause(id VideoPlayerID) error {
	if video, ok := videoPlayers[id]; ok {
		video.Call("pause")
		videoStates[id] = VideoStatePaused
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoSeek(id VideoPlayerID, timestampMs uint64) error {
	if video, ok := videoPlayers[id]; ok {
		video.Set("currentTime", float64(timestampMs)/1000.0)
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoSetLooping(id VideoPlayerID, loop bool) error {
	if video, ok := videoPlayers[id]; ok {
		video.Set("loop", loop)
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoSetMuted(id VideoPlayerID, muted bool) error {
	if video, ok := videoPlayers[id]; ok {
		video.Set("muted", muted)
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoSetVolume(id VideoPlayerID, vol float32) error {
	if video, ok := videoPlayers[id]; ok {
		video.Set("volume", vol)
		return nil
	}
	return fmt.Errorf("video player %d not found", id)
}

func VideoUpdate(id VideoPlayerID) TextureID {
	// For web, video frames are rendered directly via drawImage
	// This function returns a placeholder texture ID
	if _, ok := videoPlayers[id]; ok {
		return TextureID(id)
	}
	return 0
}

func VideoGetState(id VideoPlayerID) int {
	if state, ok := videoStates[id]; ok {
		return state
	}
	return VideoStateIdle
}

func VideoGetTime(id VideoPlayerID) uint64 {
	if video, ok := videoPlayers[id]; ok {
		return uint64(video.Get("currentTime").Float() * 1000)
	}
	return 0
}

func VideoGetInfo(id VideoPlayerID) (*VideoInfo, error) {
	// First check the cache (populated when metadata loads)
	if info, ok := videoInfoCache[id]; ok && info.Width > 0 && info.Height > 0 {
		return info, nil
	}

	// Fall back to reading from video element directly
	if video, ok := videoPlayers[id]; ok {
		durationSec := video.Get("duration").Float()
		w := uint32(video.Get("videoWidth").Int())
		h := uint32(video.Get("videoHeight").Int())

		// Cache if we got valid dimensions
		if w > 0 && h > 0 {
			videoInfoCache[id] = &VideoInfo{
				Width:      w,
				Height:     h,
				DurationMs: uint64(durationSec * 1000),
			}
		}

		return &VideoInfo{
			Width:      w,
			Height:     h,
			DurationMs: uint64(durationSec * 1000),
		}, nil
	}
	return nil, fmt.Errorf("video player %d not found", id)
}

// VideoGetElement returns the underlying HTML video element for rendering
func VideoGetElement(id VideoPlayerID) js.Value {
	if video, ok := videoPlayers[id]; ok {
		return video
	}
	return js.Undefined()
}

func VideoSetStateCallback(id VideoPlayerID, cb func(state int)) {
	videoCallbacks[id] = cb
}

// Video input (camera) storage
type VideoInputDevice struct {
	ID   string
	Name string
}

var (
	videoInputs      = make(map[VideoInputID]*videoInputState)
	nextVideoInputID VideoInputID = 1
)

type videoInputState struct {
	stream        js.Value
	video         js.Value // Hidden video element for stream
	state         int
	hasPermission bool
	width         int
	height        int
}

func VideoInputCreate() VideoInputID {
	id := nextVideoInputID
	nextVideoInputID++
	videoInputs[id] = &videoInputState{
		state: VideoInputStateIdle,
	}
	return id
}

func VideoInputDestroy(id VideoInputID) {
	if input, ok := videoInputs[id]; ok {
		if !input.stream.IsUndefined() && !input.stream.IsNull() {
			tracks := input.stream.Call("getTracks")
			for i := 0; i < tracks.Length(); i++ {
				tracks.Index(i).Call("stop")
			}
		}
		delete(videoInputs, id)
	}
}

func VideoInputRequestPermission(id VideoInputID) error {
	input, ok := videoInputs[id]
	if !ok {
		return fmt.Errorf("video input %d not found", id)
	}

	input.state = VideoInputStateRequestingPermission

	navigator := jsGlobal.Get("navigator")
	mediaDevices := navigator.Get("mediaDevices")
	if mediaDevices.IsUndefined() {
		input.state = VideoInputStateError
		return fmt.Errorf("mediaDevices not supported")
	}

	constraints := map[string]interface{}{
		"audio": false,
		"video": map[string]interface{}{
			"facingMode": "user",
			"width":      map[string]interface{}{"ideal": 640},
			"height":     map[string]interface{}{"ideal": 480},
		},
	}
	constraintsJS := js.ValueOf(constraints)

	promise := mediaDevices.Call("getUserMedia", constraintsJS)

	promise.Call("then", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		stream := args[0]
		input.stream = stream
		input.hasPermission = true
		input.state = VideoInputStateReady

		// Create hidden video element to receive stream
		video := jsDocument.Call("createElement", "video")
		video.Set("srcObject", stream)
		video.Set("playsInline", true)
		video.Set("autoplay", true)
		video.Set("muted", true)
		video.Get("style").Set("display", "none")
		jsDocument.Get("body").Call("appendChild", video)
		input.video = video

		// Get dimensions once metadata is loaded
		video.Call("addEventListener", "loadedmetadata", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
			input.width = video.Get("videoWidth").Int()
			input.height = video.Get("videoHeight").Int()
			return nil
		}))

		return nil
	}))

	promise.Call("catch", js.FuncOf(func(this js.Value, args []js.Value) interface{} {
		input.state = VideoInputStateError
		input.hasPermission = false
		return nil
	}))

	return nil
}

func VideoInputHasPermission(id VideoInputID) bool {
	if input, ok := videoInputs[id]; ok {
		return input.hasPermission
	}
	return false
}

func VideoInputListDevices(id VideoInputID) ([]VideoInputDevice, error) {
	// Would need async handling for full implementation
	return nil, nil
}

func VideoInputOpen(id VideoInputID, device string, w, h, fps uint32) error {
	return nil // Already opened during permission request
}

func VideoInputStart(id VideoInputID) error {
	if input, ok := videoInputs[id]; ok {
		if input.hasPermission {
			input.state = VideoInputStateCapturing
			if !input.video.IsUndefined() {
				input.video.Call("play")
			}
			return nil
		}
		return fmt.Errorf("no permission")
	}
	return fmt.Errorf("video input %d not found", id)
}

func VideoInputStop(id VideoInputID) error {
	if input, ok := videoInputs[id]; ok {
		input.state = VideoInputStateReady
		if !input.video.IsUndefined() {
			input.video.Call("pause")
		}
		return nil
	}
	return fmt.Errorf("video input %d not found", id)
}

func VideoInputClose(id VideoInputID) {
	if input, ok := videoInputs[id]; ok {
		if !input.stream.IsUndefined() && !input.stream.IsNull() {
			tracks := input.stream.Call("getTracks")
			for i := 0; i < tracks.Length(); i++ {
				tracks.Index(i).Call("stop")
			}
		}
		if !input.video.IsUndefined() {
			input.video.Call("remove")
		}
		input.state = VideoInputStateIdle
	}
}

func VideoInputGetState(id VideoInputID) int {
	if input, ok := videoInputs[id]; ok {
		return input.state
	}
	return VideoInputStateIdle
}

func VideoInputGetDimensions(id VideoInputID) (uint32, uint32, error) {
	if input, ok := videoInputs[id]; ok {
		return uint32(input.width), uint32(input.height), nil
	}
	return 0, 0, fmt.Errorf("video input %d not found", id)
}

// cameraTextureOffset is used to distinguish camera texture IDs from video texture IDs
// on web. Video player IDs and video input IDs both start at 1, so we offset camera
// texture IDs by this value to avoid collision.
const cameraTextureOffset = 0x80000000

func VideoInputGetFrameTexture(id VideoInputID, existingTextureID uint32) (uint32, error) {
	// For web, video input frames are rendered directly via canvas drawImage
	// Return a placeholder texture ID with offset to distinguish from video player IDs
	if _, ok := videoInputs[id]; ok {
		return cameraTextureOffset + uint32(id), nil
	}
	return 0, fmt.Errorf("video input %d not found", id)
}

// VideoInputGetElement returns the hidden video element for drawing to canvas
func VideoInputGetElement(id VideoInputID) js.Value {
	if input, ok := videoInputs[id]; ok {
		return input.video
	}
	return js.Undefined()
}

// ============================================================================
// Dialog Functions (stubs)
// ============================================================================

type FileFilter struct {
	Name       string
	Extensions []string
}

func OpenFileDialog(title, directory string, filters []FileFilter, multiple bool) ([]string, bool) {
	// File dialogs not supported on web in the same way
	return nil, false
}

func SaveFileDialog(title, directory string, filters []FileFilter) (string, bool) {
	// File dialogs not supported on web in the same way
	return "", false
}

// ============================================================================
// Tray Icon Functions (stubs - not applicable for web)
// ============================================================================

type TrayIconID uint32

func TrayIconCreate() error                                        { return fmt.Errorf("tray icon not supported on web") }
func TrayIconDestroy()                                             {}
func TrayIconSetTitle(title string)                                {}
func TrayIconSetTooltip(tooltip string)                            {}
func TrayIconSetIconFile(path string) error                        { return nil }
func TrayIconSetIconData(data []byte) error                        { return nil }
func TrayIconSetVisible(visible bool)                              {}
func TrayIconIsVisible() bool                                      { return false }
func TrayIconAddMenuItem(label string, enabled, checked bool) int  { return 0 }
func TrayIconAddSeparator() int                                    { return 0 }
func TrayIconClearMenu()                                           {}
func TrayIconSetMenuCallback(cb func(int))                         {}
func TrayIconSetMenuItemEnabled(index int, enabled bool)           {}
func TrayIconSetMenuItemChecked(index int, checked bool)           {}
func TrayIconSetMenuItemLabel(index int, label string)             {}

// Version returns the engine version
func Version() string {
	return "0.1.0-web"
}
