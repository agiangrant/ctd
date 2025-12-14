package retained

import (
	"math"
	"sync"
	"sync/atomic"
	"time"
)

// AnimationID uniquely identifies an animation.
type AnimationID uint64

var nextAnimationID atomic.Uint64

func newAnimationID() AnimationID {
	return AnimationID(nextAnimationID.Add(1))
}

// EasingFunc defines how animation progress maps to value progress.
// Input t is 0-1 (time progress), output is 0-1 (value progress).
type EasingFunc func(t float64) float64

// Common easing functions
var (
	// EaseLinear - constant speed
	EaseLinear EasingFunc = func(t float64) float64 { return t }

	// EaseInQuad - accelerate from zero
	EaseInQuad EasingFunc = func(t float64) float64 { return t * t }

	// EaseOutQuad - decelerate to zero
	EaseOutQuad EasingFunc = func(t float64) float64 { return t * (2 - t) }

	// EaseInOutQuad - accelerate then decelerate
	EaseInOutQuad EasingFunc = func(t float64) float64 {
		if t < 0.5 {
			return 2 * t * t
		}
		return -1 + (4-2*t)*t
	}

	// EaseOutCubic - smooth deceleration (good for UI)
	EaseOutCubic EasingFunc = func(t float64) float64 {
		t--
		return t*t*t + 1
	}

	// EaseInOutCubic - smooth acceleration and deceleration
	EaseInOutCubic EasingFunc = func(t float64) float64 {
		if t < 0.5 {
			return 4 * t * t * t
		}
		return (t-1)*(2*t-2)*(2*t-2) + 1
	}

	// EaseOutBack - slight overshoot then settle (bouncy feel)
	EaseOutBack EasingFunc = func(t float64) float64 {
		c1 := 1.70158
		c3 := c1 + 1
		return 1 + c3*(t-1)*(t-1)*(t-1) + c1*(t-1)*(t-1)
	}

	// EaseOutElastic - elastic wobble effect
	EaseOutElastic EasingFunc = func(t float64) float64 {
		if t == 0 || t == 1 {
			return t
		}
		c4 := (2 * math.Pi) / 3
		return math.Pow(2, -10*t)*math.Sin((t*10-0.75)*c4) + 1
	}

	// EaseOutBounce - bouncing ball effect
	EaseOutBounce EasingFunc = func(t float64) float64 {
		n1 := 7.5625
		d1 := 2.75
		if t < 1/d1 {
			return n1 * t * t
		} else if t < 2/d1 {
			t -= 1.5 / d1
			return n1*t*t + 0.75
		} else if t < 2.5/d1 {
			t -= 2.25 / d1
			return n1*t*t + 0.9375
		} else {
			t -= 2.625 / d1
			return n1*t*t + 0.984375
		}
	}
)

// Animation represents an active animation on a widget.
type Animation struct {
	id        AnimationID
	widget    *Widget
	startTime time.Time
	duration  time.Duration
	update    func(progress float64) // Called each frame with eased progress 0-1
	onComplete func()                // Called when animation finishes
	easing    EasingFunc
	loop      bool // If true, animation repeats forever
	cancelled atomic.Bool
}

// ID returns the animation's unique identifier.
func (a *Animation) ID() AnimationID {
	return a.id
}

// Cancel stops the animation.
func (a *Animation) Cancel() {
	a.cancelled.Store(true)
}

// IsCancelled returns whether the animation was cancelled.
func (a *Animation) IsCancelled() bool {
	return a.cancelled.Load()
}

// AnimationRegistry manages active animations and determines when 60 FPS mode is needed.
type AnimationRegistry struct {
	mu         sync.RWMutex
	animations map[AnimationID]*Animation

	// Callback when animation state changes (for loop to know when to switch modes)
	onActiveChange func(hasActive bool)
}

// NewAnimationRegistry creates a new animation registry.
func NewAnimationRegistry() *AnimationRegistry {
	return &AnimationRegistry{
		animations: make(map[AnimationID]*Animation),
	}
}

// OnActiveChange sets the callback for when animations become active/inactive.
func (r *AnimationRegistry) OnActiveChange(fn func(hasActive bool)) {
	r.mu.Lock()
	r.onActiveChange = fn
	r.mu.Unlock()
}

// Add registers a new animation.
func (r *AnimationRegistry) Add(anim *Animation) {
	r.mu.Lock()
	wasEmpty := len(r.animations) == 0
	r.animations[anim.id] = anim
	callback := r.onActiveChange
	r.mu.Unlock()

	// Notify if we went from no animations to having animations
	if wasEmpty && callback != nil {
		callback(true)
	}
}

// Remove unregisters an animation.
func (r *AnimationRegistry) Remove(id AnimationID) {
	r.mu.Lock()
	delete(r.animations, id)
	isEmpty := len(r.animations) == 0
	callback := r.onActiveChange
	r.mu.Unlock()

	// Notify if we went from having animations to none
	if isEmpty && callback != nil {
		callback(false)
	}
}

// HasActive returns true if there are any running animations.
func (r *AnimationRegistry) HasActive() bool {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.animations) > 0
}

// Count returns the number of active animations.
func (r *AnimationRegistry) Count() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.animations)
}

// Tick updates all animations and removes completed ones.
// Called once per frame by the loop. Returns true if any animations are still active.
func (r *AnimationRegistry) Tick(now time.Time) bool {
	r.mu.Lock()

	var toRemove []AnimationID
	var toComplete []*Animation

	for id, anim := range r.animations {
		if anim.cancelled.Load() {
			toRemove = append(toRemove, id)
			continue
		}

		elapsed := now.Sub(anim.startTime)

		if elapsed >= anim.duration {
			if anim.loop {
				// Reset for next loop iteration
				anim.startTime = now
				elapsed = 0
			} else {
				// Animation complete
				toRemove = append(toRemove, id)
				toComplete = append(toComplete, anim)
				// Final update at 100%
				if anim.update != nil {
					progress := anim.easing(1.0)
					anim.update(progress)
				}
				continue
			}
		}

		// Calculate progress and apply easing
		t := float64(elapsed) / float64(anim.duration)
		if t > 1 {
			t = 1
		}
		progress := anim.easing(t)

		// Update the animation
		if anim.update != nil {
			anim.update(progress)
		}
	}

	// Remove completed animations
	for _, id := range toRemove {
		delete(r.animations, id)
	}

	hasActive := len(r.animations) > 0
	callback := r.onActiveChange
	r.mu.Unlock()

	// Call completion callbacks outside the lock
	for _, anim := range toComplete {
		if anim.onComplete != nil {
			anim.onComplete()
		}
	}

	// Notify if all animations finished
	if len(toRemove) > 0 && !hasActive && callback != nil {
		callback(false)
	}

	return hasActive
}

// ============================================================================
// Animation Builder API
// ============================================================================

// AnimationBuilder provides a fluent API for creating animations.
type AnimationBuilder struct {
	widget     *Widget
	registry   *AnimationRegistry
	duration   time.Duration
	easing     EasingFunc
	loop       bool
	onComplete func()
}

// Animate starts building an animation for this widget.
// The widget must be associated with a loop that has an animation registry.
func (w *Widget) Animate(registry *AnimationRegistry) *AnimationBuilder {
	return &AnimationBuilder{
		widget:   w,
		registry: registry,
		duration: 300 * time.Millisecond, // Default duration
		easing:   EaseOutCubic,           // Default easing (smooth UI feel)
	}
}

// Duration sets how long the animation runs.
func (b *AnimationBuilder) Duration(d time.Duration) *AnimationBuilder {
	b.duration = d
	return b
}

// Easing sets the easing function.
func (b *AnimationBuilder) Easing(fn EasingFunc) *AnimationBuilder {
	b.easing = fn
	return b
}

// Loop makes the animation repeat forever until cancelled.
func (b *AnimationBuilder) Loop() *AnimationBuilder {
	b.loop = true
	return b
}

// OnComplete sets a callback for when the animation finishes.
func (b *AnimationBuilder) OnComplete(fn func()) *AnimationBuilder {
	b.onComplete = fn
	return b
}

// Color animates the background color from current to target.
func (b *AnimationBuilder) Color(to uint32) *Animation {
	from := uint32(0)
	b.widget.mu.RLock()
	if b.widget.backgroundColor != nil {
		from = *b.widget.backgroundColor
	}
	b.widget.mu.RUnlock()

	return b.ColorFromTo(from, to)
}

// ColorFromTo animates background color between two values.
func (b *AnimationBuilder) ColorFromTo(from, to uint32) *Animation {
	anim := &Animation{
		id:         newAnimationID(),
		widget:     b.widget,
		startTime:  time.Now(),
		duration:   b.duration,
		easing:     b.easing,
		loop:       b.loop,
		onComplete: b.onComplete,
		update: func(progress float64) {
			color := lerpColor(from, to, progress)
			b.widget.SetBackgroundColor(color)
		},
	}

	b.registry.Add(anim)
	return anim
}

// Opacity animates opacity from current to target (0.0 - 1.0).
func (b *AnimationBuilder) Opacity(to float32) *Animation {
	b.widget.mu.RLock()
	from := b.widget.opacity
	b.widget.mu.RUnlock()

	return b.OpacityFromTo(from, to)
}

// OpacityFromTo animates opacity between two values.
func (b *AnimationBuilder) OpacityFromTo(from, to float32) *Animation {
	anim := &Animation{
		id:         newAnimationID(),
		widget:     b.widget,
		startTime:  time.Now(),
		duration:   b.duration,
		easing:     b.easing,
		loop:       b.loop,
		onComplete: b.onComplete,
		update: func(progress float64) {
			value := lerp(from, to, float32(progress))
			b.widget.SetOpacity(value)
		},
	}

	b.registry.Add(anim)
	return anim
}

// Size animates width and height from current to target.
func (b *AnimationBuilder) Size(toW, toH float32) *Animation {
	b.widget.mu.RLock()
	fromW, fromH := b.widget.width, b.widget.height
	b.widget.mu.RUnlock()

	return b.SizeFromTo(fromW, fromH, toW, toH)
}

// SizeFromTo animates size between two values.
func (b *AnimationBuilder) SizeFromTo(fromW, fromH, toW, toH float32) *Animation {
	anim := &Animation{
		id:         newAnimationID(),
		widget:     b.widget,
		startTime:  time.Now(),
		duration:   b.duration,
		easing:     b.easing,
		loop:       b.loop,
		onComplete: b.onComplete,
		update: func(progress float64) {
			w := lerp(fromW, toW, float32(progress))
			h := lerp(fromH, toH, float32(progress))
			b.widget.SetSize(w, h)
		},
	}

	b.registry.Add(anim)
	return anim
}

// Position animates x and y from current to target.
func (b *AnimationBuilder) Position(toX, toY float32) *Animation {
	b.widget.mu.RLock()
	fromX, fromY := b.widget.x, b.widget.y
	b.widget.mu.RUnlock()

	return b.PositionFromTo(fromX, fromY, toX, toY)
}

// PositionFromTo animates position between two values.
func (b *AnimationBuilder) PositionFromTo(fromX, fromY, toX, toY float32) *Animation {
	anim := &Animation{
		id:         newAnimationID(),
		widget:     b.widget,
		startTime:  time.Now(),
		duration:   b.duration,
		easing:     b.easing,
		loop:       b.loop,
		onComplete: b.onComplete,
		update: func(progress float64) {
			x := lerp(fromX, toX, float32(progress))
			y := lerp(fromY, toY, float32(progress))
			b.widget.SetPosition(x, y)
		},
	}

	b.registry.Add(anim)
	return anim
}

// Custom creates an animation with a custom update function.
// The update function receives progress from 0-1.
func (b *AnimationBuilder) Custom(update func(progress float64)) *Animation {
	anim := &Animation{
		id:         newAnimationID(),
		widget:     b.widget,
		startTime:  time.Now(),
		duration:   b.duration,
		easing:     b.easing,
		loop:       b.loop,
		onComplete: b.onComplete,
		update:     update,
	}

	b.registry.Add(anim)
	return anim
}

// ============================================================================
// Helper Functions
// ============================================================================

// lerp linearly interpolates between two float32 values.
func lerp(a, b, t float32) float32 {
	return a + (b-a)*t
}

// lerpColor linearly interpolates between two RGBA colors.
func lerpColor(from, to uint32, t float64) uint32 {
	fromR := uint8((from >> 24) & 0xFF)
	fromG := uint8((from >> 16) & 0xFF)
	fromB := uint8((from >> 8) & 0xFF)
	fromA := uint8(from & 0xFF)

	toR := uint8((to >> 24) & 0xFF)
	toG := uint8((to >> 16) & 0xFF)
	toB := uint8((to >> 8) & 0xFF)
	toA := uint8(to & 0xFF)

	r := uint8(float64(fromR) + (float64(toR)-float64(fromR))*t)
	g := uint8(float64(fromG) + (float64(toG)-float64(fromG))*t)
	b := uint8(float64(fromB) + (float64(toB)-float64(fromB))*t)
	a := uint8(float64(fromA) + (float64(toA)-float64(fromA))*t)

	return uint32(r)<<24 | uint32(g)<<16 | uint32(b)<<8 | uint32(a)
}

// ============================================================================
// Predefined Animations (Tailwind-style)
// ============================================================================

// AnimationConfig holds custom configuration for predefined animations.
// Zero values mean "use default".
type AnimationConfig struct {
	Duration   float32 // Duration in milliseconds (0 = use default)
	Easing     string  // Easing function name (empty = use default)
	Iterations int     // Number of iterations (-1 = infinite, 0 = use default, >0 = N times)
}

// EasingByName returns the easing function for a given name.
// Returns nil if the name is unknown.
func EasingByName(name string) EasingFunc {
	switch name {
	case "linear":
		return EaseLinear
	case "ease-in":
		return EaseInQuad
	case "ease-out":
		return EaseOutQuad
	case "ease", "ease-in-out":
		return EaseInOutQuad
	case "cubic":
		return EaseInOutCubic
	case "back":
		return EaseOutBack
	case "elastic":
		return EaseOutElastic
	case "bounce":
		return EaseOutBounce
	default:
		return nil
	}
}

// StartPredefinedAnimation starts a predefined animation based on its name.
// Used internally by the Loop to start animations from animate-* classes.
// Returns the animation if started, nil if the animation name is unknown.
func StartPredefinedAnimation(w *Widget, registry *AnimationRegistry, name string) *Animation {
	return StartPredefinedAnimationWithConfig(w, registry, name, AnimationConfig{})
}

// StartPredefinedAnimationWithConfig starts a predefined animation with custom configuration.
// The config allows overriding duration, easing, and iteration count.
func StartPredefinedAnimationWithConfig(w *Widget, registry *AnimationRegistry, name string, cfg AnimationConfig) *Animation {
	switch name {
	case "pulse":
		return startPulseAnimation(w, registry, cfg)
	case "bounce":
		return startBounceAnimation(w, registry, cfg)
	case "spin":
		return startSpinAnimation(w, registry, cfg)
	case "ping":
		return startPingAnimation(w, registry, cfg)
	default:
		return nil
	}
}

// applyAnimationConfig applies custom config to an animation builder.
// Returns the builder for method chaining.
func applyAnimationConfig(builder *AnimationBuilder, cfg AnimationConfig, defaultDuration time.Duration, defaultEasing EasingFunc, defaultLoop bool) *AnimationBuilder {
	// Duration: use custom or default
	if cfg.Duration > 0 {
		builder.Duration(time.Duration(cfg.Duration) * time.Millisecond)
	} else {
		builder.Duration(defaultDuration)
	}

	// Easing: use custom or default
	if cfg.Easing != "" {
		if easing := EasingByName(cfg.Easing); easing != nil {
			builder.Easing(easing)
		} else {
			builder.Easing(defaultEasing)
		}
	} else {
		builder.Easing(defaultEasing)
	}

	// Iterations: -1 = infinite (explicit), 0 = use default, >0 = N times (finite)
	// Note: Tailwind animations default to infinite looping (loading indicators, etc.)
	if cfg.Iterations == -1 {
		// Explicit infinite
		builder.Loop()
	} else if cfg.Iterations == 0 {
		// Use default - which is infinite for Tailwind animations
		if defaultLoop {
			builder.Loop()
		}
	} else if cfg.Iterations > 0 {
		// Finite iterations requested - run once (no loop)
		// TODO: Implement proper repeat count tracking in Animation struct
		// For now, iterations > 0 means "run once" which is reasonable for "animate-[pulse_1s_1]"
	}

	return builder
}

// startPulseAnimation creates a pulsing opacity animation (Tailwind's animate-pulse).
// Fades opacity from 1 to 0.5 and back, like a heartbeat.
func startPulseAnimation(w *Widget, registry *AnimationRegistry, cfg AnimationConfig) *Animation {
	builder := w.Animate(registry)
	applyAnimationConfig(builder, cfg, 2*time.Second, EaseInOutCubic, true)

	return builder.Custom(func(progress float64) {
		// Pulse: 1 -> 0.5 -> 1 using a sine-like curve
		// progress goes 0->1, we want opacity to go 1->0.5->1
		opacity := float32(1.0 - 0.5*(1.0-math.Cos(progress*2*math.Pi))/2)
		w.SetOpacity(opacity)
	})
}

// startBounceAnimation creates a bouncing animation (Tailwind's animate-bounce).
// Moves the widget up and down with a bounce effect.
func startBounceAnimation(w *Widget, registry *AnimationRegistry, cfg AnimationConfig) *Animation {
	w.mu.RLock()
	baseY := w.y
	w.mu.RUnlock()

	builder := w.Animate(registry)
	applyAnimationConfig(builder, cfg, 1*time.Second, EaseLinear, true)

	return builder.Custom(func(progress float64) {
		// Tailwind bounce: jumps up, then bounces with decreasing amplitude
		// Simplified version: sine wave with bounce at the bottom
		var offsetY float32
		if progress < 0.5 {
			// Going up (ease out)
			t := progress * 2
			offsetY = -25 * float32(t*(2-t))
		} else {
			// Coming down with a little bounce
			t := (progress - 0.5) * 2
			offsetY = -25 * float32(1-t*t)
		}
		w.mu.Lock()
		w.y = baseY + offsetY
		w.dirtyMask |= DirtyPosition
		w.mu.Unlock()
		if w.tree != nil {
			w.tree.notifyUpdate(w, DirtyPosition)
		}
	})
}

// startSpinAnimation creates a spinning animation (Tailwind's animate-spin).
// Rotates the widget 360 degrees continuously.
func startSpinAnimation(w *Widget, registry *AnimationRegistry, cfg AnimationConfig) *Animation {
	builder := w.Animate(registry)
	applyAnimationConfig(builder, cfg, 1*time.Second, EaseLinear, true)

	return builder.Custom(func(progress float64) {
		// Full 360-degree rotation over the duration
		rotation := float32(progress * 2 * math.Pi)
		w.SetRotation(rotation)
	})
}

// startPingAnimation creates a ping/ripple animation (Tailwind's animate-ping).
// Creates a ripple effect by scaling up and fading out, then resetting.
func startPingAnimation(w *Widget, registry *AnimationRegistry, cfg AnimationConfig) *Animation {
	w.mu.RLock()
	baseW, baseH := w.width, w.height
	w.mu.RUnlock()

	builder := w.Animate(registry)
	applyAnimationConfig(builder, cfg, 1*time.Second, EaseOutCubic, true)

	return builder.Custom(func(progress float64) {
		// Scale up to 2x while fading out
		scale := float32(1.0 + progress)
		opacity := float32(1.0 - progress)

		w.mu.Lock()
		w.width = baseW * scale
		w.height = baseH * scale
		w.opacity = opacity
		w.dirtyMask |= DirtySize | DirtyOpacity
		w.mu.Unlock()

		if w.tree != nil {
			w.tree.notifyUpdate(w, DirtySize|DirtyOpacity)
		}
	})
}

// clamp restricts a value to a range.
func clamp(v, min, max float64) float64 {
	if v < min {
		return min
	}
	if v > max {
		return max
	}
	return v
}
