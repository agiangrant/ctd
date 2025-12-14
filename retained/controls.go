package retained

import "github.com/agiangrant/centered/internal/ffi"

// Control widgets: Checkbox, Toggle, Radio, Slider, Select
// These provide common form input controls with event handling and rendering.

// ============================================================================
// Checkbox
// ============================================================================

// Checkbox creates a checkbox widget with an optional label.
// A checkbox can be toggled on/off independently.
// Default styling: 14px font, gray-200 text color.
// Size is auto-calculated based on box (18px) + label text.
func Checkbox(label string, classes string) *Widget {
	w := NewWidget(KindCheckbox)
	w.text = label
	// Default text styling for control labels
	w.fontSize = 14
	w.textColor = 0xE5E7EBFF // gray-200
	if classes != "" {
		w.SetClasses(classes)
	}
	setupCheckboxHandlers(w)
	return w
}

func setupCheckboxHandlers(w *Widget) {
	w.OnClick(func(e *MouseEvent) {
		if w.disabled {
			return
		}
		w.mu.Lock()
		w.checked = !w.checked
		checked := w.checked
		callback := w.onChangeValue
		w.dirtyMask |= DirtyText // Rerender
		w.mu.Unlock()

		if callback != nil {
			callback(checked)
		}
	})

	// Space/Enter to toggle when focused
	w.OnKeyDown(func(e *KeyEvent) {
		if w.disabled {
			return
		}
		if e.KeyCode == uint32(ffi.KeyEnter) || e.KeyCode == uint32(ffi.KeySpace) { // Enter or Space
			w.mu.Lock()
			w.checked = !w.checked
			checked := w.checked
			callback := w.onChangeValue
			w.dirtyMask |= DirtyText
			w.mu.Unlock()

			if callback != nil {
				callback(checked)
			}
		}
	})
}

// Checked returns whether the checkbox is checked.
func (w *Widget) Checked() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.checked
}

// SetChecked sets the checked state.
func (w *Widget) SetChecked(checked bool) *Widget {
	w.mu.Lock()
	if w.checked != checked {
		w.checked = checked
		w.dirtyMask |= DirtyText
	}
	w.mu.Unlock()
	return w
}

// OnChange sets a callback for when the value changes.
// For Checkbox/Toggle: callback receives bool
// For Radio: callback receives the selected radio's value
// For Slider: callback receives float32
// For Select: callback receives the selected option's Value
func (w *Widget) OnChange(fn func(value any)) *Widget {
	w.mu.Lock()
	w.onChangeValue = fn
	w.mu.Unlock()
	return w
}

// ============================================================================
// Toggle (iOS-style switch)
// ============================================================================

// Toggle creates an iOS-style toggle switch widget.
// Visually different from Checkbox but functionally similar.
func Toggle(classes string) *Widget {
	w := NewWidget(KindToggle)
	if classes != "" {
		w.SetClasses(classes)
	}
	setupToggleHandlers(w)
	return w
}

func setupToggleHandlers(w *Widget) {
	w.OnClick(func(e *MouseEvent) {
		if w.disabled {
			return
		}
		w.mu.Lock()
		w.checked = !w.checked
		checked := w.checked
		callback := w.onChangeValue
		w.dirtyMask |= DirtyText
		w.mu.Unlock()

		if callback != nil {
			callback(checked)
		}
	})

	// Space/Enter to toggle
	w.OnKeyDown(func(e *KeyEvent) {
		if w.disabled {
			return
		}
		if e.KeyCode == uint32(ffi.KeyEnter) || e.KeyCode == uint32(ffi.KeySpace) { // Enter or Space
			w.mu.Lock()
			w.checked = !w.checked
			checked := w.checked
			callback := w.onChangeValue
			w.dirtyMask |= DirtyText
			w.mu.Unlock()

			if callback != nil {
				callback(checked)
			}
		}
	})
}

// IsOn returns whether the toggle is on (same as Checked).
func (w *Widget) IsOn() bool {
	return w.Checked()
}

// SetOn sets the toggle state (same as SetChecked).
func (w *Widget) SetOn(on bool) *Widget {
	return w.SetChecked(on)
}

// ============================================================================
// Radio
// ============================================================================

// Radio creates a radio button widget.
// Radio buttons in the same group (same parent with same radioGroup) are mutually exclusive.
// Default styling: 14px font, gray-200 text color.
// Size is auto-calculated based on circle (18px) + label text.
func Radio(label string, group string, classes string) *Widget {
	w := NewWidget(KindRadio)
	w.text = label
	w.radioGroup = group
	// Default text styling for control labels
	w.fontSize = 14
	w.textColor = 0xE5E7EBFF // gray-200
	if classes != "" {
		w.SetClasses(classes)
	}
	setupRadioHandlers(w)
	return w
}

func setupRadioHandlers(w *Widget) {
	w.OnClick(func(e *MouseEvent) {
		if w.disabled {
			return
		}
		selectRadio(w)
	})

	// Space/Enter to select, Arrow keys to navigate within group
	w.OnKeyDown(func(e *KeyEvent) {
		if w.disabled {
			return
		}
		if e.KeyCode == uint32(ffi.KeyEnter) || e.KeyCode == uint32(ffi.KeySpace) {
			selectRadio(w)
			return
		}

		// Arrow keys navigate within the same radio group
		switch ffi.Keycode(e.KeyCode) {
		case ffi.KeyUp, ffi.KeyLeft:
			navigateRadioGroup(w, -1)
		case ffi.KeyDown, ffi.KeyRight:
			navigateRadioGroup(w, 1)
		}
	})
}

// navigateRadioGroup moves focus and selection to the next/prev radio in the same group.
// direction: -1 for previous, 1 for next
func navigateRadioGroup(w *Widget, direction int) {
	w.mu.RLock()
	group := w.radioGroup
	parent := w.parent
	tree := w.tree
	w.mu.RUnlock()

	if parent == nil {
		return
	}

	// Find all radios in the same group
	parent.mu.RLock()
	siblings := make([]*Widget, 0, len(parent.children))
	for _, child := range parent.children {
		child.mu.RLock()
		if child.kind == KindRadio && child.radioGroup == group && !child.disabled {
			siblings = append(siblings, child)
		}
		child.mu.RUnlock()
	}
	parent.mu.RUnlock()

	if len(siblings) < 2 {
		return
	}

	// Find current index
	currentIdx := -1
	for i, s := range siblings {
		if s == w {
			currentIdx = i
			break
		}
	}

	if currentIdx == -1 {
		return
	}

	// Calculate next index with wrap-around
	nextIdx := (currentIdx + direction + len(siblings)) % len(siblings)
	nextRadio := siblings[nextIdx]

	// Select the next radio (this also updates the visual state)
	selectRadio(nextRadio)

	// Request focus on the next radio via the tree's event dispatcher
	if tree != nil {
		if events := tree.EventDispatcher(); events != nil {
			events.Focus(nextRadio)
		}
	}
}

// selectRadio selects this radio and deselects others in the same group.
func selectRadio(w *Widget) {
	w.mu.Lock()
	if w.checked {
		w.mu.Unlock()
		return // Already selected
	}

	group := w.radioGroup
	parent := w.parent
	w.checked = true
	w.dirtyMask |= DirtyText
	callback := w.onChangeValue
	value := w.data
	w.mu.Unlock()

	// Deselect siblings in the same group
	if parent != nil {
		parent.mu.RLock()
		siblings := parent.children
		parent.mu.RUnlock()

		for _, sibling := range siblings {
			if sibling == w {
				continue
			}
			sibling.mu.Lock()
			if sibling.kind == KindRadio && sibling.radioGroup == group && sibling.checked {
				sibling.checked = false
				sibling.dirtyMask |= DirtyText
			}
			sibling.mu.Unlock()
		}
	}

	if callback != nil {
		callback(value)
	}
}

// RadioGroup returns the radio button's group name.
func (w *Widget) RadioGroup() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.radioGroup
}

// SetRadioGroup sets the radio button's group name.
func (w *Widget) SetRadioGroup(group string) *Widget {
	w.mu.Lock()
	w.radioGroup = group
	w.mu.Unlock()
	return w
}

// ============================================================================
// Slider
// ============================================================================

// Slider creates a slider widget for selecting a numeric value.
// Default range is 0-100 with value at 50.
// Width defaults to fill parent (w-full), height is auto-sized for thumb.
func Slider(classes string) *Widget {
	w := NewWidget(KindSlider)
	w.sliderMin = 0
	w.sliderMax = 100
	w.sliderValue = 50
	w.sliderStep = 0 // Continuous
	// Sliders should fill available width by default
	w.widthMode = SizeFull
	if classes != "" {
		w.SetClasses(classes)
	}
	setupSliderHandlers(w)
	return w
}

func setupSliderHandlers(w *Widget) {
	var isDragging bool

	w.OnMouseDown(func(e *MouseEvent) {
		if w.disabled {
			return
		}
		isDragging = true
		updateSliderFromMouse(w, e.LocalX)
	})

	w.OnMouseMove(func(e *MouseEvent) {
		if w.disabled || !isDragging {
			return
		}
		updateSliderFromMouse(w, e.LocalX)
	})

	w.OnMouseUp(func(e *MouseEvent) {
		isDragging = false
	})

	// Arrow keys to adjust value
	w.OnKeyDown(func(e *KeyEvent) {
		if w.disabled {
			return
		}

		w.mu.Lock()
		step := w.sliderStep
		if step == 0 {
			step = (w.sliderMax - w.sliderMin) / 100 // 1% steps
		}
		min := w.sliderMin
		max := w.sliderMax
		value := w.sliderValue
		w.mu.Unlock()

		var newValue float32
		switch ffi.Keycode(e.KeyCode) {
		case ffi.KeyLeft, ffi.KeyDown:
			newValue = value - step
		case ffi.KeyRight, ffi.KeyUp:
			newValue = value + step
		default:
			return
		}

		// Clamp
		if newValue < min {
			newValue = min
		}
		if newValue > max {
			newValue = max
		}

		w.mu.Lock()
		if w.sliderValue != newValue {
			w.sliderValue = newValue
			w.dirtyMask |= DirtyText
			callback := w.onChangeValue
			w.mu.Unlock()

			if callback != nil {
				callback(newValue)
			}
		} else {
			w.mu.Unlock()
		}
	})
}

func updateSliderFromMouse(w *Widget, localX float32) {
	w.mu.Lock()
	// Calculate ratio from mouse position
	// Use computed layout width if available (for auto-sized widgets)
	widgetWidth := w.width
	if w.computedLayout.Valid && w.computedLayout.Width > 0 {
		widgetWidth = w.computedLayout.Width
	}
	trackWidth := widgetWidth - w.padding[1] - w.padding[3]
	if trackWidth <= 0 {
		trackWidth = widgetWidth
	}
	x := localX - w.padding[3]
	ratio := x / trackWidth
	if ratio < 0 {
		ratio = 0
	}
	if ratio > 1 {
		ratio = 1
	}

	// Map ratio to value range
	min := w.sliderMin
	max := w.sliderMax
	step := w.sliderStep
	newValue := min + ratio*(max-min)

	// Apply step snapping
	if step > 0 {
		newValue = min + float32(int((newValue-min)/step+0.5))*step
	}

	// Clamp
	if newValue < min {
		newValue = min
	}
	if newValue > max {
		newValue = max
	}

	changed := w.sliderValue != newValue
	w.sliderValue = newValue
	callback := w.onChangeValue
	if changed {
		w.dirtyMask |= DirtyText
	}
	w.mu.Unlock()

	if changed && callback != nil {
		callback(newValue)
	}
}

// SliderValue returns the current slider value.
func (w *Widget) SliderValue() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.sliderValue
}

// SetSliderValue sets the slider value.
func (w *Widget) SetSliderValue(value float32) *Widget {
	w.mu.Lock()
	if value < w.sliderMin {
		value = w.sliderMin
	}
	if value > w.sliderMax {
		value = w.sliderMax
	}
	if w.sliderValue != value {
		w.sliderValue = value
		w.dirtyMask |= DirtyText
	}
	w.mu.Unlock()
	return w
}

// SetSliderRange sets the minimum and maximum values.
func (w *Widget) SetSliderRange(min, max float32) *Widget {
	w.mu.Lock()
	w.sliderMin = min
	w.sliderMax = max
	// Clamp current value
	if w.sliderValue < min {
		w.sliderValue = min
	}
	if w.sliderValue > max {
		w.sliderValue = max
	}
	w.dirtyMask |= DirtyText
	w.mu.Unlock()
	return w
}

// SetSliderStep sets the step increment (0 = continuous).
func (w *Widget) SetSliderStep(step float32) *Widget {
	w.mu.Lock()
	w.sliderStep = step
	w.mu.Unlock()
	return w
}

// SliderRatio returns the current value as a ratio (0-1) within the range.
func (w *Widget) SliderRatio() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.sliderMax == w.sliderMin {
		return 0
	}
	return (w.sliderValue - w.sliderMin) / (w.sliderMax - w.sliderMin)
}

// SliderMin returns the slider's minimum value.
func (w *Widget) SliderMin() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.sliderMin
}

// SliderMax returns the slider's maximum value.
func (w *Widget) SliderMax() float32 {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.sliderMax
}

// ============================================================================
// Select (Dropdown)
// ============================================================================

// Select creates a dropdown select widget.
// Pass options using SetSelectOptions().
// Default styling: 14px font, gray-200 text color.
// Width defaults to fill parent (w-full), height is auto-sized.
func Select(placeholder string, classes string) *Widget {
	w := NewWidget(KindSelect)
	w.text = placeholder
	w.selectIndex = -1 // No selection
	// Default text styling
	w.fontSize = 14
	w.textColor = 0xE5E7EBFF // gray-200
	// Select dropdowns should fill available width by default
	w.widthMode = SizeFull
	if classes != "" {
		w.SetClasses(classes)
	}
	setupSelectHandlers(w)
	return w
}

func setupSelectHandlers(w *Widget) {
	w.OnClick(func(e *MouseEvent) {
		if w.disabled {
			return
		}

		w.mu.Lock()
		// Get widget dimensions for dropdown bounds calculation
		height := w.height
		if w.computedLayout.Valid {
			height = w.computedLayout.Height
		}

		// Check if dropdown is open and click is in dropdown area
		if w.selectOpen && len(w.selectOptions) > 0 {
			optionHeight := float32(32)
			dropdownTop := height + 4 // Gap between trigger and dropdown

			// Check if click is in dropdown
			if e.LocalY >= dropdownTop {
				// Calculate which option was clicked
				relativeY := e.LocalY - dropdownTop
				clickedIndex := int(relativeY / optionHeight)

				if clickedIndex >= 0 && clickedIndex < len(w.selectOptions) {
					opt := w.selectOptions[clickedIndex]
					if !opt.Disabled {
						w.selectIndex = clickedIndex
						w.selectOpen = false
						w.dirtyMask |= DirtyText
						callback := w.onChangeValue
						value := opt.Value
						w.mu.Unlock()

						if callback != nil {
							callback(value)
						}
						return
					}
				}
			}
			// Click was on the trigger area while dropdown is open - close it
			w.selectOpen = false
			w.dirtyMask |= DirtyText
			w.mu.Unlock()
			return
		}

		// Dropdown was closed, open it
		w.selectOpen = true
		w.dirtyMask |= DirtyText
		w.mu.Unlock()
	})

	// Arrow keys to navigate, Enter to select
	w.OnKeyDown(func(e *KeyEvent) {
		if w.disabled {
			return
		}

		w.mu.Lock()
		isOpen := w.selectOpen
		optCount := len(w.selectOptions)
		currentIndex := w.selectIndex
		w.mu.Unlock()

		if !isOpen {
			// Enter/Space to open
			if e.KeyCode == uint32(ffi.KeyEnter) || e.KeyCode == uint32(ffi.KeySpace) {
				w.mu.Lock()
				w.selectOpen = true
				w.dirtyMask |= DirtyText
				w.mu.Unlock()
			}
			return
		}

		switch ffi.Keycode(e.KeyCode) {
		case ffi.KeyUp:
			newIndex := currentIndex - 1
			if newIndex < 0 {
				newIndex = optCount - 1
			}
			// Skip disabled options
			for i := 0; i < optCount; i++ {
				w.mu.RLock()
				disabled := w.selectOptions[newIndex].Disabled
				w.mu.RUnlock()
				if !disabled {
					break
				}
				newIndex--
				if newIndex < 0 {
					newIndex = optCount - 1
				}
			}
			w.mu.Lock()
			w.selectIndex = newIndex
			w.dirtyMask |= DirtyText
			w.mu.Unlock()

		case ffi.KeyDown:
			newIndex := currentIndex + 1
			if newIndex >= optCount {
				newIndex = 0
			}
			// Skip disabled options
			for i := 0; i < optCount; i++ {
				w.mu.RLock()
				disabled := w.selectOptions[newIndex].Disabled
				w.mu.RUnlock()
				if !disabled {
					break
				}
				newIndex++
				if newIndex >= optCount {
					newIndex = 0
				}
			}
			w.mu.Lock()
			w.selectIndex = newIndex
			w.dirtyMask |= DirtyText
			w.mu.Unlock()

		case ffi.KeyEnter: // Confirm selection
			w.mu.Lock()
			w.selectOpen = false
			callback := w.onChangeValue
			var value any
			if w.selectIndex >= 0 && w.selectIndex < len(w.selectOptions) {
				value = w.selectOptions[w.selectIndex].Value
			}
			w.dirtyMask |= DirtyText
			w.mu.Unlock()

			if callback != nil {
				callback(value)
			}

		case ffi.KeyEscape: // Close without selecting
			w.mu.Lock()
			w.selectOpen = false
			w.dirtyMask |= DirtyText
			w.mu.Unlock()
		}
	})
}

// SetSelectOptions sets the available options for the select.
func (w *Widget) SetSelectOptions(options []SelectOption) *Widget {
	w.mu.Lock()
	w.selectOptions = options
	w.dirtyMask |= DirtyText
	w.mu.Unlock()
	return w
}

// SelectOptions returns the available options.
func (w *Widget) SelectOptions() []SelectOption {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.selectOptions
}

// SelectedIndex returns the currently selected option index (-1 if none).
func (w *Widget) SelectedIndex() int {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.selectIndex
}

// SetSelectedIndex sets the selected option by index.
func (w *Widget) SetSelectedIndex(index int) *Widget {
	w.mu.Lock()
	if index >= -1 && index < len(w.selectOptions) {
		w.selectIndex = index
		w.dirtyMask |= DirtyText
	}
	w.mu.Unlock()
	return w
}

// SelectedValue returns the value of the currently selected option, or nil.
func (w *Widget) SelectedValue() any {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.selectIndex >= 0 && w.selectIndex < len(w.selectOptions) {
		return w.selectOptions[w.selectIndex].Value
	}
	return nil
}

// SelectedLabel returns the label of the currently selected option, or empty string.
func (w *Widget) SelectedLabel() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.selectIndex >= 0 && w.selectIndex < len(w.selectOptions) {
		return w.selectOptions[w.selectIndex].Label
	}
	return ""
}

// IsSelectOpen returns whether the dropdown is currently open.
func (w *Widget) IsSelectOpen() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.selectOpen
}

// ============================================================================
// Window Control Functions
// ============================================================================

// WindowMinimize minimizes the window.
// Safe to call from any goroutine.
func WindowMinimize() {
	ffi.WindowMinimize()
}

// WindowToggleMaximize toggles the maximize state of the window.
// If maximized, it will restore to previous size. If not maximized, it will maximize.
// Safe to call from any goroutine.
func WindowToggleMaximize() {
	ffi.WindowToggleMaximize()
}

// WindowEnterFullscreen enters borderless fullscreen mode on the primary monitor.
// Safe to call from any goroutine.
func WindowEnterFullscreen() {
	ffi.WindowEnterFullscreen()
}

// WindowExitFullscreen exits fullscreen mode.
// Safe to call from any goroutine.
func WindowExitFullscreen() {
	ffi.WindowExitFullscreen()
}

// WindowToggleFullscreen toggles fullscreen mode.
// If in fullscreen, exits fullscreen. If not in fullscreen, enters fullscreen.
// Safe to call from any goroutine.
func WindowToggleFullscreen() {
	ffi.WindowToggleFullscreen()
}

// WindowClose requests the window to close, triggering a clean shutdown.
// Safe to call from any goroutine.
func WindowClose() {
	ffi.WindowClose()
}

// WindowSetTitle sets the window title.
// Safe to call from any goroutine.
func WindowSetTitle(title string) {
	ffi.WindowSetTitle(title)
}

// SafeAreaInsets represents the insets (in logical pixels) for areas that should
// avoid system UI elements like the notch, status bar, and home indicator on iOS,
// or the navigation bar and status bar on Android.
// On desktop platforms, all values are 0.
type SafeAreaInsets = ffi.SafeAreaInsets

// GetSafeAreaInsets returns the current safe area insets.
// This is primarily useful on iOS and Android where system UI elements
// occupy screen space that content should avoid.
//
// For games or apps that want full-screen edge-to-edge rendering,
// the engine renders into unsafe areas by default. Use these insets
// to position UI elements (like buttons, text) within the safe area.
//
// Example usage:
//
//	insets := retained.GetSafeAreaInsets()
//	// Add padding to the top container to avoid the notch
//	container.Class(fmt.Sprintf("pt-[%fpx]", insets.Top))
//
// On desktop platforms, this returns (0, 0, 0, 0).
func GetSafeAreaInsets() SafeAreaInsets {
	return ffi.GetSafeAreaInsets()
}

