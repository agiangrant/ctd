package centered

import (
	"encoding/json"

	"github.com/agiangrant/centered/tw"
)

// WidgetKind represents the type of widget
type WidgetKind string

const (
	// Container widgets
	WidgetVStack     WidgetKind = "VStack"
	WidgetHStack     WidgetKind = "HStack"
	WidgetZStack     WidgetKind = "ZStack"
	WidgetContainer  WidgetKind = "Container"
	WidgetScrollView WidgetKind = "ScrollView"

	// Text widgets
	WidgetText    WidgetKind = "Text"
	WidgetHeading WidgetKind = "Heading"
	WidgetLabel   WidgetKind = "Label"

	// Input widgets
	WidgetButton   WidgetKind = "Button"
	WidgetTextField WidgetKind = "TextField"
	WidgetTextArea WidgetKind = "TextArea"
	WidgetCheckbox WidgetKind = "Checkbox"
	WidgetRadio    WidgetKind = "Radio"
	WidgetSlider   WidgetKind = "Slider"
)

// Widget represents a UI widget in the tree
type Widget struct {
	Kind       WidgetKind         `json:"kind"`
	Classes    string             `json:"classes"`
	Text       string             `json:"text,omitempty"`
	CustomData string             `json:"custom_data,omitempty"`
	Children   []Widget           `json:"children,omitempty"`

	// Cached computed styles (not serialized)
	computedStyles *tw.ComputedStyles `json:"-"`
}

// NewWidget creates a new widget of the given kind
func NewWidget(kind WidgetKind) *Widget {
	return &Widget{
		Kind:     kind,
		Children: make([]Widget, 0),
	}
}

// WithClasses sets the Tailwind classes for styling
func (w *Widget) WithClasses(classes string) *Widget {
	w.Classes = classes
	// Parse and cache computed styles
	styles := tw.ParseClasses(classes)
	w.computedStyles = &styles
	return w
}

// WithText sets the text content (for text widgets)
func (w *Widget) WithText(text string) *Widget {
	w.Text = text
	return w
}

// WithCustomData sets custom JSON data
func (w *Widget) WithCustomData(data string) *Widget {
	w.CustomData = data
	return w
}

// WithChildren sets the children of this widget
func (w *Widget) WithChildren(children ...Widget) *Widget {
	w.Children = children
	return w
}

// AddChild adds a single child widget
func (w *Widget) AddChild(child Widget) *Widget {
	w.Children = append(w.Children, child)
	return w
}

// GetComputedStyles returns the cached computed styles
func (w *Widget) GetComputedStyles() *tw.ComputedStyles {
	if w.computedStyles == nil && w.Classes != "" {
		styles := tw.ParseClasses(w.Classes)
		w.computedStyles = &styles
	}
	return w.computedStyles
}

// ToJSON serializes the widget tree to JSON
func (w *Widget) ToJSON() (string, error) {
	data, err := json.Marshal(w)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// Convenience constructors

// VStack creates a vertical stack container
func VStack(classes string, children ...Widget) Widget {
	return Widget{
		Kind:     WidgetVStack,
		Classes:  classes,
		Children: children,
	}
}

// HStack creates a horizontal stack container
func HStack(classes string, children ...Widget) Widget {
	return Widget{
		Kind:     WidgetHStack,
		Classes:  classes,
		Children: children,
	}
}

// ZStack creates a z-index stack container (overlapping)
func ZStack(classes string, children ...Widget) Widget {
	return Widget{
		Kind:     WidgetZStack,
		Classes:  classes,
		Children: children,
	}
}

// Container creates a generic container
func Container(classes string, children ...Widget) Widget {
	return Widget{
		Kind:     WidgetContainer,
		Classes:  classes,
		Children: children,
	}
}

// ScrollView creates a scrollable container
func ScrollView(classes string, children ...Widget) Widget {
	return Widget{
		Kind:     WidgetScrollView,
		Classes:  classes,
		Children: children,
	}
}

// Text creates a text widget
func Text(text, classes string) Widget {
	return Widget{
		Kind:    WidgetText,
		Classes: classes,
		Text:    text,
	}
}

// Heading creates a heading widget
func Heading(text, classes string) Widget {
	return Widget{
		Kind:    WidgetHeading,
		Classes: classes,
		Text:    text,
	}
}

// Label creates a label widget
func Label(text, classes string) Widget {
	return Widget{
		Kind:    WidgetLabel,
		Classes: classes,
		Text:    text,
	}
}

// Button creates a button widget
func Button(text, classes string) Widget {
	return Widget{
		Kind:    WidgetButton,
		Classes: classes,
		Text:    text,
	}
}

// TextField creates a text input field
func TextField(placeholder, classes string) Widget {
	return Widget{
		Kind:       WidgetTextField,
		Classes:    classes,
		CustomData: placeholder,
	}
}

// TextArea creates a multi-line text input
func TextArea(placeholder, classes string) Widget {
	return Widget{
		Kind:       WidgetTextArea,
		Classes:    classes,
		CustomData: placeholder,
	}
}

// Checkbox creates a checkbox widget
func Checkbox(label, classes string) Widget {
	return Widget{
		Kind:    WidgetCheckbox,
		Classes: classes,
		Text:    label,
	}
}

// Radio creates a radio button widget
func Radio(label, classes string) Widget {
	return Widget{
		Kind:    WidgetRadio,
		Classes: classes,
		Text:    label,
	}
}

// Slider creates a slider widget
func Slider(classes string) Widget {
	return Widget{
		Kind:    WidgetSlider,
		Classes: classes,
	}
}

// WidgetTree represents the full widget tree for a frame
type WidgetTree struct {
	Root Widget `json:"root"`
}

// NewWidgetTree creates a new widget tree with the given root
func NewWidgetTree(root Widget) *WidgetTree {
	return &WidgetTree{Root: root}
}

// ToJSON serializes the widget tree to JSON
func (wt *WidgetTree) ToJSON() (string, error) {
	data, err := json.Marshal(wt)
	if err != nil {
		return "", err
	}
	return string(data), nil
}
