package main

import (
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/pelletier/go-toml/v2"
)

// FontConfig represents a font configuration (system name or bundled file path)
type FontConfig struct {
	Value    string // System font name or file path
	IsBundled bool   // true if Value is a file path, false if system font
}

// ThemeConfig represents the user's theme configuration
type ThemeConfig struct {
	Theme struct {
		Breakpoints map[string]int               `toml:"breakpoints"`
		Spacing     map[string]string            `toml:"spacing"`
		Colors      map[string]interface{}       `toml:"colors"`
		FontFamily  map[string]string            `toml:"fontFamily"`
		FontSize    map[string][]string          `toml:"fontSize"`
	} `toml:"theme"`
	Fonts     map[string]string   `toml:"fonts"`     // Font family mappings (sans, serif, mono, custom)
	Utilities map[string][]string `toml:"utilities"`
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func run() error {
	// Read theme.toml if it exists
	var userConfig ThemeConfig
	if data, err := os.ReadFile("theme.toml"); err == nil {
		if err := toml.Unmarshal(data, &userConfig); err != nil {
			return fmt.Errorf("failed to parse theme.toml: %w", err)
		}
	}

	// Build complete configuration (defaults + user overrides)
	config := buildConfig(userConfig)

	// Generate Go code
	code := generateGoCode(config)

	// Write to tw/generated.go
	if err := os.WriteFile("tw/generated.go", []byte(code), 0644); err != nil {
		return fmt.Errorf("failed to write generated.go: %w", err)
	}

	fmt.Println("✓ Generated tw/generated.go")
	return nil
}

// Config holds the complete merged configuration
type Config struct {
	Breakpoints struct {
		SM  int
		MD  int
		LG  int
		XL  int
		XXL int
	}
	Colors       map[string]string
	Spacing      map[string]float32
	FontSizes    map[string]float32
	FontWeights  map[string]int
	FontFamilies map[string]FontConfig // sans, serif, mono, custom names
	Radii        map[string]float32
	Utilities    map[string][]string
}

func buildConfig(user ThemeConfig) Config {
	config := Config{
		Colors:       getTailwindColors(),
		Spacing:      getTailwindSpacing(),
		FontSizes:    getTailwindFontSizes(),
		FontWeights:  getTailwindFontWeights(),
		FontFamilies: getDefaultFontFamilies(),
		Radii:        getTailwindBorderRadii(),
		Utilities:    user.Utilities,
	}

	// Set default breakpoints (Tailwind v4 defaults)
	config.Breakpoints.SM = 640
	config.Breakpoints.MD = 768
	config.Breakpoints.LG = 1024
	config.Breakpoints.XL = 1280
	config.Breakpoints.XXL = 1536

	// Merge user breakpoint overrides
	if v, ok := user.Theme.Breakpoints["sm"]; ok {
		config.Breakpoints.SM = v
	}
	if v, ok := user.Theme.Breakpoints["md"]; ok {
		config.Breakpoints.MD = v
	}
	if v, ok := user.Theme.Breakpoints["lg"]; ok {
		config.Breakpoints.LG = v
	}
	if v, ok := user.Theme.Breakpoints["xl"]; ok {
		config.Breakpoints.XL = v
	}
	if v, ok := user.Theme.Breakpoints["2xl"]; ok {
		config.Breakpoints.XXL = v
	}

	// Merge user overrides for spacing
	for key, value := range user.Theme.Spacing {
		// Convert rem to pixels (1rem = 16px)
		var px float32
		if strings.HasSuffix(value, "rem") {
			remValue := 0.0
			fmt.Sscanf(value, "%frem", &remValue)
			px = float32(remValue * 16)
		} else if strings.HasSuffix(value, "px") {
			fmt.Sscanf(value, "%fpx", &px)
		}
		config.Spacing[key] = px
	}

	// Merge user colors (simplified for now)
	for key, value := range user.Theme.Colors {
		if str, ok := value.(string); ok {
			config.Colors[key] = str
		}
	}

	// Merge user fonts - detect if path (bundled) or system font name
	for name, value := range user.Fonts {
		config.FontFamilies[name] = FontConfig{
			Value:     value,
			IsBundled: isFontPath(value),
		}
	}

	return config
}

// isFontPath detects if a value is a file path (bundled font) or system font name
func isFontPath(value string) bool {
	// Check for file extensions or path separators
	lower := strings.ToLower(value)
	if strings.HasSuffix(lower, ".ttf") || strings.HasSuffix(lower, ".otf") ||
		strings.HasSuffix(lower, ".woff") || strings.HasSuffix(lower, ".woff2") {
		return true
	}
	// Check for path separators (indicating a file path)
	if strings.Contains(value, "/") || strings.Contains(value, "\\") {
		return true
	}
	return false
}

// getDefaultFontFamilies returns sensible default font family mappings
func getDefaultFontFamilies() map[string]FontConfig {
	return map[string]FontConfig{
		"sans":  {Value: "system", IsBundled: false},  // System default sans-serif
		"serif": {Value: "Times New Roman", IsBundled: false},
		"mono":  {Value: "Menlo", IsBundled: false},
	}
}

// getTailwindColors returns the default Tailwind v4 color palette
func getTailwindColors() map[string]string {
	colors := make(map[string]string)

	// Grayscale palettes
	grayScales := map[string][]string{
		"slate":   {"#f8fafc", "#f1f5f9", "#e2e8f0", "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155", "#1e293b", "#0f172a", "#020617"},
		"gray":    {"#f9fafb", "#f3f4f6", "#e5e7eb", "#d1d5db", "#9ca3af", "#6b7280", "#4b5563", "#374151", "#1f2937", "#111827", "#030712"},
		"zinc":    {"#fafafa", "#f4f4f5", "#e4e4e7", "#d4d4d8", "#a1a1aa", "#71717a", "#52525b", "#3f3f46", "#27272a", "#18181b", "#09090b"},
		"neutral": {"#fafafa", "#f5f5f5", "#e5e5e5", "#d4d4d4", "#a3a3a3", "#737373", "#525252", "#404040", "#262626", "#171717", "#0a0a0a"},
		"stone":   {"#fafaf9", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#a8a29e", "#78716c", "#57534e", "#44403c", "#292524", "#1c1917", "#0c0a09"},
	}

	// Color palettes
	colorPalettes := map[string][]string{
		"red":     {"#fef2f2", "#fee2e2", "#fecaca", "#fca5a5", "#f87171", "#ef4444", "#dc2626", "#b91c1c", "#991b1b", "#7f1d1d", "#450a0a"},
		"orange":  {"#fff7ed", "#ffedd5", "#fed7aa", "#fdba74", "#fb923c", "#f97316", "#ea580c", "#c2410c", "#9a3412", "#7c2d12", "#431407"},
		"amber":   {"#fffbeb", "#fef3c7", "#fde68a", "#fcd34d", "#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e", "#78350f", "#451a03"},
		"yellow":  {"#fefce8", "#fef9c3", "#fef08a", "#fde047", "#facc15", "#eab308", "#ca8a04", "#a16207", "#854d0e", "#713f12", "#422006"},
		"lime":    {"#f7fee7", "#ecfccb", "#d9f99d", "#bef264", "#a3e635", "#84cc16", "#65a30d", "#4d7c0f", "#3f6212", "#365314", "#1a2e05"},
		"green":   {"#f0fdf4", "#dcfce7", "#bbf7d0", "#86efac", "#4ade80", "#22c55e", "#16a34a", "#15803d", "#166534", "#14532d", "#052e16"},
		"emerald": {"#ecfdf5", "#d1fae5", "#a7f3d0", "#6ee7b7", "#34d399", "#10b981", "#059669", "#047857", "#065f46", "#064e3b", "#022c22"},
		"teal":    {"#f0fdfa", "#ccfbf1", "#99f6e4", "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e", "#115e59", "#134e4a", "#042f2e"},
		"cyan":    {"#ecfeff", "#cffafe", "#a5f3fc", "#67e8f9", "#22d3ee", "#06b6d4", "#0891b2", "#0e7490", "#155e75", "#164e63", "#083344"},
		"sky":     {"#f0f9ff", "#e0f2fe", "#bae6fd", "#7dd3fc", "#38bdf8", "#0ea5e9", "#0284c7", "#0369a1", "#075985", "#0c4a6e", "#082f49"},
		"blue":    {"#eff6ff", "#dbeafe", "#bfdbfe", "#93c5fd", "#60a5fa", "#3b82f6", "#2563eb", "#1d4ed8", "#1e40af", "#1e3a8a", "#172554"},
		"indigo":  {"#eef2ff", "#e0e7ff", "#c7d2fe", "#a5b4fc", "#818cf8", "#6366f1", "#4f46e5", "#4338ca", "#3730a3", "#312e81", "#1e1b4b"},
		"violet":  {"#f5f3ff", "#ede9fe", "#ddd6fe", "#c4b5fd", "#a78bfa", "#8b5cf6", "#7c3aed", "#6d28d9", "#5b21b6", "#4c1d95", "#2e1065"},
		"purple":  {"#faf5ff", "#f3e8ff", "#e9d5ff", "#d8b4fe", "#c084fc", "#a855f7", "#9333ea", "#7e22ce", "#6b21a8", "#581c87", "#3b0764"},
		"fuchsia": {"#fdf4ff", "#fae8ff", "#f5d0fe", "#f0abfc", "#e879f9", "#d946ef", "#c026d3", "#a21caf", "#86198f", "#701a75", "#4a044e"},
		"pink":    {"#fdf2f8", "#fce7f3", "#fbcfe8", "#f9a8d4", "#f472b6", "#ec4899", "#db2777", "#be185d", "#9d174d", "#831843", "#500724"},
		"rose":    {"#fff1f2", "#ffe4e6", "#fecdd3", "#fda4af", "#fb7185", "#f43f5e", "#e11d48", "#be123c", "#9f1239", "#881337", "#4c0519"},
	}

	shades := []string{"50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"}

	// Add all grayscale colors
	for name, palette := range grayScales {
		for i, shade := range shades {
			if i < len(palette) {
				colors[name+"-"+shade] = palette[i]
			}
		}
	}

	// Add all color palette colors
	for name, palette := range colorPalettes {
		for i, shade := range shades {
			if i < len(palette) {
				colors[name+"-"+shade] = palette[i]
			}
		}
	}

	// Basic colors
	colors["white"] = "#ffffff"
	colors["black"] = "#000000"
	colors["transparent"] = "transparent"

	return colors
}

// getTailwindSpacing returns default Tailwind spacing scale (in pixels)
func getTailwindSpacing() map[string]float32 {
	spacing := map[string]float32{
		"0":    0,
		"px":   1,
		"0.5":  2,
		"1":    4,
		"1.5":  6,
		"2":    8,
		"2.5":  10,
		"3":    12,
		"3.5":  14,
		"4":    16,
		"5":    20,
		"6":    24,
		"7":    28,
		"8":    32,
		"9":    36,
		"10":   40,
		"11":   44,
		"12":   48,
		"14":   56,
		"16":   64,
		"20":   80,
		"24":   96,
		"28":   112,
		"32":   128,
		"36":   144,
		"40":   160,
		"44":   176,
		"48":   192,
		"52":   208,
		"56":   224,
		"60":   240,
		"64":   256,
		"72":   288,
		"80":   320,
		"96":   384,
	}
	return spacing
}

// getTailwindFontSizes returns default font sizes (in pixels)
func getTailwindFontSizes() map[string]float32 {
	return map[string]float32{
		"xs":   12,
		"sm":   14,
		"base": 16,
		"lg":   18,
		"xl":   20,
		"2xl":  24,
		"3xl":  30,
		"4xl":  36,
		"5xl":  48,
		"6xl":  60,
		"7xl":  72,
		"8xl":  96,
		"9xl":  128,
	}
}

// getTailwindFontWeights returns default font weights
func getTailwindFontWeights() map[string]int {
	return map[string]int{
		"thin":       100,
		"extralight": 200,
		"light":      300,
		"normal":     400,
		"medium":     500,
		"semibold":   600,
		"bold":       700,
		"extrabold":  800,
		"black":      900,
	}
}

// getTailwindBorderRadii returns default border radius values (in pixels)
func getTailwindBorderRadii() map[string]float32 {
	return map[string]float32{
		"none": 0,
		"sm":   2,
		"":     4, // DEFAULT
		"md":   6,
		"lg":   8,
		"xl":   12,
		"2xl":  16,
		"3xl":  24,
		"full": 9999,
	}
}

func generateGoCode(config Config) string {
	var b strings.Builder

	b.WriteString("// Code generated by tools/generate - DO NOT EDIT.\n\n")
	b.WriteString("package tw\n\n")
	b.WriteString("import \"fmt\"\n\n")

	// Generate ThemeBreakpoints - the configured breakpoint values
	b.WriteString("// ThemeBreakpoints returns the breakpoint configuration from theme.toml.\n")
	b.WriteString("// These values can be customized in theme.toml under [theme.breakpoints].\n")
	b.WriteString("func ThemeBreakpoints() BreakpointConfig {\n")
	b.WriteString(fmt.Sprintf("\treturn BreakpointConfig{\n\t\tSM:  %d,\n\t\tMD:  %d,\n\t\tLG:  %d,\n\t\tXL:  %d,\n\t\tXXL: %d,\n\t}\n",
		config.Breakpoints.SM, config.Breakpoints.MD, config.Breakpoints.LG, config.Breakpoints.XL, config.Breakpoints.XXL))
	b.WriteString("}\n\n")

	// NOTE: PartialStyle and FontFamilyConfig types are defined in types.go (not generated)

	// Generate ClassMap
	b.WriteString("// ClassMap contains all Tailwind utility classes\n")
	b.WriteString("var ClassMap = map[string]PartialStyle{\n")

	// Collect all classes to sort them
	var classes []string

	// LAYOUT UTILITIES
	layoutClasses := map[string]string{
		"block":        "block",
		"inline":       "inline",
		"inline-block": "inline-block",
		"flex":         "flex",
		"inline-flex":  "inline-flex",
		"grid":         "grid",
		"inline-grid":  "inline-grid",
		"hidden":       "none",
	}
	for className, displayValue := range layoutClasses {
		classes = append(classes, fmt.Sprintf("\t%q: {Display: strPtr(%q)},\n", className, displayValue))
	}

	// POSITION UTILITIES
	positionClasses := []string{"static", "relative", "absolute", "fixed", "sticky"}
	for _, pos := range positionClasses {
		classes = append(classes, fmt.Sprintf("\t%q: {Position: strPtr(%q)},\n", pos, pos))
	}

	// FLEXBOX UTILITIES
	flexDirections := map[string]string{
		"flex-row":            "row",
		"flex-row-reverse":    "row-reverse",
		"flex-col":            "column",
		"flex-col-reverse":    "column-reverse",
	}
	for className, value := range flexDirections {
		classes = append(classes, fmt.Sprintf("\t%q: {FlexDirection: strPtr(%q)},\n", className, value))
	}

	justifyContent := map[string]string{
		"justify-start":   "start",
		"justify-end":     "end",
		"justify-center":  "center",
		"justify-between": "between",
		"justify-around":  "around",
		"justify-evenly":  "evenly",
	}
	for className, value := range justifyContent {
		classes = append(classes, fmt.Sprintf("\t%q: {JustifyContent: strPtr(%q)},\n", className, value))
	}

	alignItems := map[string]string{
		"items-start":    "start",
		"items-end":      "end",
		"items-center":   "center",
		"items-stretch":  "stretch",
		"items-baseline": "baseline",
	}
	for className, value := range alignItems {
		classes = append(classes, fmt.Sprintf("\t%q: {AlignItems: strPtr(%q)},\n", className, value))
	}

	flexWrap := map[string]string{
		"flex-nowrap":       "nowrap",
		"flex-wrap":         "wrap",
		"flex-wrap-reverse": "wrap-reverse",
	}
	for className, value := range flexWrap {
		classes = append(classes, fmt.Sprintf("\t%q: {FlexWrap: strPtr(%q)},\n", className, value))
	}

	// CURSOR UTILITIES
	cursors := []string{"pointer", "default", "not-allowed", "wait", "text", "move", "help", "crosshair", "grab", "grabbing"}
	for _, cursor := range cursors {
		classes = append(classes, fmt.Sprintf("\t\"cursor-%s\": {Cursor: strPtr(%q)},\n", cursor, cursor))
	}

	// OVERFLOW UTILITIES (Tailwind v4)
	// visible: content can overflow, hidden: content is clipped, clip: content is clipped (no scroll),
	// scroll: always show scrollbars, auto: show scrollbars only when content overflows
	overflows := []string{"visible", "hidden", "clip", "scroll", "auto"}
	for _, overflow := range overflows {
		classes = append(classes, fmt.Sprintf("\t\"overflow-%s\": {OverflowX: strPtr(%q), OverflowY: strPtr(%q)},\n", overflow, overflow, overflow))
		classes = append(classes, fmt.Sprintf("\t\"overflow-x-%s\": {OverflowX: strPtr(%q)},\n", overflow, overflow))
		classes = append(classes, fmt.Sprintf("\t\"overflow-y-%s\": {OverflowY: strPtr(%q)},\n", overflow, overflow))
	}

	// Z-INDEX UTILITIES
	zIndexValues := []int{0, 10, 20, 30, 40, 50}
	for _, z := range zIndexValues {
		classes = append(classes, fmt.Sprintf("\t\"z-%d\": {ZIndex: intPtr(%d)},\n", z, z))
	}

	// TEXT ALIGNMENT
	textAligns := map[string]string{
		"text-left":    "left",
		"text-center":  "center",
		"text-right":   "right",
		"text-justify": "justify",
		"text-start":   "start",
		"text-end":     "end",
	}
	for className, value := range textAligns {
		classes = append(classes, fmt.Sprintf("\t%q: {TextAlign: strPtr(%q)},\n", className, value))
	}

	// OPACITY UTILITIES
	for i := 0; i <= 100; i += 5 {
		opacity := float32(i) / 100.0
		classes = append(classes, fmt.Sprintf("\t\"opacity-%d\": {Opacity: ptr(float32(%g))},\n", i, opacity))
	}

	// GAP UTILITIES (flexbox/grid spacing)
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"gap-%s\": {Gap: ptr(float32(%g))},\n", name, px))
	}

	// COLORS
	for name, hex := range config.Colors {
		if hex != "transparent" {
			classes = append(classes, fmt.Sprintf("\t\"text-%s\": {TextColor: ptr(hexToU32(%q))},\n", name, hex))
			classes = append(classes, fmt.Sprintf("\t\"bg-%s\": {BackgroundColor: ptr(hexToU32(%q))},\n", name, hex))
			classes = append(classes, fmt.Sprintf("\t\"border-%s\": {BorderColor: ptr(hexToU32(%q))},\n", name, hex))
		}
	}

	// FONT SIZES
	for name, size := range config.FontSizes {
		classes = append(classes, fmt.Sprintf("\t\"text-%s\": {FontSize: ptr(float32(%g))},\n", name, size))
	}

	// FONT WEIGHTS
	for name, weight := range config.FontWeights {
		classes = append(classes, fmt.Sprintf("\t\"font-%s\": {FontWeight: ptr(%d)},\n", name, weight))
	}

	// FONT FAMILIES (from theme.toml [fonts] section)
	for name := range config.FontFamilies {
		classes = append(classes, fmt.Sprintf("\t\"font-%s\": {FontFamily: strPtr(%q)},\n", name, name))
	}

	// PADDING
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"p-%s\": {PaddingTop: ptr(float32(%g)), PaddingRight: ptr(float32(%g)), PaddingBottom: ptr(float32(%g)), PaddingLeft: ptr(float32(%g))},\n", name, px, px, px, px))
		classes = append(classes, fmt.Sprintf("\t\"px-%s\": {PaddingLeft: ptr(float32(%g)), PaddingRight: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"py-%s\": {PaddingTop: ptr(float32(%g)), PaddingBottom: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"pt-%s\": {PaddingTop: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"pr-%s\": {PaddingRight: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"pb-%s\": {PaddingBottom: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"pl-%s\": {PaddingLeft: ptr(float32(%g))},\n", name, px))
	}

	// MARGIN
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"m-%s\": {MarginTop: ptr(float32(%g)), MarginRight: ptr(float32(%g)), MarginBottom: ptr(float32(%g)), MarginLeft: ptr(float32(%g))},\n", name, px, px, px, px))
		classes = append(classes, fmt.Sprintf("\t\"mx-%s\": {MarginLeft: ptr(float32(%g)), MarginRight: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"my-%s\": {MarginTop: ptr(float32(%g)), MarginBottom: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"mt-%s\": {MarginTop: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"mr-%s\": {MarginRight: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"mb-%s\": {MarginBottom: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"ml-%s\": {MarginLeft: ptr(float32(%g))},\n", name, px))
	}

	// WIDTH/HEIGHT (using spacing scale)
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"w-%s\": {Width: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"h-%s\": {Height: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"min-w-%s\": {MinWidth: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"min-h-%s\": {MinHeight: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"max-w-%s\": {MaxWidth: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"max-h-%s\": {MaxHeight: ptr(float32(%g))},\n", name, px))
	}

	// Width/height fractions as percentages
	fractions := map[string]float32{
		"1/2":  50,
		"1/3":  33.333333,
		"2/3":  66.666667,
		"1/4":  25,
		"2/4":  50,
		"3/4":  75,
		"1/5":  20,
		"2/5":  40,
		"3/5":  60,
		"4/5":  80,
		"1/6":  16.666667,
		"2/6":  33.333333,
		"3/6":  50,
		"4/6":  66.666667,
		"5/6":  83.333333,
		"1/12": 8.333333,
		"2/12": 16.666667,
		"3/12": 25,
		"4/12": 33.333333,
		"5/12": 41.666667,
		"6/12": 50,
		"7/12": 58.333333,
		"8/12": 66.666667,
		"9/12": 75,
		"10/12": 83.333333,
		"11/12": 91.666667,
	}
	for frac, percent := range fractions {
		classes = append(classes, fmt.Sprintf("\t\"w-%s\": {WidthMode: strPtr(\"percent\"), WidthPercent: ptr(float32(%g))},\n", frac, percent))
		classes = append(classes, fmt.Sprintf("\t\"h-%s\": {HeightMode: strPtr(\"percent\"), HeightPercent: ptr(float32(%g))},\n", frac, percent))
	}

	// BORDER RADIUS
	for name, px := range config.Radii {
		if name == "" {
			classes = append(classes, fmt.Sprintf("\t\"rounded\": {BorderRadius: ptr(float32(%g))},\n", px))
		} else {
			classes = append(classes, fmt.Sprintf("\t\"rounded-%s\": {BorderRadius: ptr(float32(%g))},\n", name, px))
		}
	}

	// BORDER WIDTH
	borderWidths := map[string]float32{
		"border-0": 0,
		"border":   1,
		"border-2": 2,
		"border-4": 4,
		"border-8": 8,
	}
	for className, width := range borderWidths {
		classes = append(classes, fmt.Sprintf("\t%q: {BorderWidth: ptr(float32(%g))},\n", className, width))
	}

	// SHADOWS
	shadows := map[string]string{
		"shadow-sm":   "0 1px 2px 0 rgb(0 0 0 / 0.05)",
		"shadow":      "0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1)",
		"shadow-md":   "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
		"shadow-lg":   "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)",
		"shadow-xl":   "0 20px 25px -5px rgb(0 0 0 / 0.1), 0 8px 10px -6px rgb(0 0 0 / 0.1)",
		"shadow-2xl":  "0 25px 50px -12px rgb(0 0 0 / 0.25)",
		"shadow-inner": "inset 0 2px 4px 0 rgb(0 0 0 / 0.05)",
		"shadow-none":  "none",
	}
	for className, shadow := range shadows {
		classes = append(classes, fmt.Sprintf("\t%q: {BoxShadow: strPtr(%q)},\n", className, shadow))
	}

	// TRANSFORMS - Scale
	scaleValues := []float32{0, 50, 75, 90, 95, 100, 105, 110, 125, 150}
	for _, scale := range scaleValues {
		scaleFloat := scale / 100.0
		classes = append(classes, fmt.Sprintf("\t\"scale-%d\": {Scale: ptr(float32(%g))},\n", int(scale), scaleFloat))
	}

	// TRANSFORMS - Rotate
	rotateValues := []int{0, 1, 2, 3, 6, 12, 45, 90, 180}
	for _, deg := range rotateValues {
		classes = append(classes, fmt.Sprintf("\t\"rotate-%d\": {Rotate: ptr(float32(%d))},\n", deg, deg))
		if deg != 0 {
			classes = append(classes, fmt.Sprintf("\t\"-rotate-%d\": {Rotate: ptr(float32(%d))},\n", deg, -deg))
		}
	}

	// TRANSFORMS - Translate (using spacing scale)
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"translate-x-%s\": {TranslateX: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"translate-y-%s\": {TranslateY: ptr(float32(%g))},\n", name, px))
		if px != 0 {
			classes = append(classes, fmt.Sprintf("\t\"-translate-x-%s\": {TranslateX: ptr(float32(%g))},\n", name, -px))
			classes = append(classes, fmt.Sprintf("\t\"-translate-y-%s\": {TranslateY: ptr(float32(%g))},\n", name, -px))
		}
	}

	// TRANSITIONS
	transitionProps := map[string]string{
		"transition-none":       "none",
		"transition-all":        "all",
		"transition":            "background-color, border-color, color, fill, stroke, opacity, box-shadow, transform",
		"transition-colors":     "background-color, border-color, color, fill, stroke",
		"transition-opacity":    "opacity",
		"transition-shadow":     "box-shadow",
		"transition-transform":  "transform",
	}
	for className, prop := range transitionProps {
		classes = append(classes, fmt.Sprintf("\t%q: {TransitionProperty: strPtr(%q)},\n", className, prop))
	}

	// TRANSITION DURATIONS (in milliseconds)
	durations := map[string]float32{
		"duration-75":   75,
		"duration-100":  100,
		"duration-150":  150,
		"duration-200":  200,
		"duration-300":  300,
		"duration-500":  500,
		"duration-700":  700,
		"duration-1000": 1000,
	}
	for className, ms := range durations {
		classes = append(classes, fmt.Sprintf("\t%q: {TransitionDuration: ptr(float32(%g))},\n", className, ms))
	}

	// TRANSITION TIMING FUNCTIONS
	easings := map[string]string{
		"ease-linear":    "linear",
		"ease-in":        "ease-in",
		"ease-out":       "ease-out",
		"ease-in-out":    "ease-in-out",
	}
	for className, easing := range easings {
		classes = append(classes, fmt.Sprintf("\t%q: {TransitionTiming: strPtr(%q)},\n", className, easing))
	}

	// GRID COLUMNS
	for i := 1; i <= 12; i++ {
		classes = append(classes, fmt.Sprintf("\t\"grid-cols-%d\": {GridTemplateColumns: strPtr(\"repeat(%d, minmax(0, 1fr))\")},\n", i, i))
	}
	classes = append(classes, "\t\"grid-cols-none\": {GridTemplateColumns: strPtr(\"none\")},\n")

	// GRID ROWS
	for i := 1; i <= 6; i++ {
		classes = append(classes, fmt.Sprintf("\t\"grid-rows-%d\": {GridTemplateRows: strPtr(\"repeat(%d, minmax(0, 1fr))\")},\n", i, i))
	}
	classes = append(classes, "\t\"grid-rows-none\": {GridTemplateRows: strPtr(\"none\")},\n")

	// COL SPAN
	for i := 1; i <= 12; i++ {
		classes = append(classes, fmt.Sprintf("\t\"col-span-%d\": {GridColumnSpan: intPtr(%d)},\n", i, i))
	}
	classes = append(classes, "\t\"col-auto\": {GridColumnSpan: intPtr(0)},\n") // auto
	classes = append(classes, "\t\"col-span-full\": {GridColumnSpan: intPtr(999)},\n") // full width

	// ROW SPAN
	for i := 1; i <= 6; i++ {
		classes = append(classes, fmt.Sprintf("\t\"row-span-%d\": {GridRowSpan: intPtr(%d)},\n", i, i))
	}
	classes = append(classes, "\t\"row-auto\": {GridRowSpan: intPtr(0)},\n")
	classes = append(classes, "\t\"row-span-full\": {GridRowSpan: intPtr(999)},\n")

	// FLEX UTILITIES
	flexValues := map[string]float32{
		"flex-1":       1,
		"flex-auto":    1,
		"flex-initial": 0,
		"flex-none":    0,
	}
	for className, value := range flexValues {
		if className == "flex-1" || className == "flex-auto" {
			classes = append(classes, fmt.Sprintf("\t%q: {FlexGrow: ptr(float32(%g)), FlexShrink: ptr(float32(1))},\n", className, value))
		} else {
			classes = append(classes, fmt.Sprintf("\t%q: {FlexGrow: ptr(float32(%g)), FlexShrink: ptr(float32(0))},\n", className, value))
		}
	}

	// GROW/SHRINK
	classes = append(classes, "\t\"flex-grow\": {FlexGrow: ptr(float32(1))},\n")
	classes = append(classes, "\t\"flex-grow-0\": {FlexGrow: ptr(float32(0))},\n")
	classes = append(classes, "\t\"flex-shrink\": {FlexShrink: ptr(float32(1))},\n")
	classes = append(classes, "\t\"flex-shrink-0\": {FlexShrink: ptr(float32(0))},\n")

	// FLEX BASIS
	classes = append(classes, "\t\"basis-auto\": {FlexBasisMode: strPtr(\"auto\")},\n")
	classes = append(classes, "\t\"basis-full\": {FlexBasisMode: strPtr(\"percent\"), FlexBasisPercent: ptr(float32(100))},\n")
	// Flex basis with spacing scale (same as padding/margin, includes 0)
	for name, value := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"basis-%s\": {FlexBasisMode: strPtr(\"fixed\"), FlexBasis: ptr(float32(%g))},\n", name, value))
	}
	// Flex basis fractions (same fractions as width/height)
	basisFractions := map[string]float32{
		"1/2":  50,
		"1/3":  33.333333,
		"2/3":  66.666667,
		"1/4":  25,
		"2/4":  50,
		"3/4":  75,
		"1/5":  20,
		"2/5":  40,
		"3/5":  60,
		"4/5":  80,
		"1/6":  16.666667,
		"2/6":  33.333333,
		"3/6":  50,
		"4/6":  66.666667,
		"5/6":  83.333333,
		"1/12": 8.333333,
		"2/12": 16.666667,
		"3/12": 25,
		"4/12": 33.333333,
		"5/12": 41.666667,
		"6/12": 50,
		"7/12": 58.333333,
		"8/12": 66.666667,
		"9/12": 75,
		"10/12": 83.333333,
		"11/12": 91.666667,
	}
	for frac, percent := range basisFractions {
		classes = append(classes, fmt.Sprintf("\t\"basis-%s\": {FlexBasisMode: strPtr(\"percent\"), FlexBasisPercent: ptr(float32(%g))},\n", frac, percent))
	}

	// SELF ALIGNMENT (align-self for individual flex items)
	selfAlignments := map[string]string{
		"self-auto":     "auto",
		"self-start":    "start",
		"self-end":      "end",
		"self-center":   "center",
		"self-stretch":  "stretch",
		"self-baseline": "baseline",
	}
	for className, value := range selfAlignments {
		classes = append(classes, fmt.Sprintf("\t%q: {AlignSelf: strPtr(%q)},\n", className, value))
	}

	// ORDER (flex item ordering)
	classes = append(classes, "\t\"order-first\": {Order: intPtr(-9999)},\n")
	classes = append(classes, "\t\"order-last\": {Order: intPtr(9999)},\n")
	classes = append(classes, "\t\"order-none\": {Order: intPtr(0)},\n")
	for i := 1; i <= 12; i++ {
		classes = append(classes, fmt.Sprintf("\t\"order-%d\": {Order: intPtr(%d)},\n", i, i))
	}
	// Negative orders
	for i := 1; i <= 12; i++ {
		classes = append(classes, fmt.Sprintf("\t\"-order-%d\": {Order: intPtr(%d)},\n", i, -i))
	}

	// LINE HEIGHT
	lineHeights := map[string]float32{
		"leading-none":    1.0,
		"leading-tight":   1.25,
		"leading-snug":    1.375,
		"leading-normal":  1.5,
		"leading-relaxed": 1.625,
		"leading-loose":   2.0,
	}
	for className, lh := range lineHeights {
		classes = append(classes, fmt.Sprintf("\t%q: {LineHeight: ptr(float32(%g))},\n", className, lh))
	}

	// LETTER SPACING
	letterSpacings := map[string]float32{
		"tracking-tighter": -0.05,
		"tracking-tight":   -0.025,
		"tracking-normal":  0,
		"tracking-wide":    0.025,
		"tracking-wider":   0.05,
		"tracking-widest":  0.1,
	}
	for className, spacing := range letterSpacings {
		classes = append(classes, fmt.Sprintf("\t%q: {LetterSpacing: ptr(float32(%g))},\n", className, spacing))
	}

	// ASPECT RATIO (as width/height constraints, simplified)
	aspectRatios := map[string]string{
		"aspect-auto":   "auto",
		"aspect-square": "1/1",
		"aspect-video":  "16/9",
	}
	for className, ratio := range aspectRatios {
		classes = append(classes, fmt.Sprintf("\t%q: {}, // TODO: AspectRatio %s\n", className, ratio))
	}

	// WIDTH/HEIGHT SPECIAL VALUES
	classes = append(classes, "\t\"w-auto\": {WidthMode: strPtr(\"auto\")},\n")
	classes = append(classes, "\t\"w-full\": {WidthMode: strPtr(\"full\")},\n")
	classes = append(classes, "\t\"w-screen\": {WidthMode: strPtr(\"full\")},\n") // screen = 100% of viewport
	classes = append(classes, "\t\"w-min\": {WidthMode: strPtr(\"auto\")},\n")    // min-content approximated as auto
	classes = append(classes, "\t\"w-max\": {WidthMode: strPtr(\"auto\")},\n")    // max-content approximated as auto
	classes = append(classes, "\t\"h-auto\": {HeightMode: strPtr(\"auto\")},\n")
	classes = append(classes, "\t\"h-full\": {HeightMode: strPtr(\"full\")},\n")
	classes = append(classes, "\t\"h-screen\": {HeightMode: strPtr(\"full\")},\n") // screen = 100% of viewport
	classes = append(classes, "\t\"h-min\": {HeightMode: strPtr(\"auto\")},\n")    // min-content approximated as auto
	classes = append(classes, "\t\"h-max\": {HeightMode: strPtr(\"auto\")},\n")    // max-content approximated as auto

	// INSET UTILITIES (for positioned elements)
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"inset-%s\": {Top: ptr(float32(%g)), Right: ptr(float32(%g)), Bottom: ptr(float32(%g)), Left: ptr(float32(%g))},\n", name, px, px, px, px))
		classes = append(classes, fmt.Sprintf("\t\"inset-x-%s\": {Left: ptr(float32(%g)), Right: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"inset-y-%s\": {Top: ptr(float32(%g)), Bottom: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"top-%s\": {Top: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"right-%s\": {Right: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"bottom-%s\": {Bottom: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"left-%s\": {Left: ptr(float32(%g))},\n", name, px))
	}

	// USER SELECT
	userSelects := map[string]string{
		"select-none": "none",
		"select-text": "text",
		"select-all":  "all",
		"select-auto": "auto",
	}
	for className, value := range userSelects {
		classes = append(classes, fmt.Sprintf("\t%q: {UserSelect: strPtr(%q)},\n", className, value))
	}

	// POINTER EVENTS
	pointerEvents := map[string]string{
		"pointer-events-none": "none",
		"pointer-events-auto": "auto",
	}
	for className, value := range pointerEvents {
		classes = append(classes, fmt.Sprintf("\t%q: {PointerEvents: strPtr(%q)},\n", className, value))
	}

	// OBJECT FIT (for images)
	objectFits := map[string]string{
		"object-contain":    "contain",
		"object-cover":      "cover",
		"object-fill":       "fill",
		"object-none":       "none",
		"object-scale-down": "scale-down",
	}
	for className, value := range objectFits {
		classes = append(classes, fmt.Sprintf("\t%q: {ObjectFit: strPtr(%q)},\n", className, value))
	}

	// OBJECT POSITION (for images)
	objectPositions := map[string]string{
		"object-bottom":       "bottom",
		"object-center":       "center",
		"object-left":         "left",
		"object-left-bottom":  "left-bottom",
		"object-left-top":     "left-top",
		"object-right":        "right",
		"object-right-bottom": "right-bottom",
		"object-right-top":    "right-top",
		"object-top":          "top",
	}
	for className, value := range objectPositions {
		classes = append(classes, fmt.Sprintf("\t%q: {ObjectPosition: strPtr(%q)},\n", className, value))
	}

	sort.Strings(classes)
	for _, class := range classes {
		b.WriteString(class)
	}

	b.WriteString("}\n\n")

	// NOTE: FontFamilyConfig type is defined in types.go (not generated)

	// Generate ThemeFonts map
	b.WriteString("// ThemeFonts returns the font family mappings from theme.toml.\n")
	b.WriteString("// These can be used with font-sans, font-serif, font-mono, etc. classes.\n")
	b.WriteString("func ThemeFonts() map[string]FontFamilyConfig {\n")
	b.WriteString("\treturn map[string]FontFamilyConfig{\n")
	for name, fontConfig := range config.FontFamilies {
		b.WriteString(fmt.Sprintf("\t\t%q: {Value: %q, IsBundled: %t},\n", name, fontConfig.Value, fontConfig.IsBundled))
	}
	b.WriteString("\t}\n}\n\n")

	// Helper functions
	b.WriteString(`// ptr returns a pointer to the value (helper for map initialization)
func ptr[T any](v T) *T {
	return &v
}

// strPtr returns a pointer to a string
func strPtr(s string) *string {
	return &s
}

// intPtr returns a pointer to an int
func intPtr(i int) *int {
	return &i
}

// hexToU32 converts a hex color string to a uint32 RGBA value
func hexToU32(hex string) uint32 {
	hex = hex[1:] // Remove #
	var r, g, b uint32
	fmt.Sscanf(hex, "%02x%02x%02x", &r, &g, &b)
	return (r << 24) | (g << 16) | (b << 8) | 0xFF // RGBA with full alpha
}
`)

	return b.String()
}
