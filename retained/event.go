package retained

import "sync"

// ============================================================================
// Event Types
// ============================================================================

// EventType identifies the kind of event.
type EventType uint8

const (
	// Mouse events
	EventMouseEnter EventType = iota + 1
	EventMouseLeave
	EventMouseMove
	EventMouseDown
	EventMouseUp
	EventClick
	EventDoubleClick
	EventTripleClick
	EventMouseWheel

	// Keyboard events
	EventKeyDown
	EventKeyUp
	EventKeyPress // Character input

	// Focus events
	EventFocus
	EventBlur

	// Drag events
	EventDragStart
	EventDragMove
	EventDragEnd
	EventDrop
)

// EventPhase indicates when in the event propagation cycle we are.
type EventPhase uint8

const (
	// PhaseCapture - event travels from root down to target.
	// Parents can intercept before children see it.
	PhaseCapture EventPhase = iota

	// PhaseTarget - event is at the target widget.
	PhaseTarget

	// PhaseBubble - event travels from target up to root.
	// Normal handling phase - most handlers use this.
	PhaseBubble
)

// MouseButton identifies which mouse button was pressed.
type MouseButton uint8

const (
	MouseButtonNone MouseButton = iota
	MouseButtonLeft
	MouseButtonRight
	MouseButtonMiddle
)

// Modifier keys
type Modifiers uint8

const (
	ModShift Modifiers = 1 << iota
	ModCtrl
	ModAlt
	ModSuper // Cmd on Mac, Win on Windows
)

func (m Modifiers) Shift() bool { return m&ModShift != 0 }
func (m Modifiers) Ctrl() bool  { return m&ModCtrl != 0 }
func (m Modifiers) Alt() bool   { return m&ModAlt != 0 }
func (m Modifiers) Super() bool { return m&ModSuper != 0 }

// ============================================================================
// Event Interface and Base
// ============================================================================

// Event is the interface for all events.
type Event interface {
	// Type returns the event type.
	Type() EventType

	// Target returns the widget that was hit (for mouse) or focused (for keyboard).
	Target() *Widget

	// CurrentTarget returns the widget currently handling the event during propagation.
	CurrentTarget() *Widget

	// Phase returns the current propagation phase.
	Phase() EventPhase

	// StopPropagation prevents the event from continuing to propagate.
	StopPropagation()

	// IsPropagationStopped returns true if propagation was stopped.
	IsPropagationStopped() bool

	// PreventDefault prevents the default behavior (if any).
	PreventDefault()

	// IsDefaultPrevented returns true if default was prevented.
	IsDefaultPrevented() bool

	// internal methods for event dispatch
	setCurrentTarget(w *Widget)
	setPhase(p EventPhase)
}

// eventBase provides common event functionality.
type eventBase struct {
	eventType         EventType
	target            *Widget
	currentTarget     *Widget
	phase             EventPhase
	propagationStopped bool
	defaultPrevented   bool
}

func (e *eventBase) Type() EventType              { return e.eventType }
func (e *eventBase) Target() *Widget              { return e.target }
func (e *eventBase) CurrentTarget() *Widget       { return e.currentTarget }
func (e *eventBase) Phase() EventPhase            { return e.phase }
func (e *eventBase) StopPropagation()             { e.propagationStopped = true }
func (e *eventBase) IsPropagationStopped() bool   { return e.propagationStopped }
func (e *eventBase) PreventDefault()              { e.defaultPrevented = true }
func (e *eventBase) IsDefaultPrevented() bool     { return e.defaultPrevented }
func (e *eventBase) setCurrentTarget(w *Widget)   { e.currentTarget = w }
func (e *eventBase) setPhase(p EventPhase)        { e.phase = p }

// ============================================================================
// Mouse Event
// ============================================================================

// MouseEvent represents mouse interaction events.
type MouseEvent struct {
	eventBase

	// Screen coordinates (relative to window)
	X, Y float32

	// Local coordinates (relative to target widget's top-left)
	LocalX, LocalY float32

	// Which button triggered the event (for down/up/click)
	Button MouseButton

	// Scroll delta (for wheel events)
	DeltaX, DeltaY float32

	// Modifier keys held during the event
	Modifiers Modifiers

	// Click count for detecting double/triple clicks
	ClickCount int
}

// NewMouseEvent creates a mouse event. Uses object pool for high-frequency events.
func NewMouseEvent(eventType EventType, x, y float32, button MouseButton, mods Modifiers) *MouseEvent {
	e := mouseEventPool.Get().(*MouseEvent)
	e.eventType = eventType
	e.target = nil
	e.currentTarget = nil
	e.phase = PhaseTarget
	e.propagationStopped = false
	e.defaultPrevented = false
	e.X = x
	e.Y = y
	e.LocalX = x
	e.LocalY = y
	e.Button = button
	e.DeltaX = 0
	e.DeltaY = 0
	e.Modifiers = mods
	e.ClickCount = 1
	return e
}

// Release returns the event to the pool. Call when done processing.
func (e *MouseEvent) Release() {
	mouseEventPool.Put(e)
}

// Object pool for mouse events to avoid allocations on every mouse move
var mouseEventPool = sync.Pool{
	New: func() any {
		return &MouseEvent{}
	},
}

// ============================================================================
// Keyboard Event
// ============================================================================

// KeyEvent represents keyboard events.
type KeyEvent struct {
	eventBase

	// Physical key code (platform-specific)
	KeyCode uint32

	// Logical key (e.g., 'a', 'Enter', 'Escape')
	Key string

	// For KeyPress events, the character that was typed
	Char rune

	// Modifier keys held during the event
	Modifiers Modifiers

	// True if this is a repeat event (key held down)
	Repeat bool
}

// NewKeyEvent creates a keyboard event.
func NewKeyEvent(eventType EventType, keyCode uint32, key string, char rune, mods Modifiers, repeat bool) *KeyEvent {
	e := keyEventPool.Get().(*KeyEvent)
	e.eventType = eventType
	e.target = nil
	e.currentTarget = nil
	e.phase = PhaseTarget
	e.propagationStopped = false
	e.defaultPrevented = false
	e.KeyCode = keyCode
	e.Key = key
	e.Char = char
	e.Modifiers = mods
	e.Repeat = repeat
	return e
}

// Release returns the event to the pool.
func (e *KeyEvent) Release() {
	keyEventPool.Put(e)
}

var keyEventPool = sync.Pool{
	New: func() any {
		return &KeyEvent{}
	},
}

// ============================================================================
// Focus Event
// ============================================================================

// FocusEvent represents focus change events.
type FocusEvent struct {
	eventBase

	// RelatedTarget is the widget losing focus (for Focus) or gaining focus (for Blur)
	RelatedTarget *Widget
}

// NewFocusEvent creates a focus event.
func NewFocusEvent(eventType EventType, relatedTarget *Widget) *FocusEvent {
	return &FocusEvent{
		eventBase: eventBase{
			eventType: eventType,
		},
		RelatedTarget: relatedTarget,
	}
}

// ============================================================================
// Responder Interface
// ============================================================================

// Responder is implemented by widgets (or custom components) that handle events.
// The interface enables composition - custom components can embed Widget and
// override event handling.
type Responder interface {
	// HandleEvent processes an event during the given phase.
	// Return true to stop propagation (event was consumed).
	HandleEvent(event Event, phase EventPhase) bool

	// HitTest returns true if this widget should receive events at the given
	// local coordinates. Override for custom hit shapes (circles, paths, etc.)
	// Default implementation checks rectangular bounds.
	HitTest(localX, localY float32) bool

	// CanReceiveEvents returns true if this widget can receive events.
	// Returns false for invisible or disabled widgets.
	CanReceiveEvents() bool
}

// ============================================================================
// Computed Bounds (for hit testing)
// ============================================================================

// Bounds represents the screen-space bounding box of a widget.
// Updated during rendering for efficient hit testing.
type Bounds struct {
	X, Y          float32 // Top-left corner in screen coordinates
	Width, Height float32
}

// Contains checks if a point is within the bounds.
func (b Bounds) Contains(x, y float32) bool {
	return x >= b.X && x < b.X+b.Width &&
		y >= b.Y && y < b.Y+b.Height
}

// LocalPoint converts screen coordinates to local coordinates relative to bounds.
func (b Bounds) LocalPoint(screenX, screenY float32) (localX, localY float32) {
	return screenX - b.X, screenY - b.Y
}

// ============================================================================
// Event Handler Types (for simple callback API)
// ============================================================================

// MouseHandler is a callback for mouse events.
type MouseHandler func(*MouseEvent)

// KeyHandler is a callback for keyboard events.
type KeyHandler func(*KeyEvent)

// FocusHandler is a callback for focus events.
type FocusHandler func(*FocusEvent)

// ============================================================================
// Behavior Interface (Composable Event Modifiers)
// ============================================================================

// Behavior allows composable event handling to be attached to widgets.
// Behaviors can intercept events, modify widget state, and chain together.
// Examples: Hoverable, Pressable, Focusable, Draggable, Scrollable
type Behavior interface {
	// Attach is called when the behavior is added to a widget.
	Attach(w *Widget)

	// Detach is called when the behavior is removed from a widget.
	Detach(w *Widget)

	// HandleEvent processes an event. Return true to stop propagation.
	// The behavior can modify the widget or event as needed.
	HandleEvent(w *Widget, event Event, phase EventPhase) bool
}
