package ctd

import (
	"runtime"
	"time"

	"github.com/agiangrant/ctd/internal/ffi"
)

// abs returns the absolute value of a float32.
func abs(x float32) float32 {
	if x < 0 {
		return -x
	}
	return x
}

// ============================================================================
// Event Dispatcher
// ============================================================================

// EventDispatcher handles event routing, hit testing, and state management.
// It tracks hover state, focus state, and dispatches events through the widget tree.
type EventDispatcher struct {
	// Current state
	hoveredWidget *Widget   // Deepest widget currently under the mouse
	hoveredChain  []*Widget // All widgets in hover chain (root to deepest)
	focusedWidget *Widget   // Widget with keyboard focus
	pressedWidget *Widget   // Widget where mouse down occurred
	pressedChain  []*Widget // All widgets in pressed chain (root to deepest)
	pressedButton MouseButton

	// For click detection
	lastClickTime time.Time
	lastClickX    float32
	lastClickY    float32
	clickCount    int

	// Touch/drag gesture tracking (for mobile scrolling)
	touchStartX     float32   // Where touch/mouse down started
	touchStartY     float32
	touchStartTime  time.Time // When touch started
	isDragging      bool      // True if drag threshold exceeded (scroll mode)
	dragScrollTarget *Widget  // Scrollable container being dragged
	lastTouchX      float32   // Last touch position (for delta calculation)
	lastTouchY      float32

	// Momentum scrolling state
	momentumVelocityX float32   // Current momentum velocity
	momentumVelocityY float32
	momentumTarget    *Widget   // Widget being momentum-scrolled
	momentumActive    bool      // Whether momentum scrolling is active
	lastMomentumTime  time.Time // For calculating dt

	// Configuration
	doubleClickTime time.Duration // Max time between clicks for double-click
	doubleClickDist float32       // Max distance between clicks for double-click
	dragThreshold   float32       // Distance to move before drag is detected (pixels)

	// Reference to tree for hit testing
	tree *Tree

	// Current frame number (for bounds validation)
	currentFrame uint64
}

// NewEventDispatcher creates an event dispatcher for the given tree.
func NewEventDispatcher(tree *Tree) *EventDispatcher {
	return &EventDispatcher{
		tree:            tree,
		doubleClickTime: 500 * time.Millisecond,
		doubleClickDist: 5.0,
		dragThreshold:   10.0, // 10px movement triggers drag/scroll instead of click
	}
}

// SetCurrentFrame updates the frame counter for bounds validation.
func (d *EventDispatcher) SetCurrentFrame(frame uint64) {
	d.currentFrame = frame
}

// ============================================================================
// Hit Testing (using cached bounds - O(n) but with early exit)
// ============================================================================

// HitTestResult contains the result of a hit test.
type HitTestResult struct {
	Widget *Widget
	LocalX float32
	LocalY float32
	// Chain is the path from root to target (for event propagation)
	Chain []*Widget
}

// HitTest finds the topmost widget at the given screen coordinates.
// Uses cached bounds from the last render for efficiency.
// Returns nil if no widget is at that position.
func (d *EventDispatcher) HitTest(screenX, screenY float32) *HitTestResult {
	root := d.tree.Root()
	if root == nil {
		return nil
	}

	// First, check for open dropdown overlays - these take priority over regular hit testing
	// because dropdowns render on top of everything and should capture all clicks in their area
	if result := d.hitTestOpenDropdowns(root, screenX, screenY); result != nil {
		return result
	}

	// Build the chain and find the target
	chain := make([]*Widget, 0, 16) // Pre-allocate reasonable capacity
	target := d.hitTestRecursive(root, screenX, screenY, &chain)

	if target == nil {
		return nil
	}

	// Compute local coordinates
	bounds := target.ComputedBounds()
	localX, localY := bounds.LocalPoint(screenX, screenY)

	return &HitTestResult{
		Widget: target,
		LocalX: localX,
		LocalY: localY,
		Chain:  chain,
	}
}

// hitTestOpenDropdowns checks if the point is within any open select dropdown.
// This is checked before regular hit testing because dropdowns render as overlays
// and should capture clicks regardless of other widgets at that position.
func (d *EventDispatcher) hitTestOpenDropdowns(w *Widget, screenX, screenY float32) *HitTestResult {
	// Check children first (depth-first, last child first for z-order)
	children := w.Children()
	for i := len(children) - 1; i >= 0; i-- {
		if result := d.hitTestOpenDropdowns(children[i], screenX, screenY); result != nil {
			return result
		}
	}

	// Check if this widget is an open select with dropdown
	if w.Kind() == KindSelect && w.IsSelectOpen() {
		bounds := w.ComputedBounds()
		optCount := len(w.SelectOptions())

		if optCount > 0 {
			optionHeight := float32(32)
			dropdownHeight := float32(optCount) * optionHeight
			gapHeight := float32(4)

			// Calculate dropdown bounds (below the trigger)
			dropdownY := bounds.Y + bounds.Height + gapHeight

			// Check if point is in the dropdown area (not the trigger)
			if screenX >= bounds.X && screenX <= bounds.X+bounds.Width &&
				screenY >= dropdownY && screenY <= dropdownY+dropdownHeight {
				// Point is in the dropdown - return this select widget
				// Local coordinates should be relative to the widget's origin
				localX := screenX - bounds.X
				localY := screenY - bounds.Y

				// Build chain to this widget
				chain := d.buildChainToRoot(w)

				return &HitTestResult{
					Widget: w,
					LocalX: localX,
					LocalY: localY,
					Chain:  chain,
				}
			}
		}
	}

	return nil
}

// hitTestRecursive walks the tree to find the topmost widget at the point.
// Appends widgets to the chain as it descends.
// Returns the target widget or nil.
func (d *EventDispatcher) hitTestRecursive(w *Widget, screenX, screenY float32, chain *[]*Widget) *Widget {
	if !w.CanReceiveEvents() {
		return nil
	}

	bounds := w.ComputedBounds()

	// Get effective bounds for hit testing (includes dropdown for open selects)
	effectiveBounds := d.getEffectiveHitBounds(w, bounds)

	// First check if point is in effective bounds
	if !effectiveBounds.Contains(screenX, screenY) {
		return nil
	}

	// Check custom hit test (for non-rectangular shapes)
	localX, localY := bounds.LocalPoint(screenX, screenY)
	if !w.HitTest(localX, localY) {
		return nil
	}

	// Point is in this widget - add to chain
	*chain = append(*chain, w)

	// Check if this widget is a scroll view - if so, adjust coordinates for children
	// Children's bounds are in layout space, so we need to add scroll offset to screen coords
	childX, childY := screenX, screenY
	w.mu.RLock()
	isScrollView := (w.kind == KindScrollView && w.scrollEnabled) ||
		w.overflowY == "scroll" || w.overflowY == "auto" ||
		w.overflowX == "scroll" || w.overflowX == "auto"
	if isScrollView {
		childX += w.scrollX
		childY += w.scrollY
	}
	w.mu.RUnlock()

	// Check children in reverse order (last child is drawn on top)
	children := w.Children()
	for i := len(children) - 1; i >= 0; i-- {
		child := children[i]
		if target := d.hitTestRecursive(child, childX, childY, chain); target != nil {
			return target
		}
	}

	// No child was hit, this widget is the target
	return w
}

// getEffectiveHitBounds returns the bounds to use for hit testing.
// For Select widgets with open dropdowns, this includes the dropdown area.
func (d *EventDispatcher) getEffectiveHitBounds(w *Widget, baseBounds Bounds) Bounds {
	// Check if this is an open Select dropdown
	if w.Kind() == KindSelect && w.IsSelectOpen() {
		optCount := len(w.SelectOptions())
		if optCount > 0 {
			optionHeight := float32(32)
			dropdownHeight := float32(optCount) * optionHeight
			gapHeight := float32(4)
			// Extend bounds to include dropdown
			return Bounds{
				X:      baseBounds.X,
				Y:      baseBounds.Y,
				Width:  baseBounds.Width,
				Height: baseBounds.Height + gapHeight + dropdownHeight,
			}
		}
	}
	return baseBounds
}

// ============================================================================
// Mouse Event Dispatch
// ============================================================================

// DispatchMouseMove handles mouse movement and hover state.
// Returns true if visual state changed and a redraw is needed.
func (d *EventDispatcher) DispatchMouseMove(screenX, screenY float32, mods Modifiers) bool {
	result := d.HitTest(screenX, screenY)
	var newHovered *Widget
	var newChain []*Widget
	if result != nil {
		newHovered = result.Widget
		newChain = result.Chain
	}

	needsRedraw := false

	// Check for touch/drag scrolling when pressed
	if d.pressedWidget != nil && d.dragScrollTarget != nil {
		// Calculate distance from touch start
		dx := screenX - d.touchStartX
		dy := screenY - d.touchStartY
		distSq := dx*dx + dy*dy

		// Check if we should start dragging (scroll mode)
		if !d.isDragging && distSq > d.dragThreshold*d.dragThreshold {
			// Try to scroll - only enter drag mode if scroll actually happened
			deltaX := d.lastTouchX - screenX
			deltaY := d.lastTouchY - screenY
			if d.handleDefaultScroll(d.dragScrollTarget, deltaX, deltaY) {
				d.isDragging = true
				d.lastTouchX = screenX
				d.lastTouchY = screenY
				// Blur focused widget when drag starts (for mobile scroll UX)
				// This ensures scrolling doesn't leave text inputs focused with keyboard showing
				if d.focusedWidget != nil {
					d.setFocus(nil)
				}
				return true // Redraw needed
			}
			// If scroll didn't happen (content fits), clear scroll target so we don't keep trying
			d.dragScrollTarget = nil
		}

		// If already dragging, continue applying scroll delta
		if d.isDragging {
			// Calculate delta from last position
			deltaX := d.lastTouchX - screenX
			deltaY := d.lastTouchY - screenY

			// Apply scroll
			if d.handleDefaultScroll(d.dragScrollTarget, deltaX, deltaY) {
				needsRedraw = true
			}

			d.lastTouchX = screenX
			d.lastTouchY = screenY

			// Don't dispatch regular mouse move events while drag-scrolling
			return needsRedraw
		}
	}

	// Handle hover state changes - compare chains not just the deepest widget
	// This ensures parents stay hovered when mouse moves to child
	if !d.chainsEqual(d.hoveredChain, newChain) {
		d.updateHoverState(newHovered, screenX, screenY, mods, newChain)
		// Only request redraw if hover state change actually caused visual changes
		// (e.g., hover styles that differ from default styles)
		// The style system now only marks widgets dirty if property values actually change
		if d.tree != nil && d.tree.HasPendingUpdates() {
			needsRedraw = true
		}
	}

	// Dispatch mouse move to hovered widget
	if newHovered != nil {
		e := NewMouseEvent(EventMouseMove, screenX, screenY, MouseButtonNone, mods)
		e.LocalX = result.LocalX
		e.LocalY = result.LocalY
		d.dispatchToWidget(newHovered, e, result.Chain)
		e.Release()
	}

	// If dragging (pressed widget exists), also notify that widget
	if d.pressedWidget != nil && d.pressedWidget != newHovered {
		bounds := d.pressedWidget.ComputedBounds()
		localX, localY := bounds.LocalPoint(screenX, screenY)
		e := NewMouseEvent(EventMouseMove, screenX, screenY, d.pressedButton, mods)
		e.LocalX = localX
		e.LocalY = localY
		// For drag, we dispatch directly without the chain
		d.pressedWidget.HandleEvent(e, PhaseBubble)
		e.Release()
		needsRedraw = true // Dragging always needs redraw for visual feedback
	}

	return needsRedraw
}

// DispatchMouseDown handles mouse button press.
func (d *EventDispatcher) DispatchMouseDown(screenX, screenY float32, button MouseButton, mods Modifiers) {
	// Stop any active momentum scrolling
	d.momentumActive = false
	d.momentumTarget = nil

	result := d.HitTest(screenX, screenY)
	if result == nil {
		// Click on empty space - blur focused widget
		if d.focusedWidget != nil {
			d.setFocus(nil)
		}
		return
	}

	target := result.Widget

	// Track touch start for gesture detection
	d.touchStartX = screenX
	d.touchStartY = screenY
	d.touchStartTime = time.Now()
	d.lastTouchX = screenX
	d.lastTouchY = screenY
	d.isDragging = false
	d.dragScrollTarget = nil

	// Find scrollable container in the chain for potential drag-to-scroll
	for i := len(result.Chain) - 1; i >= 0; i-- {
		w := result.Chain[i]
		if d.isScrollable(w) {
			d.dragScrollTarget = w
			break
		}
	}


	// Track pressed state - set on entire chain (like hover)
	d.pressedWidget = target
	d.pressedChain = result.Chain
	d.pressedButton = button
	for _, w := range result.Chain {
		w.setPressed(true)
	}

	// Focus is set here on mouse down, not on mouse up.
	// This is required for iOS where the software keyboard will only show
	// if becomeFirstResponder is called within the touch event handling context.
	// If the user starts dragging (scroll gesture), we blur the focus in DispatchMouseMove.
	if target != d.focusedWidget {
		d.setFocus(target)
	}

	// Dispatch the event
	e := NewMouseEvent(EventMouseDown, screenX, screenY, button, mods)
	e.LocalX = result.LocalX
	e.LocalY = result.LocalY
	d.dispatchToWidget(target, e, result.Chain)
	e.Release()
}

// DispatchMouseUp handles mouse button release.
func (d *EventDispatcher) DispatchMouseUp(screenX, screenY float32, button MouseButton, mods Modifiers) {
	// If we were drag-scrolling, start momentum scrolling
	wasDragging := d.isDragging
	if wasDragging && d.dragScrollTarget != nil {
		// Calculate velocity from recent movement
		elapsed := time.Since(d.touchStartTime).Seconds()
		if elapsed > 0 && elapsed < 0.5 { // Only if touch was recent
			dx := screenX - d.touchStartX
			dy := screenY - d.touchStartY

			// Velocity in pixels per second, scaled for momentum
			d.momentumVelocityX = -dx / float32(elapsed) * 0.3
			d.momentumVelocityY = -dy / float32(elapsed) * 0.3

			// Only start momentum if velocity is significant
			if abs(d.momentumVelocityX) > 50 || abs(d.momentumVelocityY) > 50 {
				d.momentumTarget = d.dragScrollTarget
				d.momentumActive = true
				d.lastMomentumTime = time.Now()
			}
		}
	}

	// Reset drag state
	d.isDragging = false
	d.dragScrollTarget = nil

	result := d.HitTest(screenX, screenY)
	var target *Widget
	var localX, localY float32
	var chain []*Widget

	if result != nil {
		target = result.Widget
		localX = result.LocalX
		localY = result.LocalY
		chain = result.Chain
	}

	// Dispatch mouse up to the widget under cursor (but not if we were drag-scrolling)
	if target != nil && !wasDragging {
		e := NewMouseEvent(EventMouseUp, screenX, screenY, button, mods)
		e.LocalX = localX
		e.LocalY = localY
		d.dispatchToWidget(target, e, chain)
		e.Release()
	}

	// Note: Focus is now set on mouse down (for iOS keyboard to work).
	// If dragging occurred, focus was already blurred in DispatchMouseMove.

	// Check for click (mouse up on same widget as mouse down)
	if d.pressedWidget != nil {
		// Clear pressed state on entire chain
		for _, w := range d.pressedChain {
			w.setPressed(false)
		}

		// If mouse up happened outside the pressed widget, still notify it
		// This is important for drag operations (like sliders) that need to know when dragging ends
		if target != d.pressedWidget && button == d.pressedButton && !wasDragging {
			bounds := d.pressedWidget.ComputedBounds()
			pressedLocalX, pressedLocalY := bounds.LocalPoint(screenX, screenY)
			e := NewMouseEvent(EventMouseUp, screenX, screenY, button, mods)
			e.LocalX = pressedLocalX
			e.LocalY = pressedLocalY
			d.pressedWidget.HandleEvent(e, PhaseBubble)
			e.Release()
		}

		// Only trigger click if we weren't drag-scrolling
		if target == d.pressedWidget && button == d.pressedButton && !wasDragging {
			// It's a click! Check for double-click
			d.handleClick(target, screenX, screenY, localX, localY, button, mods, chain)
		}

		d.pressedWidget = nil
		d.pressedChain = nil
		d.pressedButton = MouseButtonNone
	}
}

// handleClick processes a click and detects double-clicks.
func (d *EventDispatcher) handleClick(target *Widget, screenX, screenY, localX, localY float32, button MouseButton, mods Modifiers, chain []*Widget) {
	now := time.Now()

	// Check for double-click
	timeDiff := now.Sub(d.lastClickTime)
	distX := screenX - d.lastClickX
	distY := screenY - d.lastClickY
	dist := distX*distX + distY*distY

	if timeDiff <= d.doubleClickTime && dist <= d.doubleClickDist*d.doubleClickDist {
		d.clickCount++
	} else {
		d.clickCount = 1
	}

	d.lastClickTime = now
	d.lastClickX = screenX
	d.lastClickY = screenY

	// Dispatch click event
	e := NewMouseEvent(EventClick, screenX, screenY, button, mods)
	e.LocalX = localX
	e.LocalY = localY
	e.ClickCount = d.clickCount
	d.dispatchToWidget(target, e, chain)
	e.Release()

	// Dispatch double-click if applicable
	if d.clickCount == 2 {
		e := NewMouseEvent(EventDoubleClick, screenX, screenY, button, mods)
		e.LocalX = localX
		e.LocalY = localY
		e.ClickCount = 2
		d.dispatchToWidget(target, e, chain)
		e.Release()
	}

	// Dispatch triple-click if applicable
	if d.clickCount >= 3 {
		e := NewMouseEvent(EventTripleClick, screenX, screenY, button, mods)
		e.LocalX = localX
		e.LocalY = localY
		e.ClickCount = 3
		d.dispatchToWidget(target, e, chain)
		e.Release()
		// Reset click count after triple-click to avoid quad-click etc.
		d.clickCount = 0
	}
}

// DispatchMouseWheel handles scroll wheel events.
// If no widget handles the scroll, it bubbles up to find scrollable containers.
func (d *EventDispatcher) DispatchMouseWheel(screenX, screenY, deltaX, deltaY float32, mods Modifiers) {
	result := d.HitTest(screenX, screenY)
	if result == nil {
		return
	}

	e := NewMouseEvent(EventMouseWheel, screenX, screenY, MouseButtonNone, mods)
	e.LocalX = result.LocalX
	e.LocalY = result.LocalY
	e.DeltaX = deltaX
	e.DeltaY = deltaY
	d.dispatchToWidget(result.Widget, e, result.Chain)

	// If event wasn't handled, try default scroll behavior on parent containers
	if !e.IsPropagationStopped() {
		// Check the chain for scrollable containers (bottom-up)
		for i := len(result.Chain) - 1; i >= 0; i-- {
			w := result.Chain[i]
			if d.handleDefaultScroll(w, deltaX, deltaY) {
				break
			}
		}
	}

	e.Release()
}

// handleDefaultScroll applies default scroll behavior to a widget if it's scrollable.
// Returns true if the widget handled the scroll.
func (d *EventDispatcher) handleDefaultScroll(w *Widget, deltaX, deltaY float32) bool {
	w.mu.Lock()

	// Check if widget has scroll/auto overflow
	canScrollY := w.overflowY == "scroll" || w.overflowY == "auto" || (w.kind == KindScrollView && w.scrollEnabled)
	canScrollX := w.overflowX == "scroll" || w.overflowX == "auto"

	if !canScrollY && !canScrollX {
		w.mu.Unlock()
		return false
	}

	// Get viewport dimensions from computed layout
	viewportWidth := w.computedLayout.Width - w.padding[1] - w.padding[3]
	viewportHeight := w.computedLayout.Height - w.padding[0] - w.padding[2]

	// Calculate content size - use intrinsic size of children, not their constrained layout size
	// This is critical because children may be constrained to fit the viewport, but their
	// natural content size determines how much we can scroll
	contentWidth := float32(0)
	contentHeight := float32(0)

	// If contentWidth/contentHeight are explicitly set, use those
	if w.contentWidth > 0 {
		contentWidth = w.contentWidth
	}
	if w.contentHeight > 0 {
		contentHeight = w.contentHeight
	}

	// Otherwise calculate from children's intrinsic sizes
	if contentWidth == 0 || contentHeight == 0 {
		children := w.children
		padding := w.padding
		gap := w.gap
		w.mu.Unlock() // Unlock before calling calculateIntrinsicSize (which needs to lock children)

		// Sum up intrinsic heights of all children (assuming vertical scroll)
		totalIntrinsicHeight := float32(0)
		maxIntrinsicWidth := float32(0)
		numChildren := 0

		for _, child := range children {
			childW, childH := calculateIntrinsicSize(child, viewportWidth)
			totalIntrinsicHeight += childH
			if childW > maxIntrinsicWidth {
				maxIntrinsicWidth = childW
			}
			numChildren++
		}

		// Add gaps between children
		if numChildren > 1 {
			totalIntrinsicHeight += gap * float32(numChildren-1)
		}

		// Add padding
		if contentWidth == 0 {
			contentWidth = maxIntrinsicWidth + padding[1] + padding[3]
		}
		if contentHeight == 0 {
			contentHeight = totalIntrinsicHeight + padding[0] + padding[2]
		}

		w.mu.Lock() // Re-lock for the scroll update
	}

	scrolled := false

	// Apply Y scroll
	if canScrollY && deltaY != 0 {
		maxScrollY := contentHeight - viewportHeight
		if maxScrollY < 0 {
			maxScrollY = 0
		}

		oldScrollY := w.scrollY
		w.scrollY += deltaY // Drag up (positive delta) = scroll down (increase scrollY)

		// Clamp
		if w.scrollY < 0 {
			w.scrollY = 0
		}
		if w.scrollY > maxScrollY {
			w.scrollY = maxScrollY
		}

		if w.scrollY != oldScrollY {
			scrolled = true
		}
	}

	// Apply X scroll
	if canScrollX && deltaX != 0 {
		maxScrollX := contentWidth - viewportWidth
		if maxScrollX < 0 {
			maxScrollX = 0
		}

		oldScrollX := w.scrollX
		w.scrollX += deltaX // Drag left (positive delta) = scroll right (increase scrollX)

		// Clamp
		if w.scrollX < 0 {
			w.scrollX = 0
		}
		if w.scrollX > maxScrollX {
			w.scrollX = maxScrollX
		}

		if w.scrollX != oldScrollX {
			scrolled = true
		}
	}

	if scrolled {
		w.dirtyMask |= DirtyScroll
		if w.tree != nil {
			w.tree.notifyUpdate(w, DirtyScroll)
		}
	}

	w.mu.Unlock()
	return scrolled
}

// isScrollable returns true if the widget can be scrolled.
func (d *EventDispatcher) isScrollable(w *Widget) bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.overflowY == "scroll" || w.overflowY == "auto" ||
		w.overflowX == "scroll" || w.overflowX == "auto" ||
		(w.kind == KindScrollView && w.scrollEnabled)
}

// IsMomentumScrolling returns true if momentum scrolling is currently active.
func (d *EventDispatcher) IsMomentumScrolling() bool {
	return d.momentumActive
}

// UpdateMomentumScroll applies momentum scrolling physics.
// Returns true if scrolling occurred and a redraw is needed.
// Should be called each frame while IsMomentumScrolling() returns true.
func (d *EventDispatcher) UpdateMomentumScroll() bool {
	if !d.momentumActive || d.momentumTarget == nil {
		return false
	}

	now := time.Now()
	dt := float32(now.Sub(d.lastMomentumTime).Seconds())
	d.lastMomentumTime = now

	// Apply friction (deceleration)
	friction := float32(0.95) // Per-frame multiplier
	d.momentumVelocityX *= friction
	d.momentumVelocityY *= friction

	// Calculate scroll delta for this frame
	deltaX := d.momentumVelocityX * dt
	deltaY := d.momentumVelocityY * dt

	// Stop if velocity is negligible
	if abs(d.momentumVelocityX) < 10 && abs(d.momentumVelocityY) < 10 {
		d.momentumActive = false
		d.momentumTarget = nil
		return false
	}

	// Apply scroll
	scrolled := d.handleDefaultScroll(d.momentumTarget, deltaX, deltaY)

	// Stop momentum if scroll hit bounds (couldn't scroll)
	if !scrolled {
		d.momentumActive = false
		d.momentumTarget = nil
	}

	return scrolled
}

// ============================================================================
// Keyboard Event Dispatch
// ============================================================================

// DispatchKeyDown handles key press events.
func (d *EventDispatcher) DispatchKeyDown(keyCode uint32, key string, mods Modifiers, repeat bool) {
	if d.focusedWidget == nil {
		return
	}

	e := NewKeyEvent(EventKeyDown, keyCode, key, 0, mods, repeat)
	d.dispatchKeyEvent(d.focusedWidget, e)
	e.Release()
}

// DispatchKeyUp handles key release events.
func (d *EventDispatcher) DispatchKeyUp(keyCode uint32, key string, mods Modifiers) {
	if d.focusedWidget == nil {
		return
	}

	e := NewKeyEvent(EventKeyUp, keyCode, key, 0, mods, false)
	d.dispatchKeyEvent(d.focusedWidget, e)
	e.Release()
}

// DispatchKeyPress handles character input events.
func (d *EventDispatcher) DispatchKeyPress(char rune, mods Modifiers) {
	if d.focusedWidget == nil {
		return
	}

	e := NewKeyEvent(EventKeyPress, 0, "", char, mods, false)
	d.dispatchKeyEvent(d.focusedWidget, e)
	e.Release()
}

// dispatchKeyEvent sends a key event to the focused widget with bubbling.
func (d *EventDispatcher) dispatchKeyEvent(target *Widget, e *KeyEvent) {
	// Build the chain from target to root
	chain := d.buildChainToRoot(target)

	e.target = target
	e.currentTarget = target

	// Capture phase (root to target)
	for i := 0; i < len(chain)-1; i++ {
		e.setPhase(PhaseCapture)
		e.setCurrentTarget(chain[i])
		if chain[i].HandleEvent(e, PhaseCapture) || e.IsPropagationStopped() {
			return
		}
	}

	// Target phase
	e.setPhase(PhaseTarget)
	e.setCurrentTarget(target)
	if target.HandleEvent(e, PhaseTarget) || e.IsPropagationStopped() {
		return
	}

	// Bubble phase (target to root)
	for i := len(chain) - 2; i >= 0; i-- {
		e.setPhase(PhaseBubble)
		e.setCurrentTarget(chain[i])
		if chain[i].HandleEvent(e, PhaseBubble) || e.IsPropagationStopped() {
			return
		}
	}
}

// ============================================================================
// Focus Management
// ============================================================================

// isTextInputWidget returns true if the widget is a text input (TextField or TextArea)
func isTextInputWidget(w *Widget) bool {
	if w == nil {
		return false
	}
	kind := w.Kind()
	return kind == KindTextField || kind == KindTextArea
}

// setFocus changes the focused widget, dispatching blur/focus events.
// On mobile platforms (iOS/Android), automatically shows/hides the software keyboard
// when focusing/blurring text input widgets.
func (d *EventDispatcher) setFocus(newFocus *Widget) {
	oldFocus := d.focusedWidget

	if oldFocus == newFocus {
		return
	}

	// Blur the old widget
	if oldFocus != nil {
		oldFocus.setFocused(false)
		e := NewFocusEvent(EventBlur, newFocus)
		e.target = oldFocus
		oldFocus.HandleEvent(e, PhaseBubble)
	}

	d.focusedWidget = newFocus

	// Focus the new widget
	if newFocus != nil {
		newFocus.setFocused(true)
		e := NewFocusEvent(EventFocus, oldFocus)
		e.target = newFocus
		newFocus.HandleEvent(e, PhaseBubble)
	}

	// On mobile platforms, automatically manage software keyboard for text inputs
	if runtime.GOOS == "ios" || runtime.GOOS == "android" {
		oldIsTextInput := isTextInputWidget(oldFocus)
		newIsTextInput := isTextInputWidget(newFocus)

		if newIsTextInput && !oldIsTextInput {
			// Focusing a text input - show keyboard
			ffi.KeyboardShow()
		} else if oldIsTextInput && !newIsTextInput {
			// Blurring a text input (focus moved elsewhere or nil) - hide keyboard
			ffi.KeyboardHide()
		}
		// If both are text inputs, keyboard stays visible
	}
}

// FocusedWidget returns the currently focused widget.
func (d *EventDispatcher) FocusedWidget() *Widget {
	return d.focusedWidget
}

// Focus sets focus to a specific widget.
func (d *EventDispatcher) Focus(w *Widget) {
	d.setFocus(w)
}

// Blur removes focus from the currently focused widget.
func (d *EventDispatcher) Blur() {
	d.setFocus(nil)
}

// ============================================================================
// Hover State Management
// ============================================================================

// updateHoverState handles the transition between hovered widgets.
// It properly tracks the entire hover chain so that parent widgets stay hovered
// when the mouse moves to a child widget.
func (d *EventDispatcher) updateHoverState(newHovered *Widget, screenX, screenY float32, mods Modifiers, newChain []*Widget) {
	oldChain := d.hoveredChain

	// Build sets for efficient lookup
	oldSet := make(map[*Widget]bool, len(oldChain))
	for _, w := range oldChain {
		oldSet[w] = true
	}
	newSet := make(map[*Widget]bool, len(newChain))
	for _, w := range newChain {
		newSet[w] = true
	}

	// Dispatch MouseLeave to widgets that were hovered but are no longer in the chain
	// Process in reverse order (deepest first) for proper event order
	for i := len(oldChain) - 1; i >= 0; i-- {
		w := oldChain[i]
		if !newSet[w] {
			w.setHovered(false)

			bounds := w.ComputedBounds()
			localX, localY := bounds.LocalPoint(screenX, screenY)

			e := NewMouseEvent(EventMouseLeave, screenX, screenY, MouseButtonNone, mods)
			e.LocalX = localX
			e.LocalY = localY
			e.target = w
			w.HandleEvent(e, PhaseBubble)
			e.Release()
		}
	}

	// Dispatch MouseEnter to widgets that are newly in the chain
	// Process in order (root first) for proper event order
	for _, w := range newChain {
		if !oldSet[w] {
			w.setHovered(true)

			bounds := w.ComputedBounds()
			localX, localY := bounds.LocalPoint(screenX, screenY)

			e := NewMouseEvent(EventMouseEnter, screenX, screenY, MouseButtonNone, mods)
			e.LocalX = localX
			e.LocalY = localY
			e.target = w
			w.HandleEvent(e, PhaseBubble)
			e.Release()
		}
	}

	// Update state
	d.hoveredWidget = newHovered
	d.hoveredChain = newChain
}

// HoveredWidget returns the currently hovered widget.
func (d *EventDispatcher) HoveredWidget() *Widget {
	return d.hoveredWidget
}

// chainsEqual compares two widget chains for equality.
func (d *EventDispatcher) chainsEqual(a, b []*Widget) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// ============================================================================
// Event Dispatch Helpers
// ============================================================================

// dispatchToWidget dispatches an event to a widget with capture/bubble phases.
func (d *EventDispatcher) dispatchToWidget(target *Widget, e Event, chain []*Widget) {
	if target == nil {
		return
	}

	// Set target
	e.(*MouseEvent).target = target

	// Capture phase (from root towards target)
	for i := 0; i < len(chain)-1; i++ {
		e.setPhase(PhaseCapture)
		e.setCurrentTarget(chain[i])

		// Check for custom responder first
		if r := chain[i].GetResponder(); r != nil {
			if r.HandleEvent(e, PhaseCapture) || e.IsPropagationStopped() {
				return
			}
		} else if chain[i].HandleEvent(e, PhaseCapture) || e.IsPropagationStopped() {
			return
		}
	}

	// Target phase
	e.setPhase(PhaseTarget)
	e.setCurrentTarget(target)
	if r := target.GetResponder(); r != nil {
		if r.HandleEvent(e, PhaseTarget) || e.IsPropagationStopped() {
			return
		}
	} else if target.HandleEvent(e, PhaseTarget) || e.IsPropagationStopped() {
		return
	}

	// Bubble phase (from target towards root)
	for i := len(chain) - 2; i >= 0; i-- {
		e.setPhase(PhaseBubble)
		e.setCurrentTarget(chain[i])

		if r := chain[i].GetResponder(); r != nil {
			if r.HandleEvent(e, PhaseBubble) || e.IsPropagationStopped() {
				return
			}
		} else if chain[i].HandleEvent(e, PhaseBubble) || e.IsPropagationStopped() {
			return
		}
	}
}

// buildChainToRoot builds a slice from root to the given widget.
func (d *EventDispatcher) buildChainToRoot(w *Widget) []*Widget {
	// First, build from widget to root
	var reversedChain []*Widget
	for current := w; current != nil; current = current.Parent() {
		reversedChain = append(reversedChain, current)
	}

	// Reverse to get root-to-widget order
	chain := make([]*Widget, len(reversedChain))
	for i, w := range reversedChain {
		chain[len(reversedChain)-1-i] = w
	}

	return chain
}
