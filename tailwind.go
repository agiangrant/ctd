package ctd

import (
	"sync"

	"github.com/agiangrant/ctd/tw"
)

// styleCache caches parsed styles for repeated class strings.
// This provides O(1) lookup after first parse.
var (
	styleCache   = make(map[string]*tw.ComputedStyles)
	styleCacheMu sync.RWMutex
)

// resolveStyles returns cached or freshly parsed styles for a class string.
func resolveStyles(classes string) *tw.ComputedStyles {
	if classes == "" {
		return nil
	}

	// Check cache first (read lock)
	styleCacheMu.RLock()
	if cached, ok := styleCache[classes]; ok {
		styleCacheMu.RUnlock()
		return cached
	}
	styleCacheMu.RUnlock()

	// Parse and cache (write lock)
	styleCacheMu.Lock()
	defer styleCacheMu.Unlock()

	// Double-check after acquiring write lock
	if cached, ok := styleCache[classes]; ok {
		return cached
	}

	styles := tw.ParseClasses(classes)
	styleCache[classes] = &styles
	return &styles
}

// ApplyStyles applies parsed Tailwind styles to a widget.
// This is called automatically when classes are set via SetClasses.
func (w *Widget) ApplyStyles(styles *tw.ComputedStyles) *Widget {
	if styles == nil {
		return w
	}

	w.mu.Lock()
	defer w.mu.Unlock()

	// Apply base styles
	applyStyleProperties(w, &styles.Base)

	// Store the full computed styles for state changes (hover, focus, etc.)
	w.computedStyles = styles

	return w
}

// applyStyleProperties applies a StyleProperties to widget fields.
// Must be called with lock held.
func applyStyleProperties(w *Widget, props *tw.StyleProperties) {
	// Animation (store for later startup by Loop)
	if props.Animation != nil && *props.Animation != "none" {
		w.pendingAnimation = *props.Animation
		// Store custom config if provided
		if props.AnimationDuration != nil {
			w.pendingAnimDuration = *props.AnimationDuration
		}
		if props.AnimationEasing != nil {
			w.pendingAnimEasing = *props.AnimationEasing
		}
		if props.AnimationIterations != nil {
			// Convert 0 (infinite) to -1 for our internal representation
			if *props.AnimationIterations == 0 {
				w.pendingAnimIterations = -1 // infinite
			} else {
				w.pendingAnimIterations = *props.AnimationIterations
			}
		}
	} else if props.Animation != nil && *props.Animation == "none" {
		w.pendingAnimation = ""
		w.pendingAnimDuration = 0
		w.pendingAnimEasing = ""
		w.pendingAnimIterations = 0
		// Cancel any active class-based animation
		if w.activeAnimation != nil {
			w.activeAnimation.Cancel()
			w.activeAnimation = nil
		}
	}

	// Colors - only mark dirty if value actually changed
	if props.BackgroundColor != nil && (w.backgroundColor == nil || *w.backgroundColor != *props.BackgroundColor) {
		w.backgroundColor = props.BackgroundColor
		w.dirtyMask |= DirtyBackground
	}
	if props.TextColor != nil && w.textColor != *props.TextColor {
		w.textColor = *props.TextColor
		w.dirtyMask |= DirtyText
	}
	if props.BorderColor != nil && (w.borderColor == nil || *w.borderColor != *props.BorderColor) {
		w.borderColor = props.BorderColor
		w.dirtyMask |= DirtyBorder
	}

	// Typography - only mark dirty if value actually changed
	if props.FontFamily != nil && w.fontFamily != *props.FontFamily {
		w.fontFamily = *props.FontFamily
		w.dirtyMask |= DirtyText
	}
	if props.FontSize != nil && w.fontSize != *props.FontSize {
		w.fontSize = *props.FontSize
		w.dirtyMask |= DirtyText
	}
	if props.LineHeight != nil && w.lineHeight != *props.LineHeight {
		w.lineHeight = *props.LineHeight
		w.dirtyMask |= DirtyText
	}
	if props.TextAlign != nil && w.textAlign != *props.TextAlign {
		w.textAlign = *props.TextAlign
		w.dirtyMask |= DirtyText
	}

	// Spacing - Padding - only mark dirty if values actually changed
	if props.PaddingTop != nil && w.padding[0] != *props.PaddingTop {
		w.padding[0] = *props.PaddingTop
		w.dirtyMask |= DirtySize
	}
	if props.PaddingRight != nil && w.padding[1] != *props.PaddingRight {
		w.padding[1] = *props.PaddingRight
		w.dirtyMask |= DirtySize
	}
	if props.PaddingBottom != nil && w.padding[2] != *props.PaddingBottom {
		w.padding[2] = *props.PaddingBottom
		w.dirtyMask |= DirtySize
	}
	if props.PaddingLeft != nil && w.padding[3] != *props.PaddingLeft {
		w.padding[3] = *props.PaddingLeft
		w.dirtyMask |= DirtySize
	}

	// Sizing - only mark dirty if value actually changed
	if props.Width != nil && w.width != *props.Width {
		w.width = *props.Width
		w.dirtyMask |= DirtySize
	}
	if props.Height != nil && w.height != *props.Height {
		w.height = *props.Height
		w.dirtyMask |= DirtySize
	}
	if props.MinWidth != nil && (w.minWidth == nil || *w.minWidth != *props.MinWidth) {
		w.minWidth = props.MinWidth
		w.dirtyMask |= DirtySize
	}
	if props.MinHeight != nil && (w.minHeight == nil || *w.minHeight != *props.MinHeight) {
		w.minHeight = props.MinHeight
		w.dirtyMask |= DirtySize
	}
	if props.MaxWidth != nil && (w.maxWidth == nil || *w.maxWidth != *props.MaxWidth) {
		w.maxWidth = props.MaxWidth
		w.dirtyMask |= DirtySize
	}
	if props.MaxHeight != nil && (w.maxHeight == nil || *w.maxHeight != *props.MaxHeight) {
		w.maxHeight = props.MaxHeight
		w.dirtyMask |= DirtySize
	}

	// Size modes for responsive/flex sizing - only mark dirty if value changed
	if props.WidthMode != nil {
		var newMode SizeMode
		switch *props.WidthMode {
		case "auto":
			newMode = SizeAuto
		case "full":
			newMode = SizeFull
		case "percent":
			newMode = SizePercent
		case "flex":
			newMode = SizeFlex
		default:
			newMode = SizeFixed
		}
		if w.widthMode != newMode {
			w.widthMode = newMode
			w.dirtyMask |= DirtySize | DirtyLayout
		}
	}
	if props.HeightMode != nil {
		var newMode SizeMode
		switch *props.HeightMode {
		case "auto":
			newMode = SizeAuto
		case "full":
			newMode = SizeFull
		case "percent":
			newMode = SizePercent
		case "flex":
			newMode = SizeFlex
		default:
			newMode = SizeFixed
		}
		if w.heightMode != newMode {
			w.heightMode = newMode
			w.dirtyMask |= DirtySize | DirtyLayout
		}
	}
	if props.WidthPercent != nil && w.widthPercent != *props.WidthPercent {
		w.widthPercent = *props.WidthPercent
		w.dirtyMask |= DirtySize | DirtyLayout
	}
	if props.HeightPercent != nil && w.heightPercent != *props.HeightPercent {
		w.heightPercent = *props.HeightPercent
		w.dirtyMask |= DirtySize | DirtyLayout
	}

	// Layout - Gap
	if props.Gap != nil && w.gap != *props.Gap {
		w.gap = *props.Gap
		w.dirtyMask |= DirtySize
	}

	// Flexbox - container properties - only mark dirty if value changed
	if props.FlexDirection != nil {
		var newDir FlexDirection
		switch *props.FlexDirection {
		case "row":
			newDir = FlexRow
		case "column":
			newDir = FlexColumn
		case "row-reverse":
			newDir = FlexRowReverse
		case "column-reverse":
			newDir = FlexColumnReverse
		}
		if w.flexDirection != newDir {
			w.flexDirection = newDir
			w.dirtyMask |= DirtySize
		}
	}
	if props.JustifyContent != nil {
		var newJustify JustifyContent
		switch *props.JustifyContent {
		case "start":
			newJustify = JustifyStart
		case "end":
			newJustify = JustifyEnd
		case "center":
			newJustify = JustifyCenter
		case "between":
			newJustify = JustifyBetween
		case "around":
			newJustify = JustifyAround
		case "evenly":
			newJustify = JustifyEvenly
		}
		if w.justifyContent != newJustify {
			w.justifyContent = newJustify
			w.dirtyMask |= DirtySize
		}
	}
	if props.AlignItems != nil {
		var newAlign AlignItems
		switch *props.AlignItems {
		case "start":
			newAlign = AlignStart
		case "end":
			newAlign = AlignEnd
		case "center":
			newAlign = AlignCenter
		case "stretch":
			newAlign = AlignStretch
		case "baseline":
			newAlign = AlignBaseline
		}
		if w.alignItems != newAlign {
			w.alignItems = newAlign
			w.dirtyMask |= DirtySize
		}
	}
	if props.FlexWrap != nil {
		var newWrap FlexWrap
		switch *props.FlexWrap {
		case "nowrap":
			newWrap = FlexNoWrap
		case "wrap":
			newWrap = FlexWrapWrap
		case "wrap-reverse":
			newWrap = FlexWrapReverse
		}
		if w.flexWrap != newWrap {
			w.flexWrap = newWrap
			w.dirtyMask |= DirtySize
		}
	}

	// Flexbox - item properties - only mark dirty if value changed
	if props.FlexGrow != nil && w.flexGrow != *props.FlexGrow {
		w.flexGrow = *props.FlexGrow
		w.dirtyMask |= DirtySize
	}
	if props.FlexShrink != nil && w.flexShrink != *props.FlexShrink {
		w.flexShrink = *props.FlexShrink
		w.dirtyMask |= DirtySize
	}
	if props.FlexBasis != nil && (w.flexBasis == nil || *w.flexBasis != *props.FlexBasis) {
		w.flexBasis = props.FlexBasis
		w.dirtyMask |= DirtySize
	}
	if props.FlexBasisMode != nil {
		var newMode FlexBasisMode
		switch *props.FlexBasisMode {
		case "auto":
			newMode = FlexBasisAuto
		case "fixed":
			newMode = FlexBasisFixed
		case "percent":
			newMode = FlexBasisPercent
		case "full":
			newMode = FlexBasisFull
		}
		if w.flexBasisMode != newMode {
			w.flexBasisMode = newMode
			w.dirtyMask |= DirtySize
		}
	}
	if props.FlexBasisPercent != nil && w.flexBasisPercent != *props.FlexBasisPercent {
		w.flexBasisPercent = *props.FlexBasisPercent
		w.dirtyMask |= DirtySize
	}
	if props.AlignSelf != nil {
		var newAlign AlignSelf
		switch *props.AlignSelf {
		case "auto":
			newAlign = AlignSelfAuto
		case "start":
			newAlign = AlignSelfStart
		case "end":
			newAlign = AlignSelfEnd
		case "center":
			newAlign = AlignSelfCenter
		case "stretch":
			newAlign = AlignSelfStretch
		case "baseline":
			newAlign = AlignSelfBaseline
		}
		if w.alignSelf != newAlign {
			w.alignSelf = newAlign
			w.dirtyMask |= DirtySize
		}
	}
	if props.Order != nil && w.order != *props.Order {
		w.order = *props.Order
		w.dirtyMask |= DirtySize
	}

	// Position - only mark dirty if value changed
	if props.Position != nil {
		var newPos Position
		switch *props.Position {
		case "relative":
			newPos = PositionRelative
		case "absolute":
			newPos = PositionAbsolute
		case "fixed":
			newPos = PositionFixed
		case "sticky":
			newPos = PositionSticky
		default:
			newPos = PositionStatic
		}
		if w.position != newPos {
			w.position = newPos
			w.dirtyMask |= DirtyPosition
		}
	}
	if props.Top != nil && (w.posTop == nil || *w.posTop != *props.Top) {
		val := *props.Top
		w.posTop = &val
		w.dirtyMask |= DirtyPosition
	}
	if props.Right != nil && (w.posRight == nil || *w.posRight != *props.Right) {
		val := *props.Right
		w.posRight = &val
		w.dirtyMask |= DirtyPosition
	}
	if props.Bottom != nil && (w.posBottom == nil || *w.posBottom != *props.Bottom) {
		val := *props.Bottom
		w.posBottom = &val
		w.dirtyMask |= DirtyPosition
	}
	if props.Left != nil && (w.posLeft == nil || *w.posLeft != *props.Left) {
		val := *props.Left
		w.posLeft = &val
		w.dirtyMask |= DirtyPosition
	}
	if props.ZIndex != nil && w.zIndex != *props.ZIndex {
		w.zIndex = *props.ZIndex
		w.dirtyMask |= DirtyPosition
	}

	// Borders - only mark dirty if value changed
	if props.BorderWidth != nil && w.borderWidth != *props.BorderWidth {
		w.borderWidth = *props.BorderWidth
		w.dirtyMask |= DirtyBorder
	}
	if props.BorderRadius != nil {
		radius := *props.BorderRadius
		newRadius := [4]float32{radius, radius, radius, radius}
		if w.cornerRadius != newRadius {
			w.cornerRadius = newRadius
			w.dirtyMask |= DirtyBorder
		}
	}

	// Effects - only mark dirty if value changed
	if props.Opacity != nil && w.opacity != *props.Opacity {
		w.opacity = *props.Opacity
		w.dirtyMask |= DirtyBackground
	}

	// Overflow - only mark dirty if value changed
	if props.OverflowX != nil && w.overflowX != *props.OverflowX {
		w.overflowX = *props.OverflowX
		// Enable scroll for scroll/auto modes
		if *props.OverflowX == "scroll" || *props.OverflowX == "auto" {
			w.scrollEnabled = true
		}
		w.dirtyMask |= DirtyScroll
	}
	if props.OverflowY != nil && w.overflowY != *props.OverflowY {
		w.overflowY = *props.OverflowY
		// Enable scroll for scroll/auto modes
		if *props.OverflowY == "scroll" || *props.OverflowY == "auto" {
			w.scrollEnabled = true
		}
		w.dirtyMask |= DirtyScroll
	}

	// Object fit and position (for images) - only mark dirty if value changed
	if props.ObjectFit != nil && w.imageFit != *props.ObjectFit {
		w.imageFit = *props.ObjectFit
		w.dirtyMask |= DirtySize
	}
	if props.ObjectPosition != nil && w.imagePosition != *props.ObjectPosition {
		w.imagePosition = *props.ObjectPosition
		w.dirtyMask |= DirtySize
	}
}

// SetClasses parses and applies Tailwind classes to the widget.
// Classes are cached after first parse for O(1) subsequent lookups.
func (w *Widget) SetClasses(classes string) *Widget {
	if classes == "" {
		return w
	}

	// Store the class string
	w.mu.Lock()
	w.classes = classes
	w.mu.Unlock()

	// Resolve (cached or parse) and apply styles
	styles := resolveStyles(classes)
	return w.ApplyStyles(styles)
}

// Classes returns the Tailwind class string.
func (w *Widget) Classes() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.classes
}

// UpdateState applies styles for the given interaction state.
// Call this when hover/focus/active state changes.
// This method automatically respects the current dark mode setting from the tree.
func (w *Widget) UpdateState(state tw.State) *Widget {
	w.mu.Lock()

	if w.computedStyles == nil {
		w.mu.Unlock()
		return w
	}

	// Get dark mode state from tree
	tree := w.tree
	darkMode := false
	if tree != nil {
		darkMode = tree.DarkMode()
	}

	// Use the full resolution method that handles dark mode and state
	resolved := w.computedStyles.ResolveForWidthWithDarkModeAndState(0, tw.DefaultBreakpoints(), darkMode, state)
	applyStyleProperties(w, &resolved)

	// Notify tree of change so it triggers a redraw
	// (applyStyleProperties sets dirtyMask but doesn't notify tree)
	dirtyFlags := w.dirtyMask
	w.mu.Unlock()

	// Notify tree outside lock to avoid deadlock
	if tree != nil && dirtyFlags != 0 {
		tree.notifyUpdate(w, dirtyFlags)
	}

	return w
}

// UpdateStateForWidth applies styles with responsive breakpoint resolution.
// This combines breakpoint-aware style resolution with interactive state changes.
// Use this for truly responsive layouts where styles change based on window width.
func (w *Widget) UpdateStateForWidth(state tw.State, windowWidth float32, breakpoints tw.BreakpointConfig) *Widget {
	w.mu.Lock()

	if w.computedStyles == nil {
		w.mu.Unlock()
		return w
	}

	// Use the ResolveForWidthWithState method that handles both breakpoints and state
	resolved := w.computedStyles.ResolveForWidthWithState(windowWidth, breakpoints, state)
	applyStyleProperties(w, &resolved)

	// Notify tree of change
	dirtyFlags := w.dirtyMask
	tree := w.tree
	w.mu.Unlock()

	if tree != nil && dirtyFlags != 0 {
		tree.notifyUpdate(w, dirtyFlags)
	}

	return w
}

// UpdateStateWithDarkMode applies styles considering dark mode preference.
// This is the full style resolution method that handles breakpoints, state, and dark mode.
func (w *Widget) UpdateStateWithDarkMode(state tw.State, windowWidth float32, breakpoints tw.BreakpointConfig, darkMode bool) *Widget {
	w.mu.Lock()

	if w.computedStyles == nil {
		w.mu.Unlock()
		return w
	}

	// Use the full resolution method that handles breakpoints, dark mode, and state
	resolved := w.computedStyles.ResolveForWidthWithDarkModeAndState(windowWidth, breakpoints, darkMode, state)
	applyStyleProperties(w, &resolved)

	// Notify tree of change
	dirtyFlags := w.dirtyMask
	tree := w.tree
	w.mu.Unlock()

	if tree != nil && dirtyFlags != 0 {
		tree.notifyUpdate(w, dirtyFlags)
	}

	return w
}

// ApplyStylesWithDarkMode applies parsed Tailwind styles considering dark mode.
// This is called when initial styles need to respect dark mode.
func (w *Widget) ApplyStylesWithDarkMode(styles *tw.ComputedStyles, darkMode bool) *Widget {
	if styles == nil {
		return w
	}

	w.mu.Lock()
	defer w.mu.Unlock()

	// Resolve styles with dark mode consideration
	resolved := styles.ResolveWithDarkMode(darkMode)
	applyStyleProperties(w, &resolved)

	// Store the full computed styles for state changes (hover, focus, etc.)
	w.computedStyles = styles

	return w
}

// reapplyStylesForBreakpoint walks the widget tree and reapplies styles
// based on the current window width and breakpoint configuration.
// This is called when the window crosses a breakpoint threshold.
func reapplyStylesForBreakpoint(w *Widget, windowWidth float32, breakpoints tw.BreakpointConfig, darkMode bool) {
	if w == nil {
		return
	}

	w.mu.Lock()
	styles := w.computedStyles
	children := acquireWidgetSlice(len(w.children))
	copy(children, w.children)

	if styles != nil {
		// Resolve styles for current breakpoint and dark mode
		resolved := styles.ResolveForWidthWithDarkMode(windowWidth, breakpoints, darkMode)
		applyStyleProperties(w, &resolved)
	}
	w.mu.Unlock()

	// Recursively process children
	for _, child := range children {
		reapplyStylesForBreakpoint(child, windowWidth, breakpoints, darkMode)
	}

	releaseWidgetSlice(children)
}

// ClearStyleCache clears the style cache. Useful for testing or hot reload.
func ClearStyleCache() {
	styleCacheMu.Lock()
	styleCache = make(map[string]*tw.ComputedStyles)
	styleCacheMu.Unlock()
}

// StyleCacheSize returns the number of cached style entries.
func StyleCacheSize() int {
	styleCacheMu.RLock()
	defer styleCacheMu.RUnlock()
	return len(styleCache)
}
