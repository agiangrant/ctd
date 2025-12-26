package ctd

import (
	"errors"
	"regexp"
	"sync"
)

// ============================================================================
// FormControl Interface - For Custom Widget Support
// ============================================================================

// FormControl is implemented by widgets that can participate in forms.
// Built-in widgets (TextField, Checkbox, etc.) implement this automatically.
// Custom widgets can implement this interface to work with Form.
type FormControl interface {
	// FormValue returns the current value of the control
	FormValue() any
	// SetFormValue sets the value programmatically
	SetFormValue(value any)
	// OnFormChange registers a callback for value changes
	OnFormChange(callback func(value any))
	// FormReset resets to default/zero value
	FormReset()
}

// ============================================================================
// Form - Field Registry and Value Management
// ============================================================================

// Form manages a collection of form fields, their values, and validation.
// Fields register themselves via FormField() and can be placed anywhere in the widget tree.
type Form struct {
	name string

	mu          sync.RWMutex
	fields      map[string]*formField // name -> field info
	fieldOrder  []string              // registration order for tab navigation
	fieldErrors map[string]error      // validation errors per field
	onSubmit    func(values map[string]any, valid bool)
}

// formField tracks a registered field
type formField struct {
	control    FormControl
	widget     *Widget // For tab navigation (may be nil for non-Widget controls)
	validators []Validator
	value      any
}

// NewForm creates a new form with the given name.
func NewForm(name string) *Form {
	return &Form{
		name:        name,
		fields:      make(map[string]*formField),
		fieldOrder:  make([]string, 0),
		fieldErrors: make(map[string]error),
	}
}

// Name returns the form's name.
func (f *Form) Name() string {
	return f.name
}

// ============================================================================
// Field Registration
// ============================================================================

// RegisterField registers a FormControl with the form.
// This is the generic method for any FormControl implementation.
// For Widgets, use Widget.FormField() which calls this internally.
func (f *Form) RegisterField(name string, control FormControl, validators ...Validator) {
	f.mu.Lock()
	defer f.mu.Unlock()

	// Get initial value
	value := control.FormValue()

	// Try to get Widget if this is one (for tab navigation)
	var widget *Widget
	if w, ok := control.(*Widget); ok {
		widget = w
	}

	// Check if already registered (replace if so, but keep order position)
	if existing, ok := f.fields[name]; ok {
		existing.control = control
		existing.widget = widget
		existing.validators = validators
		existing.value = value
	} else {
		f.fields[name] = &formField{
			control:    control,
			widget:     widget,
			validators: validators,
			value:      value,
		}
		f.fieldOrder = append(f.fieldOrder, name)
	}

	// Wire up onChange to track value changes
	control.OnFormChange(func(newValue any) {
		f.mu.Lock()
		if field, ok := f.fields[name]; ok {
			field.value = newValue
		}
		// Clear error when value changes (will re-validate on submit)
		delete(f.fieldErrors, name)
		f.mu.Unlock()
	})
}

// ============================================================================
// Value Access
// ============================================================================

// Value returns the current value for a field.
func (f *Form) Value(name string) any {
	f.mu.RLock()
	defer f.mu.RUnlock()

	if field, ok := f.fields[name]; ok {
		return field.value
	}
	return nil
}

// Values returns all field values as a map.
func (f *Form) Values() map[string]any {
	f.mu.RLock()
	defer f.mu.RUnlock()

	values := make(map[string]any, len(f.fields))
	for name, field := range f.fields {
		values[name] = field.value
	}
	return values
}

// SetValue programmatically sets a field's value.
func (f *Form) SetValue(name string, value any) {
	f.mu.RLock()
	field, ok := f.fields[name]
	f.mu.RUnlock()

	if !ok {
		return
	}

	// Update via the control interface
	field.control.SetFormValue(value)

	// Update tracked value
	f.mu.Lock()
	field.value = value
	delete(f.fieldErrors, name)
	f.mu.Unlock()
}

// ============================================================================
// Validation
// ============================================================================

// Validator is a function that validates a field value.
// Returns nil if valid, or an error describing the validation failure.
type Validator func(value any) error

// FieldError returns the validation error for a field, or nil if valid.
func (f *Form) FieldError(name string) error {
	f.mu.RLock()
	defer f.mu.RUnlock()
	return f.fieldErrors[name]
}

// Errors returns all current validation errors.
func (f *Form) Errors() map[string]error {
	f.mu.RLock()
	defer f.mu.RUnlock()

	errs := make(map[string]error, len(f.fieldErrors))
	for name, err := range f.fieldErrors {
		errs[name] = err
	}
	return errs
}

// Validate runs all validators on all fields.
// Returns true if all fields are valid.
func (f *Form) Validate() bool {
	f.mu.Lock()
	defer f.mu.Unlock()

	// Clear previous errors
	f.fieldErrors = make(map[string]error)

	allValid := true
	for name, field := range f.fields {
		for _, validator := range field.validators {
			if err := validator(field.value); err != nil {
				f.fieldErrors[name] = err
				allValid = false
				break // Stop at first error for this field
			}
		}
	}

	return allValid
}

// ValidateField runs validators for a single field.
// Returns true if the field is valid.
func (f *Form) ValidateField(name string) bool {
	f.mu.Lock()
	defer f.mu.Unlock()

	field, ok := f.fields[name]
	if !ok {
		return true
	}

	delete(f.fieldErrors, name)

	for _, validator := range field.validators {
		if err := validator(field.value); err != nil {
			f.fieldErrors[name] = err
			return false
		}
	}

	return true
}

// ============================================================================
// Submit and Reset
// ============================================================================

// OnSubmit sets the callback invoked when Submit() is called.
func (f *Form) OnSubmit(callback func(values map[string]any, valid bool)) *Form {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.onSubmit = callback
	return f
}

// Submit validates the form and invokes the OnSubmit callback.
func (f *Form) Submit() {
	valid := f.Validate()

	f.mu.RLock()
	callback := f.onSubmit
	f.mu.RUnlock()

	if callback != nil {
		callback(f.Values(), valid)
	}
}

// Reset clears all field values to their zero/default values.
func (f *Form) Reset() {
	f.mu.Lock()
	controls := make([]FormControl, 0, len(f.fields))
	names := make([]string, 0, len(f.fields))
	for name, field := range f.fields {
		controls = append(controls, field.control)
		names = append(names, name)
	}
	f.fieldErrors = make(map[string]error)
	f.mu.Unlock()

	// Reset each field via interface
	for i, control := range controls {
		control.FormReset()

		// Update tracked value
		f.mu.Lock()
		if field, ok := f.fields[names[i]]; ok {
			field.value = control.FormValue()
		}
		f.mu.Unlock()
	}
}

// ============================================================================
// Tab Navigation
// ============================================================================

// Fields returns the ordered list of field names (registration order).
func (f *Form) Fields() []string {
	f.mu.RLock()
	defer f.mu.RUnlock()

	result := make([]string, len(f.fieldOrder))
	copy(result, f.fieldOrder)
	return result
}

// FieldWidget returns the widget for a field name (may be nil for non-Widget controls).
func (f *Form) FieldWidget(name string) *Widget {
	f.mu.RLock()
	defer f.mu.RUnlock()

	if field, ok := f.fields[name]; ok {
		return field.widget
	}
	return nil
}

// FieldControl returns the FormControl for a field name.
func (f *Form) FieldControl(name string) FormControl {
	f.mu.RLock()
	defer f.mu.RUnlock()

	if field, ok := f.fields[name]; ok {
		return field.control
	}
	return nil
}

// NextField returns the next field name after the given one (wraps around).
func (f *Form) NextField(current string) string {
	f.mu.RLock()
	defer f.mu.RUnlock()

	for i, name := range f.fieldOrder {
		if name == current {
			nextIdx := (i + 1) % len(f.fieldOrder)
			return f.fieldOrder[nextIdx]
		}
	}

	// Not found, return first field
	if len(f.fieldOrder) > 0 {
		return f.fieldOrder[0]
	}
	return ""
}

// PrevField returns the previous field name before the given one (wraps around).
func (f *Form) PrevField(current string) string {
	f.mu.RLock()
	defer f.mu.RUnlock()

	for i, name := range f.fieldOrder {
		if name == current {
			prevIdx := (i - 1 + len(f.fieldOrder)) % len(f.fieldOrder)
			return f.fieldOrder[prevIdx]
		}
	}

	// Not found, return last field
	if len(f.fieldOrder) > 0 {
		return f.fieldOrder[len(f.fieldOrder)-1]
	}
	return ""
}

// ============================================================================
// Tab Navigation Helpers
// ============================================================================

// NextFieldWidget returns the next field's widget given the current focused widget.
// Returns nil if not found or if next field has no widget.
func (f *Form) NextFieldWidget(current *Widget) *Widget {
	currentName := f.fieldNameForWidget(current)
	if currentName == "" {
		// Current widget not in form, return first field widget
		return f.FirstFieldWidget()
	}

	nextName := f.NextField(currentName)
	return f.FieldWidget(nextName)
}

// PrevFieldWidget returns the previous field's widget given the current focused widget.
// Returns nil if not found or if previous field has no widget.
func (f *Form) PrevFieldWidget(current *Widget) *Widget {
	currentName := f.fieldNameForWidget(current)
	if currentName == "" {
		// Current widget not in form, return last field widget
		return f.LastFieldWidget()
	}

	prevName := f.PrevField(currentName)
	return f.FieldWidget(prevName)
}

// FirstFieldWidget returns the first field's widget.
func (f *Form) FirstFieldWidget() *Widget {
	f.mu.RLock()
	defer f.mu.RUnlock()

	if len(f.fieldOrder) == 0 {
		return nil
	}

	if field, ok := f.fields[f.fieldOrder[0]]; ok {
		return field.widget
	}
	return nil
}

// LastFieldWidget returns the last field's widget.
func (f *Form) LastFieldWidget() *Widget {
	f.mu.RLock()
	defer f.mu.RUnlock()

	if len(f.fieldOrder) == 0 {
		return nil
	}

	last := f.fieldOrder[len(f.fieldOrder)-1]
	if field, ok := f.fields[last]; ok {
		return field.widget
	}
	return nil
}

// fieldNameForWidget finds the field name for a given widget.
// Returns empty string if not found.
func (f *Form) fieldNameForWidget(w *Widget) string {
	if w == nil {
		return ""
	}

	f.mu.RLock()
	defer f.mu.RUnlock()

	for name, field := range f.fields {
		if field.widget == w {
			return name
		}
	}
	return ""
}

// ContainsWidget returns true if the widget is registered with this form.
func (f *Form) ContainsWidget(w *Widget) bool {
	return f.fieldNameForWidget(w) != ""
}

// HandleTabKey processes a Tab key event for form navigation.
// Returns the widget that should receive focus, or nil if not handled.
// Pass shift=true for Shift+Tab (backward navigation).
func (f *Form) HandleTabKey(currentFocused *Widget, shift bool) *Widget {
	if currentFocused == nil {
		// No current focus, start at first or last field
		if shift {
			return f.LastFieldWidget()
		}
		return f.FirstFieldWidget()
	}

	// Check if current widget is in this form
	if !f.ContainsWidget(currentFocused) {
		return nil
	}

	// Navigate to next/prev field
	if shift {
		return f.PrevFieldWidget(currentFocused)
	}
	return f.NextFieldWidget(currentFocused)
}

// ============================================================================
// Common Validators
// ============================================================================

// Required returns a validator that checks if a value is non-empty.
func Required(message ...string) Validator {
	msg := "This field is required"
	if len(message) > 0 {
		msg = message[0]
	}

	return func(value any) error {
		if value == nil {
			return errors.New(msg)
		}

		switch v := value.(type) {
		case string:
			if v == "" {
				return errors.New(msg)
			}
		case bool:
			// Booleans are always "present"
			return nil
		case int, int32, int64, float32, float64:
			// Numbers are always "present"
			return nil
		}

		return nil
	}
}

// MinLength returns a validator that checks string minimum length.
func MinLength(min int, message ...string) Validator {
	msg := "Must be at least %d characters"
	if len(message) > 0 {
		msg = message[0]
	}

	return func(value any) error {
		if s, ok := value.(string); ok {
			if len(s) < min {
				return errors.New(msg)
			}
		}
		return nil
	}
}

// MaxLength returns a validator that checks string maximum length.
func MaxLength(max int, message ...string) Validator {
	msg := "Must be at most %d characters"
	if len(message) > 0 {
		msg = message[0]
	}

	return func(value any) error {
		if s, ok := value.(string); ok {
			if len(s) > max {
				return errors.New(msg)
			}
		}
		return nil
	}
}

// Email returns a validator that checks for valid email format.
func Email(message ...string) Validator {
	msg := "Invalid email address"
	if len(message) > 0 {
		msg = message[0]
	}

	// Simple email regex - not RFC 5322 compliant but good enough for most cases
	emailRegex := regexp.MustCompile(`^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$`)

	return func(value any) error {
		if s, ok := value.(string); ok {
			if s != "" && !emailRegex.MatchString(s) {
				return errors.New(msg)
			}
		}
		return nil
	}
}

// Pattern returns a validator that checks against a regex pattern.
func Pattern(pattern string, message ...string) Validator {
	msg := "Invalid format"
	if len(message) > 0 {
		msg = message[0]
	}

	re := regexp.MustCompile(pattern)

	return func(value any) error {
		if s, ok := value.(string); ok {
			if s != "" && !re.MatchString(s) {
				return errors.New(msg)
			}
		}
		return nil
	}
}

// Min returns a validator that checks numeric minimum value.
func Min(min float64, message ...string) Validator {
	msg := "Must be at least %v"
	if len(message) > 0 {
		msg = message[0]
	}

	return func(value any) error {
		var v float64
		switch val := value.(type) {
		case float32:
			v = float64(val)
		case float64:
			v = val
		case int:
			v = float64(val)
		case int32:
			v = float64(val)
		case int64:
			v = float64(val)
		default:
			return nil
		}

		if v < min {
			return errors.New(msg)
		}
		return nil
	}
}

// Max returns a validator that checks numeric maximum value.
func Max(max float64, message ...string) Validator {
	msg := "Must be at most %v"
	if len(message) > 0 {
		msg = message[0]
	}

	return func(value any) error {
		var v float64
		switch val := value.(type) {
		case float32:
			v = float64(val)
		case float64:
			v = val
		case int:
			v = float64(val)
		case int32:
			v = float64(val)
		case int64:
			v = float64(val)
		default:
			return nil
		}

		if v > max {
			return errors.New(msg)
		}
		return nil
	}
}

// CustomValidator returns a validator using a custom validation function.
func CustomValidator(fn func(value any) error) Validator {
	return fn
}
