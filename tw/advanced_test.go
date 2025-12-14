package tw

import "testing"

func TestTransforms(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "scale",
			input: "scale-110 hover:scale-125",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Scale == nil || *s.Base.Scale != 1.1 {
					t.Errorf("expected scale-110 = 1.1, got %v", s.Base.Scale)
				}
				if s.Hover.Scale == nil || *s.Hover.Scale != 1.25 {
					t.Errorf("expected hover scale = 1.25, got %v", s.Hover.Scale)
				}
			},
		},
		{
			name:  "rotate",
			input: "rotate-45 -rotate-90",
			validate: func(t *testing.T, s ComputedStyles) {
				// -rotate-90 should override rotate-45
				if s.Base.Rotate == nil || *s.Base.Rotate != -90 {
					t.Errorf("expected final rotate = -90, got %v", s.Base.Rotate)
				}
			},
		},
		{
			name:  "translate",
			input: "translate-x-4 translate-y-8",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.TranslateX == nil || *s.Base.TranslateX != 16.0 {
					t.Errorf("expected translate-x-4 = 16px, got %v", s.Base.TranslateX)
				}
				if s.Base.TranslateY == nil || *s.Base.TranslateY != 32.0 {
					t.Errorf("expected translate-y-8 = 32px, got %v", s.Base.TranslateY)
				}
			},
		},
		{
			name:  "negative translate",
			input: "-translate-x-2 -translate-y-4",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.TranslateX == nil || *s.Base.TranslateX != -8.0 {
					t.Errorf("expected -translate-x-2 = -8px, got %v", s.Base.TranslateX)
				}
				if s.Base.TranslateY == nil || *s.Base.TranslateY != -16.0 {
					t.Errorf("expected -translate-y-4 = -16px, got %v", s.Base.TranslateY)
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

func TestTransitions(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "transition with duration",
			input: "transition duration-300 ease-in-out",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.TransitionProperty == nil {
					t.Error("expected TransitionProperty")
				}
				if s.Base.TransitionDuration == nil || *s.Base.TransitionDuration != 300 {
					t.Errorf("expected duration = 300ms, got %v", s.Base.TransitionDuration)
				}
				if s.Base.TransitionTiming == nil || *s.Base.TransitionTiming != "ease-in-out" {
					t.Errorf("expected timing = ease-in-out, got %v", s.Base.TransitionTiming)
				}
			},
		},
		{
			name:  "transition-colors",
			input: "transition-colors duration-150",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.TransitionProperty == nil {
					t.Error("expected TransitionProperty for colors")
				}
				if s.Base.TransitionDuration == nil || *s.Base.TransitionDuration != 150 {
					t.Error("expected duration 150ms")
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

func TestShadows(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "shadow utilities",
			input: "shadow-lg hover:shadow-xl",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BoxShadow == nil {
					t.Error("expected Base.BoxShadow")
				}
				if s.Hover.BoxShadow == nil {
					t.Error("expected Hover.BoxShadow")
				}
			},
		},
		{
			name:  "shadow-none",
			input: "shadow shadow-none",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BoxShadow == nil || *s.Base.BoxShadow != "none" {
					t.Error("shadow-none should override shadow")
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

func TestGrid(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "grid layout",
			input: "grid grid-cols-3 gap-4",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Display == nil || *s.Base.Display != "grid" {
					t.Error("expected Display = grid")
				}
				if s.Base.GridTemplateColumns == nil {
					t.Error("expected GridTemplateColumns")
				}
				if s.Base.Gap == nil || *s.Base.Gap != 16.0 {
					t.Error("expected gap = 16px")
				}
			},
		},
		{
			name:  "col and row span",
			input: "col-span-6 row-span-2",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.GridColumnSpan == nil || *s.Base.GridColumnSpan != 6 {
					t.Errorf("expected col-span = 6, got %v", s.Base.GridColumnSpan)
				}
				if s.Base.GridRowSpan == nil || *s.Base.GridRowSpan != 2 {
					t.Errorf("expected row-span = 2, got %v", s.Base.GridRowSpan)
				}
			},
		},
		{
			name:  "full width grid item",
			input: "col-span-full",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.GridColumnSpan == nil || *s.Base.GridColumnSpan != 999 {
					t.Error("col-span-full should span all columns")
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

func TestFlexUtilities(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "flex-1",
			input: "flex-1",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.FlexGrow == nil || *s.Base.FlexGrow != 1 {
					t.Error("flex-1 should set grow = 1")
				}
				if s.Base.FlexShrink == nil || *s.Base.FlexShrink != 1 {
					t.Error("flex-1 should set shrink = 1")
				}
			},
		},
		{
			name:  "grow and shrink",
			input: "flex-grow flex-shrink-0",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.FlexGrow == nil || *s.Base.FlexGrow != 1 {
					t.Error("flex-grow should be 1")
				}
				if s.Base.FlexShrink == nil || *s.Base.FlexShrink != 0 {
					t.Error("flex-shrink-0 should be 0")
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

func TestTypography(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "line height",
			input: "leading-relaxed",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.LineHeight == nil || *s.Base.LineHeight != 1.625 {
					t.Errorf("leading-relaxed should be 1.625, got %v", s.Base.LineHeight)
				}
			},
		},
		{
			name:  "letter spacing",
			input: "tracking-wide",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.LetterSpacing == nil || *s.Base.LetterSpacing != 0.025 {
					t.Errorf("tracking-wide should be 0.025, got %v", s.Base.LetterSpacing)
				}
			},
		},
		{
			name:  "complete typography",
			input: "text-lg font-semibold leading-tight tracking-tight text-gray-900",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.FontSize == nil || *s.Base.FontSize != 18.0 {
					t.Error("text-lg should be 18px")
				}
				if s.Base.FontWeight == nil || *s.Base.FontWeight != 600 {
					t.Error("font-semibold should be 600")
				}
				if s.Base.LineHeight == nil {
					t.Error("leading-tight should be set")
				}
				if s.Base.LetterSpacing == nil {
					t.Error("tracking-tight should be set")
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

func TestPositioning(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "inset",
			input: "absolute inset-0",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Position == nil || *s.Base.Position != "absolute" {
					t.Error("expected absolute position")
				}
				if s.Base.Top == nil || *s.Base.Top != 0 {
					t.Error("inset-0 should set top = 0")
				}
				if s.Base.Right == nil || *s.Base.Right != 0 {
					t.Error("inset-0 should set right = 0")
				}
			},
		},
		{
			name:  "specific positioning",
			input: "fixed top-4 right-8 z-50",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Position == nil || *s.Base.Position != "fixed" {
					t.Error("expected fixed position")
				}
				if s.Base.Top == nil || *s.Base.Top != 16.0 {
					t.Error("top-4 should be 16px")
				}
				if s.Base.Right == nil || *s.Base.Right != 32.0 {
					t.Error("right-8 should be 32px")
				}
				if s.Base.ZIndex == nil || *s.Base.ZIndex != 50 {
					t.Error("z-50 should be 50")
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

func TestBorderWidths(t *testing.T) {
	result := ParseClasses("border-2 border-gray-300")

	if result.Base.BorderWidth == nil || *result.Base.BorderWidth != 2.0 {
		t.Errorf("border-2 should be 2px, got %v", result.Base.BorderWidth)
	}
	if result.Base.BorderColor == nil {
		t.Error("border-gray-300 should set border color")
	}
}

func TestRealWorldExamples(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{
			name:  "animated button",
			input: "bg-blue-500 hover:bg-blue-600 active:scale-95 transition duration-150 ease-in-out shadow-md hover:shadow-lg",
		},
		{
			name:  "card component",
			input: "bg-white dark:bg-gray-800 rounded-lg shadow-lg p-6 border border-gray-200 dark:border-gray-700",
		},
		{
			name:  "grid gallery",
			input: "grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4",
		},
		{
			name:  "centered modal",
			input: "fixed inset-0 flex items-center justify-center bg-black bg-opacity-50",
		},
		{
			name:  "hero text",
			input: "text-4xl font-bold leading-tight tracking-tight text-gray-900 dark:text-white",
		},
		{
			name:  "smooth animation",
			input: "transform transition-transform duration-300 hover:scale-110 hover:-translate-y-2",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ParseClasses(tt.input)
			// Just validate it doesn't panic - any style being set is fine
			_ = result // Successful parse is the test
		})
	}
}
