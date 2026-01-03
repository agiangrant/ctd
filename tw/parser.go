package tw

import (
	"fmt"
	"strings"
)

// State represents widget interaction state
type State int

const (
	StateDefault State = iota
	StateHover
	StateFocus
	StateActive
	StateDisabled
	StatePlaceholder
)

// Breakpoint represents responsive breakpoint
type Breakpoint int

const (
	BreakpointBase Breakpoint = iota
	BreakpointSM               // ≥640px
	BreakpointMD               // ≥768px
	BreakpointLG               // ≥1024px
	BreakpointXL               // ≥1280px
	Breakpoint2XL              // ≥1536px
)

// StyleProperties represents concrete style values
type StyleProperties struct {
	// Colors
	TextColor       *uint32
	BackgroundColor *uint32
	BorderColor     *uint32

	// Typography
	FontFamily *string  // "sans", "serif", "mono", or custom name from theme.toml
	FontSize   *float32
	FontWeight *int
	LineHeight *float32
	LetterSpacing *float32
	TextAlign  *string  // "left", "center", "right", "justify"

	// Spacing
	PaddingTop    *float32
	PaddingRight  *float32
	PaddingBottom *float32
	PaddingLeft   *float32
	MarginTop     *float32
	MarginRight   *float32
	MarginBottom  *float32
	MarginLeft    *float32

	// Sizing
	Width         *float32
	Height        *float32
	MinWidth      *float32
	MinHeight     *float32
	MaxWidth      *float32
	MaxHeight     *float32
	WidthMode     *string  // "fixed", "auto", "full", "percent", "flex" - how width is calculated
	HeightMode    *string  // "fixed", "auto", "full", "percent", "flex" - how height is calculated
	WidthPercent  *float32 // Percentage value when WidthMode is "percent"
	HeightPercent *float32 // Percentage value when HeightMode is "percent"

	// Layout
	Display      *string  // "block", "inline", "flex", "grid", "none"
	Position     *string  // "static", "relative", "absolute", "fixed", "sticky"
	Top          *float32
	Right        *float32
	Bottom       *float32
	Left         *float32
	ZIndex       *int

	// Flexbox
	FlexDirection  *string  // "row", "column", "row-reverse", "column-reverse"
	JustifyContent *string  // "start", "end", "center", "between", "around", "evenly"
	AlignItems     *string  // "start", "end", "center", "stretch", "baseline"
	AlignSelf      *string  // "auto", "start", "end", "center", "stretch", "baseline"
	FlexWrap       *string  // "nowrap", "wrap", "wrap-reverse"
	FlexGrow       *float32
	FlexShrink     *float32
	FlexBasis      *float32 // Flex basis in pixels (when mode is "fixed")
	FlexBasisMode  *string  // "auto", "full", "percent", "fixed"
	FlexBasisPercent *float32 // Percentage value when FlexBasisMode is "percent"
	Gap            *float32
	Order          *int     // Flex item order

	// Grid
	GridTemplateColumns *string
	GridTemplateRows    *string
	GridColumnSpan      *int
	GridRowSpan         *int

	// Borders
	BorderWidth  *float32
	BorderRadius *float32
	BorderStyle  *string  // "solid", "dashed", "dotted", "none"

	// Effects
	Opacity     *float32
	BoxShadow   *string

	// Transforms
	Scale      *float32
	Rotate     *float32
	TranslateX *float32
	TranslateY *float32

	// Transitions
	TransitionProperty *string
	TransitionDuration *float32
	TransitionTiming   *string

	// Animations
	Animation           *string  // "pulse", "bounce", "spin", "ping", "none"
	AnimationDuration   *float32 // Override default duration (in ms)
	AnimationEasing     *string  // "linear", "ease-in", "ease-out", "ease-in-out"
	AnimationIterations *int     // Number of times to repeat (0 = infinite)

	// Interactivity
	Cursor         *string  // "pointer", "default", "not-allowed", "text", etc.
	PointerEvents  *string  // "auto", "none"
	UserSelect     *string  // "none", "text", "all", "auto"

	// Overflow
	OverflowX *string  // "visible", "hidden", "scroll", "auto"
	OverflowY *string

	// Object fit and position (for images)
	ObjectFit      *string // "contain", "cover", "fill", "none", "scale-down"
	ObjectPosition *string // "center", "top", "bottom", "left", "right", "top-left", etc.
}

// ComputedStyles represents styles organized by state and breakpoint
type ComputedStyles struct {
	// Base styles (always apply)
	Base StyleProperties

	// State variants
	Hover       StyleProperties
	Focus       StyleProperties
	Active      StyleProperties
	Disabled    StyleProperties
	Placeholder StyleProperties // For placeholder: variant (text input placeholders)

	// Responsive variants (apply at different breakpoints)
	SM  StyleProperties
	MD  StyleProperties
	LG  StyleProperties
	XL  StyleProperties
	XXL StyleProperties

	// Dark mode variants
	Dark struct {
		Base        StyleProperties
		Hover       StyleProperties
		Focus       StyleProperties
		Active      StyleProperties
		Disabled    StyleProperties
		Placeholder StyleProperties
	}

	// Dark mode + responsive (if needed later)
	// DarkSM, DarkMD, etc.
}

// ParsedClass represents a class with its variant modifiers
type ParsedClass struct {
	Breakpoint     Breakpoint
	State          State
	DarkMode       bool
	BaseClass      string
	ArbitraryValue *ArbitraryValue // For arbitrary values like w-[33%]
}

// ArbitraryValue represents a runtime-parsed arbitrary value
type ArbitraryValue struct {
	Property string // e.g., "w", "bg", "text", "rotate"
	Value    string // e.g., "33%", "#1da1f2", "22px", "17deg"
}

// ParseClasses parses a Tailwind class string and returns computed styles
// Example: "bg-blue-500 hover:bg-blue-600 dark:text-white md:flex w-[33%]"
func ParseClasses(classStr string) ComputedStyles {
	var computed ComputedStyles

	// Split by whitespace
	classes := strings.Fields(classStr)

	// Parse each class
	for _, class := range classes {
		parsed := parseClass(class)

		var partial PartialStyle

		// Handle arbitrary values (runtime parsing)
		if parsed.ArbitraryValue != nil {
			partial = parseArbitraryValue(parsed.ArbitraryValue)
		} else {
			// Look up base utility in ClassMap (uses registered or framework default)
			var ok bool
			partial, ok = GetClassMap()[parsed.BaseClass]
			if !ok {
				// Unknown class, silently ignore (like Tailwind CSS)
				continue
			}
		}

		// Apply to appropriate bucket based on variants
		target := getTargetProperties(&computed, parsed)
		target.Merge(partial)
	}

	return computed
}

// parseClass splits a class into variant modifiers and base utility
// "hover:dark:bg-blue-500" → ParsedClass{State: Hover, DarkMode: true, BaseClass: "bg-blue-500"}
// "w-[33%]" → ParsedClass{ArbitraryValue: {Property: "w", Value: "33%"}}
func parseClass(class string) ParsedClass {
	parts := strings.Split(class, ":")

	pc := ParsedClass{
		Breakpoint: BreakpointBase,
		State:      StateDefault,
		DarkMode:   false,
		BaseClass:  parts[len(parts)-1], // Last part is always the base utility
	}

	// Parse variant prefixes
	for i := 0; i < len(parts)-1; i++ {
		switch parts[i] {
		// State variants
		case "hover":
			pc.State = StateHover
		case "focus":
			pc.State = StateFocus
		case "active":
			pc.State = StateActive
		case "disabled":
			pc.State = StateDisabled
		case "placeholder":
			pc.State = StatePlaceholder

		// Dark mode
		case "dark":
			pc.DarkMode = true

		// Responsive breakpoints
		case "sm":
			pc.Breakpoint = BreakpointSM
		case "md":
			pc.Breakpoint = BreakpointMD
		case "lg":
			pc.Breakpoint = BreakpointLG
		case "xl":
			pc.Breakpoint = BreakpointXL
		case "2xl":
			pc.Breakpoint = Breakpoint2XL
		}
	}

	// Check if base class is an arbitrary value: property-[value]
	if strings.Contains(pc.BaseClass, "[") && strings.HasSuffix(pc.BaseClass, "]") {
		pc.ArbitraryValue = extractArbitraryValue(pc.BaseClass)
		pc.BaseClass = "" // Clear base class since we're using arbitrary
	}

	return pc
}

// extractArbitraryValue parses arbitrary value syntax
// "w-[33%]" → ArbitraryValue{Property: "w", Value: "33%"}
// "bg-[#1da1f2]" → ArbitraryValue{Property: "bg", Value: "#1da1f2"}
func extractArbitraryValue(class string) *ArbitraryValue {
	// Find the opening bracket
	bracketIdx := strings.Index(class, "[")
	if bracketIdx == -1 {
		return nil
	}

	property := strings.TrimSuffix(class[:bracketIdx], "-") // Remove trailing dash
	value := strings.TrimSuffix(class[bracketIdx+1:], "]")

	return &ArbitraryValue{
		Property: property,
		Value:    value,
	}
}

// parseArbitraryValue converts arbitrary value to PartialStyle at runtime
func parseArbitraryValue(arb *ArbitraryValue) PartialStyle {
	var partial PartialStyle

	// Parse based on property type
	switch {
	// Width
	case arb.Property == "w":
		if val := parseDimension(arb.Value); val != nil {
			partial.Width = val
		}

	// Height
	case arb.Property == "h":
		if val := parseDimension(arb.Value); val != nil {
			partial.Height = val
		}

	// Min/Max width/height
	case arb.Property == "min-w":
		if val := parseDimension(arb.Value); val != nil {
			partial.MinWidth = val
		}
	case arb.Property == "max-w":
		if val := parseDimension(arb.Value); val != nil {
			partial.MaxWidth = val
		}
	case arb.Property == "min-h":
		if val := parseDimension(arb.Value); val != nil {
			partial.MinHeight = val
		}
	case arb.Property == "max-h":
		if val := parseDimension(arb.Value); val != nil {
			partial.MaxHeight = val
		}

	// Padding
	case arb.Property == "p":
		if val := parseDimension(arb.Value); val != nil {
			partial.PaddingTop = val
			partial.PaddingRight = val
			partial.PaddingBottom = val
			partial.PaddingLeft = val
		}
	case arb.Property == "px":
		if val := parseDimension(arb.Value); val != nil {
			partial.PaddingLeft = val
			partial.PaddingRight = val
		}
	case arb.Property == "py":
		if val := parseDimension(arb.Value); val != nil {
			partial.PaddingTop = val
			partial.PaddingBottom = val
		}
	case arb.Property == "pt":
		partial.PaddingTop = parseDimension(arb.Value)
	case arb.Property == "pr":
		partial.PaddingRight = parseDimension(arb.Value)
	case arb.Property == "pb":
		partial.PaddingBottom = parseDimension(arb.Value)
	case arb.Property == "pl":
		partial.PaddingLeft = parseDimension(arb.Value)

	// Margin
	case arb.Property == "m":
		if val := parseDimension(arb.Value); val != nil {
			partial.MarginTop = val
			partial.MarginRight = val
			partial.MarginBottom = val
			partial.MarginLeft = val
		}
	case arb.Property == "mx":
		if val := parseDimension(arb.Value); val != nil {
			partial.MarginLeft = val
			partial.MarginRight = val
		}
	case arb.Property == "my":
		if val := parseDimension(arb.Value); val != nil {
			partial.MarginTop = val
			partial.MarginBottom = val
		}
	case arb.Property == "mt":
		partial.MarginTop = parseDimension(arb.Value)
	case arb.Property == "mr":
		partial.MarginRight = parseDimension(arb.Value)
	case arb.Property == "mb":
		partial.MarginBottom = parseDimension(arb.Value)
	case arb.Property == "ml":
		partial.MarginLeft = parseDimension(arb.Value)

	// Gap
	case arb.Property == "gap":
		partial.Gap = parseDimension(arb.Value)

	// Position
	case arb.Property == "top":
		partial.Top = parseDimension(arb.Value)
	case arb.Property == "right":
		partial.Right = parseDimension(arb.Value)
	case arb.Property == "bottom":
		partial.Bottom = parseDimension(arb.Value)
	case arb.Property == "left":
		partial.Left = parseDimension(arb.Value)

	// Colors
	case arb.Property == "bg":
		if color := parseColor(arb.Value); color != nil {
			partial.BackgroundColor = color
		}
	case arb.Property == "text":
		if color := parseColor(arb.Value); color != nil {
			partial.TextColor = color
		}
	case arb.Property == "border":
		if color := parseColor(arb.Value); color != nil {
			partial.BorderColor = color
		}

	// Typography
	case strings.HasPrefix(arb.Property, "text-"):
		// Could be text-[22px] for font size
		if val := parseDimension(arb.Value); val != nil {
			partial.FontSize = val
		}

	// Transforms
	case arb.Property == "scale":
		if val := parseFloat(arb.Value); val != nil {
			partial.Scale = val
		}
	case arb.Property == "rotate":
		if val := parseDegrees(arb.Value); val != nil {
			partial.Rotate = val
		}
	case arb.Property == "translate-x":
		partial.TranslateX = parseDimension(arb.Value)
	case arb.Property == "translate-y":
		partial.TranslateY = parseDimension(arb.Value)

	// Border radius
	case arb.Property == "rounded":
		partial.BorderRadius = parseDimension(arb.Value)

	// Opacity
	case arb.Property == "opacity":
		if val := parseFloat(arb.Value); val != nil {
			partial.Opacity = val
		}

	// Animation: animate-[name_duration_easing_iterations]
	// Examples:
	//   animate-[pulse_500ms]
	//   animate-[bounce_1s_ease-in-out]
	//   animate-[pulse_2s_linear_3]
	//   animate-[spin_1s_ease-in_infinite]
	case arb.Property == "animate":
		parseAnimationArbitrary(arb.Value, &partial)

	// Object fit: object-[cover], object-[contain], etc.
	// Object position: object-[center], object-[top_left], object-[50%_25%]
	case arb.Property == "object":
		// Check if it's a fit value
		switch arb.Value {
		case "contain", "cover", "fill", "none", "scale-down":
			partial.ObjectFit = strPtr(arb.Value)
		default:
			// Treat as position - convert underscores to hyphens for compound positions
			pos := strings.ReplaceAll(arb.Value, "_", "-")
			partial.ObjectPosition = strPtr(pos)
		}
	}

	return partial
}

// parseAnimationArbitrary parses animation arbitrary value syntax
// Format: name_duration_easing_iterations (underscore-separated)
// Examples:
//   pulse_500ms         -> pulse animation at 500ms duration
//   bounce_1s_ease-out  -> bounce with 1s duration and ease-out easing
//   spin_2s_linear_3    -> spin 3 times at 2s duration with linear easing
//   pulse_1s__infinite  -> pulse infinite with default easing (double underscore = skip)
func parseAnimationArbitrary(value string, partial *PartialStyle) {
	parts := strings.Split(value, "_")
	if len(parts) == 0 {
		return
	}

	// First part is always the animation name
	name := parts[0]
	if name == "" {
		return
	}
	partial.Animation = &name

	// Parse remaining parts positionally: duration, easing, iterations
	for i := 1; i < len(parts); i++ {
		part := parts[i]
		if part == "" {
			continue // Skip empty parts (allows double underscore to skip)
		}

		// Try to parse as duration (ends with ms or s)
		if duration := parseAnimationDuration(part); duration != nil {
			partial.AnimationDuration = duration
			continue
		}

		// Try to parse as iteration count ("infinite" or number)
		if part == "infinite" {
			zero := 0
			partial.AnimationIterations = &zero // 0 = infinite
			continue
		}
		if iter := parseInt(part); iter != nil {
			partial.AnimationIterations = iter
			continue
		}

		// Otherwise treat as easing function
		if isValidEasing(part) {
			partial.AnimationEasing = &part
		}
	}
}

// parseAnimationDuration parses duration strings like "500ms", "1s", "1.5s"
func parseAnimationDuration(value string) *float32 {
	if strings.HasSuffix(value, "ms") {
		numStr := strings.TrimSuffix(value, "ms")
		var ms float32
		if _, err := fmt.Sscanf(numStr, "%f", &ms); err == nil {
			return &ms
		}
	} else if strings.HasSuffix(value, "s") {
		numStr := strings.TrimSuffix(value, "s")
		var sec float32
		if _, err := fmt.Sscanf(numStr, "%f", &sec); err == nil {
			ms := sec * 1000 // Convert to milliseconds
			return &ms
		}
	}
	return nil
}

// parseInt parses an integer string
func parseInt(value string) *int {
	var result int
	if _, err := fmt.Sscanf(value, "%d", &result); err == nil {
		return &result
	}
	return nil
}

// isValidEasing checks if the string is a valid easing function name
func isValidEasing(value string) bool {
	validEasings := map[string]bool{
		"linear":        true,
		"ease":          true,
		"ease-in":       true,
		"ease-out":      true,
		"ease-in-out":   true,
		"cubic":         true,
		"back":          true,
		"elastic":       true,
		"bounce":        true,
	}
	return validEasings[value]
}

// parseDimension parses CSS dimension values (px, %, rem, etc.)
func parseDimension(value string) *float32 {
	// Remove whitespace
	value = strings.TrimSpace(value)

	var numStr string
	var multiplier float32 = 1.0

	// Handle different units by stripping suffix and applying multiplier
	if strings.HasSuffix(value, "px") {
		numStr = strings.TrimSuffix(value, "px")
		multiplier = 1.0
	} else if strings.HasSuffix(value, "%") {
		numStr = strings.TrimSuffix(value, "%")
		multiplier = 1.0 // Store as percentage value
	} else if strings.HasSuffix(value, "rem") {
		numStr = strings.TrimSuffix(value, "rem")
		multiplier = 16.0 // Convert rem to pixels (1rem = 16px)
	} else if strings.HasSuffix(value, "em") {
		numStr = strings.TrimSuffix(value, "em")
		multiplier = 16.0 // Convert em to pixels (approximate)
	} else {
		// Plain number (assume pixels)
		numStr = value
		multiplier = 1.0
	}

	// Parse the numeric part
	var num float32
	if _, err := fmt.Sscanf(numStr, "%f", &num); err == nil {
		result := num * multiplier
		return &result
	}

	return nil
}

// parseColor parses color values (#hex, rgb(), etc.)
func parseColor(value string) *uint32 {
	value = strings.TrimSpace(value)

	// Hex color: #RRGGBB or #RGB
	if strings.HasPrefix(value, "#") {
		hex := value[1:]

		// Expand shorthand: #RGB → #RRGGBB
		if len(hex) == 3 {
			hex = string([]byte{hex[0], hex[0], hex[1], hex[1], hex[2], hex[2]})
		}

		if len(hex) == 6 {
			var r, g, b uint32
			if _, err := fmt.Sscanf(hex, "%02x%02x%02x", &r, &g, &b); err == nil {
				color := (r << 24) | (g << 16) | (b << 8) | 0xFF // RGBA
				return &color
			}
		}
	}

	// TODO: Support rgb(), rgba(), hsl(), etc.

	return nil
}

// parseFloat parses a float value
func parseFloat(value string) *float32 {
	var result float32
	if _, err := fmt.Sscanf(value, "%f", &result); err == nil {
		return &result
	}
	return nil
}

// parseDegrees parses degree values (with or without "deg")
func parseDegrees(value string) *float32 {
	value = strings.TrimSpace(value)
	var deg float32

	if strings.HasSuffix(value, "deg") {
		if _, err := fmt.Sscanf(value, "%fdeg", &deg); err == nil {
			return &deg
		}
	} else {
		// Try plain number
		if _, err := fmt.Sscanf(value, "%f", &deg); err == nil {
			return &deg
		}
	}

	return nil
}

// getTargetProperties returns the appropriate StyleProperties to apply to
func getTargetProperties(computed *ComputedStyles, parsed ParsedClass) *StyleProperties {
	// Handle dark mode variants
	if parsed.DarkMode {
		switch parsed.State {
		case StateHover:
			return &computed.Dark.Hover
		case StateFocus:
			return &computed.Dark.Focus
		case StateActive:
			return &computed.Dark.Active
		case StateDisabled:
			return &computed.Dark.Disabled
		case StatePlaceholder:
			return &computed.Dark.Placeholder
		default:
			return &computed.Dark.Base
		}
	}

	// Handle responsive variants (ignore state for now in responsive)
	// In a full implementation, we'd support md:hover:bg-blue-500
	if parsed.Breakpoint != BreakpointBase {
		switch parsed.Breakpoint {
		case BreakpointSM:
			return &computed.SM
		case BreakpointMD:
			return &computed.MD
		case BreakpointLG:
			return &computed.LG
		case BreakpointXL:
			return &computed.XL
		case Breakpoint2XL:
			return &computed.XXL
		}
	}

	// Handle state variants at base breakpoint
	switch parsed.State {
	case StateHover:
		return &computed.Hover
	case StateFocus:
		return &computed.Focus
	case StateActive:
		return &computed.Active
	case StateDisabled:
		return &computed.Disabled
	case StatePlaceholder:
		return &computed.Placeholder
	default:
		return &computed.Base
	}
}

// Merge merges a PartialStyle into these StyleProperties
// Later values override earlier ones (last class wins)
func (s *StyleProperties) Merge(p PartialStyle) {
	if p.TextColor != nil {
		s.TextColor = p.TextColor
	}
	if p.BackgroundColor != nil {
		s.BackgroundColor = p.BackgroundColor
	}
	if p.BorderColor != nil {
		s.BorderColor = p.BorderColor
	}
	if p.FontFamily != nil {
		s.FontFamily = p.FontFamily
	}
	if p.FontSize != nil {
		s.FontSize = p.FontSize
	}
	if p.FontWeight != nil {
		s.FontWeight = p.FontWeight
	}
	if p.LineHeight != nil {
		s.LineHeight = p.LineHeight
	}
	if p.LetterSpacing != nil {
		s.LetterSpacing = p.LetterSpacing
	}
	if p.TextAlign != nil {
		s.TextAlign = p.TextAlign
	}
	if p.PaddingTop != nil {
		s.PaddingTop = p.PaddingTop
	}
	if p.PaddingRight != nil {
		s.PaddingRight = p.PaddingRight
	}
	if p.PaddingBottom != nil {
		s.PaddingBottom = p.PaddingBottom
	}
	if p.PaddingLeft != nil {
		s.PaddingLeft = p.PaddingLeft
	}
	if p.MarginTop != nil {
		s.MarginTop = p.MarginTop
	}
	if p.MarginRight != nil {
		s.MarginRight = p.MarginRight
	}
	if p.MarginBottom != nil {
		s.MarginBottom = p.MarginBottom
	}
	if p.MarginLeft != nil {
		s.MarginLeft = p.MarginLeft
	}
	if p.Width != nil {
		s.Width = p.Width
	}
	if p.Height != nil {
		s.Height = p.Height
	}
	if p.MinWidth != nil {
		s.MinWidth = p.MinWidth
	}
	if p.MinHeight != nil {
		s.MinHeight = p.MinHeight
	}
	if p.MaxWidth != nil {
		s.MaxWidth = p.MaxWidth
	}
	if p.MaxHeight != nil {
		s.MaxHeight = p.MaxHeight
	}
	if p.WidthMode != nil {
		s.WidthMode = p.WidthMode
	}
	if p.HeightMode != nil {
		s.HeightMode = p.HeightMode
	}
	if p.WidthPercent != nil {
		s.WidthPercent = p.WidthPercent
	}
	if p.HeightPercent != nil {
		s.HeightPercent = p.HeightPercent
	}
	if p.Display != nil {
		s.Display = p.Display
	}
	if p.Position != nil {
		s.Position = p.Position
	}
	if p.Top != nil {
		s.Top = p.Top
	}
	if p.Right != nil {
		s.Right = p.Right
	}
	if p.Bottom != nil {
		s.Bottom = p.Bottom
	}
	if p.Left != nil {
		s.Left = p.Left
	}
	if p.ZIndex != nil {
		s.ZIndex = p.ZIndex
	}
	if p.FlexDirection != nil {
		s.FlexDirection = p.FlexDirection
	}
	if p.JustifyContent != nil {
		s.JustifyContent = p.JustifyContent
	}
	if p.AlignItems != nil {
		s.AlignItems = p.AlignItems
	}
	if p.FlexWrap != nil {
		s.FlexWrap = p.FlexWrap
	}
	if p.FlexGrow != nil {
		s.FlexGrow = p.FlexGrow
	}
	if p.FlexShrink != nil {
		s.FlexShrink = p.FlexShrink
	}
	if p.FlexBasis != nil {
		s.FlexBasis = p.FlexBasis
	}
	if p.FlexBasisMode != nil {
		s.FlexBasisMode = p.FlexBasisMode
	}
	if p.FlexBasisPercent != nil {
		s.FlexBasisPercent = p.FlexBasisPercent
	}
	if p.AlignSelf != nil {
		s.AlignSelf = p.AlignSelf
	}
	if p.Order != nil {
		s.Order = p.Order
	}
	if p.Gap != nil {
		s.Gap = p.Gap
	}
	if p.GridTemplateColumns != nil {
		s.GridTemplateColumns = p.GridTemplateColumns
	}
	if p.GridTemplateRows != nil {
		s.GridTemplateRows = p.GridTemplateRows
	}
	if p.GridColumnSpan != nil {
		s.GridColumnSpan = p.GridColumnSpan
	}
	if p.GridRowSpan != nil {
		s.GridRowSpan = p.GridRowSpan
	}
	if p.BorderWidth != nil {
		s.BorderWidth = p.BorderWidth
	}
	if p.BorderRadius != nil {
		s.BorderRadius = p.BorderRadius
	}
	if p.BorderStyle != nil {
		s.BorderStyle = p.BorderStyle
	}
	if p.Opacity != nil {
		s.Opacity = p.Opacity
	}
	if p.BoxShadow != nil {
		s.BoxShadow = p.BoxShadow
	}
	if p.Scale != nil {
		s.Scale = p.Scale
	}
	if p.Rotate != nil {
		s.Rotate = p.Rotate
	}
	if p.TranslateX != nil {
		s.TranslateX = p.TranslateX
	}
	if p.TranslateY != nil {
		s.TranslateY = p.TranslateY
	}
	if p.TransitionProperty != nil {
		s.TransitionProperty = p.TransitionProperty
	}
	if p.TransitionDuration != nil {
		s.TransitionDuration = p.TransitionDuration
	}
	if p.TransitionTiming != nil {
		s.TransitionTiming = p.TransitionTiming
	}
	if p.Animation != nil {
		s.Animation = p.Animation
	}
	if p.AnimationDuration != nil {
		s.AnimationDuration = p.AnimationDuration
	}
	if p.AnimationEasing != nil {
		s.AnimationEasing = p.AnimationEasing
	}
	if p.AnimationIterations != nil {
		s.AnimationIterations = p.AnimationIterations
	}
	if p.Cursor != nil {
		s.Cursor = p.Cursor
	}
	if p.PointerEvents != nil {
		s.PointerEvents = p.PointerEvents
	}
	if p.UserSelect != nil {
		s.UserSelect = p.UserSelect
	}
	if p.OverflowX != nil {
		s.OverflowX = p.OverflowX
	}
	if p.OverflowY != nil {
		s.OverflowY = p.OverflowY
	}
	if p.ObjectFit != nil {
		s.ObjectFit = p.ObjectFit
	}
	if p.ObjectPosition != nil {
		s.ObjectPosition = p.ObjectPosition
	}
}
