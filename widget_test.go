package centered

import (
	"encoding/json"
	"testing"
)

func TestWidgetCreation(t *testing.T) {
	tests := []struct {
		name     string
		widget   Widget
		wantKind WidgetKind
	}{
		{
			name:     "VStack",
			widget:   VStack("flex flex-col"),
			wantKind: WidgetVStack,
		},
		{
			name:     "HStack",
			widget:   HStack("flex flex-row"),
			wantKind: WidgetHStack,
		},
		{
			name:     "Text",
			widget:   Text("Hello", "text-lg"),
			wantKind: WidgetText,
		},
		{
			name:     "Button",
			widget:   Button("Click", "bg-blue-500"),
			wantKind: WidgetButton,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.widget.Kind != tt.wantKind {
				t.Errorf("widget.Kind = %v, want %v", tt.widget.Kind, tt.wantKind)
			}
		})
	}
}

func TestWidgetWithClasses(t *testing.T) {
	widget := NewWidget(WidgetButton).
		WithClasses("bg-blue-500 hover:bg-blue-600 text-white px-4 py-2 rounded")

	if widget.Classes == "" {
		t.Error("expected classes to be set")
	}

	// Verify that styles are computed
	styles := widget.GetComputedStyles()
	if styles == nil {
		t.Error("expected computed styles to be set")
	}

	// Base background color should be set
	if styles.Base.BackgroundColor == nil {
		t.Error("expected base background color to be set")
	}

	// Hover background color should be set
	if styles.Hover.BackgroundColor == nil {
		t.Error("expected hover background color to be set")
	}
}

func TestWidgetChildren(t *testing.T) {
	parent := VStack("flex flex-col gap-4",
		Text("Child 1", "text-lg"),
		Text("Child 2", "text-sm"),
		Button("Click", "bg-blue-500"),
	)

	if len(parent.Children) != 3 {
		t.Errorf("expected 3 children, got %d", len(parent.Children))
	}

	if parent.Children[0].Kind != WidgetText {
		t.Error("first child should be Text widget")
	}

	if parent.Children[2].Kind != WidgetButton {
		t.Error("third child should be Button widget")
	}
}

func TestWidgetSerialization(t *testing.T) {
	widget := VStack("flex flex-col gap-4",
		Text("Hello World", "text-2xl font-bold"),
		Button("Click Me", "bg-blue-500 px-4 py-2 rounded"),
	)

	jsonStr, err := widget.ToJSON()
	if err != nil {
		t.Fatalf("failed to serialize widget: %v", err)
	}

	// Verify it's valid JSON
	var parsed map[string]interface{}
	if err := json.Unmarshal([]byte(jsonStr), &parsed); err != nil {
		t.Fatalf("failed to parse serialized JSON: %v", err)
	}

	// Verify basic structure
	if parsed["kind"] != string(WidgetVStack) {
		t.Errorf("expected kind=VStack, got %v", parsed["kind"])
	}

	children, ok := parsed["children"].([]interface{})
	if !ok {
		t.Fatal("expected children to be an array")
	}

	if len(children) != 2 {
		t.Errorf("expected 2 children in JSON, got %d", len(children))
	}
}

func TestWidgetTree(t *testing.T) {
	root := VStack("w-full h-full",
		Heading("Title", "text-3xl"),
		Container("p-4",
			Text("Content", "text-base"),
		),
	)

	tree := NewWidgetTree(root)

	jsonStr, err := tree.ToJSON()
	if err != nil {
		t.Fatalf("failed to serialize widget tree: %v", err)
	}

	// Verify it's valid JSON
	var parsed map[string]interface{}
	if err := json.Unmarshal([]byte(jsonStr), &parsed); err != nil {
		t.Fatalf("failed to parse tree JSON: %v", err)
	}

	// Verify root exists
	rootObj, ok := parsed["root"].(map[string]interface{})
	if !ok {
		t.Fatal("expected root to be an object")
	}

	if rootObj["kind"] != string(WidgetVStack) {
		t.Error("expected root to be VStack")
	}
}

func TestBuilderPattern(t *testing.T) {
	widget := NewWidget(WidgetButton).
		WithClasses("bg-blue-500 hover:bg-blue-600").
		WithText("Click Me").
		WithCustomData(`{"id": "submit-btn"}`)

	if widget.Kind != WidgetButton {
		t.Error("expected kind to be Button")
	}

	if widget.Classes == "" {
		t.Error("expected classes to be set")
	}

	if widget.Text != "Click Me" {
		t.Error("expected text to be set")
	}

	if widget.CustomData == "" {
		t.Error("expected custom data to be set")
	}
}

func TestAddChild(t *testing.T) {
	parent := NewWidget(WidgetVStack).
		WithClasses("flex flex-col")

	parent.AddChild(Text("Child 1", "text-lg"))
	parent.AddChild(Text("Child 2", "text-sm"))

	if len(parent.Children) != 2 {
		t.Errorf("expected 2 children, got %d", len(parent.Children))
	}
}

func TestArbitraryValuesIntegration(t *testing.T) {
	// Test that arbitrary values work with the widget API
	widget := Container("w-[80%] h-[400px] bg-[#1da1f2] rounded-[16px]",
		Text("Custom sized container", "text-[20px]"),
	)

	styles := widget.GetComputedStyles()
	if styles == nil {
		t.Fatal("expected computed styles")
	}

	// Verify arbitrary width was parsed
	if styles.Base.Width == nil || *styles.Base.Width != 80.0 {
		t.Errorf("expected width=80, got %v", styles.Base.Width)
	}

	// Verify arbitrary height was parsed
	if styles.Base.Height == nil || *styles.Base.Height != 400.0 {
		t.Errorf("expected height=400, got %v", styles.Base.Height)
	}

	// Verify arbitrary background color was parsed
	if styles.Base.BackgroundColor == nil {
		t.Error("expected background color to be set")
	}

	// Verify arbitrary border radius was parsed
	if styles.Base.BorderRadius == nil || *styles.Base.BorderRadius != 16.0 {
		t.Errorf("expected border radius=16, got %v", styles.Base.BorderRadius)
	}
}

func BenchmarkWidgetCreation(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = VStack("flex flex-col gap-4",
			Text("Hello", "text-lg"),
			Button("Click", "bg-blue-500 px-4 py-2 rounded"),
		)
	}
}

func BenchmarkWidgetSerialization(b *testing.B) {
	widget := VStack("flex flex-col gap-4",
		Text("Hello World", "text-2xl font-bold"),
		Button("Click Me", "bg-blue-500 px-4 py-2 rounded"),
		HStack("flex gap-2",
			Button("A", "bg-gray-200 px-2 py-1"),
			Button("B", "bg-gray-200 px-2 py-1"),
			Button("C", "bg-gray-200 px-2 py-1"),
		),
	)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, _ = widget.ToJSON()
	}
}
