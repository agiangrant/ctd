package tw

// PartialStyle represents a partial style that can be merged.
// Used in ClassMap for individual utility class definitions.
type PartialStyle struct {
	// Colors
	TextColor       *uint32
	BackgroundColor *uint32
	BorderColor     *uint32

	// Typography
	FontFamily    *string
	FontSize      *float32
	FontWeight    *int
	LineHeight    *float32
	LetterSpacing *float32
	TextAlign     *string

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
	WidthMode     *string
	HeightMode    *string
	WidthPercent  *float32
	HeightPercent *float32

	// Layout
	Display  *string
	Position *string
	Top      *float32
	Right    *float32
	Bottom   *float32
	Left     *float32
	ZIndex   *int

	// Flexbox
	FlexDirection    *string
	JustifyContent   *string
	AlignItems       *string
	AlignSelf        *string
	FlexWrap         *string
	FlexGrow         *float32
	FlexShrink       *float32
	FlexBasis        *float32
	FlexBasisMode    *string
	FlexBasisPercent *float32
	Gap              *float32
	Order            *int

	// Grid
	GridTemplateColumns *string
	GridTemplateRows    *string
	GridColumnSpan      *int
	GridRowSpan         *int

	// Borders
	BorderWidth  *float32
	BorderRadius *float32
	BorderStyle  *string

	// Effects
	Opacity   *float32
	BoxShadow *string

	// Transforms
	Scale      *float32
	Rotate     *float32
	TranslateX *float32
	TranslateY *float32

	// Transitions
	TransitionProperty *string
	TransitionDuration *float32
	TransitionTiming   *string

	// Interactivity
	Cursor        *string
	PointerEvents *string
	UserSelect    *string

	// Overflow
	OverflowX *string
	OverflowY *string

	// Object (images)
	ObjectFit      *string
	ObjectPosition *string

	// Animation
	Animation           *string
	AnimationDuration   *float32
	AnimationEasing     *string
	AnimationIterations *int
}

// FontFamilyConfig represents a font family configuration.
// Value is either a system font name or a file path to a bundled font.
type FontFamilyConfig struct {
	Value     string // System font name or file path
	IsBundled bool   // true if Value is a file path, false if system font
}

// ThemeConfig holds the consumer's theme configuration.
// This is registered via SetConfig() at app startup.
type ThemeConfig struct {
	ClassMap    map[string]PartialStyle
	Fonts       map[string]FontFamilyConfig
	Breakpoints BreakpointConfig
}

// registeredConfig holds the consumer's theme configuration.
// If nil, falls back to framework defaults (from generated.go).
var registeredConfig *ThemeConfig

// SetConfig registers the consumer's theme configuration.
// This should be called at app startup before any parsing occurs.
// Typically called by the generated Register() function in the consumer's tw package.
func SetConfig(config ThemeConfig) {
	registeredConfig = &config
}

// GetClassMap returns the registered ClassMap or falls back to the framework default.
func GetClassMap() map[string]PartialStyle {
	if registeredConfig != nil && registeredConfig.ClassMap != nil {
		return registeredConfig.ClassMap
	}
	return ClassMap // Fallback to generated.go default
}

// GetFonts returns the registered fonts or falls back to the framework default.
func GetFonts() map[string]FontFamilyConfig {
	if registeredConfig != nil && registeredConfig.Fonts != nil {
		return registeredConfig.Fonts
	}
	return ThemeFonts() // Fallback to generated.go default
}

// GetBreakpoints returns the registered breakpoints or falls back to the framework default.
func GetBreakpoints() BreakpointConfig {
	if registeredConfig != nil {
		return registeredConfig.Breakpoints
	}
	return ThemeBreakpoints() // Fallback to generated.go default
}
