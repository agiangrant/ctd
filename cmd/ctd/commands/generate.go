package commands

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"sort"
	"strings"

	"github.com/pelletier/go-toml/v2"
)

// Generate implements the 'ctd generate' command
func Generate(args []string) error {
	fs := flag.NewFlagSet("generate", flag.ExitOnError)
	themeFile := fs.String("theme", "theme.toml", "Path to theme.toml file")
	outputDir := fs.String("output", "tw", "Output directory for generated.go")
	watch := fs.Bool("watch", false, "Watch for changes and regenerate")
	fs.Parse(args)

	if *watch {
		return generateWatch(*themeFile, *outputDir)
	}

	return generateOnce(*themeFile, *outputDir)
}

func generateOnce(themeFile, outputDir string) error {
	// Read theme.toml if it exists
	var userConfig ThemeConfigGen
	if data, err := os.ReadFile(themeFile); err == nil {
		if err := toml.Unmarshal(data, &userConfig); err != nil {
			return fmt.Errorf("failed to parse %s: %w", themeFile, err)
		}
	}

	// Build complete configuration (defaults + user overrides)
	config := buildConfigGen(userConfig)

	// Generate Go code
	code := generateGoCodeGen(config)

	// Ensure output directory exists
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		return fmt.Errorf("failed to create output directory: %w", err)
	}

	// Write to tw/generated.go
	outputPath := outputDir + "/generated.go"
	if err := os.WriteFile(outputPath, []byte(code), 0644); err != nil {
		return fmt.Errorf("failed to write %s: %w", outputPath, err)
	}

	fmt.Printf("✓ Generated %s\n", outputPath)
	return nil
}

func generateWatch(themeFile, outputDir string) error {
	fmt.Printf("Watching %s for changes...\n", themeFile)
	fmt.Println("Press Ctrl+C to stop")

	// Get initial mod time
	info, err := os.Stat(themeFile)
	if err != nil {
		info = nil
	}
	var lastMod int64
	if info != nil {
		lastMod = info.ModTime().UnixNano()
	}

	// Initial generation
	if err := generateOnce(themeFile, outputDir); err != nil {
		fmt.Printf("Warning: %v\n", err)
	}

	// Watch loop
	for {
		info, err := os.Stat(themeFile)
		if err == nil {
			currentMod := info.ModTime().UnixNano()
			if currentMod != lastMod {
				lastMod = currentMod
				fmt.Printf("\n%s changed, regenerating...\n", themeFile)
				if err := generateOnce(themeFile, outputDir); err != nil {
					fmt.Printf("Error: %v\n", err)
				}
			}
		}
		cmd := exec.Command("sleep", "1")
		cmd.Run()
	}
}

// ThemeConfigGen represents the user's theme configuration
type ThemeConfigGen struct {
	Theme struct {
		Breakpoints map[string]int         `toml:"breakpoints"`
		Spacing     map[string]string      `toml:"spacing"`
		Colors      map[string]interface{} `toml:"colors"`
	} `toml:"theme"`
	Fonts     map[string]string   `toml:"fonts"`
	Utilities map[string][]string `toml:"utilities"`
}

// FontConfigGen represents a font configuration (system name or bundled file path)
type FontConfigGen struct {
	Value     string // System font name or file path
	IsBundled bool   // true if Value is a file path, false if system font
}

// ConfigGen holds the merged configuration
type ConfigGen struct {
	Breakpoints  struct{ SM, MD, LG, XL, XXL int }
	Colors       map[string]string
	Spacing      map[string]float32
	FontSizes    map[string]float32
	FontWeights  map[string]int
	FontFamilies map[string]FontConfigGen
	Radii        map[string]float32
}

func buildConfigGen(user ThemeConfigGen) ConfigGen {
	config := ConfigGen{
		Colors:      getTailwindColorsGen(),
		Spacing:     getTailwindSpacingGen(),
		FontSizes:   getTailwindFontSizesGen(),
		FontWeights: getTailwindFontWeightsGen(),
		FontFamilies: map[string]FontConfigGen{
			"sans":  {Value: "system", IsBundled: false},
			"serif": {Value: "Times New Roman", IsBundled: false},
			"mono":  {Value: "Menlo", IsBundled: false},
		},
		Radii: getTailwindBorderRadiiGen(),
	}

	config.Breakpoints.SM = 640
	config.Breakpoints.MD = 768
	config.Breakpoints.LG = 1024
	config.Breakpoints.XL = 1280
	config.Breakpoints.XXL = 1536

	// Merge user overrides
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

	for key, value := range user.Theme.Spacing {
		var px float32
		if strings.HasSuffix(value, "rem") {
			var rem float64
			fmt.Sscanf(value, "%frem", &rem)
			px = float32(rem * 16)
		} else if strings.HasSuffix(value, "px") {
			fmt.Sscanf(value, "%fpx", &px)
		}
		config.Spacing[key] = px
	}

	for key, value := range user.Theme.Colors {
		if str, ok := value.(string); ok {
			// Simple color: primary = "#FF6B35"
			config.Colors[key] = str
		} else if nested, ok := value.(map[string]interface{}); ok {
			// Nested color palette: [theme.colors.gray] 900 = "#1a1a2e"
			for shade, shadeValue := range nested {
				if shadeStr, ok := shadeValue.(string); ok {
					config.Colors[key+"-"+shade] = shadeStr
				}
			}
		}
	}

	// Merge user fonts - detect if path (bundled) or system font name
	for name, value := range user.Fonts {
		config.FontFamilies[name] = FontConfigGen{
			Value:     value,
			IsBundled: isFontPathGen(value),
		}
	}

	return config
}

// isFontPathGen detects if a value is a file path (bundled font) or system font name
func isFontPathGen(value string) bool {
	lower := strings.ToLower(value)
	if strings.HasSuffix(lower, ".ttf") || strings.HasSuffix(lower, ".otf") ||
		strings.HasSuffix(lower, ".woff") || strings.HasSuffix(lower, ".woff2") {
		return true
	}
	if strings.Contains(value, "/") || strings.Contains(value, "\\") {
		return true
	}
	return false
}

func getTailwindColorsGen() map[string]string {
	colors := make(map[string]string)
	shades := []string{"50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"}

	palettes := map[string][]string{
		"slate":   {"#f8fafc", "#f1f5f9", "#e2e8f0", "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155", "#1e293b", "#0f172a", "#020617"},
		"gray":    {"#f9fafb", "#f3f4f6", "#e5e7eb", "#d1d5db", "#9ca3af", "#6b7280", "#4b5563", "#374151", "#1f2937", "#111827", "#030712"},
		"zinc":    {"#fafafa", "#f4f4f5", "#e4e4e7", "#d4d4d8", "#a1a1aa", "#71717a", "#52525b", "#3f3f46", "#27272a", "#18181b", "#09090b"},
		"neutral": {"#fafafa", "#f5f5f5", "#e5e5e5", "#d4d4d4", "#a3a3a3", "#737373", "#525252", "#404040", "#262626", "#171717", "#0a0a0a"},
		"stone":   {"#fafaf9", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#a8a29e", "#78716c", "#57534e", "#44403c", "#292524", "#1c1917", "#0c0a09"},
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

	for name, palette := range palettes {
		for i, shade := range shades {
			if i < len(palette) {
				colors[name+"-"+shade] = palette[i]
			}
		}
	}

	colors["white"] = "#ffffff"
	colors["black"] = "#000000"
	colors["transparent"] = "transparent"

	return colors
}

func getTailwindSpacingGen() map[string]float32 {
	return map[string]float32{
		"0": 0, "px": 1, "0.5": 2, "1": 4, "1.5": 6, "2": 8, "2.5": 10,
		"3": 12, "3.5": 14, "4": 16, "5": 20, "6": 24, "7": 28, "8": 32,
		"9": 36, "10": 40, "11": 44, "12": 48, "14": 56, "16": 64,
		"20": 80, "24": 96, "28": 112, "32": 128, "36": 144, "40": 160,
		"44": 176, "48": 192, "52": 208, "56": 224, "60": 240, "64": 256,
		"72": 288, "80": 320, "96": 384,
	}
}

func getTailwindFontSizesGen() map[string]float32 {
	return map[string]float32{
		"xs": 12, "sm": 14, "base": 16, "lg": 18, "xl": 20,
		"2xl": 24, "3xl": 30, "4xl": 36, "5xl": 48, "6xl": 60,
		"7xl": 72, "8xl": 96, "9xl": 128,
	}
}

func getTailwindFontWeightsGen() map[string]int {
	return map[string]int{
		"thin": 100, "extralight": 200, "light": 300, "normal": 400,
		"medium": 500, "semibold": 600, "bold": 700, "extrabold": 800, "black": 900,
	}
}

func getTailwindBorderRadiiGen() map[string]float32 {
	return map[string]float32{
		"none": 0, "sm": 2, "": 4, "md": 6, "lg": 8,
		"xl": 12, "2xl": 16, "3xl": 24, "full": 9999,
	}
}

func generateGoCodeGen(config ConfigGen) string {
	var b strings.Builder

	b.WriteString("// Code generated by ctd generate - DO NOT EDIT.\n\n")
	b.WriteString("package tw\n\n")
	b.WriteString("import (\n")
	b.WriteString("\t\"fmt\"\n\n")
	b.WriteString("\tctdtw \"github.com/agiangrant/ctd/tw\"\n")
	b.WriteString(")\n\n")

	// Register function - allows consumers to register their theme with the framework
	b.WriteString("// Register registers this theme configuration with the ctd framework.\n")
	b.WriteString("// Call this at app startup before creating any widgets.\n")
	b.WriteString("func Register() {\n")
	b.WriteString("\tctdtw.SetConfig(ctdtw.ThemeConfig{\n")
	b.WriteString("\t\tClassMap:    classMap,\n")
	b.WriteString("\t\tFonts:       themeFonts,\n")
	b.WriteString("\t\tBreakpoints: themeBreakpoints,\n")
	b.WriteString("\t})\n")
	b.WriteString("}\n\n")

	// Breakpoints (private, accessed via Register)
	b.WriteString("// themeBreakpoints is the breakpoint configuration.\n")
	b.WriteString(fmt.Sprintf("var themeBreakpoints = ctdtw.BreakpointConfig{SM: %d, MD: %d, LG: %d, XL: %d, XXL: %d}\n\n",
		config.Breakpoints.SM, config.Breakpoints.MD, config.Breakpoints.LG, config.Breakpoints.XL, config.Breakpoints.XXL))

	// ClassMap (private, accessed via Register)
	b.WriteString("// classMap contains all Tailwind utility classes\n")
	b.WriteString("var classMap = map[string]ctdtw.PartialStyle{\n")

	var classes []string

	// Layout
	for cls, val := range map[string]string{"block": "block", "inline": "inline", "flex": "flex", "grid": "grid", "hidden": "none"} {
		classes = append(classes, fmt.Sprintf("\t%q: {Display: strPtr(%q)},\n", cls, val))
	}

	// Position
	for _, pos := range []string{"static", "relative", "absolute", "fixed", "sticky"} {
		classes = append(classes, fmt.Sprintf("\t%q: {Position: strPtr(%q)},\n", pos, pos))
	}

	// Flexbox
	for cls, val := range map[string]string{"flex-row": "row", "flex-col": "column", "flex-row-reverse": "row-reverse", "flex-col-reverse": "column-reverse"} {
		classes = append(classes, fmt.Sprintf("\t%q: {FlexDirection: strPtr(%q)},\n", cls, val))
	}
	for cls, val := range map[string]string{"justify-start": "start", "justify-end": "end", "justify-center": "center", "justify-between": "between", "justify-around": "around"} {
		classes = append(classes, fmt.Sprintf("\t%q: {JustifyContent: strPtr(%q)},\n", cls, val))
	}
	for cls, val := range map[string]string{"items-start": "start", "items-end": "end", "items-center": "center", "items-stretch": "stretch", "items-baseline": "baseline"} {
		classes = append(classes, fmt.Sprintf("\t%q: {AlignItems: strPtr(%q)},\n", cls, val))
	}
	for cls, val := range map[string]string{"flex-wrap": "wrap", "flex-nowrap": "nowrap"} {
		classes = append(classes, fmt.Sprintf("\t%q: {FlexWrap: strPtr(%q)},\n", cls, val))
	}
	classes = append(classes, "\t\"flex-1\": {FlexGrow: ptr(float32(1)), FlexShrink: ptr(float32(1))},\n")
	classes = append(classes, "\t\"flex-none\": {FlexGrow: ptr(float32(0)), FlexShrink: ptr(float32(0))},\n")
	classes = append(classes, "\t\"flex-grow\": {FlexGrow: ptr(float32(1))},\n")
	classes = append(classes, "\t\"flex-grow-0\": {FlexGrow: ptr(float32(0))},\n")
	classes = append(classes, "\t\"flex-shrink\": {FlexShrink: ptr(float32(1))},\n")
	classes = append(classes, "\t\"flex-shrink-0\": {FlexShrink: ptr(float32(0))},\n")

	// Text alignment
	for cls, val := range map[string]string{"text-left": "left", "text-center": "center", "text-right": "right"} {
		classes = append(classes, fmt.Sprintf("\t%q: {TextAlign: strPtr(%q)},\n", cls, val))
	}

	// Colors
	for name, hex := range config.Colors {
		if hex != "transparent" {
			classes = append(classes, fmt.Sprintf("\t\"text-%s\": {TextColor: ptr(hexToU32(%q))},\n", name, hex))
			classes = append(classes, fmt.Sprintf("\t\"bg-%s\": {BackgroundColor: ptr(hexToU32(%q))},\n", name, hex))
			classes = append(classes, fmt.Sprintf("\t\"border-%s\": {BorderColor: ptr(hexToU32(%q))},\n", name, hex))
		}
	}

	// Font sizes
	for name, size := range config.FontSizes {
		classes = append(classes, fmt.Sprintf("\t\"text-%s\": {FontSize: ptr(float32(%g))},\n", name, size))
	}

	// Font weights
	for name, weight := range config.FontWeights {
		classes = append(classes, fmt.Sprintf("\t\"font-%s\": {FontWeight: ptr(%d)},\n", name, weight))
	}

	// Font families
	for name := range config.FontFamilies {
		classes = append(classes, fmt.Sprintf("\t\"font-%s\": {FontFamily: strPtr(%q)},\n", name, name))
	}

	// Spacing (padding, margin, gap)
	for name, px := range config.Spacing {
		classes = append(classes, fmt.Sprintf("\t\"p-%s\": {PaddingTop: ptr(float32(%g)), PaddingRight: ptr(float32(%g)), PaddingBottom: ptr(float32(%g)), PaddingLeft: ptr(float32(%g))},\n", name, px, px, px, px))
		classes = append(classes, fmt.Sprintf("\t\"px-%s\": {PaddingLeft: ptr(float32(%g)), PaddingRight: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"py-%s\": {PaddingTop: ptr(float32(%g)), PaddingBottom: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"pt-%s\": {PaddingTop: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"pr-%s\": {PaddingRight: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"pb-%s\": {PaddingBottom: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"pl-%s\": {PaddingLeft: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"m-%s\": {MarginTop: ptr(float32(%g)), MarginRight: ptr(float32(%g)), MarginBottom: ptr(float32(%g)), MarginLeft: ptr(float32(%g))},\n", name, px, px, px, px))
		classes = append(classes, fmt.Sprintf("\t\"mx-%s\": {MarginLeft: ptr(float32(%g)), MarginRight: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"my-%s\": {MarginTop: ptr(float32(%g)), MarginBottom: ptr(float32(%g))},\n", name, px, px))
		classes = append(classes, fmt.Sprintf("\t\"mt-%s\": {MarginTop: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"mr-%s\": {MarginRight: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"mb-%s\": {MarginBottom: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"ml-%s\": {MarginLeft: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"gap-%s\": {Gap: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"w-%s\": {Width: ptr(float32(%g))},\n", name, px))
		classes = append(classes, fmt.Sprintf("\t\"h-%s\": {Height: ptr(float32(%g))},\n", name, px))
	}

	// Width/height special values
	classes = append(classes, "\t\"w-auto\": {WidthMode: strPtr(\"auto\")},\n")
	classes = append(classes, "\t\"w-full\": {WidthMode: strPtr(\"full\")},\n")
	classes = append(classes, "\t\"h-auto\": {HeightMode: strPtr(\"auto\")},\n")
	classes = append(classes, "\t\"h-full\": {HeightMode: strPtr(\"full\")},\n")

	// Border radius
	for name, px := range config.Radii {
		if name == "" {
			classes = append(classes, fmt.Sprintf("\t\"rounded\": {BorderRadius: ptr(float32(%g))},\n", px))
		} else {
			classes = append(classes, fmt.Sprintf("\t\"rounded-%s\": {BorderRadius: ptr(float32(%g))},\n", name, px))
		}
	}

	// Border width
	for cls, w := range map[string]float32{"border": 1, "border-0": 0, "border-2": 2, "border-4": 4, "border-8": 8} {
		classes = append(classes, fmt.Sprintf("\t%q: {BorderWidth: ptr(float32(%g))},\n", cls, w))
	}

	// Opacity
	for i := 0; i <= 100; i += 5 {
		classes = append(classes, fmt.Sprintf("\t\"opacity-%d\": {Opacity: ptr(float32(%g))},\n", i, float32(i)/100.0))
	}

	// Overflow
	for _, v := range []string{"visible", "hidden", "scroll", "auto"} {
		classes = append(classes, fmt.Sprintf("\t\"overflow-%s\": {OverflowX: strPtr(%q), OverflowY: strPtr(%q)},\n", v, v, v))
	}

	// Cursor
	for _, c := range []string{"pointer", "default", "not-allowed", "wait", "text"} {
		classes = append(classes, fmt.Sprintf("\t\"cursor-%s\": {Cursor: strPtr(%q)},\n", c, c))
	}

	// Z-index
	for _, z := range []int{0, 10, 20, 30, 40, 50} {
		classes = append(classes, fmt.Sprintf("\t\"z-%d\": {ZIndex: intPtr(%d)},\n", z, z))
	}

	sort.Strings(classes)
	for _, class := range classes {
		b.WriteString(class)
	}

	b.WriteString("}\n\n")

	// themeFonts (private, accessed via Register)
	b.WriteString("// themeFonts is the font family mappings from theme.toml.\n")
	b.WriteString("var themeFonts = map[string]ctdtw.FontFamilyConfig{\n")
	for name, fontConfig := range config.FontFamilies {
		b.WriteString(fmt.Sprintf("\t%q: {Value: %q, IsBundled: %t},\n", name, fontConfig.Value, fontConfig.IsBundled))
	}
	b.WriteString("}\n\n")

	// Helper functions
	b.WriteString(helperFuncsGen)

	return b.String()
}

const helperFuncsGen = `func ptr[T any](v T) *T { return &v }
func strPtr(s string) *string { return &s }
func intPtr(i int) *int { return &i }

func hexToU32(hex string) uint32 {
	hex = hex[1:]
	var r, g, b uint32
	fmt.Sscanf(hex, "%02x%02x%02x", &r, &g, &b)
	return (r << 24) | (g << 16) | (b << 8) | 0xFF
}
`
