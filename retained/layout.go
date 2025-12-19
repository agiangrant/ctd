package retained

import (
	"fmt"
	"sort"
	"strings"

	"github.com/agiangrant/centered/internal/ffi"
)

var layoutDebug = false // Set to true for debug logging

func debugLog(format string, args ...interface{}) {
	if layoutDebug {
		fmt.Printf(format+"\n", args...)
	}
}

// SizeMode specifies how a dimension (width or height) should be calculated.
type SizeMode int

const (
	// SizeFixed uses an explicit pixel value (default).
	SizeFixed SizeMode = iota

	// SizeAuto sizes to fit content.
	SizeAuto

	// SizeFull fills the parent's available space (w-full, h-full).
	SizeFull

	// SizePercent uses a percentage of parent's size.
	SizePercent

	// SizeFlex distributes remaining space based on flex-grow (flex-1, etc).
	SizeFlex
)

// ComputedLayout stores the resolved position and size after layout pass.
// This is computed once when layout is dirty and reused for rendering.
type ComputedLayout struct {
	// Final computed values in pixels
	X      float32
	Y      float32
	Width  float32
	Height float32

	// Whether this layout is valid (computed and not dirty)
	Valid bool
}

// LayoutConstraints passed from parent to child during layout.
type LayoutConstraints struct {
	// Available space for the child (after parent padding)
	AvailableWidth  float32
	AvailableHeight float32

	// Parent's content box position (for relative positioning)
	ParentX float32
	ParentY float32

	// Containing block for absolute positioning (nearest positioned ancestor)
	// For absolute children, this is the bounds of the nearest ancestor with
	// position: relative, absolute, fixed, or sticky. Defaults to root/viewport.
	ContainingBlockX      float32
	ContainingBlockY      float32
	ContainingBlockWidth  float32
	ContainingBlockHeight float32
}

// calculateIntrinsicSize calculates the minimum size a widget needs based on its content.
// This is used for auto-sizing containers when no explicit size is set.
// availableWidth is used as constraint for height calculation (e.g., text wrapping).
func calculateIntrinsicSize(w *Widget, availableWidth float32) (width, height float32) {
	w.mu.RLock()
	kind := w.kind
	fontSize := w.fontSize
	lineHeight := w.lineHeight
	padding := w.padding
	gap := w.gap
	flexDir := w.flexDirection
	flexWrap := w.flexWrap
	explicitW := w.width
	explicitH := w.height
	widthMode := w.widthMode
	heightMode := w.heightMode
	widthPercent := w.widthPercent
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()
	defer releaseWidgetSlice(children)

	// If explicit size is set, use it
	if widthMode == SizeFixed && explicitW > 0 {
		width = explicitW
	}
	// For SizeFull widgets, use the available width (they fill their parent)
	if widthMode == SizeFull && availableWidth > 0 {
		width = availableWidth
	}
	// For percentage-based widths, calculate from available width
	if widthMode == SizePercent && availableWidth > 0 && widthPercent > 0 {
		width = availableWidth * (widthPercent / 100)
	}
	if heightMode == SizeFixed && explicitH > 0 {
		height = explicitH
	}

	// Text widgets - use font metrics and measured text width
	if kind == KindText || kind == KindButton {
		if fontSize > 0 {
			lh := lineHeight
			if lh == 0 {
				lh = 1.4
			}
			singleLineHeight := fontSize * lh

			// For Text widgets, calculate height based on wrapped lines
			// when width is constrained by parent
			text := w.Text()
			textW := w.TextWidth()
			contentW := availableWidth - padding[1] - padding[3]

			// Count explicit newlines in the text - these create lines regardless of wrapping
			explicitNewlines := strings.Count(text, "\n")

			if kind == KindText && height == 0 {
				if contentW > 0 && textW > contentW {
					// Text will wrap - estimate number of lines needed
					// Use WrapTextWithFont to calculate actual wrapped lines (supports bundled fonts)
					lines := WrapTextWithFont(text, contentW, fontSize, w.FontName(), w.FontFamily())
					numLines := len(lines)
					if numLines < 1 {
						numLines = 1
					}
					height = (fontSize * lh * float32(numLines)) + padding[0] + padding[2]
				} else if explicitNewlines > 0 {
					// Text has explicit newlines but doesn't need word wrapping
					// Each \n creates an additional line
					numLines := explicitNewlines + 1
					height = (fontSize * lh * float32(numLines)) + padding[0] + padding[2]
				} else {
					// Single line, no wrapping needed
					height = singleLineHeight + padding[0] + padding[2]
				}
			} else if height == 0 {
				// Button or height already set
				height = singleLineHeight + padding[0] + padding[2]
			}

			// Use measured text width for proper layout
			if width == 0 {
				width = textW + padding[1] + padding[3]
			}
		}
		return
	}

	// Control widgets - use element sizes + text labels
	switch kind {
	case KindCheckbox, KindRadio:
		// Checkbox/Radio: box/circle (18px) + gap (8px) + text label
		const boxSize = float32(18)
		const gapAfterBox = float32(8)

		// Use font size for text height calculation
		fs := fontSize
		if fs == 0 {
			fs = 14 // default
		}
		lh := lineHeight
		if lh == 0 {
			lh = 1.4
		}
		textHeight := fs * lh

		// Height is max of box and text height
		if height == 0 {
			h := boxSize
			if textHeight > h {
				h = textHeight
			}
			height = h + padding[0] + padding[2]
		}

		// Width includes box + gap + text width
		if width == 0 {
			width = boxSize + gapAfterBox + w.TextWidth() + padding[1] + padding[3]
		}
		return

	case KindToggle:
		// Toggle is fixed size: 44x24
		const toggleWidth = float32(44)
		const toggleHeight = float32(24)

		if width == 0 {
			width = toggleWidth + padding[1] + padding[3]
		}
		if height == 0 {
			height = toggleHeight + padding[0] + padding[2]
		}
		return

	case KindSlider:
		// Slider: minimum width for usability, height based on thumb
		const minTrackWidth = float32(120)
		const thumbSize = float32(16)
		const trackPadding = float32(8) // extra space for thumb on edges

		if width == 0 {
			width = minTrackWidth + trackPadding*2 + padding[1] + padding[3]
		}
		if height == 0 {
			height = thumbSize + padding[0] + padding[2]
		}
		return

	case KindSelect:
		// Select: text + dropdown arrow + padding
		const arrowWidth = float32(20)   // space for dropdown arrow
		const minSelectWidth = float32(120)
		const selectHeight = float32(36)

		fs := fontSize
		if fs == 0 {
			fs = 14
		}

		if width == 0 {
			textW := w.TextWidth()
			if textW < minSelectWidth-arrowWidth {
				textW = minSelectWidth - arrowWidth
			}
			width = textW + arrowWidth + padding[1] + padding[3] + 24 // 24 for inner padding
		}
		if height == 0 {
			height = selectHeight + padding[0] + padding[2]
		}
		return

	case KindTextField:
		// TextField: single line text input
		// Default: fill available width, fixed height based on font size
		const minTextFieldWidth = float32(120)
		const defaultTextFieldHeight = float32(36)

		fs := fontSize
		if fs == 0 {
			fs = 14
		}

		// Width: fill parent (will be set later), or use explicit if set
		if width == 0 {
			// Use available width for text fields - they should fill their container
			if availableWidth > 0 {
				width = availableWidth
			} else {
				width = minTextFieldWidth + padding[1] + padding[3]
			}
		}
		if height == 0 {
			height = defaultTextFieldHeight + padding[0] + padding[2]
		}
		return

	case KindTextArea:
		// TextArea: multi-line text input
		// Default: fill available width, fixed minimum height
		const minTextAreaWidth = float32(120)
		const defaultTextAreaHeight = float32(100)

		fs := fontSize
		if fs == 0 {
			fs = 14
		}

		if width == 0 {
			if availableWidth > 0 {
				width = availableWidth
			} else {
				width = minTextAreaWidth + padding[1] + padding[3]
			}
		}
		if height == 0 {
			height = defaultTextAreaHeight + padding[0] + padding[2]
		}
		return

	case KindButton:
		// Button: intrinsic size based on text content
		// Buttons should have minimum padding and size themselves to content
		const minButtonWidth = float32(60)
		const minButtonHeight = float32(32)

		fs := fontSize
		if fs == 0 {
			fs = 14
		}
		lh := lineHeight
		if lh == 0 {
			lh = 1.4
		}
		textHeight := fs * lh

		if width == 0 {
			textW := w.TextWidth()
			width = textW + padding[1] + padding[3]
			if width < minButtonWidth {
				width = minButtonWidth
			}
		}
		if height == 0 {
			height = textHeight + padding[0] + padding[2]
			if height < minButtonHeight {
				height = minButtonHeight
			}
		}
		return
	}

	// Container widgets - calculate based on children
	if kind == KindVStack || kind == KindHStack || kind == KindContainer || kind == KindZStack {
		contentW := availableWidth - padding[1] - padding[3]
		var maxChildWidth, maxChildHeight, totalChildHeight, totalChildWidth float32

		// Cache intrinsic sizes to avoid O(n²) recalculation for flex-wrap
		// childSizes stores (width, height, position) for each child
		type childSize struct {
			w, h float32
			pos  Position
		}
		childSizes := make([]childSize, len(children))

		// First pass: calculate intrinsic sizes for all children (once)
		for i, child := range children {
			child.mu.RLock()
			pos := child.position
			child.mu.RUnlock()

			childSizes[i].pos = pos

			// Skip absolute/fixed children
			if pos == PositionAbsolute || pos == PositionFixed {
				continue
			}

			childW, childH := calculateIntrinsicSize(child, contentW)
			childSizes[i].w = childW
			childSizes[i].h = childH

			// Track both totals and maxes - which we use depends on layout direction
			totalChildHeight += childH
			totalChildWidth += childW
			if childW > maxChildWidth {
				maxChildWidth = childW
			}
			if childH > maxChildHeight {
				maxChildHeight = childH
			}
		}

		// Count flow children (non-absolute/fixed)
		numChildren := 0
		for _, cs := range childSizes {
			if cs.pos != PositionAbsolute && cs.pos != PositionFixed {
				numChildren++
			}
		}

		// Determine effective layout direction
		// VStack/Container with flex-col: vertical stacking
		// HStack/Container with flex-row: horizontal stacking
		// ZStack: overlay (max both)
		isVerticalStack := kind == KindVStack || (kind == KindContainer && (flexDir == FlexColumn || flexDir == FlexColumnReverse))
		isHorizontalStack := kind == KindHStack || (kind == KindContainer && (flexDir == FlexRow || flexDir == FlexRowReverse))

		// Add gaps between children for stacking layouts
		if numChildren > 1 {
			if isVerticalStack {
				totalChildHeight += gap * float32(numChildren-1)
			} else if isHorizontalStack {
				totalChildWidth += gap * float32(numChildren-1)
			}
		}

		// Calculate final sizes with padding
		if isVerticalStack {
			if width == 0 {
				width = maxChildWidth + padding[1] + padding[3]
			}
			if height == 0 {
				height = totalChildHeight + padding[0] + padding[2]
			}
		} else if isHorizontalStack {
			if width == 0 {
				width = totalChildWidth + padding[1] + padding[3]
			}
			if height == 0 {
				// Check for flex-wrap: simulate line breaking to get total height
				if flexWrap != FlexNoWrap && contentW > 0 {
					// Reuse cached sizes for line breaking calculation (O(n) instead of O(n²))
					type lineInfo struct {
						mainTotal float32
						crossMax  float32
					}
					var lines []lineInfo
					var currentLine lineInfo
					itemCount := 0

					for _, cs := range childSizes {
						if cs.pos == PositionAbsolute || cs.pos == PositionFixed {
							continue
						}

						childW, childH := cs.w, cs.h
						gapForItem := float32(0)
						if itemCount > 0 {
							gapForItem = gap
						}

						// Check if this item would overflow the line
						if itemCount > 0 && currentLine.mainTotal+gapForItem+childW > contentW {
							// Start a new line
							lines = append(lines, currentLine)
							currentLine = lineInfo{mainTotal: childW, crossMax: childH}
							itemCount = 1
						} else {
							// Add to current line
							if itemCount > 0 {
								currentLine.mainTotal += gap
							}
							currentLine.mainTotal += childW
							if childH > currentLine.crossMax {
								currentLine.crossMax = childH
							}
							itemCount++
						}
					}
					// Don't forget the last line
					if itemCount > 0 {
						lines = append(lines, currentLine)
					}

					// Sum line heights plus gaps between lines
					var totalHeight float32
					for _, line := range lines {
						totalHeight += line.crossMax
					}
					if len(lines) > 1 {
						totalHeight += gap * float32(len(lines)-1)
					}
					height = totalHeight + padding[0] + padding[2]
				} else {
					height = maxChildHeight + padding[0] + padding[2]
				}
			}
		} else {
			// ZStack or Container without flex direction: overlay, max both
			if width == 0 {
				width = maxChildWidth + padding[1] + padding[3]
			}
			if height == 0 {
				height = maxChildHeight + padding[0] + padding[2]
			}
		}
	}

	return
}

// ComputeLayout performs a three-pass layout over the widget tree.
// Pass 1 (forward): Resolve widths and positions top-down
// Pass 2 (backward): Propagate heights bottom-up for auto-height containers
// Pass 3 (forward): Re-position children based on updated heights
// This uses dirty tracking to avoid unnecessary recalculations.
// Returns true if any layout was recomputed.
func ComputeLayout(root *Widget, windowWidth, windowHeight float32) bool {
	if root == nil {
		return false
	}

	// Check if layout needs recomputation
	if !needsLayoutPass(root) {
		return false
	}

	// Pre-pass: Batch measure all dirty text widths in a single FFI call
	// This dramatically reduces FFI overhead when many text widgets need measurement
	batchMeasureTextWidths(root)

	// Root widget constraints come from window size
	// The initial containing block is the viewport itself
	constraints := LayoutConstraints{
		AvailableWidth:        windowWidth,
		AvailableHeight:       windowHeight,
		ParentX:               0,
		ParentY:               0,
		ContainingBlockX:      0,
		ContainingBlockY:      0,
		ContainingBlockWidth:  windowWidth,
		ContainingBlockHeight: windowHeight,
	}

	// Pass 1: Compute layout recursively (forward pass - resolves widths and initial heights)
	computeWidgetLayout(root, constraints)

	// Pass 2: Propagate heights backward for auto-height containers
	// This handles the case where text wrapping causes children to grow taller
	propagateHeightsUp(root)

	// Pass 3: Re-position children based on updated heights
	// This fixes sibling positions after height changes propagate up
	repositionChildren(root)

	return true
}

// propagateHeightsUp walks the tree bottom-up and updates auto-height containers
// to reflect the actual heights of their children (after text wrapping).
func propagateHeightsUp(w *Widget) {
	if w == nil {
		return
	}

	w.mu.RLock()
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()
	defer releaseWidgetSlice(children)

	// First, recursively process all children (bottom-up)
	for _, child := range children {
		propagateHeightsUp(child)
	}

	// Now update this widget's height if it's auto-height
	w.mu.Lock()
	defer w.mu.Unlock()

	// Only adjust height for auto-height containers
	if w.heightMode != SizeAuto && w.heightMode != SizeFixed {
		return
	}
	// Skip if explicit height was set
	if w.heightMode == SizeFixed && w.height > 0 {
		return
	}

	// Only process layout containers
	if w.kind != KindVStack && w.kind != KindHStack && w.kind != KindContainer && w.kind != KindZStack {
		return
	}

	// Calculate the required height based on children's computed layouts
	padding := w.padding
	gap := w.gap
	kind := w.kind

	var requiredHeight float32

	if kind == KindVStack {
		// VStack: sum of child heights plus gaps
		var totalChildHeight float32
		numChildren := 0
		for _, child := range children {
			child.mu.RLock()
			pos := child.position
			childH := child.computedLayout.Height
			child.mu.RUnlock()

			if pos == PositionAbsolute || pos == PositionFixed {
				continue
			}
			totalChildHeight += childH
			numChildren++
		}
		if numChildren > 1 {
			totalChildHeight += gap * float32(numChildren-1)
		}
		requiredHeight = totalChildHeight + padding[0] + padding[2]
	} else if kind == KindHStack {
		// HStack: check for flex-wrap (multiple lines)
		flexWrap := w.flexWrap

		if flexWrap != FlexNoWrap {
			// Flex-wrap enabled: group children by Y position (line), sum max heights
			// Children on the same line have the same Y position (within tolerance)
			type lineInfo struct {
				y         float32
				maxHeight float32
			}
			var lines []lineInfo
			const tolerance = float32(0.5)

			for _, child := range children {
				child.mu.RLock()
				pos := child.position
				childY := child.computedLayout.Y
				childH := child.computedLayout.Height
				child.mu.RUnlock()

				if pos == PositionAbsolute || pos == PositionFixed {
					continue
				}

				// Find which line this child belongs to
				foundLine := false
				for i := range lines {
					if childY >= lines[i].y-tolerance && childY <= lines[i].y+tolerance {
						// Same line
						if childH > lines[i].maxHeight {
							lines[i].maxHeight = childH
						}
						foundLine = true
						break
					}
				}
				if !foundLine {
					// New line
					lines = append(lines, lineInfo{y: childY, maxHeight: childH})
				}
			}

			// Sum all line heights plus gaps between lines
			var totalHeight float32
			for _, line := range lines {
				totalHeight += line.maxHeight
			}
			if len(lines) > 1 {
				totalHeight += gap * float32(len(lines)-1)
			}
			requiredHeight = totalHeight + padding[0] + padding[2]
		} else {
			// No wrap: max child height (single line)
			var maxChildHeight float32
			for _, child := range children {
				child.mu.RLock()
				pos := child.position
				childH := child.computedLayout.Height
				child.mu.RUnlock()

				if pos == PositionAbsolute || pos == PositionFixed {
					continue
				}
				if childH > maxChildHeight {
					maxChildHeight = childH
				}
			}
			requiredHeight = maxChildHeight + padding[0] + padding[2]
		}
	} else {
		// ZStack/Container: max child height
		var maxChildHeight float32
		for _, child := range children {
			child.mu.RLock()
			pos := child.position
			childH := child.computedLayout.Height
			child.mu.RUnlock()

			if pos == PositionAbsolute || pos == PositionFixed {
				continue
			}
			if childH > maxChildHeight {
				maxChildHeight = childH
			}
		}
		requiredHeight = maxChildHeight + padding[0] + padding[2]
	}

	// Update height if children require more space
	if requiredHeight > w.computedLayout.Height {
		w.computedLayout.Height = requiredHeight
	}
}

// repositionChildren walks the tree top-down and re-positions children
// based on their updated heights (after the backward pass).
func repositionChildren(w *Widget) {
	if w == nil {
		return
	}

	w.mu.RLock()
	kind := w.kind
	padding := w.padding
	gap := w.gap
	parentX := w.computedLayout.X
	parentY := w.computedLayout.Y
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()
	defer releaseWidgetSlice(children)

	// Only reposition for VStack (vertical layout with sequential positioning)
	// HStack children positions depend on widths which don't change in pass 2
	if kind == KindVStack {
		contentY := parentY + padding[0]
		contentX := parentX + padding[1]

		for _, child := range children {
			child.mu.Lock()
			pos := child.position
			if pos != PositionAbsolute && pos != PositionFixed {
				// Calculate position delta
				oldX := child.computedLayout.X
				oldY := child.computedLayout.Y
				deltaX := contentX - oldX
				deltaY := contentY - oldY

				// Update position
				child.computedLayout.X = contentX
				child.computedLayout.Y = contentY

				// Move cursor down by this child's height + gap
				contentY += child.computedLayout.Height + gap

				child.mu.Unlock()

				// If position changed, propagate delta to all descendants
				if deltaX != 0 || deltaY != 0 {
					propagatePositionDelta(child, deltaX, deltaY)
				}
			} else {
				child.mu.Unlock()
			}
		}
	}

	// Recursively reposition children of children
	for _, child := range children {
		repositionChildren(child)
	}
}

// propagatePositionDelta shifts all descendants by the given delta.
// This is called when a widget's position changes due to siblings resizing.
func propagatePositionDelta(w *Widget, deltaX, deltaY float32) {
	w.mu.RLock()
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()

	for _, child := range children {
		child.mu.Lock()
		child.computedLayout.X += deltaX
		child.computedLayout.Y += deltaY
		child.mu.Unlock()

		// Recursively propagate to grandchildren
		propagatePositionDelta(child, deltaX, deltaY)
	}
	releaseWidgetSlice(children)
}

// needsLayoutPass checks if any widget in the tree needs layout recomputation.
func needsLayoutPass(w *Widget) bool {
	w.mu.RLock()
	needsLayout := w.layoutDirty
	children := w.children
	w.mu.RUnlock()

	if needsLayout {
		return true
	}

	for _, child := range children {
		if needsLayoutPass(child) {
			return true
		}
	}

	return false
}

// computeWidgetLayout computes layout for a single widget and its children.
func computeWidgetLayout(w *Widget, constraints LayoutConstraints) {
	w.mu.Lock()

	debugLog("computeWidgetLayout: kind=%d, constraints: avail(%.0f,%.0f) parent(%.0f,%.0f)",
		w.kind, constraints.AvailableWidth, constraints.AvailableHeight, constraints.ParentX, constraints.ParentY)
	debugLog("  widget props: size(%.0f,%.0f) mode(%d,%d) flexGrow=%.1f",
		w.width, w.height, w.widthMode, w.heightMode, w.flexGrow)

	// Resolve this widget's size based on its size modes
	// Note: For percentage-based sizes, the parent has already calculated our size
	// and passed it as constraints.AvailableWidth/Height. We should use that directly
	// to avoid double-applying the percentage.
	var resolvedWidth, resolvedHeight float32
	if w.widthMode == SizePercent && constraints.AvailableWidth > 0 {
		// Parent already resolved our percentage width - use it directly
		resolvedWidth = constraints.AvailableWidth
	} else {
		resolvedWidth = resolveSize(w.width, w.widthMode, w.widthPercent, constraints.AvailableWidth)
	}
	if w.heightMode == SizePercent && constraints.AvailableHeight > 0 {
		// Parent already resolved our percentage height - use it directly
		resolvedHeight = constraints.AvailableHeight
	} else {
		resolvedHeight = resolveSize(w.height, w.heightMode, w.heightPercent, constraints.AvailableHeight)
	}

	debugLog("  after resolveSize: (%.0f,%.0f)", resolvedWidth, resolvedHeight)

	// For flex items: if size resolved to 0, use the available space from parent's flex calculation
	// This is critical for flex-1 items where the parent distributes remaining space
	if w.flexGrow > 0 {
		if resolvedWidth == 0 && constraints.AvailableWidth > 0 {
			resolvedWidth = constraints.AvailableWidth
			debugLog("  flex-item width adjusted to: %.0f", resolvedWidth)
		}
		if resolvedHeight == 0 && constraints.AvailableHeight > 0 {
			resolvedHeight = constraints.AvailableHeight
			debugLog("  flex-item height adjusted to: %.0f", resolvedHeight)
		}
	}

	// For container widgets (HStack/VStack), if size resolved to 0 but parent calculated
	// an intrinsic size for us (via constraints), adopt that size
	// EXCEPT for absolute/fixed positioned elements - they use intrinsic sizing or edge-based sizing
	position := w.position
	if w.kind == KindHStack || w.kind == KindVStack || w.kind == KindContainer {
		if position != PositionAbsolute && position != PositionFixed {
			if resolvedWidth == 0 && constraints.AvailableWidth > 0 {
				resolvedWidth = constraints.AvailableWidth
				debugLog("  container width from parent: %.0f", resolvedWidth)
			}
			if resolvedHeight == 0 && constraints.AvailableHeight > 0 {
				resolvedHeight = constraints.AvailableHeight
				debugLog("  container height from parent: %.0f", resolvedHeight)
			}
		}
	}

	// Text widgets: adopt parent width for wrapping, then calculate height based on wrapped text
	if w.kind == KindText {
		// Text widgets should fill their parent's available width (enables text wrapping)
		if resolvedWidth == 0 && constraints.AvailableWidth > 0 {
			resolvedWidth = constraints.AvailableWidth
		}

		// Calculate height based on wrapped text if we have a width constraint
		if resolvedHeight == 0 && w.fontSize > 0 {
			lineHeight := w.lineHeight
			if lineHeight == 0 {
				lineHeight = 1.4 // Default line height multiplier when not configured
			}

			// Check if text needs wrapping
			textW := w.textWidthLocked()
			contentW := resolvedWidth - w.padding[1] - w.padding[3]

			if contentW > 0 && textW > contentW {
				// Text will wrap - calculate actual wrapped lines
				text := w.text
				fontSize := w.fontSize
				fontName := w.fontName
				fontFamily := w.fontFamily
				// Release lock temporarily for WrapTextWithFont (it may call FFI)
				w.mu.Unlock()
				lines := WrapTextWithFont(text, contentW, fontSize, fontName, fontFamily)
				w.mu.Lock()
				numLines := len(lines)
				if numLines < 1 {
					numLines = 1
				}
				resolvedHeight = (w.fontSize * lineHeight * float32(numLines)) + w.padding[0] + w.padding[2]
			} else {
				// Single line
				resolvedHeight = w.fontSize*lineHeight + w.padding[0] + w.padding[2]
			}
		}
	}
	// Button height needs to include padding
	if w.kind == KindButton && resolvedHeight == 0 && w.fontSize > 0 {
		lineHeight := w.lineHeight
		if lineHeight == 0 {
			lineHeight = 1.4
		}
		resolvedHeight = w.fontSize*lineHeight + w.padding[0] + w.padding[2]
		if resolvedHeight < 32 {
			resolvedHeight = 32
		}
	}

	// Auto-size for control widgets when height resolves to 0
	// Use intrinsic size calculation to get proper dimensions
	if resolvedHeight == 0 {
		switch w.kind {
		case KindCheckbox, KindRadio:
			// Checkbox/Radio: box (18px) or text height, whichever is larger
			const boxSize = float32(18)
			textHeight := float32(0)
			if w.fontSize > 0 {
				lineHeight := w.lineHeight
				if lineHeight == 0 {
					lineHeight = 1.4
				}
				textHeight = w.fontSize * lineHeight
			}
			if boxSize > textHeight {
				resolvedHeight = boxSize
			} else {
				resolvedHeight = textHeight
			}
		case KindToggle:
			// Toggle is fixed size: 24px height
			resolvedHeight = 24
		case KindSlider:
			// Slider height based on thumb size (16px) plus some padding
			resolvedHeight = 20
		case KindSelect:
			// Select dropdown button height
			resolvedHeight = 36
		}
	}

	// Auto-size width for control widgets when width resolves to 0
	// This is critical for hit testing - widgets with 0 width can't receive events
	if resolvedWidth == 0 {
		switch w.kind {
		case KindCheckbox, KindRadio:
			// Checkbox/Radio: box (18px) + spacing (8px) + text width
			const boxSize = float32(18)
			const spacing = float32(8)
			textWidth := w.textWidthLocked() // Use locked version since we hold the lock
			resolvedWidth = boxSize + spacing + textWidth + w.padding[1] + w.padding[3]
		case KindToggle:
			// Toggle is fixed size: 44px width
			resolvedWidth = 44
		case KindSlider, KindSelect, KindTextField, KindTextArea:
			// These should fill parent width
			// If still 0, use available width from constraints
			if constraints.AvailableWidth > 0 {
				resolvedWidth = constraints.AvailableWidth
			}
		case KindButton:
			// Button width based on text content
			textWidth := w.textWidthLocked() // Use locked version since we hold the lock
			resolvedWidth = textWidth + w.padding[1] + w.padding[3]
			if resolvedWidth < 60 {
				resolvedWidth = 60
			}
		}
	}

	// Auto-size height for TextField, TextArea, Button when height resolves to 0
	if resolvedHeight == 0 {
		switch w.kind {
		case KindTextField:
			resolvedHeight = 36
		case KindTextArea:
			resolvedHeight = 100
		case KindButton:
			fs := w.fontSize
			if fs == 0 {
				fs = 14
			}
			lh := w.lineHeight
			if lh == 0 {
				lh = 1.4
			}
			resolvedHeight = fs*lh + w.padding[0] + w.padding[2]
			if resolvedHeight < 32 {
				resolvedHeight = 32
			}
		}
	}

	// Read position offset fields
	posTop := w.posTop
	posRight := w.posRight
	posBottom := w.posBottom
	posLeft := w.posLeft
	// position was already read earlier for the container sizing logic

	// Resolve position based on position mode
	var resolvedX, resolvedY float32
	switch position {
	case PositionStatic:
		// Static: position determined by parent layout (use constraints)
		resolvedX = constraints.ParentX
		resolvedY = constraints.ParentY

	case PositionRelative:
		// Relative: offset from normal flow position using top/left (or legacy x/y)
		resolvedX = constraints.ParentX
		resolvedY = constraints.ParentY
		// Apply offsets from normal position
		if posLeft != nil {
			resolvedX += *posLeft
		} else if posRight != nil {
			resolvedX -= *posRight
		} else {
			resolvedX += w.x // Legacy fallback
		}
		if posTop != nil {
			resolvedY += *posTop
		} else if posBottom != nil {
			resolvedY -= *posBottom
		} else {
			resolvedY += w.y // Legacy fallback
		}

	case PositionAbsolute:
		// Absolute: positioned relative to containing block (nearest positioned ancestor)
		cbX := constraints.ContainingBlockX
		cbY := constraints.ContainingBlockY
		cbW := constraints.ContainingBlockWidth
		cbH := constraints.ContainingBlockHeight

		// Resolve horizontal position
		if posLeft != nil && posRight != nil {
			// Both set: left wins, but could also determine width
			resolvedX = cbX + *posLeft
			// If width not explicitly set, stretch between left and right
			if resolvedWidth == 0 {
				resolvedWidth = cbW - *posLeft - *posRight
				if resolvedWidth < 0 {
					resolvedWidth = 0
				}
			}
		} else if posLeft != nil {
			resolvedX = cbX + *posLeft
		} else if posRight != nil {
			resolvedX = cbX + cbW - resolvedWidth - *posRight
		} else {
			// No horizontal offset specified - default to left edge of containing block
			resolvedX = cbX
		}

		// Resolve vertical position
		if posTop != nil && posBottom != nil {
			// Both set: top wins, but could also determine height
			resolvedY = cbY + *posTop
			// If height not explicitly set, stretch between top and bottom
			if resolvedHeight == 0 {
				resolvedHeight = cbH - *posTop - *posBottom
				if resolvedHeight < 0 {
					resolvedHeight = 0
				}
			}
		} else if posTop != nil {
			resolvedY = cbY + *posTop
		} else if posBottom != nil {
			resolvedY = cbY + cbH - resolvedHeight - *posBottom
		} else {
			// No vertical offset specified - default to top edge of containing block
			resolvedY = cbY
		}

	case PositionFixed:
		// Fixed: positioned relative to viewport (0,0 is top-left of window)
		// Containing block for fixed is always the viewport
		// (Note: in CSS, transforms on ancestors create a new containing block, but we ignore that)

		// Resolve horizontal position
		if posLeft != nil && posRight != nil {
			resolvedX = *posLeft
			if resolvedWidth == 0 {
				// Use window width from initial constraints (passed down)
				// For simplicity, we'll need the root constraints - use containing block as approximation
				viewportW := constraints.ContainingBlockWidth
				resolvedWidth = viewportW - *posLeft - *posRight
				if resolvedWidth < 0 {
					resolvedWidth = 0
				}
			}
		} else if posLeft != nil {
			resolvedX = *posLeft
		} else if posRight != nil {
			viewportW := constraints.ContainingBlockWidth
			resolvedX = viewportW - resolvedWidth - *posRight
		} else {
			resolvedX = 0
		}

		// Resolve vertical position
		if posTop != nil && posBottom != nil {
			resolvedY = *posTop
			if resolvedHeight == 0 {
				viewportH := constraints.ContainingBlockHeight
				resolvedHeight = viewportH - *posTop - *posBottom
				if resolvedHeight < 0 {
					resolvedHeight = 0
				}
			}
		} else if posTop != nil {
			resolvedY = *posTop
		} else if posBottom != nil {
			viewportH := constraints.ContainingBlockHeight
			resolvedY = viewportH - resolvedHeight - *posBottom
		} else {
			resolvedY = 0
		}

	case PositionSticky:
		// Sticky: positioned in normal flow like static/relative
		// The top/left values define the "sticky threshold" (when to stick during scroll),
		// NOT an offset from normal position. The actual sticking happens during render.
		resolvedX = constraints.ParentX
		resolvedY = constraints.ParentY
		// Note: posTop/posLeft are used during render for sticky threshold, not for layout offset
	}

	// For absolute/fixed positioned elements, calculate intrinsic size if size is still 0
	// This makes elements wrap their content when no explicit size or edge-based sizing is used
	if (position == PositionAbsolute || position == PositionFixed) && (resolvedWidth == 0 || resolvedHeight == 0) {
		// Release lock before calling calculateIntrinsicSize (it needs to acquire RLock)
		cbWidth := constraints.ContainingBlockWidth
		w.mu.Unlock()
		intrinsicW, intrinsicH := calculateIntrinsicSize(w, cbWidth)
		w.mu.Lock()

		if resolvedWidth == 0 {
			resolvedWidth = intrinsicW
		}
		if resolvedHeight == 0 {
			resolvedHeight = intrinsicH
		}

		// For right/bottom positioning with intrinsic size, recalculate position
		// (we need the actual size to position from right/bottom edge)
		if posRight != nil && posLeft == nil {
			cbX := constraints.ContainingBlockX
			cbW := constraints.ContainingBlockWidth
			resolvedX = cbX + cbW - resolvedWidth - *posRight
		}
		if posBottom != nil && posTop == nil {
			cbY := constraints.ContainingBlockY
			cbH := constraints.ContainingBlockHeight
			resolvedY = cbY + cbH - resolvedHeight - *posBottom
		}
	}

	// Store computed layout
	w.computedLayout = ComputedLayout{
		X:      resolvedX,
		Y:      resolvedY,
		Width:  resolvedWidth,
		Height: resolvedHeight,
		Valid:  true,
	}
	w.layoutDirty = false

	debugLog("  FINAL layout: (%.0f,%.0f,%.0f,%.0f)", resolvedX, resolvedY, resolvedWidth, resolvedHeight)

	// Copy data needed for child layout
	padding := w.padding
	gap := w.gap
	flexDir := w.flexDirection
	justify := w.justifyContent
	align := w.alignItems
	flexWrap := w.flexWrap
	kind := w.kind
	overflowX := w.overflowX
	overflowY := w.overflowY
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)

	// Determine if this widget becomes a new containing block for descendants
	// A widget is a containing block if it has position: relative, absolute, fixed, or sticky
	isContainingBlock := position == PositionRelative || position == PositionAbsolute ||
		position == PositionFixed || position == PositionSticky

	w.mu.Unlock()

	// Layout children based on this widget's flex properties
	if len(children) > 0 {
		layoutChildren(children, kind, resolvedX, resolvedY, resolvedWidth, resolvedHeight,
			padding, gap, flexDir, justify, align, flexWrap, overflowX, overflowY,
			constraints, isContainingBlock)
	}

	releaseWidgetSlice(children)
}

// resolveSize converts a SizeMode to an actual pixel value.
func resolveSize(explicit float32, mode SizeMode, percent float32, available float32) float32 {
	switch mode {
	case SizeFixed:
		return explicit
	case SizeAuto:
		// Auto returns explicit size if set, otherwise 0 (will be adjusted later)
		if explicit > 0 {
			return explicit
		}
		return 0
	case SizeFull:
		return available
	case SizePercent:
		return available * (percent / 100)
	case SizeFlex:
		// Flex sizing is handled during child layout distribution
		return explicit
	default:
		return explicit
	}
}

// layoutChildren performs flexbox layout on a set of children.
func layoutChildren(
	children []*Widget,
	parentKind WidgetKind,
	parentX, parentY, parentWidth, parentHeight float32,
	padding [4]float32,
	gap float32,
	flexDir FlexDirection,
	justify JustifyContent,
	align AlignItems,
	flexWrap FlexWrap,
	overflowX, overflowY string,
	parentConstraints LayoutConstraints,
	parentIsContainingBlock bool,
) {
	// Content area after padding
	contentX := parentX + padding[3]
	contentY := parentY + padding[0]
	contentWidth := parentWidth - padding[1] - padding[3]
	contentHeight := parentHeight - padding[0] - padding[2]

	// Determine the containing block for absolute children
	// If parent is a containing block (has position: relative/absolute/fixed/sticky),
	// then this parent becomes the containing block. Otherwise, inherit from parent's constraints.
	var containingBlockX, containingBlockY, containingBlockW, containingBlockH float32
	if parentIsContainingBlock {
		// This parent is the new containing block (use its padding box)
		containingBlockX = parentX
		containingBlockY = parentY
		containingBlockW = parentWidth
		containingBlockH = parentHeight
	} else {
		// Inherit containing block from parent
		containingBlockX = parentConstraints.ContainingBlockX
		containingBlockY = parentConstraints.ContainingBlockY
		containingBlockW = parentConstraints.ContainingBlockWidth
		containingBlockH = parentConstraints.ContainingBlockHeight
	}

	// Separate absolute/fixed children from flow children
	type childInfo struct {
		widget       *Widget
		width        float32
		height       float32
		widthMode    SizeMode
		heightMode   SizeMode
		widthPct     float32
		heightPct    float32
		flexGrow     float32
		flexShrink   float32
		flexBasis    float32       // Fixed basis size in pixels
		flexBasisMode FlexBasisMode // How to interpret flex basis
		flexBasisPct float32       // Percentage basis (0-100)
		alignSelf    AlignSelf     // Individual item alignment
		order        int           // Item order for sorting
		position     Position
		relOffsetX   float32
		relOffsetY   float32
		// For text auto-sizing
		kind       WidgetKind
		fontSize   float32
		lineHeight float32
		// For container intrinsic sizing
		padding  [4]float32
		childGap float32
	}

	var flowChildren []childInfo
	var absChildren []*Widget

	for _, child := range children {
		child.mu.RLock()
		var basisVal float32
		if child.flexBasis != nil {
			basisVal = *child.flexBasis
		}
		info := childInfo{
			widget:        child,
			width:         child.width,
			height:        child.height,
			widthMode:     child.widthMode,
			heightMode:    child.heightMode,
			widthPct:      child.widthPercent,
			heightPct:     child.heightPercent,
			flexGrow:      child.flexGrow,
			flexShrink:    child.flexShrink,
			flexBasis:     basisVal,
			flexBasisMode: child.flexBasisMode,
			flexBasisPct:  child.flexBasisPercent,
			alignSelf:     child.alignSelf,
			order:         child.order,
			position:      child.position,
			relOffsetX:    child.x,
			relOffsetY:    child.y,
			kind:          child.kind,
			fontSize:      child.fontSize,
			lineHeight:    child.lineHeight,
			padding:       child.padding,
			childGap:      child.gap,
		}
		child.mu.RUnlock()

		if info.position == PositionAbsolute || info.position == PositionFixed {
			absChildren = append(absChildren, child)
		} else {
			flowChildren = append(flowChildren, info)
		}
	}

	// Sort flow children by order (stable sort to preserve original order for equal values)
	sort.SliceStable(flowChildren, func(i, j int) bool {
		return flowChildren[i].order < flowChildren[j].order
	})

	// Determine axis orientation
	isMainAxisHorizontal := flexDir == FlexRow || flexDir == FlexRowReverse
	if parentKind == KindVStack {
		isMainAxisHorizontal = false
	} else if parentKind == KindHStack {
		isMainAxisHorizontal = true
	}

	// For ZStack, all children get the full content area (overlay behavior)
	// Container with flex classes uses flex layout (handled below)
	if parentKind == KindZStack {
		for _, info := range flowChildren {
			childConstraints := LayoutConstraints{
				AvailableWidth:        contentWidth,
				AvailableHeight:       contentHeight,
				ParentX:               contentX,
				ParentY:               contentY,
				ContainingBlockX:      containingBlockX,
				ContainingBlockY:      containingBlockY,
				ContainingBlockWidth:  containingBlockW,
				ContainingBlockHeight: containingBlockH,
			}
			if info.position == PositionRelative {
				childConstraints.ParentX += info.relOffsetX
				childConstraints.ParentY += info.relOffsetY
			}
			computeWidgetLayout(info.widget, childConstraints)
		}
		// Layout absolute children
		for _, child := range absChildren {
			childConstraints := LayoutConstraints{
				AvailableWidth:        contentWidth,
				AvailableHeight:       contentHeight,
				ParentX:               0,
				ParentY:               0,
				ContainingBlockX:      containingBlockX,
				ContainingBlockY:      containingBlockY,
				ContainingBlockWidth:  containingBlockW,
				ContainingBlockHeight: containingBlockH,
			}
			computeWidgetLayout(child, childConstraints)
		}
		return
	}

	// Resolve sizes and calculate totals for flex distribution
	mainAxisSize := contentWidth
	crossAxisSize := contentHeight
	if !isMainAxisHorizontal {
		mainAxisSize = contentHeight
		crossAxisSize = contentWidth
	}

	// Check if this container allows overflow in the main axis direction
	// When overflow is scroll/auto, children should use intrinsic sizes and can extend beyond container
	isMainAxisScrollable := false
	if isMainAxisHorizontal {
		isMainAxisScrollable = overflowX == "scroll" || overflowX == "auto"
	} else {
		isMainAxisScrollable = overflowY == "scroll" || overflowY == "auto"
	}

	// Calculate total gap space - gaps only appear between items (N-1 gaps for N items)
	var totalGapSpace float32
	if len(flowChildren) > 1 {
		totalGapSpace = gap * float32(len(flowChildren)-1)
	}

	// Available space for percentage calculations (after accounting for gaps)
	availableMainForPercent := mainAxisSize - totalGapSpace

	// First pass: resolve non-flex sizes and count flex items
	var totalFixedMain float32
	var totalFlexGrow float32
	var flexItems []int // indices of flex items in flowChildren

	resolvedSizes := make([]struct{ main, cross float32 }, len(flowChildren))

	for i, info := range flowChildren {
		debugLog("  child[%d] kind=%v flexGrow=%.1f widthMode=%d heightMode=%d",
			i, info.kind, info.flexGrow, info.widthMode, info.heightMode)
		// For percentage-based widths in HStack (or heights in VStack),
		// calculate against available space minus gaps
		availWidthForPercent := contentWidth
		availHeightForPercent := contentHeight
		if isMainAxisHorizontal && info.widthMode == SizePercent {
			availWidthForPercent = availableMainForPercent
		} else if !isMainAxisHorizontal && info.heightMode == SizePercent {
			availHeightForPercent = availableMainForPercent
		}

		// Resolve width
		childWidth := resolveSize(info.width, info.widthMode, info.widthPct, availWidthForPercent)
		// Resolve height
		childHeight := resolveSize(info.height, info.heightMode, info.heightPct, availHeightForPercent)

		// Auto-size text widgets - both width and height
		if (info.kind == KindText || info.kind == KindButton) && info.fontSize > 0 {
			lh := info.lineHeight
			if lh == 0 {
				lh = 1.4 // Default line height multiplier
			}
			singleLineHeight := info.fontSize * lh

			// Get text width for width calculation
			textW := info.widget.TextWidth()

			// Text widget width depends on parent context:
			// - In VStack or Container: fill parent width (enables text wrapping)
			// - In HStack: use intrinsic text width (so siblings get their share)
			if info.kind == KindText && childWidth == 0 {
				if !isMainAxisHorizontal && contentWidth > 0 {
					// VStack or vertical Container: fill parent width for wrapping
					childWidth = contentWidth
				} else if parentKind == KindContainer && contentWidth > 0 {
					// Container (even horizontal): Text should fill it for wrapping
					childWidth = contentWidth
				} else {
					// HStack: use intrinsic text width
					childWidth = textW + info.padding[1] + info.padding[3]
				}
			} else if childWidth == 0 {
				// Non-text widgets use measured text width for buttons
				childWidth = textW + info.padding[1] + info.padding[3]
			}

			// Auto-size height based on whether text will wrap
			if childHeight == 0 {
				childContentW := childWidth - info.padding[1] - info.padding[3]
				if info.kind == KindText && childContentW > 0 && textW > childContentW {
					// Text will wrap - calculate actual wrapped lines
					lines := WrapTextWithFont(info.widget.Text(), childContentW, info.fontSize, info.widget.FontName(), info.widget.FontFamily())
					numLines := len(lines)
					if numLines < 1 {
						numLines = 1
					}
					childHeight = (singleLineHeight * float32(numLines)) + info.padding[0] + info.padding[2]
				} else {
					// Single line
					childHeight = singleLineHeight + info.padding[0] + info.padding[2]
				}
			}
		}

		// Auto-size control widgets
		if childHeight == 0 {
			switch info.kind {
			case KindCheckbox, KindRadio:
				// Checkbox/Radio: box (18px) or text height, whichever is larger
				const boxSize = float32(18)
				textHeight := float32(0)
				if info.fontSize > 0 {
					lh := info.lineHeight
					if lh == 0 {
						lh = 1.4
					}
					textHeight = info.fontSize * lh
				}
				if boxSize > textHeight {
					childHeight = boxSize
				} else {
					childHeight = textHeight
				}
			case KindToggle:
				childHeight = 24
			case KindSlider:
				childHeight = 20
			case KindSelect:
				childHeight = 36
			case KindTextField:
				childHeight = 36 // Default text field height
			case KindTextArea:
				childHeight = 100 // Default text area height
			case KindButton:
				// Button height based on text
				fs := info.fontSize
				if fs == 0 {
					fs = 14
				}
				lh := info.lineHeight
				if lh == 0 {
					lh = 1.4
				}
				childHeight = fs*lh + info.padding[0] + info.padding[2]
				if childHeight < 32 {
					childHeight = 32
				}
			}
		}

		// Auto-size width for control widgets
		if childWidth == 0 {
			switch info.kind {
			case KindCheckbox, KindRadio:
				// Checkbox/Radio: box (18px) + spacing (8px) + text width
				const boxSize = float32(18)
				const spacing = float32(8)
				textWidth := info.widget.TextWidth()
				childWidth = boxSize + spacing + textWidth + info.padding[1] + info.padding[3]
			case KindToggle:
				childWidth = 44
			case KindTextField, KindTextArea, KindSlider, KindSelect:
				// Fill available width in the cross axis direction
				if isMainAxisHorizontal {
					// In HStack, these should have a minimum width
					childWidth = 200 // reasonable default for form inputs
				} else {
					// In VStack, fill the content width
					childWidth = contentWidth
				}
			case KindButton:
				// Button width based on text content
				textWidth := info.widget.TextWidth()
				childWidth = textWidth + info.padding[1] + info.padding[3]
				if childWidth < 60 {
					childWidth = 60
				}
			}
		}

		// Auto-size container widgets (HStack/VStack) when size resolves to 0
		// This calculates intrinsic size based on their children
		if (info.kind == KindHStack || info.kind == KindVStack || info.kind == KindContainer) &&
			(childWidth == 0 || childHeight == 0) {
			intrinsicW, intrinsicH := calculateIntrinsicSize(info.widget, contentWidth)
			if childWidth == 0 && intrinsicW > 0 {
				childWidth = intrinsicW
			}
			if childHeight == 0 && intrinsicH > 0 {
				childHeight = intrinsicH
			}
			debugLog("  child[%d] %s intrinsic size: (%.0f,%.0f)", i, info.kind, intrinsicW, intrinsicH)
		}

		mainSize := childWidth
		crossSize := childHeight
		if !isMainAxisHorizontal {
			mainSize = childHeight
			crossSize = childWidth
		}

		// Apply flex-basis if set (overrides main size for flex items)
		// For percentage-based basis, use available space minus gaps (same as w-1/2, etc.)
		basisSize := float32(0)
		switch info.flexBasisMode {
		case FlexBasisFixed:
			basisSize = info.flexBasis
		case FlexBasisPercent:
			basisSize = availableMainForPercent * (info.flexBasisPct / 100)
		case FlexBasisFull:
			basisSize = availableMainForPercent
		case FlexBasisAuto:
			// Auto uses the main size as calculated above
			basisSize = mainSize
		}

		debugLog("  child[%d] flexBasisMode=%d flexBasisPct=%.1f basisSize=%.1f mainSize=%.1f crossSize=%.1f availableMainForPercent=%.1f",
			i, info.flexBasisMode, info.flexBasisPct, basisSize, mainSize, crossSize, availableMainForPercent)

		// Items with flex-grow > 0 participate in flex distribution
		// This is how Tailwind's flex-1, flex-grow, etc. work
		if info.flexGrow > 0 {
			flexItems = append(flexItems, i)
			totalFlexGrow += info.flexGrow
			// For flex items with an explicit basis (not auto), use that basis
			// This includes basis-0 which means "start at 0 and grow"
			if info.flexBasisMode != FlexBasisAuto {
				totalFixedMain += basisSize
				mainSize = basisSize
			} else if isMainAxisScrollable {
				// In scrollable containers, flex items with auto basis use their intrinsic size
				// and are treated as fixed-size items (no growing)
				totalFixedMain += mainSize
			}
		} else {
			// Non-flex items use their resolved size or basis
			if info.flexBasisMode != FlexBasisAuto && basisSize > 0 {
				mainSize = basisSize
				debugLog("    -> set mainSize to basisSize: %.1f", mainSize)
			}
			totalFixedMain += mainSize
		}

		resolvedSizes[i].main = mainSize
		resolvedSizes[i].cross = crossSize
		debugLog("    -> resolved main=%.1f cross=%.1f", mainSize, crossSize)
	}

	// Group children into flex lines (for flex-wrap support)
	// A line contains children that fit within mainAxisSize
	type flexLine struct {
		startIdx  int     // Index in flowChildren where this line starts
		endIdx    int     // Index in flowChildren where this line ends (exclusive)
		mainTotal float32 // Total main size of items in this line (before flex distribution)
		crossMax  float32 // Maximum cross size among items in this line
		flexGrow  float32 // Total flex-grow for items in this line
		flexItems []int   // Indices of flex items in this line (relative to flowChildren)
	}

	var lines []flexLine

	if flexWrap == FlexNoWrap || isMainAxisScrollable {
		// No wrapping - all children on one line
		var lineFlexGrow float32
		var lineFlexItems []int
		var lineMainTotal float32
		var lineCrossMax float32
		for i, info := range flowChildren {
			lineMainTotal += resolvedSizes[i].main
			if resolvedSizes[i].cross > lineCrossMax {
				lineCrossMax = resolvedSizes[i].cross
			}
			if info.flexGrow > 0 {
				lineFlexGrow += info.flexGrow
				lineFlexItems = append(lineFlexItems, i)
			}
		}
		lines = append(lines, flexLine{
			startIdx:  0,
			endIdx:    len(flowChildren),
			mainTotal: lineMainTotal,
			crossMax:  lineCrossMax,
			flexGrow:  lineFlexGrow,
			flexItems: lineFlexItems,
		})
	} else {
		// Wrapping enabled - group children into lines
		lineStart := 0
		var lineMainTotal float32
		var lineCrossMax float32
		var lineFlexGrow float32
		var lineFlexItems []int
		itemCount := 0

		for i, info := range flowChildren {
			itemSize := resolvedSizes[i].main
			gapForItem := float32(0)
			if itemCount > 0 {
				gapForItem = gap
			}

			// Check if this item would overflow the line
			// Always put at least one item per line
			if itemCount > 0 && lineMainTotal+gapForItem+itemSize > mainAxisSize {
				// Start a new line
				lines = append(lines, flexLine{
					startIdx:  lineStart,
					endIdx:    i,
					mainTotal: lineMainTotal,
					crossMax:  lineCrossMax,
					flexGrow:  lineFlexGrow,
					flexItems: lineFlexItems,
				})
				// Reset for new line
				lineStart = i
				lineMainTotal = itemSize
				lineCrossMax = resolvedSizes[i].cross
				if info.flexGrow > 0 {
					lineFlexGrow = info.flexGrow
					lineFlexItems = []int{i}
				} else {
					lineFlexGrow = 0
					lineFlexItems = nil
				}
				itemCount = 1
			} else {
				// Add to current line
				if itemCount > 0 {
					lineMainTotal += gap
				}
				lineMainTotal += itemSize
				if resolvedSizes[i].cross > lineCrossMax {
					lineCrossMax = resolvedSizes[i].cross
				}
				if info.flexGrow > 0 {
					lineFlexGrow += info.flexGrow
					lineFlexItems = append(lineFlexItems, i)
				}
				itemCount++
			}
		}

		// Don't forget the last line
		if itemCount > 0 {
			lines = append(lines, flexLine{
				startIdx:  lineStart,
				endIdx:    len(flowChildren),
				mainTotal: lineMainTotal,
				crossMax:  lineCrossMax,
				flexGrow:  lineFlexGrow,
				flexItems: lineFlexItems,
			})
		}
	}

	// For flex-wrap-reverse, reverse the order of lines
	if flexWrap == FlexWrapReverse {
		for i, j := 0, len(lines)-1; i < j; i, j = i+1, j-1 {
			lines[i], lines[j] = lines[j], lines[i]
		}
	}

	// Distribute flex-grow within each line and apply to resolvedSizes
	for _, line := range lines {
		if line.flexGrow > 0 && !isMainAxisScrollable {
			numItems := line.endIdx - line.startIdx
			numGaps := 0
			if numItems > 1 {
				numGaps = numItems - 1
			}
			availableForItems := mainAxisSize - gap*float32(numGaps)

			// Calculate items-only total (without gaps)
			// For auto-basis flex items, don't count their intrinsic size since they will be replaced
			var itemsOnlyTotal float32
			for i := line.startIdx; i < line.endIdx; i++ {
				info := flowChildren[i]
				// Skip auto-basis flex items - they will have their size replaced, not added to
				if info.flexGrow > 0 && info.flexBasisMode == FlexBasisAuto {
					continue
				}
				itemsOnlyTotal += resolvedSizes[i].main
			}
			remaining := availableForItems - itemsOnlyTotal

			debugLog("  FLEX DIST: itemsOnlyTotal=%.1f remaining=%.1f lineFlexGrow=%.1f",
			itemsOnlyTotal, remaining, line.flexGrow)
		if remaining > 0 {
				for _, idx := range line.flexItems {
					info := flowChildren[idx]
					extraSpace := remaining * (info.flexGrow / line.flexGrow)
					debugLog("    flex item[%d]: extraSpace=%.1f basisMode=%d oldMain=%.1f",
						idx, extraSpace, info.flexBasisMode, resolvedSizes[idx].main)
					if info.flexBasisMode != FlexBasisAuto {
						resolvedSizes[idx].main += extraSpace
					} else {
						resolvedSizes[idx].main = extraSpace
					}
					debugLog("    flex item[%d]: newMain=%.1f", idx, resolvedSizes[idx].main)
				}

				// Recalculate cross sizes for containers after flex distribution
				if isMainAxisHorizontal {
					for _, idx := range line.flexItems {
						info := flowChildren[idx]
						newWidth := resolvedSizes[idx].main
						if info.kind == KindHStack || info.kind == KindVStack || info.kind == KindContainer {
							_, newHeight := calculateIntrinsicSize(info.widget, newWidth)
							if newHeight > 0 {
								resolvedSizes[idx].cross = newHeight
							}
						}
					}
				}
			}
		}
	}

	// Recalculate line cross maxes after flex distribution (heights may have changed)
	for lineIdx := range lines {
		lines[lineIdx].crossMax = 0
		for i := lines[lineIdx].startIdx; i < lines[lineIdx].endIdx; i++ {
			if resolvedSizes[i].cross > lines[lineIdx].crossMax {
				lines[lineIdx].crossMax = resolvedSizes[i].cross
			}
		}
	}

	// Reverse items within lines if needed (row-reverse or column-reverse)
	isReversed := flexDir == FlexRowReverse || flexDir == FlexColumnReverse
	if isReversed {
		for _, line := range lines {
			// Reverse the slice of flowChildren and resolvedSizes for this line
			for i, j := line.startIdx, line.endIdx-1; i < j; i, j = i+1, j-1 {
				flowChildren[i], flowChildren[j] = flowChildren[j], flowChildren[i]
				resolvedSizes[i], resolvedSizes[j] = resolvedSizes[j], resolvedSizes[i]
			}
		}
	}

	// Position children line by line
	crossCursor := float32(0)
	for _, line := range lines {
		numItems := line.endIdx - line.startIdx
		if numItems == 0 {
			continue
		}

		// Calculate line's main axis total after flex distribution
		var lineMainTotal float32
		for i := line.startIdx; i < line.endIdx; i++ {
			lineMainTotal += resolvedSizes[i].main
		}
		numGaps := 0
		if numItems > 1 {
			numGaps = numItems - 1
		}
		totalGaps := gap * float32(numGaps)

		// Calculate justify-content spacing for this line
		freeSpace := mainAxisSize - lineMainTotal - totalGaps
		var mainStart float32
		var spaceBetween float32

		if isMainAxisScrollable || freeSpace <= 0 {
			mainStart = 0
			spaceBetween = 0
		} else {
			switch justify {
			case JustifyStart:
				mainStart = 0
			case JustifyEnd:
				mainStart = freeSpace
			case JustifyCenter:
				mainStart = freeSpace / 2
			case JustifyBetween:
				if numItems > 1 {
					spaceBetween = freeSpace / float32(numItems-1)
				}
			case JustifyAround:
				spaceBetween = freeSpace / float32(numItems)
				mainStart = spaceBetween / 2
			case JustifyEvenly:
				spaceBetween = freeSpace / float32(numItems+1)
				mainStart = spaceBetween
			}
		}

		// Position each child in this line
		mainCursor := mainStart
		lineCrossSize := line.crossMax
		if flexWrap == FlexNoWrap {
			// Single line uses full cross axis
			lineCrossSize = crossAxisSize
		}

		for i := line.startIdx; i < line.endIdx; i++ {
			info := flowChildren[i]
			mainSize := resolvedSizes[i].main
			crossSize := resolvedSizes[i].cross
			debugLog("  POSITION child[%d]: mainSize=%.1f crossSize=%.1f", i, mainSize, crossSize)

			// Determine effective alignment: align-self overrides align-items
			effectiveAlign := align
			if info.alignSelf != AlignSelfAuto {
				switch info.alignSelf {
				case AlignSelfStart:
					effectiveAlign = AlignStart
				case AlignSelfEnd:
					effectiveAlign = AlignEnd
				case AlignSelfCenter:
					effectiveAlign = AlignCenter
				case AlignSelfStretch:
					effectiveAlign = AlignStretch
				case AlignSelfBaseline:
					effectiveAlign = AlignBaseline
				}
			}

			// Calculate cross-axis position within the line
			var crossPos float32
			switch effectiveAlign {
			case AlignStart:
				crossPos = 0
			case AlignEnd:
				crossPos = lineCrossSize - crossSize
			case AlignCenter:
				crossPos = (lineCrossSize - crossSize) / 2
			case AlignStretch:
				crossPos = 0
				crossSize = lineCrossSize // stretch to fill line
			case AlignBaseline:
				crossPos = 0 // simplified - use start
			}

			// Set child position
			var childX, childY, childW, childH float32
			if isMainAxisHorizontal {
				childX = contentX + mainCursor
				childY = contentY + crossCursor + crossPos
				childW = mainSize
				childH = crossSize
			} else {
				childX = contentX + crossCursor + crossPos
				childY = contentY + mainCursor
				childW = crossSize
				childH = mainSize
			}

			// Apply relative offset
			if info.position == PositionRelative {
				childX += info.relOffsetX
				childY += info.relOffsetY
			}

			childConstraints := LayoutConstraints{
				AvailableWidth:        childW,
				AvailableHeight:       childH,
				ParentX:               childX,
				ParentY:               childY,
				ContainingBlockX:      containingBlockX,
				ContainingBlockY:      containingBlockY,
				ContainingBlockWidth:  containingBlockW,
				ContainingBlockHeight: containingBlockH,
			}
			computeWidgetLayout(info.widget, childConstraints)

			// Move main cursor
			mainCursor += mainSize + gap
			if justify == JustifyBetween || justify == JustifyAround || justify == JustifyEvenly {
				mainCursor += spaceBetween
			}
		}

		// Move cross cursor to next line
		crossCursor += line.crossMax + gap
	}

	// Layout absolute/fixed children
	for _, child := range absChildren {
		childConstraints := LayoutConstraints{
			AvailableWidth:        contentWidth,
			AvailableHeight:       contentHeight,
			ParentX:               0,
			ParentY:               0,
			ContainingBlockX:      containingBlockX,
			ContainingBlockY:      containingBlockY,
			ContainingBlockWidth:  containingBlockW,
			ContainingBlockHeight: containingBlockH,
		}
		computeWidgetLayout(child, childConstraints)
	}
}

// InvalidateLayout marks a widget's layout as needing recomputation.
// This is called when size-affecting properties change.
func InvalidateLayout(w *Widget) {
	w.mu.Lock()
	w.layoutDirty = true
	w.computedLayout.Valid = false
	w.mu.Unlock()
}

// InvalidateTreeLayout marks the entire tree as needing layout recomputation.
func InvalidateTreeLayout(root *Widget) {
	if root == nil {
		return
	}

	var invalidate func(w *Widget)
	invalidate = func(w *Widget) {
		w.mu.Lock()
		w.layoutDirty = true
		w.computedLayout.Valid = false
		children := acquireWidgetSlice(len(w.children))
		copy(children, w.children)
		w.mu.Unlock()

		for _, child := range children {
			invalidate(child)
		}

		releaseWidgetSlice(children)
	}

	invalidate(root)
}

// batchMeasureTextWidths collects all widgets with dirty text measurements
// and measures them in a single FFI call. This dramatically reduces overhead
// when many text widgets need measurement during layout.
func batchMeasureTextWidths(root *Widget) {
	if root == nil {
		return
	}

	// Collect widgets that need text measurement
	var dirtyWidgets []*Widget
	var collectDirty func(w *Widget)
	collectDirty = func(w *Widget) {
		w.mu.RLock()
		text := w.text
		textWidthDirty := w.textWidthDirty
		fontSize := w.fontSize
		children := acquireWidgetSlice(len(w.children))
		copy(children, w.children)
		w.mu.RUnlock()

		// Check if this widget needs text measurement
		if text != "" && fontSize > 0 && textWidthDirty {
			dirtyWidgets = append(dirtyWidgets, w)
		}

		// Recurse into children
		for _, child := range children {
			collectDirty(child)
		}

		releaseWidgetSlice(children)
	}
	collectDirty(root)

	// If no widgets need measurement, we're done
	if len(dirtyWidgets) == 0 {
		return
	}

	// Build batch measurement requests
	requests := make([]ffi.TextMeasurementRequest, len(dirtyWidgets))
	for i, w := range dirtyWidgets {
		w.mu.RLock()
		text := w.text
		fontName := w.fontName
		fontSize := w.fontSize
		w.mu.RUnlock()

		// Use fontName, or fall back to system default if empty
		if fontName == "" {
			fontName = "system"
		}

		requests[i] = ffi.TextMeasurementRequest{
			Text:     text,
			FontName: fontName,
			FontSize: fontSize,
		}
	}

	// Execute batch measurement
	widths := ffi.MeasureTextWidthBatch(requests)

	// Apply results back to widgets
	for i, w := range dirtyWidgets {
		w.mu.Lock()
		w.textWidth = widths[i]
		w.textWidthDirty = false
		w.mu.Unlock()
	}
}

// ClampScrollPositions walks the tree and clamps scroll positions to valid bounds.
// This should be called after layout is computed to handle window resize scenarios
// where the scroll position may exceed the new maximum bounds.
func ClampScrollPositions(root *Widget) {
	if root == nil {
		return
	}

	var clampScroll func(w *Widget)
	clampScroll = func(w *Widget) {
		w.mu.Lock()

		// Check if widget is scrollable
		canScrollY := w.overflowY == "scroll" || w.overflowY == "auto" || (w.kind == KindScrollView && w.scrollEnabled)
		canScrollX := w.overflowX == "scroll" || w.overflowX == "auto"

		if canScrollY || canScrollX {
			// Calculate content bounds (same logic as handleDefaultScroll)
			contentStartX := w.computedLayout.X + w.padding[3]
			contentStartY := w.computedLayout.Y + w.padding[0]

			contentHeight := float32(0)
			contentWidth := float32(0)

			for _, child := range w.children {
				child.mu.RLock()
				childBottom := child.computedLayout.Y - contentStartY + child.computedLayout.Height
				childRight := child.computedLayout.X - contentStartX + child.computedLayout.Width
				child.mu.RUnlock()

				if childBottom > contentHeight {
					contentHeight = childBottom
				}
				if childRight > contentWidth {
					contentWidth = childRight
				}
			}

			// Add bottom/right padding
			contentHeight += w.padding[2]
			contentWidth += w.padding[1]

			viewportHeight := w.computedLayout.Height - w.padding[0] - w.padding[2]
			viewportWidth := w.computedLayout.Width - w.padding[1] - w.padding[3]

			// Clamp Y scroll
			if canScrollY {
				maxScrollY := contentHeight - viewportHeight
				if maxScrollY < 0 {
					maxScrollY = 0
				}
				if w.scrollY > maxScrollY {
					w.scrollY = maxScrollY
				}
				if w.scrollY < 0 {
					w.scrollY = 0
				}
			}

			// Clamp X scroll
			if canScrollX {
				maxScrollX := contentWidth - viewportWidth
				if maxScrollX < 0 {
					maxScrollX = 0
				}
				if w.scrollX > maxScrollX {
					w.scrollX = maxScrollX
				}
				if w.scrollX < 0 {
					w.scrollX = 0
				}
			}
		}

		children := acquireWidgetSlice(len(w.children))
		copy(children, w.children)
		w.mu.Unlock()

		// Recursively process children
		for _, child := range children {
			clampScroll(child)
		}

		releaseWidgetSlice(children)
	}

	clampScroll(root)
}

// SyncBoundsFromLayout updates the cached screen-space bounds (used for hit testing)
// from the computed layout positions. This should be called after ComputeLayout
// to ensure hit testing works correctly before the next render.
// This is especially important after window resize, where the layout changes
// but we haven't rendered yet.
func SyncBoundsFromLayout(root *Widget) {
	if root == nil {
		return
	}

	var syncBounds func(w *Widget, depth int)
	syncBounds = func(w *Widget, depth int) {
		w.mu.Lock()
		layout := w.computedLayout
		// Update bounds from layout
		w.computedBounds = Bounds{
			X:      layout.X,
			Y:      layout.Y,
			Width:  layout.Width,
			Height: layout.Height,
		}
		children := acquireWidgetSlice(len(w.children))
		copy(children, w.children)
		w.mu.Unlock()

		// Recursively sync children
		for _, child := range children {
			syncBounds(child, depth+1)
		}

		releaseWidgetSlice(children)
	}

	syncBounds(root, 0)
}
