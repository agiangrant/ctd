package retained

import "sync"

// ============================================================================
// Widget Slice Pooling
// ============================================================================
//
// This file provides pooled slice allocation for widget children to reduce
// GC pressure during layout, rendering, and event dispatch. These operations
// frequently copy children slices under read locks, causing thousands of
// allocations per frame in large widget trees.
//
// Usage:
//   children := acquireWidgetSlice(len(w.children))
//   copy(children, w.children)
//   ... use children ...
//   releaseWidgetSlice(children)

// widgetSlicePool pools []*Widget slices to reduce allocations.
// Slices are bucketed by capacity for efficient reuse.
var widgetSlicePool = sync.Pool{
	New: func() interface{} {
		// Start with a reasonable capacity that covers most cases
		return make([]*Widget, 0, 16)
	},
}

// acquireWidgetSlice gets a widget slice from the pool with at least the given length.
// The returned slice has len == n and may have cap > n.
// Caller must call releaseWidgetSlice when done.
func acquireWidgetSlice(n int) []*Widget {
	slice := widgetSlicePool.Get().([]*Widget)

	// If the pooled slice is too small, allocate a new one
	if cap(slice) < n {
		// Return the small slice to the pool for others
		widgetSlicePool.Put(slice[:0])
		// Allocate a new slice with some extra capacity
		return make([]*Widget, n, n*2)
	}

	// Resize to the requested length
	return slice[:n]
}

// releaseWidgetSlice returns a widget slice to the pool.
// The slice should not be used after calling this.
func releaseWidgetSlice(slice []*Widget) {
	if slice == nil {
		return
	}

	// Clear the slice to avoid holding references (helps GC)
	for i := range slice {
		slice[i] = nil
	}

	// Only pool slices up to a reasonable size to avoid memory bloat
	if cap(slice) <= 256 {
		widgetSlicePool.Put(slice[:0])
	}
}

// ============================================================================
// Hover Chain Pooling
// ============================================================================

// hoverChainPool pools slices used for hit test chains and hover tracking.
var hoverChainPool = sync.Pool{
	New: func() interface{} {
		return make([]*Widget, 0, 32)
	},
}

// acquireHoverChain gets a widget slice for hover chain tracking.
func acquireHoverChain(n int) []*Widget {
	slice := hoverChainPool.Get().([]*Widget)
	if cap(slice) < n {
		hoverChainPool.Put(slice[:0])
		return make([]*Widget, n, n*2)
	}
	return slice[:n]
}

// releaseHoverChain returns a hover chain slice to the pool.
func releaseHoverChain(slice []*Widget) {
	if slice == nil {
		return
	}
	for i := range slice {
		slice[i] = nil
	}
	if cap(slice) <= 64 {
		hoverChainPool.Put(slice[:0])
	}
}

// ============================================================================
// Map Pooling for Hover Set Comparisons
// ============================================================================

// hoverSetPool pools maps used for hover chain set comparisons.
var hoverSetPool = sync.Pool{
	New: func() interface{} {
		return make(map[*Widget]bool, 32)
	},
}

// acquireHoverSet gets a map for hover chain set operations.
func acquireHoverSet() map[*Widget]bool {
	return hoverSetPool.Get().(map[*Widget]bool)
}

// releaseHoverSet returns a hover set map to the pool after clearing it.
func releaseHoverSet(m map[*Widget]bool) {
	if m == nil {
		return
	}
	// Clear the map
	for k := range m {
		delete(m, k)
	}
	hoverSetPool.Put(m)
}
