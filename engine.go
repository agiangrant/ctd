package centered

import (
	"encoding/json"
	"fmt"
	"io/ioutil"

	"github.com/agiangrant/centered/internal/ffi"
)

// Engine represents the Centered UI engine
type Engine struct {
	handle ffi.EngineHandle
	mode   ffi.RenderMode
	width  uint32
	height uint32

	// For retained mode: track previous frame
	lastFrame *WidgetTree
}

// EngineConfig contains configuration for the engine
type EngineConfig struct {
	Width  uint32
	Height uint32
	Mode   ffi.RenderMode
}

// DefaultEngineConfig returns the default engine configuration
func DefaultEngineConfig() EngineConfig {
	return EngineConfig{
		Width:  800,
		Height: 600,
		Mode:   ffi.RenderModeRetained,
	}
}

// NewEngine creates a new engine with the given configuration
func NewEngine(config EngineConfig) (*Engine, error) {
	ffiConfig := ffi.EngineConfig{
		Width:  config.Width,
		Height: config.Height,
		Mode:   config.Mode,
	}

	handle, err := ffi.InitEngine(ffiConfig)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize engine: %w", err)
	}

	return &Engine{
		handle: handle,
		mode:   config.Mode,
		width:  config.Width,
		height: config.Height,
	}, nil
}

// Shutdown destroys the engine and frees resources
func (e *Engine) Shutdown() {
	if e.handle != nil {
		ffi.DestroyEngine(e.handle)
		e.handle = nil
	}
}

// LoadStylesFromFile loads theme styles from a TOML file
func (e *Engine) LoadStylesFromFile(path string) error {
	data, err := ioutil.ReadFile(path)
	if err != nil {
		return fmt.Errorf("failed to read styles file: %w", err)
	}

	return e.LoadStyles(string(data))
}

// LoadStyles loads theme styles from a TOML string
func (e *Engine) LoadStyles(toml string) error {
	if err := ffi.LoadStyles(e.handle, toml); err != nil {
		return fmt.Errorf("failed to load styles: %w", err)
	}
	return nil
}

// Resize resizes the rendering surface
func (e *Engine) Resize(width, height uint32) error {
	if err := ffi.Resize(e.handle, width, height); err != nil {
		return fmt.Errorf("failed to resize: %w", err)
	}
	e.width = width
	e.height = height
	return nil
}

// GetMode returns the current rendering mode
func (e *Engine) GetMode() ffi.RenderMode {
	return e.mode
}

// GetSize returns the current width and height
func (e *Engine) GetSize() (uint32, uint32) {
	return e.width, e.height
}

// RenderFrame renders a complete frame (immediate mode or retained mode)
// This is the primary API - one call per frame
func (e *Engine) RenderFrame(root Widget) ([]Event, error) {
	tree := NewWidgetTree(root)

	// For immediate mode: always send the full tree
	if e.mode == ffi.RenderModeImmediate {
		return e.submitFullFrame(tree)
	}

	// For retained mode: compute delta if we have a previous frame
	if e.lastFrame != nil {
		delta := computeDelta(e.lastFrame, tree)
		if delta.IsEmpty() {
			// No changes, return empty events
			return []Event{}, nil
		}
		events, err := e.submitDelta(delta)
		if err != nil {
			return nil, err
		}
		e.lastFrame = tree
		return events, nil
	}

	// First frame in retained mode: send full tree
	events, err := e.submitFullFrame(tree)
	if err != nil {
		return nil, err
	}
	e.lastFrame = tree
	return events, nil
}

// submitFullFrame sends the complete widget tree to the engine
func (e *Engine) submitFullFrame(tree *WidgetTree) ([]Event, error) {
	frameJSON, err := tree.ToJSON()
	if err != nil {
		return nil, fmt.Errorf("failed to serialize frame: %w", err)
	}

	eventsJSON, err := ffi.SubmitFrame(e.handle, frameJSON)
	if err != nil {
		return nil, fmt.Errorf("failed to submit frame: %w", err)
	}

	var eventBatch EventBatch
	if err := json.Unmarshal([]byte(eventsJSON), &eventBatch); err != nil {
		return nil, fmt.Errorf("failed to parse events: %w", err)
	}

	return eventBatch.Events, nil
}

// submitDelta sends a delta update to the engine
func (e *Engine) submitDelta(delta *WidgetDelta) ([]Event, error) {
	deltaJSON, err := delta.ToJSON()
	if err != nil {
		return nil, fmt.Errorf("failed to serialize delta: %w", err)
	}

	eventsJSON, err := ffi.SubmitDelta(e.handle, deltaJSON)
	if err != nil {
		return nil, fmt.Errorf("failed to submit delta: %w", err)
	}

	var eventBatch EventBatch
	if err := json.Unmarshal([]byte(eventsJSON), &eventBatch); err != nil {
		return nil, fmt.Errorf("failed to parse events: %w", err)
	}

	return eventBatch.Events, nil
}

// Version returns the engine version
func Version() string {
	return ffi.Version()
}

// WidgetDelta represents changes between two widget trees
type WidgetDelta struct {
	// For now, we'll keep it simple and just send the full tree
	// TODO: Implement proper diffing for retained mode optimization
	FullTree *WidgetTree `json:"full_tree,omitempty"`
}

// IsEmpty returns true if the delta has no changes
func (wd *WidgetDelta) IsEmpty() bool {
	return wd.FullTree == nil
}

// ToJSON serializes the delta to JSON
func (wd *WidgetDelta) ToJSON() (string, error) {
	data, err := json.Marshal(wd)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// computeDelta computes the differences between two widget trees
// For now, we do a simple comparison - optimization can come later
func computeDelta(old, new *WidgetTree) *WidgetDelta {
	// Simple implementation: just check if trees are identical
	// If different, send the full tree
	// TODO: Implement proper tree diffing
	oldJSON, _ := old.ToJSON()
	newJSON, _ := new.ToJSON()

	if oldJSON == newJSON {
		return &WidgetDelta{} // Empty delta
	}

	return &WidgetDelta{
		FullTree: new,
	}
}

// Event represents a UI event from the engine
type Event struct {
	Type   string `json:"type"`
	Target string `json:"target,omitempty"`
	Data   string `json:"data,omitempty"`
}

// EventBatch is a collection of events
type EventBatch struct {
	Events []Event `json:"events"`
}
