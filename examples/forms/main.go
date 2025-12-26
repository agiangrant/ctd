// Example demonstrating the Form system with validation and tab navigation
package main

import (
	"fmt"
	"log"
	"runtime"

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := ctd.DefaultLoopConfig()
	loop := ctd.NewLoop(config)

	tree := loop.Tree()

	// Create the form
	form := ctd.NewForm("registration")

	// Status labels for feedback
	var statusLabel *ctd.Widget
	var errorLabels map[string]*ctd.Widget

	// Build the UI
	root, errLabels := buildUI(form, &statusLabel)
	errorLabels = errLabels
	tree.SetRoot(root)

	// Set up form submit handler
	form.OnSubmit(func(values map[string]any, valid bool) {
		if valid {
			statusLabel.SetText(fmt.Sprintf("Form submitted! Values: %v", values))
			statusLabel.SetTextColor(ctd.ColorGreen500)
			// Clear all error labels
			for _, label := range errorLabels {
				label.SetText("")
			}
		} else {
			statusLabel.SetText("Please fix the errors below")
			statusLabel.SetTextColor(ctd.ColorRed500)
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
				if focused == nil || focused.Kind() != ctd.KindTextArea {
					form.Submit()
					return true
				}
			}
		}
		return false
	})

	// Frame callback
	loop.OnFrame(func(frame *ctd.Frame) {
		fps := 1.0 / frame.DeltaTime
		frame.DrawText(
			fmt.Sprintf("FPS: %.1f", fps),
			float32(750), 10, 14, ctd.ColorWhite,
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

func buildUI(form *ctd.Form, statusLabel **ctd.Widget) (*ctd.Widget, map[string]*ctd.Widget) {
	errorLabels := make(map[string]*ctd.Widget)

	root := ctd.Container("").
		WithSize(900, 700).
		WithBackground(ctd.Hex("#1a1a2e"))

	// Header
	header := ctd.HStack("bg-gray-800 rounded-lg p-4",
		ctd.Text("User Registration Form", "text-white text-2xl").
			WithTextStyle(ctd.ColorWhite, 24))

	// Status display
	status := ctd.Text("Fill out the form and press Enter to submit", "text-gray-400")
	*statusLabel = status

	// Form content panel
	formPanel := ctd.VStack("flex-1 gap-3 p-4 rounded-lg bg-gray-950")

	// Username field
	usernameLabel := ctd.Text("Username *", "").WithTextStyle(ctd.ColorGray300, 14)
	usernameField := ctd.TextField("Enter username", "bg-gray-700 text-white rounded-md px-3 py-2").
		FormField(form, "username", ctd.Required("Username is required"), ctd.MinLength(3, "Username must be at least 3 characters"))
	usernameError := ctd.Text("", "text-red-400 text-sm")
	errorLabels["username"] = usernameError

	// Email field
	emailLabel := ctd.Text("Email *", "").WithTextStyle(ctd.ColorGray300, 14)
	emailField := ctd.TextField("Enter email", "email").
		WithBackground(ctd.Hex("#374151")).
		WithCornerRadius(6).
		FormField(form, "email", ctd.Required("Email is required"), ctd.Email("Please enter a valid email"))
	emailError := ctd.Text("", "").WithTextStyle(ctd.ColorRed400, 12)
	errorLabels["email"] = emailError

	// Age field (using slider)
	ageLabel := ctd.Text("Age", "text-gray-400 text-sm")
	ageValue := ctd.Text("Age: 25", "text-gray-400 text-sm")
	ageSlider := ctd.Slider("px-3 py-2").
		SetSliderRange(18, 100).
		SetSliderValue(25).
		SetSliderStep(1).
		FormField(form, "age", ctd.Min(18, "Must be at least 18"))

	ageSlider.OnChange(func(value any) {
		v := value.(float32)
		ageValue.SetText(fmt.Sprintf("Age: %.0f", v))
	})

	// Newsletter checkbox
	newsletterCheck := ctd.Checkbox("Subscribe to newsletter", "").
		FormField(form, "newsletter")

	// Priority radio group
	priorityLabel := ctd.Text("Contact Priority", "").WithTextStyle(ctd.ColorGray300, 14)
	priorityLow := ctd.Radio("Low", "priority", "priority-low").
		SetData("low").
		FormField(form, "priority")
	priorityMed := ctd.Radio("Medium", "priority", "priority-med").
		SetData("medium").
		SetChecked(true).
		FormField(form, "priority")
	priorityHigh := ctd.Radio("High", "priority", "priority-high").
		SetData("high").
		FormField(form, "priority")

	// Country select
	countryLabel := ctd.Text("Country *", "text-gray-400 text-md")
	countrySelect := ctd.Select("Select country", "bg-gray-700 rounded-md").
		WithBackground(ctd.Hex("#374151")).
		WithCornerRadius(6).
		SetSelectOptions([]ctd.SelectOption{
			{Label: "United States", Value: "us"},
			{Label: "Canada", Value: "ca"},
			{Label: "United Kingdom", Value: "uk"},
			{Label: "Germany", Value: "de"},
			{Label: "France", Value: "fr"},
		}).
		FormField(form, "country", ctd.Required("Please select a country"))
	countryError := ctd.Text("", "text-red-400 text-sm")
	errorLabels["country"] = countryError

	// Dark mode toggle
	darkModeRow := ctd.HStack("gap-1",
		ctd.Text("Enable Dark Mode", "text-gray-400 text-sm"),
		ctd.Toggle("darkmode").SetOn(true).FormField(form, "darkmode"),
	)

	// Buttons
	submitBtn := ctd.Button("Submit", "bg-blue-500 text-white rounded-md px-3 py-2").
		WithTextStyle(ctd.ColorWhite, 14)

	resetBtn := ctd.Button("Reset", "bg-gray-600 text-white rounded-md px-3 py-2").
		WithTextStyle(ctd.ColorWhite, 14)

	submitBtn.OnClick(func(e *ctd.MouseEvent) {
		form.Submit()
	})

	resetBtn.OnClick(func(e *ctd.MouseEvent) {
		form.Reset()
		status.SetText("Form reset")
		status.SetTextColor(ctd.ColorGray400)
		ageValue.SetText("Age: 25")
		// Clear errors
		for _, label := range errorLabels {
			label.SetText("")
		}
	})

	buttonRow := ctd.HStack("gap-2",
		submitBtn,
		resetBtn,
	)

	formPanel.WithChildren(
		usernameLabel, usernameField, usernameError,
		emailLabel, emailField, emailError,
		ageLabel, ageValue, ageSlider,
		newsletterCheck,
		priorityLabel,
		ctd.HStack("gap-2", priorityLow, priorityMed, priorityHigh),
		countryLabel, countrySelect, countryError,
		darkModeRow,
		buttonRow,
	)

	// Instructions panel
	instructionsPanel := ctd.VStack("w-24",
		ctd.Text("Instructions", "text-white text-lg"),
		ctd.Text("", "h-2"), // spacer
		ctd.Text("• Enter: Submit form", "text-gray-400"),
		ctd.Text("• ESC: Exit application", "text-gray-400"),
		ctd.Text("• Click fields to interact", "text-gray-400"),
		ctd.Text("", "h-2"), // spacer
		ctd.Text("Form Features:", "text-white text-md"),
		ctd.Text("• Field validation on submit", "text-gray-400"),
		ctd.Text("• Tab navigation API", "text-gray-400"),
		ctd.Text("• Form reset functionality", "text-gray-400"),
		ctd.Text("• Value tracking & retrieval", "text-gray-400"),
		ctd.Text("• Custom validators", "text-gray-400"),
	)

	// Validators panel
	validatorsPanel := ctd.VStack("flex-1",
		ctd.Text("Built-in Validators", "text-white text-md"),
		ctd.Text("", "h-2"), // spacer
		ctd.Text("• Required(msg)", "text-gray-400"),
		ctd.Text("• MinLength(n, msg)", "text-gray-400"),
		ctd.Text("• MaxLength(n, msg)", "text-gray-400"),
		ctd.Text("• Email(msg)", "text-gray-400"),
		ctd.Text("• Pattern(regex, msg)", "text-gray-400"),
		ctd.Text("• Min(n, msg)", "text-gray-400"),
		ctd.Text("• Max(n, msg)", "text-gray-400"),
		ctd.Text("• CustomValidator(fn)", "text-gray-400"),
	)

	rightPanel := ctd.VStack("flex-1 gap-4")
	rightPanel.WithChildren(instructionsPanel, validatorsPanel)

	bodyLayout := ctd.HStack("gap-8 justify-between")
	bodyLayout.WithChildren(formPanel, rightPanel)
	rootLayout := ctd.VStack("flex-1 gap-4 py-3 px-4")
	rootLayout.WithChildren(header, status, bodyLayout)

	root.WithChildren(rootLayout)

	return root, errorLabels
}
