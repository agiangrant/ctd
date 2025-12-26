package ctd

import (
	"hash/fnv"
	"sync"
	"sync/atomic"

	"github.com/agiangrant/ctd/internal/ffi"
)

// UpdateType identifies what kind of update occurred.
type UpdateType uint8

const (
	UpdateProperty UpdateType = iota // Widget property changed
	UpdateAdd                        // Widget added to tree
	UpdateRemove                     // Widget removed from tree
	UpdateReorder                    // Children reordered
)

// Update represents a single widget state change.
// These are batched and sent to the renderer each frame.
type Update struct {
	Type      UpdateType
	WidgetID  WidgetID
	DirtyMask uint64 // Which properties changed (for UpdateProperty)
	Widget    *Widget // Reference for full state access
}

// TreeConfig configures the widget tree behavior.
type TreeConfig struct {
	// ChannelCount is the number of sharded update channels.
	// Higher values reduce contention with many goroutines.
	// Default: 100
	ChannelCount int

	// ChannelBufferSize is the buffer size per channel.
	// Larger buffers prevent blocking but use more memory.
	// Default: 1000
	ChannelBufferSize int
}

// DefaultTreeConfig returns sensible defaults.
func DefaultTreeConfig() TreeConfig {
	return TreeConfig{
		ChannelCount:      100,
		ChannelBufferSize: 1000,
	}
}

// DarkModeProvider is a function that returns the current dark mode state.
// This allows widgets to check dark mode without needing a direct Loop reference.
type DarkModeProvider func() bool

// Tree manages the widget hierarchy and update dispatch.
// It uses sharded channels for high-throughput concurrent updates.
type Tree struct {
	mu   sync.RWMutex
	root *Widget

	// Widget registry for ID lookups
	widgets sync.Map // map[WidgetID]*Widget

	// Sharded update channels
	channels []chan Update
	chanMask uint64 // For fast modulo (power of 2 - 1)

	// Frame tracking
	frameNumber atomic.Uint64

	// Pending updates collector (for game loop batching)
	pendingMu sync.Mutex
	pending   []Update

	// Dirty tracking - set immediately on any update, cleared on CollectUpdates
	hasDirty atomic.Bool

	// Closed state
	closed atomic.Bool

	// Event dispatcher reference (set by Loop after creation)
	// Allows widgets to request focus changes
	events *EventDispatcher

	// Dark mode provider (set by Loop after creation)
	// Allows widgets to check dark mode state for style updates
	darkModeProvider DarkModeProvider
}

// NewTree creates a widget tree with the specified configuration.
func NewTree(config TreeConfig) *Tree {
	// Round channel count up to power of 2 for fast modulo
	chanCount := nextPowerOf2(config.ChannelCount)
	if chanCount < 1 {
		chanCount = 1
	}

	bufSize := config.ChannelBufferSize
	if bufSize < 1 {
		bufSize = 1000
	}

	t := &Tree{
		channels: make([]chan Update, chanCount),
		chanMask: uint64(chanCount - 1),
		pending:  make([]Update, 0, 1024),
	}

	// Create channels
	for i := range t.channels {
		t.channels[i] = make(chan Update, bufSize)
	}

	// Start drain goroutines for each channel
	for i := range t.channels {
		go t.drainChannel(i)
	}

	return t
}

// SetEventDispatcher sets the event dispatcher reference.
// This allows widgets in the tree to request focus changes.
// Called by Loop after creating the EventDispatcher.
func (t *Tree) SetEventDispatcher(events *EventDispatcher) {
	t.mu.Lock()
	t.events = events
	t.mu.Unlock()
}

// EventDispatcher returns the event dispatcher, or nil if not set.
func (t *Tree) EventDispatcher() *EventDispatcher {
	t.mu.RLock()
	defer t.mu.RUnlock()
	return t.events
}

// SetDarkModeProvider sets the function that provides dark mode state.
// This is called by Loop after creation to enable dark mode awareness in widgets.
func (t *Tree) SetDarkModeProvider(provider DarkModeProvider) {
	t.mu.Lock()
	t.darkModeProvider = provider
	t.mu.Unlock()
}

// DarkMode returns the current dark mode state.
// Returns false if no provider is set.
func (t *Tree) DarkMode() bool {
	t.mu.RLock()
	provider := t.darkModeProvider
	t.mu.RUnlock()

	if provider != nil {
		return provider()
	}
	return false
}

// nextPowerOf2 rounds up to the next power of 2.
func nextPowerOf2(n int) int {
	if n <= 1 {
		return 1
	}
	n--
	n |= n >> 1
	n |= n >> 2
	n |= n >> 4
	n |= n >> 8
	n |= n >> 16
	return n + 1
}

// drainChannel collects updates from a single channel.
func (t *Tree) drainChannel(index int) {
	ch := t.channels[index]
	for update := range ch {
		if t.closed.Load() {
			return
		}
		t.pendingMu.Lock()
		t.pending = append(t.pending, update)
		t.pendingMu.Unlock()
	}
}

// channelFor returns the channel index for a widget ID.
// Uses FNV hash for good distribution.
func (t *Tree) channelFor(id WidgetID) int {
	h := fnv.New64a()
	var buf [8]byte
	buf[0] = byte(id)
	buf[1] = byte(id >> 8)
	buf[2] = byte(id >> 16)
	buf[3] = byte(id >> 24)
	buf[4] = byte(id >> 32)
	buf[5] = byte(id >> 40)
	buf[6] = byte(id >> 48)
	buf[7] = byte(id >> 56)
	h.Write(buf[:])
	return int(h.Sum64() & t.chanMask)
}

// notifyUpdate is called by widgets when their state changes.
func (t *Tree) notifyUpdate(w *Widget, dirtyMask uint64) {
	if t.closed.Load() {
		return
	}

	// Mark dirty immediately for synchronous checking
	t.hasDirty.Store(true)

	update := Update{
		Type:      UpdateProperty,
		WidgetID:  w.id,
		DirtyMask: dirtyMask,
		Widget:    w,
	}

	// Send to sharded channel (non-blocking with buffer)
	chanIdx := t.channelFor(w.id)
	select {
	case t.channels[chanIdx] <- update:
	default:
		// Channel full - collect directly (fallback)
		t.pendingMu.Lock()
		t.pending = append(t.pending, update)
		t.pendingMu.Unlock()
	}
}

// SetRoot sets the root widget of the tree.
func (t *Tree) SetRoot(w *Widget) {
	t.mu.Lock()
	t.root = w
	// Get dark mode before releasing lock, since registerWidget needs it
	// but can't call DarkMode() while we hold the write lock
	provider := t.darkModeProvider
	t.mu.Unlock()

	darkMode := false
	if provider != nil {
		darkMode = provider()
	}
	t.registerWidgetWithDarkMode(w, darkMode)
}

// Root returns the root widget.
func (t *Tree) Root() *Widget {
	t.mu.RLock()
	defer t.mu.RUnlock()
	return t.root
}

// registerWidgetWithDarkMode recursively registers a widget and its children.
// darkMode is passed as parameter to avoid deadlock (caller may hold t.mu).
func (t *Tree) registerWidgetWithDarkMode(w *Widget, darkMode bool) {
	if w == nil {
		return
	}

	w.mu.Lock()
	w.tree = t
	styles := w.computedStyles
	w.mu.Unlock()

	t.widgets.Store(w.id, w)

	// Re-apply styles with dark mode now that widget has tree reference
	// This fixes the issue where styles are applied before widget is in tree
	if styles != nil {
		resolved := styles.ResolveWithDarkMode(darkMode)
		w.mu.Lock()
		applyStyleProperties(w, &resolved)
		w.mu.Unlock()
	}

	// Notify addition
	t.notifyAdd(w)

	// Register children
	for _, child := range w.Children() {
		t.registerWidgetWithDarkMode(child, darkMode)
	}
}

// notifyAdd sends a widget addition update.
func (t *Tree) notifyAdd(w *Widget) {
	if t.closed.Load() {
		return
	}

	update := Update{
		Type:     UpdateAdd,
		WidgetID: w.id,
		Widget:   w,
	}

	chanIdx := t.channelFor(w.id)
	select {
	case t.channels[chanIdx] <- update:
	default:
		t.pendingMu.Lock()
		t.pending = append(t.pending, update)
		t.pendingMu.Unlock()
	}
}

// Widget returns a widget by ID.
func (t *Tree) Widget(id WidgetID) *Widget {
	if v, ok := t.widgets.Load(id); ok {
		return v.(*Widget)
	}
	return nil
}

// CollectUpdates drains all pending updates and returns them.
// This is called by the game loop each frame.
func (t *Tree) CollectUpdates() []Update {
	t.pendingMu.Lock()
	defer t.pendingMu.Unlock()

	// Clear the dirty flag since we're collecting all updates
	t.hasDirty.Store(false)

	if len(t.pending) == 0 {
		return nil
	}

	// Swap with empty slice
	updates := t.pending
	t.pending = make([]Update, 0, cap(updates))

	// Clear dirty state on collected widgets
	for _, u := range updates {
		if u.Widget != nil {
			u.Widget.ClearDirty()
		}
	}

	return updates
}

// HasPendingUpdates returns true if any widget has been modified since the
// last call to CollectUpdates. This is used to determine if a redraw is needed.
func (t *Tree) HasPendingUpdates() bool {
	return t.hasDirty.Load()
}

// DeduplicateUpdates merges multiple updates to the same widget.
// Returns a map of widget ID to combined dirty mask.
func (t *Tree) DeduplicateUpdates(updates []Update) map[WidgetID]*WidgetDelta {
	deltas := make(map[WidgetID]*WidgetDelta)

	for _, u := range updates {
		switch u.Type {
		case UpdateProperty:
			if delta, ok := deltas[u.WidgetID]; ok {
				delta.DirtyMask |= u.DirtyMask
			} else {
				deltas[u.WidgetID] = &WidgetDelta{
					ID:        u.WidgetID,
					Widget:    u.Widget,
					DirtyMask: u.DirtyMask,
					IsNew:     false,
				}
			}

		case UpdateAdd:
			// New widget - mark everything dirty
			deltas[u.WidgetID] = &WidgetDelta{
				ID:        u.WidgetID,
				Widget:    u.Widget,
				DirtyMask: 0xFFFFFFFFFFFFFFFF, // All properties
				IsNew:     true,
			}

		case UpdateRemove:
			// Removed widget - mark for removal
			deltas[u.WidgetID] = &WidgetDelta{
				ID:        u.WidgetID,
				Widget:    nil,
				IsRemoved: true,
			}
		}
	}

	return deltas
}

// WidgetDelta represents the changes to send to the renderer.
type WidgetDelta struct {
	ID        WidgetID
	Widget    *Widget
	DirtyMask uint64
	IsNew     bool
	IsRemoved bool
}

// FrameNumber returns the current frame count.
func (t *Tree) FrameNumber() uint64 {
	return t.frameNumber.Load()
}

// IncrementFrame advances the frame counter.
func (t *Tree) IncrementFrame() uint64 {
	return t.frameNumber.Add(1)
}

// Close shuts down the tree and its channels.
func (t *Tree) Close() {
	if t.closed.Swap(true) {
		return // Already closed
	}

	// Clean up tray icons before closing
	t.Walk(func(w *Widget) bool {
		if w.kind == KindTrayIcon {
			w.mu.Lock()
			if w.trayCreated {
				ffi.TrayIconDestroy()
				w.trayCreated = false
			}
			w.mu.Unlock()
		}
		return true
	})

	for _, ch := range t.channels {
		close(ch)
	}
}

// Update provides a callback for batched tree modifications.
// All changes within the callback are collected before notification.
func (t *Tree) Update(fn func(root *Widget)) {
	t.mu.RLock()
	root := t.root
	t.mu.RUnlock()

	if root != nil {
		fn(root)
	}
}

// Walk traverses the tree depth-first, calling fn for each widget.
func (t *Tree) Walk(fn func(w *Widget) bool) {
	t.mu.RLock()
	root := t.root
	t.mu.RUnlock()

	if root != nil {
		walkWidget(root, fn)
	}
}

func walkWidget(w *Widget, fn func(w *Widget) bool) bool {
	if !fn(w) {
		return false
	}
	for _, child := range w.Children() {
		if !walkWidget(child, fn) {
			return false
		}
	}
	return true
}

// Find searches for a widget matching the predicate.
func (t *Tree) Find(pred func(w *Widget) bool) *Widget {
	var found *Widget
	t.Walk(func(w *Widget) bool {
		if pred(w) {
			found = w
			return false // Stop walking
		}
		return true
	})
	return found
}

// FindByData finds a widget with the specified data value.
func (t *Tree) FindByData(data any) *Widget {
	return t.Find(func(w *Widget) bool {
		return w.Data() == data
	})
}
