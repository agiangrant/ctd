// Example demonstrating the Form system with validation and tab navigation
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

	// Create the form
	form := retained.NewForm("registration")

	// Status labels for feedback
	var statusLabel *retained.Widget
	var errorLabels map[string]*retained.Widget

	// Build the UI
	root, errLabels := buildUI(form, &statusLabel)
	errorLabels = errLabels
	tree.SetRoot(root)

	// Set up form submit handler
	form.OnSubmit(func(values map[string]any, valid bool) {
		if valid {
			statusLabel.SetText(fmt.Sprintf("Form submitted! Values: %v", values))
			statusLabel.SetTextColor(retained.ColorGreen500)
			// Clear all error labels
			for _, label := range errorLabels {
				label.SetText("")
			}
		} else {
			statusLabel.SetText("Please fix the errors below")
			statusLabel.SetTextColor(retained.ColorRed500)
			// Show validation errors
			errors := form.Errors()
			for name, label := range errorLabels {
				if err, ok := errors[name]; ok {
					label.SetText(err.Error())
				} else {
					label.SetText("")
				}
			}
		}
	})

	// Handle resize
	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	// Handle events
	loop.OnEvent(func(event ffi.Event) bool {
		switch event.Type {
		case ffi.EventKeyPressed:
			keycode := event.Keycode()

			// Escape to quit
			if keycode == uint32(ffi.KeyEscape) {
				ffi.RequestExit()
				return true
			}

			// Tab navigation between form fields
			if keycode == uint32(ffi.KeyTab) {
				currentFocused := loop.Events().FocusedWidget()
				shift := event.HasShift()
				nextWidget := form.HandleTabKey(currentFocused, shift)
				if nextWidget != nil {
					loop.Events().Focus(nextWidget)
					return true
				}
			}

			// Enter to submit (only when not in a TextArea)
			if keycode == uint32(ffi.KeyEnter) {
				focused := loop.Events().FocusedWidget()
				// Don't submit if we're in a TextArea (which handles Enter for newlines)
				if focused == nil || focused.Kind() != retained.KindTextArea {
					form.Submit()
					return true
				}
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
	appConfig.Title = "Form System Demo"
	appConfig.Width = 900
	appConfig.Height = 700

	log.Println("Starting form demo...")
	log.Println("  - Enter to submit form")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI(form *retained.Form, statusLabel **retained.Widget) (*retained.Widget, map[string]*retained.Widget) {
	errorLabels := make(map[string]*retained.Widget)

	root := retained.Container("").
		WithSize(900, 700).
		WithBackground(retained.Hex("#1a1a2e"))

	// Header
	header := retained.HStack("bg-gray-800 rounded-lg p-4",
		retained.Text("User Registration Form", "text-white text-2xl").
			WithTextStyle(retained.ColorWhite, 24))

	// Status display
	status := retained.Text("Fill out the form and press Enter to submit", "text-gray-400")
	*statusLabel = status

	// Form content panel
	formPanel := retained.VStack("flex-1 gap-3 p-4 rounded-lg bg-gray-950")

	// Username field
	usernameLabel := retained.Text("Username *", "").WithTextStyle(retained.ColorGray300, 14)
	usernameField := retained.TextField("Enter username", "bg-gray-700 text-white rounded-md px-3 py-2").
		FormField(form, "username", retained.Required("Username is required"), retained.MinLength(3, "Username must be at least 3 characters"))
	usernameError := retained.Text("", "text-red-400 text-sm")
	errorLabels["username"] = usernameError

	// Email field
	emailLabel := retained.Text("Email *", "").WithTextStyle(retained.ColorGray300, 14)
	emailField := retained.TextField("Enter email", "email").
		WithBackground(retained.Hex("#374151")).
		WithCornerRadius(6).
		FormField(form, "email", retained.Required("Email is required"), retained.Email("Please enter a valid email"))
	emailError := retained.Text("", "").WithTextStyle(retained.ColorRed400, 12)
	errorLabels["email"] = emailError

	// Age field (using slider)
	ageLabel := retained.Text("Age", "text-gray-400 text-sm")
	ageValue := retained.Text("Age: 25", "text-gray-400 text-sm")
	ageSlider := retained.Slider("px-3 py-2").
		SetSliderRange(18, 100).
		SetSliderValue(25).
		SetSliderStep(1).
		FormField(form, "age", retained.Min(18, "Must be at least 18"))

	ageSlider.OnChange(func(value any) {
		v := value.(float32)
		ageValue.SetText(fmt.Sprintf("Age: %.0f", v))
	})

	// Newsletter checkbox
	newsletterCheck := retained.Checkbox("Subscribe to newsletter", "").
		FormField(form, "newsletter")

	// Priority radio group
	priorityLabel := retained.Text("Contact Priority", "").WithTextStyle(retained.ColorGray300, 14)
	priorityLow := retained.Radio("Low", "priority", "priority-low").
		SetData("low").
		FormField(form, "priority")
	priorityMed := retained.Radio("Medium", "priority", "priority-med").
		SetData("medium").
		SetChecked(true).
		FormField(form, "priority")
	priorityHigh := retained.Radio("High", "priority", "priority-high").
		SetData("high").
		FormField(form, "priority")

	// Country select
	countryLabel := retained.Text("Country *", "text-gray-400 text-md")
	countrySelect := retained.Select("Select country", "bg-gray-700 rounded-md").
		WithBackground(retained.Hex("#374151")).
		WithCornerRadius(6).
		SetSelectOptions([]retained.SelectOption{
			{Label: "United States", Value: "us"},
			{Label: "Canada", Value: "ca"},
			{Label: "United Kingdom", Value: "uk"},
			{Label: "Germany", Value: "de"},
			{Label: "France", Value: "fr"},
		}).
		FormField(form, "country", retained.Required("Please select a country"))
	countryError := retained.Text("", "text-red-400 text-sm")
	errorLabels["country"] = countryError

	// Dark mode toggle
	darkModeRow := retained.HStack("gap-1",
		retained.Text("Enable Dark Mode", "text-gray-400 text-sm"),
		retained.Toggle("darkmode").SetOn(true).FormField(form, "darkmode"),
	)

	// Buttons
	submitBtn := retained.Button("Submit", "bg-blue-500 text-white rounded-md px-3 py-2").
		WithTextStyle(retained.ColorWhite, 14)

	resetBtn := retained.Button("Reset", "bg-gray-600 text-white rounded-md px-3 py-2").
		WithTextStyle(retained.ColorWhite, 14)

	submitBtn.OnClick(func(e *retained.MouseEvent) {
		form.Submit()
	})

	resetBtn.OnClick(func(e *retained.MouseEvent) {
		form.Reset()
		status.SetText("Form reset")
		status.SetTextColor(retained.ColorGray400)
		ageValue.SetText("Age: 25")
		// Clear errors
		for _, label := range errorLabels {
			label.SetText("")
		}
	})

	buttonRow := retained.HStack("gap-2",
		submitBtn,
		resetBtn,
	)

	formPanel.WithChildren(
		usernameLabel, usernameField, usernameError,
		emailLabel, emailField, emailError,
		ageLabel, ageValue, ageSlider,
		newsletterCheck,
		priorityLabel,
		retained.HStack("gap-2", priorityLow, priorityMed, priorityHigh),
		countryLabel, countrySelect, countryError,
		darkModeRow,
		buttonRow,
	)

	// Instructions panel
	instructionsPanel := retained.VStack("w-24",
		retained.Text("Instructions", "text-white text-lg"),
		retained.Text("", "h-2"), // spacer
		retained.Text("• Enter: Submit form", "text-gray-400"),
		retained.Text("• ESC: Exit application", "text-gray-400"),
		retained.Text("• Click fields to interact", "text-gray-400"),
		retained.Text("", "h-2"), // spacer
		retained.Text("Form Features:", "text-white text-md"),
		retained.Text("• Field validation on submit", "text-gray-400"),
		retained.Text("• Tab navigation API", "text-gray-400"),
		retained.Text("• Form reset functionality", "text-gray-400"),
		retained.Text("• Value tracking & retrieval", "text-gray-400"),
		retained.Text("• Custom validators", "text-gray-400"),
	)

	// Validators panel
	validatorsPanel := retained.VStack("flex-1",
		retained.Text("Built-in Validators", "text-white text-md"),
		retained.Text("", "h-2"), // spacer
		retained.Text("• Required(msg)", "text-gray-400"),
		retained.Text("• MinLength(n, msg)", "text-gray-400"),
		retained.Text("• MaxLength(n, msg)", "text-gray-400"),
		retained.Text("• Email(msg)", "text-gray-400"),
		retained.Text("• Pattern(regex, msg)", "text-gray-400"),
		retained.Text("• Min(n, msg)", "text-gray-400"),
		retained.Text("• Max(n, msg)", "text-gray-400"),
		retained.Text("• CustomValidator(fn)", "text-gray-400"),
	)

	rightPanel := retained.VStack("flex-1 gap-4")
	rightPanel.WithChildren(instructionsPanel, validatorsPanel)

	bodyLayout := retained.HStack("gap-8 justify-between")
	bodyLayout.WithChildren(formPanel, rightPanel)
	rootLayout := retained.VStack("flex-1 gap-4 py-3 px-4")
	rootLayout.WithChildren(header, status, bodyLayout)

	root.WithChildren(rootLayout)

	return root, errorLabels
}
