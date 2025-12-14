// Example demonstrating form control widgets
package main

import (
	"fmt"
	"log"
	"runtime"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)

	tree := loop.Tree()

	// Status label to show current state
	var statusLabel *retained.Widget

	// Build the UI
	root := buildUI(&statusLabel)
	tree.SetRoot(root)

	// Handle resize
	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	// Handle events
	loop.OnEvent(func(event ffi.Event) bool {
		switch event.Type {
		case ffi.EventKeyPressed:
			if event.Keycode() == uint32(ffi.KeyEscape) {
				ffi.RequestExit()
				return true
			}
		}
		return false
	})

	// Frame callback
	loop.OnFrame(func(frame *retained.Frame) {
		fps := 1.0 / frame.DeltaTime
		frame.DrawText(
			fmt.Sprintf("FPS: %.1f", fps),
			float32(750), 10, 14, retained.ColorWhite,
		)
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Form Controls Demo"
	appConfig.Width = 900
	appConfig.Height = 700

	log.Println("Starting form controls demo...")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI(statusLabel **retained.Widget) *retained.Widget {
	root := retained.Container("").
		WithSize(900, 700).
		WithBackground(retained.Hex("#1a1a2e"))

	// Header
	header := retained.HStack("",
		retained.Text("Form Controls Demo", "").
			WithTextStyle(retained.ColorWhite, 24),
	).
		WithFrame(20, 20, 860, 50).
		WithBackground(retained.Hex("#16213e")).
		WithCornerRadius(8).
		WithPadding(12)

	// Status display
	status := retained.Text("Interact with controls to see updates", "").
		WithFrame(20, 80, 860, 30).
		WithTextStyle(retained.ColorGray400, 14)
	*statusLabel = status

	// Checkbox section
	checkboxPanel := buildCheckboxSection(status)

	// Toggle section
	togglePanel := buildToggleSection(status)

	// Radio section
	radioPanel := buildRadioSection(status)

	// Slider section
	sliderPanel := buildSliderSection(status)

	// Select section
	selectPanel := buildSelectSection(status)

	root.WithChildren(header, status, checkboxPanel, togglePanel, radioPanel, sliderPanel, selectPanel)

	return root
}

func buildCheckboxSection(statusLabel *retained.Widget) *retained.Widget {
	// Checkboxes
	cb1 := retained.Checkbox("Enable notifications", "")
	cb2 := retained.Checkbox("Send email updates", "")
	cb3 := retained.Checkbox("Auto-save enabled", "").SetChecked(true)

	// Wire up change handlers
	cb1.OnChange(func(value any) {
		checked := value.(bool)
		statusLabel.SetText(fmt.Sprintf("Notifications: %v", checked))
	})

	cb2.OnChange(func(value any) {
		checked := value.(bool)
		statusLabel.SetText(fmt.Sprintf("Email updates: %v", checked))
	})

	cb3.OnChange(func(value any) {
		checked := value.(bool)
		statusLabel.SetText(fmt.Sprintf("Auto-save: %v", checked))
	})

	return retained.VStack("",
		retained.Text("Checkboxes", "").WithTextStyle(retained.ColorWhite, 16),
		cb1,
		cb2,
		cb3,
	).
		WithFrame(20, 120, 280, 160).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(10)
}

func buildToggleSection(statusLabel *retained.Widget) *retained.Widget {
	// Toggles
	t1 := retained.Toggle("")
	t2 := retained.Toggle("").SetOn(true)
	t3 := retained.Toggle("").SetDisabled(true)

	// Labels for toggles
	label1 := retained.HStack("",
		retained.Text("Dark Mode", "").WithTextStyle(retained.ColorGray300, 14),
		t1,
	).WithGap(10)

	label2 := retained.HStack("",
		retained.Text("Airplane Mode", "").WithTextStyle(retained.ColorGray300, 14),
		t2,
	).WithGap(10)

	label3 := retained.HStack("",
		retained.Text("Disabled Toggle", "").WithTextStyle(retained.ColorGray500, 14),
		t3,
	).WithGap(10)

	t1.OnChange(func(value any) {
		on := value.(bool)
		statusLabel.SetText(fmt.Sprintf("Dark Mode: %v", on))
	})

	t2.OnChange(func(value any) {
		on := value.(bool)
		statusLabel.SetText(fmt.Sprintf("Airplane Mode: %v", on))
	})

	return retained.VStack("",
		retained.Text("Toggles (iOS-style)", "").WithTextStyle(retained.ColorWhite, 16),
		label1,
		label2,
		label3,
	).
		WithFrame(320, 120, 280, 160).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(10)
}

func buildRadioSection(statusLabel *retained.Widget) *retained.Widget {
	// Radio buttons in a group
	r1 := retained.Radio("Small", "size", "").SetData("small")
	r2 := retained.Radio("Medium", "size", "").SetData("medium").SetChecked(true)
	r3 := retained.Radio("Large", "size", "").SetData("large")

	// Handler for radio group
	handler := func(value any) {
		statusLabel.SetText(fmt.Sprintf("Size selected: %v", value))
	}
	r1.OnChange(handler)
	r2.OnChange(handler)
	r3.OnChange(handler)

	return retained.VStack("",
		retained.Text("Radio Buttons", "").WithTextStyle(retained.ColorWhite, 16),
		retained.Text("Select size:", "").WithTextStyle(retained.ColorGray400, 12),
		r1,
		r2,
		r3,
	).
		WithFrame(620, 120, 260, 180).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)
}

func buildSliderSection(statusLabel *retained.Widget) *retained.Widget {
	// Volume slider (0-100)
	volumeSlider := retained.Slider("").
		SetSliderRange(0, 100).
		SetSliderValue(70).
		SetSliderStep(1)

	volumeLabel := retained.Text("Volume: 70", "").WithTextStyle(retained.ColorGray300, 14)

	volumeSlider.OnChange(func(value any) {
		v := value.(float32)
		volumeLabel.SetText(fmt.Sprintf("Volume: %.0f", v))
		statusLabel.SetText(fmt.Sprintf("Volume changed to: %.0f", v))
	})

	// Brightness slider (continuous)
	brightnessSlider := retained.Slider("").
		SetSliderRange(0, 1).
		SetSliderValue(0.5)

	brightnessLabel := retained.Text("Brightness: 50%", "").WithTextStyle(retained.ColorGray300, 14)

	brightnessSlider.OnChange(func(value any) {
		v := value.(float32)
		brightnessLabel.SetText(fmt.Sprintf("Brightness: %.0f%%", v*100))
		statusLabel.SetText(fmt.Sprintf("Brightness changed to: %.0f%%", v*100))
	})

	// Temperature slider with range
	tempSlider := retained.Slider("").
		SetSliderRange(60, 80).
		SetSliderValue(72).
		SetSliderStep(0.5)

	tempLabel := retained.Text("Temperature: 72.0°F", "").WithTextStyle(retained.ColorGray300, 14)

	tempSlider.OnChange(func(value any) {
		v := value.(float32)
		tempLabel.SetText(fmt.Sprintf("Temperature: %.1f°F", v))
		statusLabel.SetText(fmt.Sprintf("Temperature set to: %.1f°F", v))
	})

	return retained.VStack("",
		retained.Text("Sliders", "").WithTextStyle(retained.ColorWhite, 16),
		volumeLabel,
		volumeSlider,
		brightnessLabel,
		brightnessSlider,
		tempLabel,
		tempSlider,
	).
		WithFrame(20, 300, 420, 220).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)
}

func buildSelectSection(statusLabel *retained.Widget) *retained.Widget {
	// Country select
	countrySelect := retained.Select("Select a country", "").
		SetSelectOptions([]retained.SelectOption{
			{Label: "United States", Value: "us"},
			{Label: "Canada", Value: "ca"},
			{Label: "United Kingdom", Value: "uk"},
			{Label: "Germany", Value: "de"},
			{Label: "France", Value: "fr"},
			{Label: "Japan", Value: "jp"},
		})

	countrySelect.OnChange(func(value any) {
		statusLabel.SetText(fmt.Sprintf("Country selected: %v", value))
	})

	// Priority select
	prioritySelect := retained.Select("Select priority", "").
		SetSelectOptions([]retained.SelectOption{
			{Label: "Low", Value: 1},
			{Label: "Medium", Value: 2},
			{Label: "High", Value: 3},
			{Label: "Critical", Value: 4, Disabled: true},
		}).
		SetSelectedIndex(1) // Default to Medium

	prioritySelect.OnChange(func(value any) {
		statusLabel.SetText(fmt.Sprintf("Priority level: %v", value))
	})

	return retained.VStack("",
		retained.Text("Dropdown Selects", "").WithTextStyle(retained.ColorWhite, 16),
		retained.Text("Country:", "").WithTextStyle(retained.ColorGray400, 12),
		countrySelect,
		retained.Text("Priority:", "").WithTextStyle(retained.ColorGray400, 12),
		prioritySelect,
	).
		WithFrame(460, 300, 420, 220).
		WithBackground(retained.Hex("#0f3460")).
		WithCornerRadius(12).
		WithPadding(16).
		WithGap(8)
}
