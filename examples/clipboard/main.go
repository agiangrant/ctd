// System Services Widget Demo
//
// This example demonstrates system service widgets - non-rendering
// data sources that provide access to system clipboard, file dialogs, and tray icons.
//
// Features demonstrated:
// - Clipboard widget for reading/writing system clipboard
// - Clipboard change monitoring
// - FilePicker widget for opening native file dialogs
// - Single and multiple file selection
// - File type filters
// - TrayIcon widget for system tray/menu bar icons

package main

import (
	"fmt"
	"log"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

var (
	// Widgets we need to reference
	clipboardDisplay *retained.Widget
	selectedFiles    *retained.Widget
	trayStatus       *retained.Widget
	clipboard        *retained.Widget
	filePicker       *retained.Widget
	trayIcon         *retained.Widget
)

func main() {
	log.Println("Starting System Services Demo...")
	log.Println("  - Copy text anywhere to see clipboard monitoring")
	log.Println("  - Use buttons to interact with clipboard and file picker")
	log.Println("  - Check the menu bar for the tray icon")
	log.Println("  - Press ESC to quit")

	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)
	tree := loop.Tree()

	// Build the UI
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
	appConfig.Title = "System Services Demo"
	appConfig.Width = 700
	appConfig.Height = 600

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI() *retained.Widget {
	// Create non-rendering clipboard widget
	clipboard = retained.Clipboard().
		SetClipboardMonitor(true).
		OnClipboardChange(func(text string) {
			// Update display when clipboard changes
			if clipboardDisplay != nil {
				display := text
				if len(display) > 80 {
					display = display[:80] + "..."
				}
				display = strings.ReplaceAll(display, "\n", " ")
				clipboardDisplay.SetText(fmt.Sprintf("Clipboard: %s", display))
			}
		})

	// Create non-rendering file picker widget
	filePicker = retained.FilePicker().
		SetFilePickerTitle("Select Files").
		SetFilePickerMultiple(true).
		OnFileSelect(func(paths []string) {
			if selectedFiles != nil {
				if len(paths) == 1 {
					selectedFiles.SetText(fmt.Sprintf("Selected: %s", filepath.Base(paths[0])))
				} else {
					selectedFiles.SetText(fmt.Sprintf("Selected %d files", len(paths)))
				}
			}
		}).
		OnFileCancel(func() {
			if selectedFiles != nil {
				selectedFiles.SetText("Selection cancelled")
			}
		})

	// Main container
	root := retained.Container("bg-gray-900").
		WithSize(700, 500)

	// Title
	title := retained.Text("Clipboard & FilePicker Demo", "text-white text-2xl font-bold")

	// =========================================================================
	// Clipboard Section
	// =========================================================================
	clipboardSection := retained.Container("bg-gray-800 rounded-lg")

	clipboardTitle := retained.Text("Clipboard", "text-blue-400 text-lg font-semibold")

	clipboardDisplay = retained.Text("Clipboard: (monitoring...)", "text-gray-300 text-sm")

	// Clipboard buttons
	btnReadClipboard := retained.Container("bg-blue-600 hover:bg-blue-500 rounded").
		WithChildren(
			retained.Text("Read Clipboard", "text-white text-sm"),
		)
	btnReadClipboard.OnClick(func(e *retained.MouseEvent) {
		text := clipboard.ClipboardText()
		if text == "" {
			text = "(empty)"
		}
		if len(text) > 50 {
			text = text[:50] + "..."
		}
		text = strings.ReplaceAll(text, "\n", " ")
		clipboardDisplay.SetText(fmt.Sprintf("Clipboard: %s", text))
	})

	btnCopyHello := retained.Container("bg-green-600 hover:bg-green-500 rounded").
		WithChildren(
			retained.Text("Copy 'Hello!'", "text-white text-sm"),
		)
	btnCopyHello.OnClick(func(e *retained.MouseEvent) {
		clipboard.SetClipboardText("Hello from Centered!")
		clipboardDisplay.SetText("Clipboard: Hello from Centered!")
	})

	btnCopyTimestamp := retained.Container("bg-purple-600 hover:bg-purple-500 rounded").
		WithChildren(
			retained.Text("Copy Timestamp", "text-white text-sm"),
		)
	btnCopyTimestamp.OnClick(func(e *retained.MouseEvent) {
		ts := time.Now().Format("2006-01-02 15:04:05")
		clipboard.SetClipboardText(ts)
		clipboardDisplay.SetText(fmt.Sprintf("Clipboard: %s", ts))
	})

	// =========================================================================
	// FilePicker Section
	// =========================================================================
	fileSection := retained.Container("bg-gray-800 rounded-lg")

	fileTitle := retained.Text("File Picker", "text-green-400 text-lg font-semibold")

	selectedFiles = retained.Text("No files selected", "text-gray-300 text-sm")

	// File picker buttons
	btnOpenAny := retained.Container("bg-blue-600 hover:bg-blue-500 rounded").
		WithChildren(
			retained.Text("Open Any File", "text-white text-sm"),
		)
	btnOpenAny.OnClick(func(e *retained.MouseEvent) {
		filePicker.SetFilePickerFilters(nil)
		filePicker.SetFilePickerMultiple(false)
		filePicker.SetFilePickerTitle("Select a File")
		filePicker.OpenFile()
	})

	btnOpenImages := retained.Container("bg-green-600 hover:bg-green-500 rounded").
		WithChildren(
			retained.Text("Open Images", "text-white text-sm"),
		)
	btnOpenImages.OnClick(func(e *retained.MouseEvent) {
		filePicker.SetFilePickerFilters([]retained.FileFilter{
			{Name: "Images", Extensions: []string{"png", "jpg", "jpeg", "gif", "webp"}},
		})
		filePicker.SetFilePickerMultiple(true)
		filePicker.SetFilePickerTitle("Select Images")
		filePicker.OpenFile()
	})

	btnSave := retained.Container("bg-purple-600 hover:bg-purple-500 rounded").
		WithChildren(
			retained.Text("Save File", "text-white text-sm"),
		)
	btnSave.OnClick(func(e *retained.MouseEvent) {
		filePicker.SetFilePickerFilters([]retained.FileFilter{
			{Name: "Text Files", Extensions: []string{"txt"}},
		})
		filePicker.SetFilePickerTitle("Save As")
		filePicker.OnFileSelect(func(paths []string) {
			if len(paths) > 0 && selectedFiles != nil {
				selectedFiles.SetText(fmt.Sprintf("Would save to: %s", filepath.Base(paths[0])))
			}
		})
		filePicker.SaveFile()
	})

	// =========================================================================
	// TrayIcon Section
	// =========================================================================
	traySection := retained.Container("bg-gray-800 rounded-lg")

	trayTitle := retained.Text("Tray Icon", "text-yellow-400 text-lg font-semibold")

	trayStatus = retained.Text("Tray icon active in menu bar", "text-gray-300 text-sm")

	// Create tray icon with menu
	clickCount := 0
	trayIcon = retained.TrayIconWithTitle("Demo").
		SetTrayTooltip("System Services Demo").
		SetTrayMenu([]retained.MenuItem{
			{Label: "Show Window", Enabled: true, OnClick: func() {
				trayStatus.SetText("Menu: Show Window clicked")
			}},
			{Label: "Copy Timestamp", Enabled: true, OnClick: func() {
				ts := time.Now().Format("15:04:05")
				clipboard.SetClipboardText(ts)
				trayStatus.SetText(fmt.Sprintf("Copied: %s", ts))
			}},
			{Separator: true},
			{Label: "Click Counter: 0", Enabled: true, OnClick: func() {
				clickCount++
				// Update the menu item dynamically
				menu := trayIcon.TrayMenu()
				if len(menu) > 3 {
					menu[3].Label = fmt.Sprintf("Click Counter: %d", clickCount)
					trayIcon.SetTrayMenu(menu)
				}
				trayStatus.SetText(fmt.Sprintf("Counter: %d", clickCount))
			}},
			{Separator: true},
			{Label: "Quit", Enabled: true, OnClick: func() {
				ffi.RequestExit()
			}},
		})

	// Tray icon buttons
	btnShowTray := retained.Container("bg-blue-600 hover:bg-blue-500 rounded").
		WithChildren(
			retained.Text("Show Tray", "text-white text-sm"),
		)
	btnShowTray.OnClick(func(e *retained.MouseEvent) {
		trayIcon.SetTrayVisible(true)
		trayStatus.SetText("Tray icon shown")
	})

	btnHideTray := retained.Container("bg-red-600 hover:bg-red-500 rounded").
		WithChildren(
			retained.Text("Hide Tray", "text-white text-sm"),
		)
	btnHideTray.OnClick(func(e *retained.MouseEvent) {
		trayIcon.SetTrayVisible(false)
		trayStatus.SetText("Tray icon hidden")
	})

	btnUpdateTitle := retained.Container("bg-green-600 hover:bg-green-500 rounded").
		WithChildren(
			retained.Text("Update Title", "text-white text-sm"),
		)
	btnUpdateTitle.OnClick(func(e *retained.MouseEvent) {
		ts := time.Now().Format("15:04")
		trayIcon.SetTrayTitle(ts)
		trayStatus.SetText(fmt.Sprintf("Title updated to: %s", ts))
	})

	// =========================================================================
	// Instructions
	// =========================================================================
	instructions := retained.Text("The clipboard is monitored - copy text anywhere to see it update above. Check the menu bar for the tray icon.", "text-gray-500 text-xs")

	container := retained.Container("flex flex-row w-full flex-wrap gap-4 p-6 bg-gray-900")

	// Add all children to root
	container.WithChildren(
		title,
		// Clipboard section
		clipboardSection,
		clipboardTitle,
		clipboardDisplay,
		btnReadClipboard,
		btnCopyHello,
		btnCopyTimestamp,
		// File picker section
		fileSection,
		fileTitle,
		selectedFiles,
		btnOpenAny,
		btnOpenImages,
		btnSave,
		// Tray icon section
		traySection,
		trayTitle,
		trayStatus,
		btnShowTray,
		btnHideTray,
		btnUpdateTitle,
		// Instructions
		instructions,
		// Non-rendering widgets (must be in tree for updates)
		clipboard,
		filePicker,
		trayIcon,
	)

	root.WithChildren(container)

	return root
}
