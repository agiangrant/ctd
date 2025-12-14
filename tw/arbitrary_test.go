package tw

import "testing"

func TestArbitraryValues(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "arbitrary width percentage",
			input: "w-[33%]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Width == nil || *s.Base.Width != 33.0 {
					t.Errorf("w-[33%%] should be 33, got %v", s.Base.Width)
				}
			},
		},
		{
			name:  "arbitrary height pixels",
			input: "h-[250px]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Height == nil || *s.Base.Height != 250.0 {
					t.Errorf("h-[250px] should be 250, got %v", s.Base.Height)
				}
			},
		},
		{
			name:  "arbitrary rem value",
			input: "p-[2.5rem]",
			validate: func(t *testing.T, s ComputedStyles) {
				expected := float32(2.5 * 16) // 2.5rem = 40px
				if s.Base.PaddingTop == nil || *s.Base.PaddingTop != expected {
					t.Errorf("p-[2.5rem] should be 40px, got %v", s.Base.PaddingTop)
				}
			},
		},
		{
			name:  "arbitrary hex color",
			input: "bg-[#1da1f2]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BackgroundColor == nil {
					t.Error("bg-[#1da1f2] should set background color")
				}
			},
		},
		{
			name:  "arbitrary shorthand hex color",
			input: "text-[#fff]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.TextColor == nil {
					t.Error("text-[#fff] should set text color")
				}
				// #fff expands to #ffffff → 0xFFFFFFFF
				expected := uint32(0xFFFFFFFF)
				if *s.Base.TextColor != expected {
					t.Errorf("text-[#fff] should expand to white, got %08x", *s.Base.TextColor)
				}
			},
		},
		{
			name:  "arbitrary rotate degrees",
			input: "rotate-[17deg]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Rotate == nil || *s.Base.Rotate != 17.0 {
					t.Errorf("rotate-[17deg] should be 17, got %v", s.Base.Rotate)
				}
			},
		},
		{
			name:  "arbitrary scale",
			input: "scale-[1.15]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Scale == nil || *s.Base.Scale != 1.15 {
					t.Errorf("scale-[1.15] should be 1.15, got %v", s.Base.Scale)
				}
			},
		},
		{
			name:  "arbitrary opacity",
			input: "opacity-[0.87]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Opacity == nil || *s.Base.Opacity != 0.87 {
					t.Errorf("opacity-[0.87] should be 0.87, got %v", s.Base.Opacity)
				}
			},
		},
		{
			name:  "arbitrary with variant",
			input: "hover:bg-[#ff6b35]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Hover.BackgroundColor == nil {
					t.Error("hover:bg-[#ff6b35] should set hover background color")
				}
			},
		},
		{
			name:  "arbitrary gap",
			input: "gap-[18px]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Gap == nil || *s.Base.Gap != 18.0 {
					t.Errorf("gap-[18px] should be 18, got %v", s.Base.Gap)
				}
			},
		},
		{
			name:  "arbitrary translate",
			input: "translate-x-[50px] translate-y-[-20px]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.TranslateX == nil || *s.Base.TranslateX != 50.0 {
					t.Errorf("translate-x-[50px] should be 50, got %v", s.Base.TranslateX)
				}
				if s.Base.TranslateY == nil || *s.Base.TranslateY != -20.0 {
					t.Errorf("translate-y-[-20px] should be -20, got %v", s.Base.TranslateY)
				}
			},
		},
		{
			name:  "arbitrary min/max sizing",
			input: "min-w-[200px] max-w-[800px]",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.MinWidth == nil || *s.Base.MinWidth != 200.0 {
					t.Errorf("min-w-[200px] should be 200, got %v", s.Base.MinWidth)
				}
				if s.Base.MaxWidth == nil || *s.Base.MaxWidth != 800.0 {
					t.Errorf("max-w-[800px] should be 800, got %v", s.Base.MaxWidth)
				}
			},
		},
		{
			name:  "mix arbitrary and predefined",
			input: "w-[33%] h-64 bg-[#1da1f2] text-white p-4 rounded-lg",
			validate: func(t *testing.T, s ComputedStyles) {
				// Arbitrary width
				if s.Base.Width == nil {
					t.Error("w-[33%] should set width")
				}
				// Predefined height
				if s.Base.Height == nil {
					t.Error("h-64 should set height")
				}
				// Arbitrary bg
				if s.Base.BackgroundColor == nil {
					t.Error("bg-[#1da1f2] should set background")
				}
				// Predefined text color
				if s.Base.TextColor == nil {
					t.Error("text-white should set text color")
				}
				// Predefined padding
				if s.Base.PaddingTop == nil {
					t.Error("p-4 should set padding")
				}
				// Predefined border radius
				if s.Base.BorderRadius == nil {
					t.Error("rounded-lg should set border radius")
				}
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ParseClasses(tt.input)
			tt.validate(t, result)
		})
	}
}

func TestArbitraryValueParsing(t *testing.T) {
	// Test dimension parsing
	tests := []struct {
		value    string
		expected float32
	}{
		{"33%", 33.0},
		{"250px", 250.0},
		{"2.5rem", 40.0},  // 2.5 * 16
		{"1.5em", 24.0},   // 1.5 * 16
		{"42", 42.0},      // Plain number
	}

	for _, tt := range tests {
		result := parseDimension(tt.value)
		if result == nil || *result != tt.expected {
			t.Errorf("parseDimension(%q) = %v, expected %f", tt.value, result, tt.expected)
		}
	}

	// Test color parsing
	colorTests := []struct {
		value    string
		expected uint32
	}{
		{"#ffffff", 0xFFFFFFFF},
		{"#000000", 0x000000FF},
		{"#1da1f2", 0x1DA1F2FF},
		{"#fff", 0xFFFFFFFF},    // Shorthand
		{"#000", 0x000000FF},    // Shorthand
	}

	for _, tt := range colorTests {
		result := parseColor(tt.value)
		if result == nil || *result != tt.expected {
			t.Errorf("parseColor(%q) = %08x, expected %08x", tt.value, *result, tt.expected)
		}
	}
}

func BenchmarkArbitraryValues(b *testing.B) {
	input := "w-[33%] h-[250px] bg-[#1da1f2] p-[2.5rem] rounded-[12px]"
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		ParseClasses(input)
	}
}
