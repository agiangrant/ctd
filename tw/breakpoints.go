package tw

// BreakpointConfig holds the pixel thresholds for responsive breakpoints.
// Tailwind uses mobile-first design: styles apply at the breakpoint width and above.
type BreakpointConfig struct {
	SM  float32 // ≥640px by default
	MD  float32 // ≥768px by default
	LG  float32 // ≥1024px by default
	XL  float32 // ≥1280px by default
	XXL float32 // ≥1536px by default (2xl)
}

// DefaultBreakpoints returns the standard Tailwind CSS v4 breakpoint values.
func DefaultBreakpoints() BreakpointConfig {
	return BreakpointConfig{
		SM:  640,
		MD:  768,
		LG:  1024,
		XL:  1280,
		XXL: 1536,
	}
}

// ActiveBreakpoint returns which breakpoint is currently active for a given width.
// Returns the highest breakpoint that the width satisfies.
func (c BreakpointConfig) ActiveBreakpoint(width float32) Breakpoint {
	if width >= c.XXL {
		return Breakpoint2XL
	}
	if width >= c.XL {
		return BreakpointXL
	}
	if width >= c.LG {
		return BreakpointLG
	}
	if width >= c.MD {
		return BreakpointMD
	}
	if width >= c.SM {
		return BreakpointSM
	}
	return BreakpointBase
}

// ResolveForWidth merges styles from base up through the active breakpoint.
// This implements Tailwind's mobile-first cascade: base → sm → md → lg → xl → 2xl
// Only properties that are explicitly set at each level override previous values.
func (cs *ComputedStyles) ResolveForWidth(width float32, config BreakpointConfig) StyleProperties {
	result := cs.Base

	// Mobile-first: apply breakpoint styles in order if width meets threshold
	if width >= config.SM {
		mergeStyleProperties(&result, &cs.SM)
	}
	if width >= config.MD {
		mergeStyleProperties(&result, &cs.MD)
	}
	if width >= config.LG {
		mergeStyleProperties(&result, &cs.LG)
	}
	if width >= config.XL {
		mergeStyleProperties(&result, &cs.XL)
	}
	if width >= config.XXL {
		mergeStyleProperties(&result, &cs.XXL)
	}

	return result
}

// ResolveForWidthWithState merges styles for both breakpoint and interactive state.
// Order: base → breakpoint styles → state styles (hover/focus/etc from active breakpoint)
func (cs *ComputedStyles) ResolveForWidthWithState(width float32, config BreakpointConfig, state State) StyleProperties {
	// First get breakpoint-resolved base styles
	result := cs.ResolveForWidth(width, config)

	// Then apply state-specific styles based on the state
	switch state {
	case StateHover:
		mergeStyleProperties(&result, &cs.Hover)
	case StateFocus:
		mergeStyleProperties(&result, &cs.Focus)
	case StateActive:
		mergeStyleProperties(&result, &cs.Active)
	case StateDisabled:
		mergeStyleProperties(&result, &cs.Disabled)
	case StatePlaceholder:
		mergeStyleProperties(&result, &cs.Placeholder)
	}

	return result
}

// ResolveWithDarkMode resolves styles considering dark mode.
// If darkMode is true, applies dark: variant styles on top of base styles.
// Order: base → dark:base (if darkMode)
func (cs *ComputedStyles) ResolveWithDarkMode(darkMode bool) StyleProperties {
	result := cs.Base

	if darkMode {
		mergeStyleProperties(&result, &cs.Dark.Base)
	}

	return result
}

// ResolveForWidthWithDarkMode merges styles for breakpoint and dark mode.
// Order: base → breakpoint → dark:base → dark:breakpoint (if darkMode)
func (cs *ComputedStyles) ResolveForWidthWithDarkMode(width float32, config BreakpointConfig, darkMode bool) StyleProperties {
	// First get breakpoint-resolved base styles
	result := cs.ResolveForWidth(width, config)

	if darkMode {
		// Apply dark mode base
		mergeStyleProperties(&result, &cs.Dark.Base)
		// Note: If we wanted dark mode breakpoint styles (dark:sm:, dark:md:, etc.),
		// we would add them here. For now, dark: applies at all breakpoints.
	}

	return result
}

// ResolveForWidthWithDarkModeAndState is the full resolution method.
// Order: base → breakpoint → state → dark:base → dark:state (if darkMode)
func (cs *ComputedStyles) ResolveForWidthWithDarkModeAndState(width float32, config BreakpointConfig, darkMode bool, state State) StyleProperties {
	// First get breakpoint-resolved base styles with state
	result := cs.ResolveForWidthWithState(width, config, state)

	if darkMode {
		// Apply dark mode base
		mergeStyleProperties(&result, &cs.Dark.Base)

		// Apply dark mode state-specific styles
		switch state {
		case StateHover:
			mergeStyleProperties(&result, &cs.Dark.Hover)
		case StateFocus:
			mergeStyleProperties(&result, &cs.Dark.Focus)
		case StateActive:
			mergeStyleProperties(&result, &cs.Dark.Active)
		case StateDisabled:
			mergeStyleProperties(&result, &cs.Dark.Disabled)
		case StatePlaceholder:
			mergeStyleProperties(&result, &cs.Dark.Placeholder)
		}
	}

	return result
}

// mergeStyleProperties copies non-nil values from src to dst.
// Only properties that are explicitly set (non-nil) in src will override dst.
func mergeStyleProperties(dst, src *StyleProperties) {
	// Colors
	if src.TextColor != nil {
		dst.TextColor = src.TextColor
	}
	if src.BackgroundColor != nil {
		dst.BackgroundColor = src.BackgroundColor
	}
	if src.BorderColor != nil {
		dst.BorderColor = src.BorderColor
	}

	// Typography
	if src.FontSize != nil {
		dst.FontSize = src.FontSize
	}
	if src.FontWeight != nil {
		dst.FontWeight = src.FontWeight
	}
	if src.LineHeight != nil {
		dst.LineHeight = src.LineHeight
	}
	if src.LetterSpacing != nil {
		dst.LetterSpacing = src.LetterSpacing
	}
	if src.TextAlign != nil {
		dst.TextAlign = src.TextAlign
	}

	// Spacing
	if src.PaddingTop != nil {
		dst.PaddingTop = src.PaddingTop
	}
	if src.PaddingRight != nil {
		dst.PaddingRight = src.PaddingRight
	}
	if src.PaddingBottom != nil {
		dst.PaddingBottom = src.PaddingBottom
	}
	if src.PaddingLeft != nil {
		dst.PaddingLeft = src.PaddingLeft
	}
	if src.MarginTop != nil {
		dst.MarginTop = src.MarginTop
	}
	if src.MarginRight != nil {
		dst.MarginRight = src.MarginRight
	}
	if src.MarginBottom != nil {
		dst.MarginBottom = src.MarginBottom
	}
	if src.MarginLeft != nil {
		dst.MarginLeft = src.MarginLeft
	}

	// Sizing
	if src.Width != nil {
		dst.Width = src.Width
	}
	if src.Height != nil {
		dst.Height = src.Height
	}
	if src.MinWidth != nil {
		dst.MinWidth = src.MinWidth
	}
	if src.MinHeight != nil {
		dst.MinHeight = src.MinHeight
	}
	if src.MaxWidth != nil {
		dst.MaxWidth = src.MaxWidth
	}
	if src.MaxHeight != nil {
		dst.MaxHeight = src.MaxHeight
	}
	if src.WidthMode != nil {
		dst.WidthMode = src.WidthMode
	}
	if src.HeightMode != nil {
		dst.HeightMode = src.HeightMode
	}
	if src.WidthPercent != nil {
		dst.WidthPercent = src.WidthPercent
	}
	if src.HeightPercent != nil {
		dst.HeightPercent = src.HeightPercent
	}

	// Layout
	if src.Display != nil {
		dst.Display = src.Display
	}
	if src.Position != nil {
		dst.Position = src.Position
	}
	if src.Top != nil {
		dst.Top = src.Top
	}
	if src.Right != nil {
		dst.Right = src.Right
	}
	if src.Bottom != nil {
		dst.Bottom = src.Bottom
	}
	if src.Left != nil {
		dst.Left = src.Left
	}
	if src.ZIndex != nil {
		dst.ZIndex = src.ZIndex
	}

	// Flexbox
	if src.FlexDirection != nil {
		dst.FlexDirection = src.FlexDirection
	}
	if src.JustifyContent != nil {
		dst.JustifyContent = src.JustifyContent
	}
	if src.AlignItems != nil {
		dst.AlignItems = src.AlignItems
	}
	if src.FlexWrap != nil {
		dst.FlexWrap = src.FlexWrap
	}
	if src.FlexGrow != nil {
		dst.FlexGrow = src.FlexGrow
	}
	if src.FlexShrink != nil {
		dst.FlexShrink = src.FlexShrink
	}
	if src.Gap != nil {
		dst.Gap = src.Gap
	}

	// Grid
	if src.GridTemplateColumns != nil {
		dst.GridTemplateColumns = src.GridTemplateColumns
	}
	if src.GridTemplateRows != nil {
		dst.GridTemplateRows = src.GridTemplateRows
	}
	if src.GridColumnSpan != nil {
		dst.GridColumnSpan = src.GridColumnSpan
	}
	if src.GridRowSpan != nil {
		dst.GridRowSpan = src.GridRowSpan
	}

	// Borders
	if src.BorderWidth != nil {
		dst.BorderWidth = src.BorderWidth
	}
	if src.BorderRadius != nil {
		dst.BorderRadius = src.BorderRadius
	}
	if src.BorderStyle != nil {
		dst.BorderStyle = src.BorderStyle
	}

	// Effects
	if src.Opacity != nil {
		dst.Opacity = src.Opacity
	}
	if src.BoxShadow != nil {
		dst.BoxShadow = src.BoxShadow
	}

	// Transforms
	if src.Scale != nil {
		dst.Scale = src.Scale
	}
	if src.Rotate != nil {
		dst.Rotate = src.Rotate
	}
	if src.TranslateX != nil {
		dst.TranslateX = src.TranslateX
	}
	if src.TranslateY != nil {
		dst.TranslateY = src.TranslateY
	}

	// Transitions
	if src.TransitionProperty != nil {
		dst.TransitionProperty = src.TransitionProperty
	}
	if src.TransitionDuration != nil {
		dst.TransitionDuration = src.TransitionDuration
	}
	if src.TransitionTiming != nil {
		dst.TransitionTiming = src.TransitionTiming
	}

	// Interactivity
	if src.Cursor != nil {
		dst.Cursor = src.Cursor
	}
	if src.PointerEvents != nil {
		dst.PointerEvents = src.PointerEvents
	}
	if src.UserSelect != nil {
		dst.UserSelect = src.UserSelect
	}

	// Overflow
	if src.OverflowX != nil {
		dst.OverflowX = src.OverflowX
	}
	if src.OverflowY != nil {
		dst.OverflowY = src.OverflowY
	}
}
