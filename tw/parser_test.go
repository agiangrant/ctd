package tw

import "testing"

func TestParseClassesWithVariants(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		validate func(*testing.T, ComputedStyles)
	}{
		{
			name:  "basic classes without variants",
			input: "bg-blue-500 text-white p-4",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BackgroundColor == nil {
					t.Error("expected Base.BackgroundColor to be set")
				}
				if s.Base.TextColor == nil {
					t.Error("expected Base.TextColor to be set")
				}
				if s.Base.PaddingTop == nil || *s.Base.PaddingTop != 16.0 {
					t.Errorf("expected Base.PaddingTop=16.0, got %v", s.Base.PaddingTop)
				}
			},
		},
		{
			name:  "hover variant",
			input: "bg-blue-500 hover:bg-blue-600",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BackgroundColor == nil {
					t.Error("expected Base.BackgroundColor to be set")
				}
				if s.Hover.BackgroundColor == nil {
					t.Error("expected Hover.BackgroundColor to be set")
				}
				// Hover color should be different from base
				if s.Base.BackgroundColor != nil && s.Hover.BackgroundColor != nil {
					if *s.Base.BackgroundColor == *s.Hover.BackgroundColor {
						t.Error("hover color should be different from base")
					}
				}
			},
		},
		{
			name:  "focus variant",
			input: "border-gray-300 focus:border-blue-500",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BorderColor == nil {
					t.Error("expected Base.BorderColor to be set")
				}
				if s.Focus.BorderColor == nil {
					t.Error("expected Focus.BorderColor to be set")
				}
			},
		},
		{
			name:  "dark mode variant",
			input: "bg-white dark:bg-gray-800 text-gray-900 dark:text-white",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BackgroundColor == nil {
					t.Error("expected Base.BackgroundColor to be set")
				}
				if s.Dark.Base.BackgroundColor == nil {
					t.Error("expected Dark.Base.BackgroundColor to be set")
				}
				if s.Base.TextColor == nil {
					t.Error("expected Base.TextColor to be set")
				}
				if s.Dark.Base.TextColor == nil {
					t.Error("expected Dark.Base.TextColor to be set")
				}
			},
		},
		{
			name:  "compound variant: dark mode + hover",
			input: "bg-white hover:bg-gray-100 dark:bg-gray-800 dark:hover:bg-gray-700",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.BackgroundColor == nil {
					t.Error("expected Base.BackgroundColor")
				}
				if s.Hover.BackgroundColor == nil {
					t.Error("expected Hover.BackgroundColor")
				}
				if s.Dark.Base.BackgroundColor == nil {
					t.Error("expected Dark.Base.BackgroundColor")
				}
				if s.Dark.Hover.BackgroundColor == nil {
					t.Error("expected Dark.Hover.BackgroundColor")
				}
			},
		},
		{
			name:  "responsive variant",
			input: "flex-col md:flex-row",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.FlexDirection == nil || *s.Base.FlexDirection != "column" {
					t.Errorf("expected Base.FlexDirection=column, got %v", s.Base.FlexDirection)
				}
				if s.MD.FlexDirection == nil || *s.MD.FlexDirection != "row" {
					t.Errorf("expected MD.FlexDirection=row, got %v", s.MD.FlexDirection)
				}
			},
		},
		{
			name:  "layout utilities",
			input: "flex justify-center items-center gap-4",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Display == nil || *s.Base.Display != "flex" {
					t.Errorf("expected Display=flex, got %v", s.Base.Display)
				}
				if s.Base.JustifyContent == nil || *s.Base.JustifyContent != "center" {
					t.Errorf("expected JustifyContent=center, got %v", s.Base.JustifyContent)
				}
				if s.Base.AlignItems == nil || *s.Base.AlignItems != "center" {
					t.Errorf("expected AlignItems=center, got %v", s.Base.AlignItems)
				}
				if s.Base.Gap == nil || *s.Base.Gap != 16.0 {
					t.Errorf("expected Gap=16.0, got %v", s.Base.Gap)
				}
			},
		},
		{
			name:  "cursor and interactivity",
			input: "cursor-pointer hover:opacity-80",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Cursor == nil || *s.Base.Cursor != "pointer" {
					t.Errorf("expected Cursor=pointer, got %v", s.Base.Cursor)
				}
				if s.Hover.Opacity == nil || *s.Hover.Opacity != 0.80 {
					t.Errorf("expected Hover.Opacity=0.80, got %v", s.Hover.Opacity)
				}
			},
		},
		{
			name:  "position and z-index",
			input: "fixed top-0 right-0 z-50",
			validate: func(t *testing.T, s ComputedStyles) {
				if s.Base.Position == nil || *s.Base.Position != "fixed" {
					t.Errorf("expected Position=fixed, got %v", s.Base.Position)
				}
				if s.Base.ZIndex == nil || *s.Base.ZIndex != 50 {
					t.Errorf("expected ZIndex=50, got %v", s.Base.ZIndex)
				}
			},
		},
		{
			name:  "overflow utilities",
			input: "overflow-hidden overflow-y-scroll",
			validate: func(t *testing.T, s ComputedStyles) {
				// overflow-y-scroll should override overflow-hidden for Y axis
				if s.Base.OverflowY == nil || *s.Base.OverflowY != "scroll" {
					t.Errorf("expected OverflowY=scroll, got %v", s.Base.OverflowY)
				}
				if s.Base.OverflowX == nil || *s.Base.OverflowX != "hidden" {
					t.Errorf("expected OverflowX=hidden, got %v", s.Base.OverflowX)
				}
			},
		},
		{
			name:  "complex real-world button",
			input: "bg-blue-500 hover:bg-blue-600 active:bg-blue-700 text-white font-semibold px-4 py-2 rounded cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed",
			validate: func(t *testing.T, s ComputedStyles) {
				// Base
				if s.Base.BackgroundColor == nil {
					t.Error("expected Base.BackgroundColor")
				}
				if s.Base.Cursor == nil || *s.Base.Cursor != "pointer" {
					t.Error("expected Base.Cursor=pointer")
				}

				// Hover
				if s.Hover.BackgroundColor == nil {
					t.Error("expected Hover.BackgroundColor")
				}

				// Active
				if s.Active.BackgroundColor == nil {
					t.Error("expected Active.BackgroundColor")
				}

				// Disabled
				if s.Disabled.Opacity == nil || *s.Disabled.Opacity != 0.50 {
					t.Error("expected Disabled.Opacity=0.50")
				}
				if s.Disabled.Cursor == nil || *s.Disabled.Cursor != "not-allowed" {
					t.Error("expected Disabled.Cursor=not-allowed")
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

func TestClassMapCompleteness(t *testing.T) {
	// Verify key utility categories exist
	categories := map[string][]string{
		"layout": {"flex", "block", "inline", "grid", "hidden"},
		"position": {"relative", "absolute", "fixed", "sticky"},
		"flexbox": {"flex-row", "flex-col", "justify-center", "items-center"},
		"cursor": {"cursor-pointer", "cursor-not-allowed", "cursor-default"},
		"overflow": {"overflow-hidden", "overflow-scroll", "overflow-auto"},
		"z-index": {"z-0", "z-10", "z-50"},
		"opacity": {"opacity-0", "opacity-50", "opacity-100"},
		"text-align": {"text-left", "text-center", "text-right"},
	}

	for category, classes := range categories {
		for _, class := range classes {
			if _, ok := ClassMap[class]; !ok {
				t.Errorf("Category %s: missing class %s", category, class)
			}
		}
	}
}

func BenchmarkParseClasses(b *testing.B) {
	input := "bg-blue-500 hover:bg-blue-600 text-white font-bold px-4 py-2 rounded cursor-pointer"
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		ParseClasses(input)
	}
}

func BenchmarkParseClassesWithVariants(b *testing.B) {
	input := "bg-white dark:bg-gray-800 hover:bg-gray-100 dark:hover:bg-gray-700 md:flex lg:grid text-gray-900 dark:text-white"
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		ParseClasses(input)
	}
}
