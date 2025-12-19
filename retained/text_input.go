package retained

import (
	"strings"
	"sync"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/agiangrant/centered/internal/ffi"
)

// UndoState captures a snapshot of text buffer state for undo/redo.
type UndoState struct {
	content         []rune
	cursor          int
	selectionAnchor int
}

// TextBuffer manages editable text content with cursor and selection.
// This is the core editing engine used by TextField and TextArea.
// It's designed to be extensible for rich text editors.
type TextBuffer struct {
	mu sync.RWMutex

	// Content
	content []rune // Using runes for proper Unicode handling

	// Cursor position (index into content, 0 = before first char)
	cursor int

	// Selection (anchor is where selection started, cursor is where it ends)
	// If anchor == cursor, no selection
	selectionAnchor int

	// Cursor blink state
	cursorVisible   bool      // Current blink state
	lastBlinkToggle time.Time // Last time blink state changed
	blinkInterval   time.Duration

	// Configuration
	multiline   bool // Allow newlines
	maxLength   int  // 0 = no limit
	placeholder string

	// Password mode
	password     bool   // Mask characters with bullet
	passwordChar rune   // Character to show (default '•')

	// Read-only mode
	readOnly bool // Prevent editing but allow selection/copy

	// Placeholder styling
	placeholderColor uint32 // Color for placeholder text (0 = use default gray)

	// Validation and filtering
	validator    func(text string) bool        // Returns true if text is valid
	charFilter   func(r rune) bool             // Returns true if char is allowed
	inputPattern string                         // Regex pattern for validation (optional)

	// Undo/Redo stacks
	undoStack []UndoState
	redoStack []UndoState
	maxUndo   int // Maximum undo levels (0 = default 100)

	// Callbacks
	onChange func(text string)
}

// NewTextBuffer creates a new text buffer.
func NewTextBuffer() *TextBuffer {
	return &TextBuffer{
		content:         make([]rune, 0, 64),
		cursorVisible:   true,
		lastBlinkToggle: time.Now(),
		blinkInterval:   530 * time.Millisecond, // Standard cursor blink rate
	}
}

// Text returns the current text content.
func (b *TextBuffer) Text() string {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return string(b.content)
}

// SetText replaces all text content.
func (b *TextBuffer) SetText(text string) {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.content = []rune(text)
	// Clamp cursor to valid range
	if b.cursor > len(b.content) {
		b.cursor = len(b.content)
	}
	if b.selectionAnchor > len(b.content) {
		b.selectionAnchor = len(b.content)
	}
}

// Length returns the number of characters (runes).
func (b *TextBuffer) Length() int {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return len(b.content)
}

// Cursor returns the current cursor position.
func (b *TextBuffer) Cursor() int {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.cursor
}

// SetCursor moves the cursor to a position, clearing selection.
func (b *TextBuffer) SetCursor(pos int) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.cursor = b.clampPosition(pos)
	b.selectionAnchor = b.cursor
}

// Selection returns the selection range (start, end) where start <= end.
// Returns (cursor, cursor) if no selection.
func (b *TextBuffer) Selection() (int, int) {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.selectionRange()
}

// selectionRange returns ordered selection bounds (must hold lock).
func (b *TextBuffer) selectionRange() (int, int) {
	if b.selectionAnchor < b.cursor {
		return b.selectionAnchor, b.cursor
	}
	return b.cursor, b.selectionAnchor
}

// HasSelection returns true if text is selected.
func (b *TextBuffer) HasSelection() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.selectionAnchor != b.cursor
}

// SelectedText returns the currently selected text.
func (b *TextBuffer) SelectedText() string {
	b.mu.RLock()
	defer b.mu.RUnlock()
	start, end := b.selectionRange()
	if start == end {
		return ""
	}
	return string(b.content[start:end])
}

// SelectAll selects all text.
func (b *TextBuffer) SelectAll() {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.selectionAnchor = 0
	b.cursor = len(b.content)
}

// SelectLine selects the line containing the given position.
// For multiline text, selects from line start to line end (including newline if present).
// For single-line text, selects all.
func (b *TextBuffer) SelectLine(pos int) {
	b.mu.Lock()
	defer b.mu.Unlock()

	if !b.multiline {
		// Single line - select all
		b.selectionAnchor = 0
		b.cursor = len(b.content)
		return
	}

	// Find line start (search backwards for newline or start)
	lineStart := pos
	for lineStart > 0 && b.content[lineStart-1] != '\n' {
		lineStart--
	}

	// Find line end (search forwards for newline or end)
	lineEnd := pos
	for lineEnd < len(b.content) && b.content[lineEnd] != '\n' {
		lineEnd++
	}
	// Include the newline character if present
	if lineEnd < len(b.content) && b.content[lineEnd] == '\n' {
		lineEnd++
	}

	b.selectionAnchor = lineStart
	b.cursor = lineEnd
}

// ClearSelection clears the selection without moving cursor.
func (b *TextBuffer) ClearSelection() {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.selectionAnchor = b.cursor
}

// SetSelection sets both cursor and anchor for selection.
func (b *TextBuffer) SetSelection(anchor, cursor int) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.selectionAnchor = b.clampPosition(anchor)
	b.cursor = b.clampPosition(cursor)
}

// CursorVisible returns whether the cursor should be shown (for blink animation).
func (b *TextBuffer) CursorVisible() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.cursorVisible
}

// UpdateBlink updates the cursor blink state based on elapsed time.
// Returns true if the cursor visibility changed (requires redraw).
func (b *TextBuffer) UpdateBlink() bool {
	b.mu.Lock()
	defer b.mu.Unlock()

	elapsed := time.Since(b.lastBlinkToggle)
	if elapsed >= b.blinkInterval {
		b.cursorVisible = !b.cursorVisible
		b.lastBlinkToggle = time.Now()
		return true
	}
	return false
}

// ResetBlink makes the cursor visible and resets the blink timer.
// Call this when the user types or moves the cursor.
func (b *TextBuffer) ResetBlink() {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.cursorVisible = true
	b.lastBlinkToggle = time.Now()
}

// TimeUntilNextBlink returns the duration until the cursor should toggle.
// Returns 0 if the toggle time has passed.
func (b *TextBuffer) TimeUntilNextBlink() time.Duration {
	b.mu.RLock()
	defer b.mu.RUnlock()

	elapsed := time.Since(b.lastBlinkToggle)
	if elapsed >= b.blinkInterval {
		return 0
	}
	return b.blinkInterval - elapsed
}

// Insert inserts text at the cursor position.
// If there's a selection, it replaces the selected text.
// Respects read-only mode and character filtering.
func (b *TextBuffer) Insert(text string) {
	b.mu.Lock()
	defer b.mu.Unlock()

	// Check read-only mode
	if b.readOnly {
		return
	}

	// Filter newlines if not multiline
	if !b.multiline {
		text = strings.ReplaceAll(text, "\n", "")
		text = strings.ReplaceAll(text, "\r", "")
	}

	runes := []rune(text)

	// Apply character filter if set
	if b.charFilter != nil {
		filtered := make([]rune, 0, len(runes))
		for _, r := range runes {
			if b.charFilter(r) {
				filtered = append(filtered, r)
			}
		}
		runes = filtered
	}

	// Check max length
	if b.maxLength > 0 {
		start, end := b.selectionRange()
		currentLen := len(b.content) - (end - start)
		available := b.maxLength - currentLen
		if available < 0 {
			available = 0
		}
		if len(runes) > available {
			runes = runes[:available]
		}
	}

	if len(runes) == 0 && b.selectionAnchor == b.cursor {
		return
	}

	// Save state for undo
	b.saveUndoState()

	// Delete selection first if any
	start, end := b.selectionRange()
	if start != end {
		b.content = append(b.content[:start], b.content[end:]...)
		b.cursor = start
		b.selectionAnchor = start
	}

	// Insert new text
	newContent := make([]rune, 0, len(b.content)+len(runes))
	newContent = append(newContent, b.content[:b.cursor]...)
	newContent = append(newContent, runes...)
	newContent = append(newContent, b.content[b.cursor:]...)
	b.content = newContent

	b.cursor += len(runes)
	b.selectionAnchor = b.cursor

	b.notifyChange()
}

// Delete removes characters. count > 0 deletes forward, count < 0 deletes backward.
// If there's a selection, it deletes the selection regardless of count.
// Respects read-only mode.
func (b *TextBuffer) Delete(count int) {
	b.mu.Lock()
	defer b.mu.Unlock()

	// Check read-only mode
	if b.readOnly {
		return
	}

	start, end := b.selectionRange()
	if start != end {
		// Save state for undo
		b.saveUndoState()
		// Delete selection
		b.content = append(b.content[:start], b.content[end:]...)
		b.cursor = start
		b.selectionAnchor = start
		b.notifyChange()
		return
	}

	if count == 0 {
		return
	}

	// Save state for undo
	b.saveUndoState()

	if count > 0 {
		// Delete forward
		delEnd := b.cursor + count
		if delEnd > len(b.content) {
			delEnd = len(b.content)
		}
		if b.cursor < delEnd {
			b.content = append(b.content[:b.cursor], b.content[delEnd:]...)
			b.notifyChange()
		}
	} else {
		// Delete backward
		delStart := b.cursor + count
		if delStart < 0 {
			delStart = 0
		}
		if delStart < b.cursor {
			b.content = append(b.content[:delStart], b.content[b.cursor:]...)
			b.cursor = delStart
			b.selectionAnchor = delStart
			b.notifyChange()
		}
	}
}

// DeleteWord deletes a word. forward=true deletes next word, false deletes previous word.
// Respects read-only mode.
func (b *TextBuffer) DeleteWord(forward bool) {
	b.mu.Lock()
	defer b.mu.Unlock()

	// Check read-only mode
	if b.readOnly {
		return
	}

	// If there's a selection, just delete it
	start, end := b.selectionRange()
	if start != end {
		// Save state for undo
		b.saveUndoState()
		b.content = append(b.content[:start], b.content[end:]...)
		b.cursor = start
		b.selectionAnchor = start
		b.notifyChange()
		return
	}

	// Save state for undo
	b.saveUndoState()

	if forward {
		wordEnd := b.findWordEnd(b.cursor)
		if wordEnd > b.cursor {
			b.content = append(b.content[:b.cursor], b.content[wordEnd:]...)
			b.notifyChange()
		}
	} else {
		wordStart := b.findWordStart(b.cursor)
		if wordStart < b.cursor {
			b.content = append(b.content[:wordStart], b.content[b.cursor:]...)
			b.cursor = wordStart
			b.selectionAnchor = wordStart
			b.notifyChange()
		}
	}
}

// MoveCursor moves the cursor by delta characters.
// If extend is true, extends selection; otherwise clears it.
func (b *TextBuffer) MoveCursor(delta int, extend bool) {
	b.mu.Lock()
	defer b.mu.Unlock()

	if !extend && b.selectionAnchor != b.cursor {
		// Collapse selection to the appropriate end
		if delta < 0 {
			start, _ := b.selectionRange()
			b.cursor = start
		} else {
			_, end := b.selectionRange()
			b.cursor = end
		}
		b.selectionAnchor = b.cursor
		return
	}

	newPos := b.clampPosition(b.cursor + delta)
	b.cursor = newPos
	if !extend {
		b.selectionAnchor = b.cursor
	}
}

// MoveToLineStart moves cursor to beginning of current line.
func (b *TextBuffer) MoveToLineStart(extend bool) {
	b.mu.Lock()
	defer b.mu.Unlock()

	lineStart := b.findLineStart(b.cursor)
	b.cursor = lineStart
	if !extend {
		b.selectionAnchor = b.cursor
	}
}

// MoveToLineEnd moves cursor to end of current line.
func (b *TextBuffer) MoveToLineEnd(extend bool) {
	b.mu.Lock()
	defer b.mu.Unlock()

	lineEnd := b.findLineEnd(b.cursor)
	b.cursor = lineEnd
	if !extend {
		b.selectionAnchor = b.cursor
	}
}

// MoveWord moves cursor by one word.
func (b *TextBuffer) MoveWord(forward bool, extend bool) {
	b.mu.Lock()
	defer b.mu.Unlock()

	if forward {
		b.cursor = b.findWordEnd(b.cursor)
	} else {
		b.cursor = b.findWordStart(b.cursor)
	}
	if !extend {
		b.selectionAnchor = b.cursor
	}
}

// MoveToStart moves cursor to the beginning.
func (b *TextBuffer) MoveToStart(extend bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.cursor = 0
	if !extend {
		b.selectionAnchor = 0
	}
}

// MoveToEnd moves cursor to the end.
func (b *TextBuffer) MoveToEnd(extend bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.cursor = len(b.content)
	if !extend {
		b.selectionAnchor = len(b.content)
	}
}

// SelectWordAt selects the word at the given position (for double-click).
func (b *TextBuffer) SelectWordAt(pos int) {
	b.mu.Lock()
	defer b.mu.Unlock()

	pos = b.clampPosition(pos)
	b.selectionAnchor = b.findWordStart(pos)
	b.cursor = b.findWordEnd(pos)
}

// SetMultiline enables or disables multiline mode.
func (b *TextBuffer) SetMultiline(multiline bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.multiline = multiline
}

// SetMaxLength sets the maximum number of characters (0 = no limit).
func (b *TextBuffer) SetMaxLength(max int) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.maxLength = max
}

// SetPlaceholder sets placeholder text shown when empty.
func (b *TextBuffer) SetPlaceholder(placeholder string) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.placeholder = placeholder
}

// Placeholder returns the placeholder text.
func (b *TextBuffer) Placeholder() string {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.placeholder
}

// SetPassword enables or disables password masking.
func (b *TextBuffer) SetPassword(enabled bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.password = enabled
	if enabled && b.passwordChar == 0 {
		b.passwordChar = '•' // Default bullet character
	}
}

// IsPassword returns whether password mode is enabled.
func (b *TextBuffer) IsPassword() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.password
}

// SetPasswordChar sets the character used to mask password text.
func (b *TextBuffer) SetPasswordChar(char rune) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.passwordChar = char
}

// DisplayText returns the text to display, masking characters if in password mode.
func (b *TextBuffer) DisplayText() string {
	b.mu.RLock()
	defer b.mu.RUnlock()

	if !b.password {
		return string(b.content)
	}

	// Mask all characters with the password character
	maskChar := b.passwordChar
	if maskChar == 0 {
		maskChar = '•'
	}

	masked := make([]rune, len(b.content))
	for i := range masked {
		masked[i] = maskChar
	}
	return string(masked)
}

// SetReadOnly enables or disables read-only mode.
func (b *TextBuffer) SetReadOnly(readOnly bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.readOnly = readOnly
}

// IsReadOnly returns whether read-only mode is enabled.
func (b *TextBuffer) IsReadOnly() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.readOnly
}

// SetPlaceholderColor sets the color for placeholder text.
// Use 0 for default gray.
func (b *TextBuffer) SetPlaceholderColor(color uint32) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.placeholderColor = color
}

// PlaceholderColor returns the placeholder text color.
func (b *TextBuffer) PlaceholderColor() uint32 {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.placeholderColor
}

// SetValidator sets a validation function that checks if text is valid.
// The function receives the full text and returns true if valid.
func (b *TextBuffer) SetValidator(fn func(text string) bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.validator = fn
}

// IsValid returns whether the current text passes validation.
// Returns true if no validator is set.
func (b *TextBuffer) IsValid() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	if b.validator == nil {
		return true
	}
	return b.validator(string(b.content))
}

// SetCharFilter sets a filter function for input characters.
// The function receives a rune and returns true if it should be allowed.
func (b *TextBuffer) SetCharFilter(fn func(r rune) bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.charFilter = fn
}

// OnChange sets a callback for text changes.
func (b *TextBuffer) OnChange(fn func(text string)) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.onChange = fn
}

// saveUndoState saves the current state to the undo stack.
// Must be called before content-modifying operations.
// Caller must hold lock.
func (b *TextBuffer) saveUndoState() {
	maxUndo := b.maxUndo
	if maxUndo == 0 {
		maxUndo = 100 // Default
	}

	// Save current state
	state := UndoState{
		content:         make([]rune, len(b.content)),
		cursor:          b.cursor,
		selectionAnchor: b.selectionAnchor,
	}
	copy(state.content, b.content)

	b.undoStack = append(b.undoStack, state)

	// Trim if exceeds max
	if len(b.undoStack) > maxUndo {
		b.undoStack = b.undoStack[1:]
	}

	// Clear redo stack on new change
	b.redoStack = nil
}

// Undo reverts to the previous state.
func (b *TextBuffer) Undo() bool {
	b.mu.Lock()
	defer b.mu.Unlock()

	if len(b.undoStack) == 0 {
		return false
	}

	// Save current state to redo stack
	currentState := UndoState{
		content:         make([]rune, len(b.content)),
		cursor:          b.cursor,
		selectionAnchor: b.selectionAnchor,
	}
	copy(currentState.content, b.content)
	b.redoStack = append(b.redoStack, currentState)

	// Pop and restore from undo stack
	state := b.undoStack[len(b.undoStack)-1]
	b.undoStack = b.undoStack[:len(b.undoStack)-1]

	b.content = make([]rune, len(state.content))
	copy(b.content, state.content)
	b.cursor = state.cursor
	b.selectionAnchor = state.selectionAnchor

	b.notifyChange()
	return true
}

// Redo restores the previously undone state.
func (b *TextBuffer) Redo() bool {
	b.mu.Lock()
	defer b.mu.Unlock()

	if len(b.redoStack) == 0 {
		return false
	}

	// Save current state to undo stack
	currentState := UndoState{
		content:         make([]rune, len(b.content)),
		cursor:          b.cursor,
		selectionAnchor: b.selectionAnchor,
	}
	copy(currentState.content, b.content)
	b.undoStack = append(b.undoStack, currentState)

	// Pop and restore from redo stack
	state := b.redoStack[len(b.redoStack)-1]
	b.redoStack = b.redoStack[:len(b.redoStack)-1]

	b.content = make([]rune, len(state.content))
	copy(b.content, state.content)
	b.cursor = state.cursor
	b.selectionAnchor = state.selectionAnchor

	b.notifyChange()
	return true
}

// Helper methods (must hold lock)

func (b *TextBuffer) clampPosition(pos int) int {
	if pos < 0 {
		return 0
	}
	if pos > len(b.content) {
		return len(b.content)
	}
	return pos
}

func (b *TextBuffer) notifyChange() {
	if b.onChange != nil {
		// Call outside lock to avoid deadlocks
		text := string(b.content)
		go b.onChange(text)
	}
}

func (b *TextBuffer) findWordStart(pos int) int {
	if pos <= 0 {
		return 0
	}
	// Skip any whitespace before cursor
	for pos > 0 && unicode.IsSpace(b.content[pos-1]) {
		pos--
	}
	// Find start of word
	for pos > 0 && !unicode.IsSpace(b.content[pos-1]) {
		pos--
	}
	return pos
}

func (b *TextBuffer) findWordEnd(pos int) int {
	length := len(b.content)
	if pos >= length {
		return length
	}
	// Skip any whitespace after cursor
	for pos < length && unicode.IsSpace(b.content[pos]) {
		pos++
	}
	// Find end of word
	for pos < length && !unicode.IsSpace(b.content[pos]) {
		pos++
	}
	return pos
}

func (b *TextBuffer) findLineStart(pos int) int {
	for pos > 0 && b.content[pos-1] != '\n' {
		pos--
	}
	return pos
}

func (b *TextBuffer) findLineEnd(pos int) int {
	length := len(b.content)
	for pos < length && b.content[pos] != '\n' {
		pos++
	}
	return pos
}

// ============================================================================
// TextField/TextArea Setup
// ============================================================================

// InitTextField initializes a widget with text input capabilities.
// Call this after creating a TextField or TextArea widget.
func InitTextField(w *Widget, placeholder string, multiline bool) {
	w.textBuffer = NewTextBuffer()
	w.textBuffer.SetPlaceholder(placeholder)
	w.textBuffer.SetMultiline(multiline)

	// Default styling for text fields
	if w.fontSize == 0 {
		w.fontSize = 14
	}
	if w.padding == [4]float32{} {
		w.padding = [4]float32{8, 12, 8, 12}
	}

	// Set up default input handling
	setupTextFieldHandlers(w)
}

// TextArea creates a multi-line text input widget.
func TextArea(placeholder string, classes string) *Widget {
	w := NewWidget(KindTextArea)
	w.textBuffer = NewTextBuffer()
	w.textBuffer.SetMultiline(true)
	w.textBuffer.SetPlaceholder(placeholder)

	// Default styling for text areas
	w.fontSize = 14
	w.padding = [4]float32{8, 12, 8, 12}

	if classes != "" {
		w.SetClasses(classes)
	}

	// Set up default input handling
	setupTextFieldHandlers(w)

	return w
}

// setupTextFieldHandlers sets up keyboard and mouse handlers for text input.
func setupTextFieldHandlers(w *Widget) {
	// Key down handler for navigation and editing keys
	w.OnKeyDown(func(e *KeyEvent) {
		if w.textBuffer == nil {
			return
		}

		extend := e.Modifiers&ModShift != 0
		wordJump := e.Modifiers&ModAlt != 0   // Option on macOS
		lineJump := e.Modifiers&ModSuper != 0 // Cmd on macOS

		switch ffi.Keycode(e.KeyCode) {
		case ffi.KeyLeft:
			if lineJump {
				w.textBuffer.MoveToLineStart(extend)
			} else if wordJump {
				w.textBuffer.MoveWord(false, extend)
			} else {
				w.textBuffer.MoveCursor(-1, extend)
			}
			w.markDirty(DirtyText)

		case ffi.KeyRight:
			if lineJump {
				w.textBuffer.MoveToLineEnd(extend)
			} else if wordJump {
				w.textBuffer.MoveWord(true, extend)
			} else {
				w.textBuffer.MoveCursor(1, extend)
			}
			w.markDirty(DirtyText)

		case ffi.KeyUp:
			if w.textBuffer.multiline {
				// Move up a line in wrapped text
				contentWidth := w.width - w.padding[1] - w.padding[3]
				text := w.textBuffer.Text()
				currentCursor := w.textBuffer.Cursor()
				newCursor := MoveCursorVertical(text, currentCursor, -1, contentWidth, w.fontSize, "system")
				if extend {
					w.textBuffer.SetSelection(w.textBuffer.selectionAnchor, newCursor)
				} else {
					w.textBuffer.SetCursor(newCursor)
				}
			} else {
				w.textBuffer.MoveToStart(extend)
			}
			w.markDirty(DirtyText)

		case ffi.KeyDown:
			if w.textBuffer.multiline {
				// Move down a line in wrapped text
				contentWidth := w.width - w.padding[1] - w.padding[3]
				text := w.textBuffer.Text()
				currentCursor := w.textBuffer.Cursor()
				newCursor := MoveCursorVertical(text, currentCursor, 1, contentWidth, w.fontSize, "system")
				if extend {
					w.textBuffer.SetSelection(w.textBuffer.selectionAnchor, newCursor)
				} else {
					w.textBuffer.SetCursor(newCursor)
				}
			} else {
				w.textBuffer.MoveToEnd(extend)
			}
			w.markDirty(DirtyText)

		case ffi.KeyHome:
			if lineJump || !w.textBuffer.multiline {
				w.textBuffer.MoveToStart(extend)
			} else {
				w.textBuffer.MoveToLineStart(extend)
			}
			w.markDirty(DirtyText)

		case ffi.KeyEnd:
			if lineJump || !w.textBuffer.multiline {
				w.textBuffer.MoveToEnd(extend)
			} else {
				w.textBuffer.MoveToLineEnd(extend)
			}
			w.markDirty(DirtyText)

		case ffi.KeyBackspace:
			if wordJump {
				w.textBuffer.DeleteWord(false)
			} else {
				w.textBuffer.Delete(-1)
			}
			w.markDirty(DirtyText)

		case ffi.KeyDelete:
			if wordJump {
				w.textBuffer.DeleteWord(true)
			} else {
				w.textBuffer.Delete(1)
			}
			w.markDirty(DirtyText)

		case ffi.KeyEnter:
			if w.textBuffer.multiline {
				w.textBuffer.Insert("\n")
				w.markDirty(DirtyText)
			}
			// For single-line, Enter typically submits - handled by app

		case ffi.KeyTab:
			if w.textBuffer.multiline {
				w.textBuffer.Insert("\t")
				w.markDirty(DirtyText)
			}
			// For single-line, Tab typically moves focus - handled by app

		case ffi.KeyA:
			if e.Modifiers&ModSuper != 0 {
				w.textBuffer.SelectAll()
				w.markDirty(DirtyText)
			}

		case ffi.KeyC:
			if e.Modifiers&ModSuper != 0 {
				// Copy selected text to clipboard
				// Don't allow copy in password mode for security
				if !w.textBuffer.IsPassword() {
					selectedText := w.textBuffer.SelectedText()
					if selectedText != "" {
						ffi.ClipboardSetString(selectedText)
					}
				}
			}

		case ffi.KeyX:
			if e.Modifiers&ModSuper != 0 {
				// Cut - copy then delete
				// Don't allow copy in password mode, but still allow delete
				selectedText := w.textBuffer.SelectedText()
				if selectedText != "" {
					if !w.textBuffer.IsPassword() {
						ffi.ClipboardSetString(selectedText)
					}
					w.textBuffer.Delete(0) // Delete selection
					w.markDirty(DirtyText)
				}
			}

		case ffi.KeyV:
			if e.Modifiers&ModSuper != 0 {
				// Paste from clipboard
				clipText := ffi.ClipboardGetString()
				if clipText != "" {
					w.textBuffer.Insert(clipText)
					w.markDirty(DirtyText)
				}
			}

		case ffi.KeyZ:
			if e.Modifiers&ModSuper != 0 {
				if e.Modifiers&ModShift != 0 {
					// Redo (Cmd+Shift+Z)
					w.textBuffer.Redo()
				} else {
					// Undo (Cmd+Z)
					w.textBuffer.Undo()
				}
				w.markDirty(DirtyText)
			}
		}

		// Reset cursor blink on any key action
		w.textBuffer.ResetBlink()
	})

	// Character input handler for typing
	w.OnKeyPress(func(e *KeyEvent) {
		if w.textBuffer == nil {
			return
		}

		// Skip control characters and when Cmd/Ctrl is held
		if e.Modifiers&(ModSuper|ModCtrl) != 0 {
			return
		}

		char := e.Char
		if char == 0 || !unicode.IsPrint(char) {
			return
		}

		w.textBuffer.Insert(string(char))
		w.textBuffer.ResetBlink()
		w.markDirty(DirtyText)
	})

	// Track if we're doing a drag selection
	var isDragging bool

	// Mouse down to start selection or scrollbar drag
	w.OnMouseDown(func(e *MouseEvent) {
		if w.textBuffer == nil {
			return
		}

		// Check if click is on scrollbar (for multiline/TextArea)
		if w.textBuffer.multiline {
			scrollBarWidth := float32(8)
			scrollBarX := w.width - scrollBarWidth

			if e.LocalX >= scrollBarX {
				// Click is on scrollbar area - start scrollbar drag
				w.mu.Lock()
				w.scrollbarDragging = true
				w.scrollbarDragStartY = e.LocalY
				w.scrollbarDragStartScrollY = w.scrollY
				w.mu.Unlock()
				return
			}
		}

		// Calculate cursor position from click
		var pos int
		if w.textBuffer.multiline {
			pos = w.cursorPositionFromXY(e.LocalX, e.LocalY)
		} else {
			pos = w.cursorPositionFromX(e.LocalX)
		}

		if e.Modifiers&ModShift != 0 {
			// Extend selection from current anchor
			w.textBuffer.mu.Lock()
			w.textBuffer.cursor = pos
			w.textBuffer.mu.Unlock()
		} else {
			// Start new selection - set both anchor and cursor
			w.textBuffer.SetCursor(pos)
		}

		isDragging = true
		w.textBuffer.ResetBlink()
		w.markDirty(DirtyText)
	})

	// Mouse move to extend selection while dragging or scroll while scrollbar dragging
	w.OnMouseMove(func(e *MouseEvent) {
		if w.textBuffer == nil {
			return
		}

		// Handle scrollbar dragging
		w.mu.RLock()
		scrollbarDragging := w.scrollbarDragging
		w.mu.RUnlock()

		if scrollbarDragging {
			w.mu.Lock()
			// Calculate scroll based on mouse delta
			contentWidth := w.width - w.padding[1] - w.padding[3]
			contentHeight := w.height - w.padding[0] - w.padding[2]
			text := w.textBuffer.Text()
			lines := WrapText(text, contentWidth, w.fontSize, "system")
			lineHeight := w.fontSize * 1.5
			totalTextHeight := float32(len(lines)) * lineHeight
			maxScroll := totalTextHeight - contentHeight
			if maxScroll < 0 {
				maxScroll = 0
			}

			// Calculate how much the mouse moved as a ratio of the track height
			trackHeight := w.height - w.padding[0] - w.padding[2]
			viewportRatio := contentHeight / totalTextHeight
			thumbHeight := trackHeight * viewportRatio
			if thumbHeight < 20 {
				thumbHeight = 20
			}
			scrollableTrackHeight := trackHeight - thumbHeight

			if scrollableTrackHeight > 0 {
				deltaY := e.LocalY - w.scrollbarDragStartY
				scrollDelta := (deltaY / scrollableTrackHeight) * maxScroll
				w.scrollY = w.scrollbarDragStartScrollY + scrollDelta

				// Clamp scroll
				if w.scrollY < 0 {
					w.scrollY = 0
				}
				if w.scrollY > maxScroll {
					w.scrollY = maxScroll
				}
			}
			w.mu.Unlock()
			w.markDirty(DirtyScroll)
			return
		}

		if !isDragging {
			return
		}

		// Calculate cursor position from current mouse position
		var pos int
		if w.textBuffer.multiline {
			pos = w.cursorPositionFromXY(e.LocalX, e.LocalY)
		} else {
			pos = w.cursorPositionFromX(e.LocalX)
		}

		// Extend selection - keep anchor, move cursor
		w.textBuffer.mu.Lock()
		w.textBuffer.cursor = pos
		w.textBuffer.mu.Unlock()

		w.textBuffer.ResetBlink()
		w.markDirty(DirtyText)
	})

	// Mouse up to end drag selection or scrollbar drag
	w.OnMouseUp(func(e *MouseEvent) {
		isDragging = false
		w.mu.Lock()
		w.scrollbarDragging = false
		w.mu.Unlock()
	})

	// Double-click to select word
	w.OnDoubleClick(func(e *MouseEvent) {
		if w.textBuffer == nil {
			return
		}

		var pos int
		if w.textBuffer.multiline {
			pos = w.cursorPositionFromXY(e.LocalX, e.LocalY)
		} else {
			pos = w.cursorPositionFromX(e.LocalX)
		}
		w.textBuffer.SelectWordAt(pos)
		w.markDirty(DirtyText)
	})

	// Triple-click to select line (multiline) or all (single line)
	w.OnTripleClick(func(e *MouseEvent) {
		if w.textBuffer == nil {
			return
		}

		var pos int
		if w.textBuffer.multiline {
			pos = w.cursorPositionFromXY(e.LocalX, e.LocalY)
		} else {
			pos = w.cursorPositionFromX(e.LocalX)
		}
		w.textBuffer.SelectLine(pos)
		w.markDirty(DirtyText)
	})

	// Mouse wheel scrolling for TextArea
	w.OnMouseWheel(func(e *MouseEvent) {
		if w.textBuffer == nil || !w.textBuffer.multiline {
			return
		}

		// DeltaY from winit is already in logical pixels on macOS
		// Use it directly for smooth trackpad scrolling
		scrollAmount := e.DeltaY

		w.mu.Lock()
		// Calculate max scroll based on content
		lineHeight := w.fontSize * 1.5
		contentWidth := w.width - w.padding[1] - w.padding[3]
		contentHeight := w.height - w.padding[0] - w.padding[2]
		text := w.textBuffer.Text()
		lines := WrapText(text, contentWidth, w.fontSize, "system")
		totalTextHeight := float32(len(lines)) * lineHeight
		maxScroll := totalTextHeight - contentHeight
		if maxScroll < 0 {
			maxScroll = 0
		}

		// Apply scroll (negative deltaY = scroll down, positive = scroll up)
		w.scrollY -= scrollAmount
		if w.scrollY < 0 {
			w.scrollY = 0
		}
		if w.scrollY > maxScroll {
			w.scrollY = maxScroll
		}
		w.mu.Unlock()

		w.markDirty(DirtyScroll)
	})
}

// cursorPositionFromX calculates the character index from an x coordinate.
// For single-line TextField only.
func (w *Widget) cursorPositionFromX(x float32) int {
	w.mu.RLock()
	text := w.textBuffer.Text()
	fontSize := w.fontSize
	padding := w.padding[3] // left padding
	multiline := w.textBuffer.multiline
	w.mu.RUnlock()

	// For multiline, this should use cursorPositionFromXY instead
	if multiline {
		return w.cursorPositionFromXY(x, 0)
	}

	// Adjust for padding
	x -= padding

	if x <= 0 || text == "" {
		return 0
	}

	// Binary search for the position
	runes := []rune(text)
	for i := range runes {
		charX := ffi.MeasureTextToCursor(text, i+1, "system", fontSize)
		if x < charX {
			// Check if closer to this char or previous
			prevX := float32(0)
			if i > 0 {
				prevX = ffi.MeasureTextToCursor(text, i, "system", fontSize)
			}
			if x-prevX < charX-x {
				return i
			}
			return i + 1
		}
	}

	return len(runes)
}

// cursorPositionFromXY calculates the character index from x,y coordinates in a TextArea.
// Takes scroll offset into account.
func (w *Widget) cursorPositionFromXY(x, y float32) int {
	w.mu.RLock()
	text := w.textBuffer.Text()
	fontSize := w.fontSize
	paddingLeft := w.padding[3]
	paddingTop := w.padding[0]
	contentWidth := w.width - w.padding[1] - w.padding[3]
	scrollY := w.scrollY
	w.mu.RUnlock()

	if text == "" {
		return 0
	}

	// Adjust for padding
	x -= paddingLeft
	y -= paddingTop

	// Add scroll offset to get position in content space
	y += scrollY

	if x < 0 {
		x = 0
	}

	// Calculate line height
	lineHeight := fontSize * 1.5

	// Wrap text to get lines
	lines := WrapText(text, contentWidth, fontSize, "system")
	if len(lines) == 0 {
		return 0
	}

	// Determine which line was clicked
	clickedRow := int(y / lineHeight)
	if clickedRow < 0 {
		clickedRow = 0
	}
	if clickedRow >= len(lines) {
		clickedRow = len(lines) - 1
	}

	// Find character position within the line
	return CursorIndexFromPosition(lines, clickedRow, x, fontSize, "system")
}

// Text input widget methods

// InputText returns the text content of a TextField or TextArea.
func (w *Widget) InputText() string {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.textBuffer == nil {
		return ""
	}
	return w.textBuffer.Text()
}

// SetInputText sets the text content of a TextField or TextArea.
func (w *Widget) SetInputText(text string) *Widget {
	w.mu.Lock()
	if w.textBuffer == nil {
		w.mu.Unlock()
		return w
	}
	w.textBuffer.SetText(text)
	w.dirtyMask |= DirtyText
	w.mu.Unlock()
	return w
}

// InputCursor returns the cursor position.
func (w *Widget) InputCursor() int {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.textBuffer == nil {
		return 0
	}
	return w.textBuffer.Cursor()
}

// InputSelection returns the selection range (start, end).
func (w *Widget) InputSelection() (int, int) {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.textBuffer == nil {
		return 0, 0
	}
	return w.textBuffer.Selection()
}

// SetInputPlaceholder sets the placeholder text.
func (w *Widget) SetInputPlaceholder(placeholder string) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetPlaceholder(placeholder)
	}
	w.mu.Unlock()
	return w
}

// SetInputMaxLength sets the maximum number of characters.
func (w *Widget) SetInputMaxLength(max int) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetMaxLength(max)
	}
	w.mu.Unlock()
	return w
}

// OnInputChange sets a callback for when the input text changes.
func (w *Widget) OnInputChange(fn func(text string)) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.OnChange(fn)
	}
	w.mu.Unlock()
	return w
}

// SetPassword enables or disables password masking.
// When enabled, characters are displayed as bullets (•).
func (w *Widget) SetPassword(enabled bool) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetPassword(enabled)
	}
	w.dirtyMask |= DirtyText
	w.mu.Unlock()
	return w
}

// SetPasswordChar sets the character used to mask password text.
// Default is '•' (bullet).
func (w *Widget) SetPasswordChar(char rune) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetPasswordChar(char)
	}
	w.dirtyMask |= DirtyText
	w.mu.Unlock()
	return w
}

// IsPassword returns whether password mode is enabled.
func (w *Widget) IsPassword() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.textBuffer == nil {
		return false
	}
	return w.textBuffer.IsPassword()
}

// SetReadOnly enables or disables read-only mode.
// In read-only mode, text can be selected and copied but not edited.
func (w *Widget) SetReadOnly(readOnly bool) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetReadOnly(readOnly)
	}
	w.mu.Unlock()
	return w
}

// IsReadOnly returns whether read-only mode is enabled.
func (w *Widget) IsReadOnly() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.textBuffer == nil {
		return false
	}
	return w.textBuffer.IsReadOnly()
}

// SetPlaceholderColor sets the color for placeholder text (RGBA format).
// Use 0 for the default gray color.
func (w *Widget) SetPlaceholderColor(color uint32) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetPlaceholderColor(color)
	}
	w.mu.Unlock()
	return w
}

// SetInputValidator sets a validation function for the input.
// The function receives the full text and should return true if valid.
// Call IsInputValid() to check validation state.
func (w *Widget) SetInputValidator(fn func(text string) bool) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetValidator(fn)
	}
	w.mu.Unlock()
	return w
}

// IsInputValid returns whether the current input passes validation.
// Returns true if no validator is set.
func (w *Widget) IsInputValid() bool {
	w.mu.RLock()
	defer w.mu.RUnlock()
	if w.textBuffer == nil {
		return true
	}
	return w.textBuffer.IsValid()
}

// SetCharFilter sets a filter function for input characters.
// The function receives each typed character and should return true to allow it.
// This is useful for restricting input to numbers only, etc.
func (w *Widget) SetCharFilter(fn func(r rune) bool) *Widget {
	w.mu.Lock()
	if w.textBuffer != nil {
		w.textBuffer.SetCharFilter(fn)
	}
	w.mu.Unlock()
	return w
}

// ============================================================================
// Text Wrapping
// ============================================================================

// WrappedLine represents a single line in wrapped text.
type WrappedLine struct {
	Text       string // The text content of this line
	StartIndex int    // Character index in original text where this line starts
	EndIndex   int    // Character index in original text where this line ends (exclusive)
}

// WrapText breaks text into lines that fit within maxWidth pixels.
// Returns a slice of WrappedLine with info about each line.
// For bundled fonts, use WrapTextWithFont instead.
func WrapText(text string, maxWidth, fontSize float32, fontName string) []WrappedLine {
	return WrapTextWithFont(text, maxWidth, fontSize, fontName, "")
}

// WrapTextWithFont breaks text into lines that fit within maxWidth pixels,
// supporting both system fonts and bundled fonts via font family.
// fontFamily should be the theme key (e.g., "sans", "serif", "mono") or empty for system font.
func WrapTextWithFont(text string, maxWidth, fontSize float32, fontName string, fontFamily string) []WrappedLine {
	if text == "" {
		return []WrappedLine{{Text: "", StartIndex: 0, EndIndex: 0}}
	}

	// Create measurement function based on font family
	measureText := func(t string) float32 {
		return measureTextWidthExtFunc(t, fontName, fontFamily, fontSize)
	}

	var lines []WrappedLine
	runes := []rune(text)
	lineStart := 0

	for lineStart < len(runes) {
		// Find line end (either newline or wrap point)
		lineEnd := lineStart
		lastWordEnd := lineStart
		lastWordEndRune := lineStart

		for lineEnd < len(runes) {
			r := runes[lineEnd]

			// Hard break on newline
			if r == '\n' {
				lines = append(lines, WrappedLine{
					Text:       string(runes[lineStart:lineEnd]),
					StartIndex: lineStart,
					EndIndex:   lineEnd,
				})
				lineStart = lineEnd + 1 // Skip the newline
				lineEnd = lineStart
				lastWordEnd = lineStart
				lastWordEndRune = lineStart
				continue
			}

			// Track word boundaries for soft wrapping
			if unicode.IsSpace(r) {
				lastWordEnd = lineEnd + 1
				lastWordEndRune = lineEnd + 1
			}

			// Check if we've exceeded width
			lineText := string(runes[lineStart : lineEnd+1])
			lineWidth := measureText(lineText)

			if lineWidth > maxWidth && lineEnd > lineStart {
				// Need to wrap
				var breakPoint int
				if lastWordEnd > lineStart {
					// Break at last word boundary
					breakPoint = lastWordEndRune
				} else {
					// No word boundary, break at current character
					breakPoint = lineEnd
				}

				lines = append(lines, WrappedLine{
					Text:       string(runes[lineStart:breakPoint]),
					StartIndex: lineStart,
					EndIndex:   breakPoint,
				})

				// Skip whitespace at start of next line
				lineStart = breakPoint
				for lineStart < len(runes) && runes[lineStart] == ' ' {
					lineStart++
				}
				lineEnd = lineStart
				lastWordEnd = lineStart
				lastWordEndRune = lineStart
				continue
			}

			lineEnd++
		}

		// Add remaining text as final line
		if lineStart < len(runes) {
			lines = append(lines, WrappedLine{
				Text:       string(runes[lineStart:]),
				StartIndex: lineStart,
				EndIndex:   len(runes),
			})
		} else if lineStart == len(runes) && len(runes) > 0 && runes[len(runes)-1] == '\n' {
			// Text ends with newline - add empty line for cursor positioning
			lines = append(lines, WrappedLine{
				Text:       "",
				StartIndex: lineStart,
				EndIndex:   lineStart,
			})
		}
		break
	}

	// Ensure at least one line exists
	if len(lines) == 0 {
		lines = append(lines, WrappedLine{Text: "", StartIndex: 0, EndIndex: 0})
	}

	return lines
}

// CursorPositionInWrappedText returns the row and column of a cursor position in wrapped text.
// row is 0-indexed line number, colX is the x-offset in pixels within that line.
func CursorPositionInWrappedText(lines []WrappedLine, cursorIndex int, fontSize float32, fontName string) (row int, colX float32) {
	for i, line := range lines {
		// Check if cursor is within this line (or at the end of it for the last line)
		if cursorIndex >= line.StartIndex && (cursorIndex < line.EndIndex || (i == len(lines)-1 && cursorIndex <= line.EndIndex)) {
			// Cursor is on this line
			offsetInLine := cursorIndex - line.StartIndex
			if offsetInLine > 0 && line.Text != "" {
				colX = ffi.MeasureTextToCursor(line.Text, offsetInLine, fontName, fontSize)
			}
			return i, colX
		}
		// Special case: cursor exactly at end of a line that ends with newline
		if cursorIndex == line.EndIndex && i < len(lines)-1 {
			return i, ffi.MeasureTextToCursor(line.Text, len([]rune(line.Text)), fontName, fontSize)
		}
	}
	// Cursor is at the very end
	if len(lines) > 0 {
		lastLine := lines[len(lines)-1]
		return len(lines) - 1, ffi.MeasureTextToCursor(lastLine.Text, len([]rune(lastLine.Text)), fontName, fontSize)
	}
	return 0, 0
}

// CursorIndexFromPosition returns the character index in the original text
// given a row and x-position in pixels.
func CursorIndexFromPosition(lines []WrappedLine, row int, xPos float32, fontSize float32, fontName string) int {
	if len(lines) == 0 {
		return 0
	}

	// Clamp row to valid range
	if row < 0 {
		row = 0
	}
	if row >= len(lines) {
		row = len(lines) - 1
	}

	line := lines[row]
	lineRunes := []rune(line.Text)

	// Find character position by measuring text widths
	// Binary search would be faster, but linear is fine for typical line lengths
	for i := 0; i <= len(lineRunes); i++ {
		charX := ffi.MeasureTextToCursor(line.Text, i, fontName, fontSize)
		if i == len(lineRunes) {
			// At end of line
			return line.StartIndex + i
		}
		nextCharX := ffi.MeasureTextToCursor(line.Text, i+1, fontName, fontSize)
		// Check if xPos is closer to current position or next
		midpoint := (charX + nextCharX) / 2
		if xPos < midpoint {
			return line.StartIndex + i
		}
	}

	return line.EndIndex
}

// MoveCursorVertical moves the cursor up or down by the specified number of lines
// in wrapped text, trying to maintain the same horizontal position.
// Returns the new cursor position.
func MoveCursorVertical(text string, currentCursor int, delta int, contentWidth float32, fontSize float32, fontName string) int {
	if contentWidth <= 0 {
		return currentCursor
	}

	lines := WrapText(text, contentWidth, fontSize, fontName)
	if len(lines) == 0 {
		return 0
	}

	// Find current row and x position
	currentRow, currentX := CursorPositionInWrappedText(lines, currentCursor, fontSize, fontName)

	// Calculate target row
	targetRow := currentRow + delta

	// Clamp to valid range
	if targetRow < 0 {
		targetRow = 0
	}
	if targetRow >= len(lines) {
		targetRow = len(lines) - 1
	}

	// If no movement possible, return current position
	if targetRow == currentRow {
		if delta < 0 {
			// At top, move to start
			return 0
		} else if delta > 0 {
			// At bottom, move to end
			return len([]rune(text))
		}
		return currentCursor
	}

	// Find cursor position in target row that matches the x position
	return CursorIndexFromPosition(lines, targetRow, currentX, fontSize, fontName)
}

// ============================================================================
// Cursor Blink Animation
// ============================================================================

// CursorBlinkInterval is the blink rate for text cursors.
const CursorBlinkInterval = 530 * time.Millisecond

// TextInputState tracks cursor blink state for rendering.
type TextInputState struct {
	cursorVisible bool
	lastBlink     time.Time
}

// UpdateCursorBlink updates the cursor visibility based on time.
// Returns true if visibility changed.
func (s *TextInputState) UpdateCursorBlink(now time.Time) bool {
	if now.Sub(s.lastBlink) >= CursorBlinkInterval {
		s.cursorVisible = !s.cursorVisible
		s.lastBlink = now
		return true
	}
	return false
}

// ResetCursorBlink makes the cursor visible (called on keystroke).
func (s *TextInputState) ResetCursorBlink(now time.Time) {
	s.cursorVisible = true
	s.lastBlink = now
}

// CursorVisible returns whether the cursor should be visible.
func (s *TextInputState) CursorVisible() bool {
	return s.cursorVisible
}

// Helper for rendering - get display text
func getDisplayText(w *Widget) string {
	if w.textBuffer == nil {
		return ""
	}
	text := w.textBuffer.Text()
	if text == "" {
		return w.textBuffer.Placeholder()
	}
	return text
}

// Helper for rendering - check if showing placeholder
func isShowingPlaceholder(w *Widget) bool {
	if w.textBuffer == nil {
		return false
	}
	return w.textBuffer.Text() == "" && w.textBuffer.Placeholder() != ""
}

// Helper to get cursor X position for rendering
func getCursorX(w *Widget) float32 {
	if w.textBuffer == nil {
		return 0
	}

	text := w.textBuffer.Text()
	cursor := w.textBuffer.Cursor()

	if cursor == 0 || text == "" {
		return 0
	}

	return ffi.MeasureTextToCursor(text, cursor, "system", w.fontSize)
}

// Helper to get selection bounds for rendering
func getSelectionBounds(w *Widget) (startX, endX float32, hasSelection bool) {
	if w.textBuffer == nil {
		return 0, 0, false
	}

	start, end := w.textBuffer.Selection()
	if start == end {
		return 0, 0, false
	}

	text := w.textBuffer.Text()
	startX = ffi.MeasureTextToCursor(text, start, "system", w.fontSize)
	endX = ffi.MeasureTextToCursor(text, end, "system", w.fontSize)

	return startX, endX, true
}

// byteIndexToRuneIndex converts a byte index to a rune index.
// Useful when handling platform events that report byte positions.
func byteIndexToRuneIndex(s string, byteIndex int) int {
	runeIndex := 0
	for i := range s {
		if i >= byteIndex {
			break
		}
		runeIndex++
	}
	return runeIndex
}

// runeIndexToByteIndex converts a rune index to a byte index.
func runeIndexToByteIndex(s string, runeIndex int) int {
	byteIndex := 0
	for i, r := range s {
		if i >= runeIndex {
			break
		}
		byteIndex += utf8.RuneLen(r)
	}
	return byteIndex
}
