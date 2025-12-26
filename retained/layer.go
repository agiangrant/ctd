// Package retained provides layer abstraction for regional re-rendering.
// Layers group widgets that can be rendered to offscreen textures and
// composited together, enabling partial updates when only some regions change.
package retained

import (
	"sync"
	"sync/atomic"

	"github.com/agiangrant/centered/internal/ffi"
)

// LayerID uniquely identifies a render layer.
type LayerID uint32

var nextLayerID atomic.Uint32

func newLayerID() LayerID {
	return LayerID(nextLayerID.Add(1))
}

// OpacityKind classifies whether a layer is fully opaque or has transparency.
// This affects compositing order - transparent layers require layers behind them
// to be rendered first.
type OpacityKind int

const (
	// OpacityOpaque means the layer is fully opaque - no content behind it shows through.
	// Safe to cache independently without considering what's behind.
	OpacityOpaque OpacityKind = iota

	// OpacityTransparent means the layer has some transparency - content behind may show.
	// When this layer is dirty, layers behind it that overlap must also be re-rendered.
	OpacityTransparent
)

// Layer represents a cacheable render region.
// Layers can be rendered to offscreen textures and composited together.
type Layer struct {
	ID     LayerID
	Bounds Bounds // Screen-space bounds of the layer

	// Widgets assigned to this layer (in render order)
	Widgets []*Widget

	// Rendering properties
	Opacity       OpacityKind // Whether layer is opaque or transparent
	ZOrder        int         // Compositing order (lower = further back)
	AlwaysDynamic bool        // If true, layer is always re-rendered (for video, animations)

	// Dirty tracking
	Dirty     bool   // True if layer needs re-rendering
	LastFrame uint64 // Frame number when layer was last rendered

	// Cached render target (GPU texture ID, managed by Rust)
	TextureID uint32 // 0 = no cached texture
}

// LayerManager organizes widgets into layers for efficient regional re-rendering.
type LayerManager struct {
	mu sync.RWMutex

	// All layers, sorted by ZOrder for compositing
	layers []*Layer

	// Map from widget to its assigned layer
	widgetLayer map[WidgetID]LayerID

	// Map from layer ID to layer
	layerByID map[LayerID]*Layer

	// Frame counter for dirty tracking
	frameCounter uint64
}

// NewLayerManager creates a new layer manager.
func NewLayerManager() *LayerManager {
	return &LayerManager{
		widgetLayer: make(map[WidgetID]LayerID),
		layerByID:   make(map[LayerID]*Layer),
	}
}

// CreateLayer creates a new render layer with the given bounds.
func (lm *LayerManager) CreateLayer(bounds Bounds, opacity OpacityKind, zOrder int) *Layer {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	layer := &Layer{
		ID:      newLayerID(),
		Bounds:  bounds,
		Opacity: opacity,
		ZOrder:  zOrder,
		Dirty:   true, // New layers need initial render
	}

	lm.layers = append(lm.layers, layer)
	lm.layerByID[layer.ID] = layer

	// Keep layers sorted by ZOrder
	lm.sortLayers()

	return layer
}

// AssignWidget assigns a widget to a layer.
func (lm *LayerManager) AssignWidget(widget *Widget, layerID LayerID) {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	// Remove from previous layer if any
	if oldLayerID, exists := lm.widgetLayer[widget.id]; exists {
		if oldLayer := lm.layerByID[oldLayerID]; oldLayer != nil {
			// Remove widget from old layer
			for i, w := range oldLayer.Widgets {
				if w.id == widget.id {
					oldLayer.Widgets = append(oldLayer.Widgets[:i], oldLayer.Widgets[i+1:]...)
					break
				}
			}
		}
	}

	// Add to new layer
	if layer := lm.layerByID[layerID]; layer != nil {
		layer.Widgets = append(layer.Widgets, widget)
		lm.widgetLayer[widget.id] = layerID
		layer.Dirty = true
	}
}

// MarkLayerDirty marks a layer as needing re-render.
// If the layer is transparent, also marks overlapping layers behind it.
func (lm *LayerManager) MarkLayerDirty(layerID LayerID) {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	layer := lm.layerByID[layerID]
	if layer == nil {
		return
	}

	layer.Dirty = true

	// NOTE: We're NOT marking layers behind transparent layers dirty because:
	// 1. We're not doing texture compositing yet - we flatten all commands
	// 2. Commands are rendered in z-order, so transparency works correctly
	// 3. This allows for incremental rendering when only one layer changes
	//
	// When we add texture caching, we'll need to revisit this - cached textures
	// for layers behind a dirty transparent layer would need to be re-composited.
}

// MarkWidgetDirty marks the layer containing a widget as dirty.
func (lm *LayerManager) MarkWidgetDirty(widgetID WidgetID) {
	lm.mu.RLock()
	layerID, exists := lm.widgetLayer[widgetID]
	lm.mu.RUnlock()

	if exists {
		lm.MarkLayerDirty(layerID)
	}
}

// GetDirtyLayers returns all layers that need re-rendering.
func (lm *LayerManager) GetDirtyLayers() []*Layer {
	lm.mu.RLock()
	defer lm.mu.RUnlock()

	var dirty []*Layer
	for _, layer := range lm.layers {
		if layer.Dirty || layer.AlwaysDynamic {
			dirty = append(dirty, layer)
		}
	}
	return dirty
}

// GetAllLayers returns all layers in compositing order (back to front).
func (lm *LayerManager) GetAllLayers() []*Layer {
	lm.mu.RLock()
	defer lm.mu.RUnlock()

	// Return a copy to avoid race conditions
	result := make([]*Layer, len(lm.layers))
	copy(result, lm.layers)
	return result
}

// ClearLayerDirty marks a layer as clean after rendering.
func (lm *LayerManager) ClearLayerDirty(layerID LayerID, frameNum uint64) {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	if layer := lm.layerByID[layerID]; layer != nil {
		layer.Dirty = false
		layer.LastFrame = frameNum
	}
}

// RemoveLayer removes a layer and unassigns all its widgets.
func (lm *LayerManager) RemoveLayer(layerID LayerID) {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	layer := lm.layerByID[layerID]
	if layer == nil {
		return
	}

	// Unassign all widgets
	for _, w := range layer.Widgets {
		delete(lm.widgetLayer, w.id)
	}

	// Remove from layers slice
	for i, l := range lm.layers {
		if l.ID == layerID {
			lm.layers = append(lm.layers[:i], lm.layers[i+1:]...)
			break
		}
	}

	delete(lm.layerByID, layerID)
}

// InvalidateAll marks all layers as dirty.
// Called on window resize or other global changes.
func (lm *LayerManager) InvalidateAll() {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	for _, layer := range lm.layers {
		layer.Dirty = true
	}
}

// GetWidgetLayer returns the layer ID for a widget, if assigned.
func (lm *LayerManager) GetWidgetLayer(widgetID WidgetID) (LayerID, bool) {
	lm.mu.RLock()
	defer lm.mu.RUnlock()

	id, exists := lm.widgetLayer[widgetID]
	return id, exists
}

// sortLayers sorts layers by ZOrder (ascending - lower values are further back).
func (lm *LayerManager) sortLayers() {
	// Simple insertion sort since layer count is typically small
	for i := 1; i < len(lm.layers); i++ {
		j := i
		for j > 0 && lm.layers[j].ZOrder < lm.layers[j-1].ZOrder {
			lm.layers[j], lm.layers[j-1] = lm.layers[j-1], lm.layers[j]
			j--
		}
	}
}

// ClassifyOpacity determines if a widget (and its subtree) is opaque or transparent.
func ClassifyOpacity(w *Widget) OpacityKind {
	w.mu.RLock()
	defer w.mu.RUnlock()

	// Check widget's own opacity
	if w.opacity < 1.0 {
		return OpacityTransparent
	}

	// Check background color alpha
	if w.backgroundColor != nil {
		alpha := (*w.backgroundColor) & 0xFF
		if alpha < 255 {
			return OpacityTransparent
		}
	} else {
		// No background = transparent
		return OpacityTransparent
	}

	// Check children recursively
	for _, child := range w.children {
		if ClassifyOpacity(child) == OpacityTransparent {
			return OpacityTransparent
		}
	}

	return OpacityOpaque
}

// GenerateLayerCommands generates render commands for a layer's widgets.
// This is called when a layer needs to be re-rendered to its offscreen texture.
func (lm *LayerManager) GenerateLayerCommands(
	layer *Layer,
	renderFunc func(w *Widget, offsetX, offsetY float32) []ffi.RenderCommand,
) []ffi.RenderCommand {
	var commands []ffi.RenderCommand

	// Render each widget in the layer
	for _, w := range layer.Widgets {
		// Offset widget rendering relative to layer origin
		offsetX := -layer.Bounds.X
		offsetY := -layer.Bounds.Y
		widgetCmds := renderFunc(w, offsetX, offsetY)
		commands = append(commands, widgetCmds...)
	}

	return commands
}

// AutoAssignLayers automatically assigns widgets to layers based on their properties.
// Call this to set up initial layer structure.
func (lm *LayerManager) AutoAssignLayers(root *Widget, windowWidth, windowHeight float32) {
	lm.mu.Lock()
	defer lm.mu.Unlock()

	// Clear existing assignments
	lm.layers = nil
	lm.widgetLayer = make(map[WidgetID]LayerID)
	lm.layerByID = make(map[LayerID]*Layer)

	// For now, create a simple single-layer setup
	// More sophisticated assignment would create separate layers for:
	// - Each ScrollView (content scrolls independently)
	// - Video/camera widgets (always dynamic)
	// - Animating widgets (frequently updated)
	// - Large static subtrees (rarely change)

	// Create root layer for the entire window
	rootLayer := &Layer{
		ID: newLayerID(),
		Bounds: Bounds{
			X:      0,
			Y:      0,
			Width:  windowWidth,
			Height: windowHeight,
		},
		Opacity: OpacityOpaque, // Assume root is opaque (has background)
		ZOrder:  0,
		Dirty:   true,
	}

	lm.layers = append(lm.layers, rootLayer)
	lm.layerByID[rootLayer.ID] = rootLayer

	// Assign all widgets to root layer
	lm.assignSubtree(root, rootLayer.ID)
}

// assignSubtree recursively assigns a widget and its children to a layer.
func (lm *LayerManager) assignSubtree(w *Widget, layerID LayerID) {
	if w == nil {
		return
	}

	layer := lm.layerByID[layerID]
	if layer != nil {
		layer.Widgets = append(layer.Widgets, w)
		lm.widgetLayer[w.id] = layerID
	}

	w.mu.RLock()
	children := make([]*Widget, len(w.children))
	copy(children, w.children)
	w.mu.RUnlock()

	for _, child := range children {
		lm.assignSubtree(child, layerID)
	}
}

// Bounds helper method for intersection testing.
func (b Bounds) Intersects(other Bounds) bool {
	return !(b.X+b.Width < other.X || other.X+other.Width < b.X ||
		b.Y+b.Height < other.Y || other.Y+other.Height < b.Y)
}
