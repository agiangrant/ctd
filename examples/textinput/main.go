// Text Input Example
// Demonstrates TextField and TextArea widgets with cursor, selection, and keyboard navigation.
package main

import (
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

	root := buildUI()
	tree.SetRoot(root)

	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Text Input Demo"
	appConfig.Width = 900
	appConfig.Height = 700

	log.Println("Starting Text Input Demo...")
	log.Println("  - Click on a field to focus it")
	log.Println("  - Type to enter text")
	log.Println("  - Arrow keys to move cursor")
	log.Println("  - Shift+arrows to select")
	log.Println("  - Cmd/Ctrl+A to select all")
	log.Println("  - Double-click to select word")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI() *ctd.Widget {
	root := ctd.Container("bg-gray-900").
		WithSize(900, 700)

	// Header
	header := ctd.Text("Text Input Widgets Demo", "text-white text-2xl").
		WithFrame(20, 20, 400, 30)

	subtitle := ctd.Text("Click to focus, type to edit, arrow keys to navigate", "text-gray-400 text-sm").
		WithFrame(20, 55, 500, 20)

	// ============ Left Column: Form Example ============

	formPanel := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(20, 100, 420, 580)

	formTitle := ctd.Text("Form Example", "text-yellow-400 text-lg").
		WithFrame(36, 116, 200, 24)

	// Username field
	usernameLabel := ctd.Text("Username", "text-gray-300 text-sm").
		WithFrame(36, 150, 100, 20)
	usernameField := ctd.TextField("Enter username...", "bg-gray-700 text-white rounded px-3 py-2").
		WithFrame(36, 175, 380, 40).
		WithData("usernameField")

	// Password field (masked)
	passwordLabel := ctd.Text("Password (masked)", "text-gray-300 text-sm").
		WithFrame(36, 225, 200, 20)
	passwordField := ctd.TextField("Enter password...", "bg-gray-700 text-white rounded px-3 py-2").
		WithFrame(36, 250, 380, 40).
		SetPassword(true).
		WithData("passwordField")

	// Numbers only field
	numbersLabel := ctd.Text("Numbers only (filtered)", "text-gray-300 text-sm").
		WithFrame(36, 300, 200, 20)
	numbersField := ctd.TextField("Enter numbers...", "bg-gray-700 text-white rounded px-3 py-2").
		WithFrame(36, 325, 380, 40).
		SetCharFilter(func(r rune) bool {
			return r >= '0' && r <= '9'
		}).
		WithData("numbersField")

	// Read-only field
	readonlyLabel := ctd.Text("Read-only (select & copy only)", "text-gray-300 text-sm").
		WithFrame(36, 375, 250, 20)
	readonlyField := ctd.TextField("", "bg-gray-700 text-gray-400 rounded px-3 py-2").
		WithFrame(36, 400, 380, 40).
		SetReadOnly(true).
		SetInputText("This text is read-only").
		WithData("readonlyField")

	// Custom placeholder color (using Tailwind placeholder: variant)
	customPlaceholderLabel := ctd.Text("Custom placeholder (Tailwind)", "text-gray-300 text-sm").
		WithFrame(36, 450, 250, 20)
	customPlaceholderField := ctd.TextField("Blue placeholder text...", "bg-gray-700 text-white rounded px-3 py-2 placeholder:text-blue-400").
		WithFrame(36, 475, 380, 40).
		WithData("customPlaceholderField")

	// Bio textarea
	bioLabel := ctd.Text("Bio (multiline)", "text-gray-300 text-sm").
		WithFrame(36, 525, 100, 20)
	bioField := ctd.TextArea("Tell us about yourself...", "bg-gray-700 text-white rounded px-3 py-2").
		WithFrame(36, 550, 380, 100).
		WithData("bioField")

	formPanel.WithChildren(
		formTitle,
		usernameLabel, usernameField,
		passwordLabel, passwordField,
		numbersLabel, numbersField,
		readonlyLabel, readonlyField,
		customPlaceholderLabel, customPlaceholderField,
		bioLabel, bioField,
	)

	// ============ Right Column: Features Demo ============

	featuresPanel := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(460, 100, 420, 580)

	featuresTitle := ctd.Text("Keyboard Shortcuts", "text-yellow-400 text-lg").
		WithFrame(476, 116, 200, 24)

	shortcuts := []struct {
		key  string
		desc string
	}{
		{"←/→", "Move cursor left/right"},
		{"↑/↓", "Move cursor up/down (TextArea)"},
		{"Home/End", "Jump to line start/end"},
		{"Cmd+←/→", "Jump to line start/end"},
		{"Option+←/→", "Move by word"},
		{"Shift+arrows", "Extend selection"},
		{"Cmd+A", "Select all"},
		{"Cmd+C", "Copy to clipboard"},
		{"Cmd+V", "Paste from clipboard"},
		{"Cmd+X", "Cut to clipboard"},
		{"Cmd+Z", "Undo"},
		{"Cmd+Shift+Z", "Redo"},
		{"Double-click", "Select word"},
		{"Triple-click", "Select line/all"},
		{"Backspace", "Delete before cursor"},
		{"Delete", "Delete after cursor"},
		{"Option+Backspace", "Delete word before"},
	}

	var shortcutWidgets []*ctd.Widget
	shortcutWidgets = append(shortcutWidgets, featuresTitle)

	yOffset := float32(150)
	for _, s := range shortcuts {
		keyWidget := ctd.Text(s.key, "text-blue-400 text-xs").
			WithFrame(476, yOffset, 120, 16)
		descWidget := ctd.Text(s.desc, "text-gray-400 text-xs").
			WithFrame(600, yOffset, 250, 16)
		shortcutWidgets = append(shortcutWidgets, keyWidget, descWidget)
		yOffset += 18
	}

	// Add new features section
	newFeaturesTitle := ctd.Text("New Features", "text-yellow-400 text-lg").
		WithFrame(476, yOffset+10, 200, 24)
	shortcutWidgets = append(shortcutWidgets, newFeaturesTitle)
	yOffset += 40

	features := []struct {
		name string
		desc string
	}{
		{"Password", "Masks characters with bullets"},
		{"Read-only", "Select & copy, but no edit"},
		{"Char filter", "Restrict allowed characters"},
		{"Validation", "Check if input is valid"},
		{"Placeholder", "Custom placeholder color"},
	}

	for _, f := range features {
		nameWidget := ctd.Text(f.name, "text-green-400 text-xs").
			WithFrame(476, yOffset, 80, 16)
		descWidget := ctd.Text(f.desc, "text-gray-400 text-xs").
			WithFrame(560, yOffset, 290, 16)
		shortcutWidgets = append(shortcutWidgets, nameWidget, descWidget)
		yOffset += 18
	}

	featuresPanel.WithChildren(shortcutWidgets...)

	root.WithChildren(
		header,
		subtitle,
		formPanel,
		featuresPanel,
	)

	return root
}
