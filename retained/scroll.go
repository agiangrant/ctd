package retained

import (
	"time"
)

// ============================================================================
// Scroll Animation Utilities
// ============================================================================

// ScrollToConfig configures a scroll animation.
type ScrollToConfig struct {
	Duration   time.Duration // Animation duration (default: 250ms)
	Easing     EasingFunc    // Easing function (default: EaseOutCubic)
	Padding    float32       // Padding from edges when scrolling to widget (default: 20)
	OnComplete func()        // Called when animation completes
}

// DefaultScrollToConfig returns sensible defaults for scroll animations.
func DefaultScrollToConfig() ScrollToConfig {
	return ScrollToConfig{
		Duration: 250 * time.Millisecond,
		Easing:   EaseOutCubic,
		Padding:  20,
	}
}

// ScrollToY animates the scroll container to a specific scroll Y position.
// The widget must be a scrollable container.
func (b *AnimationBuilder) ScrollToY(targetY float32) *Animation {
	b.widget.mu.RLock()
	fromY := b.widget.scrollY
	b.widget.mu.RUnlock()

	return b.ScrollFromTo(fromY, targetY)
}

// ScrollFromTo animates scroll position between two values.
func (b *AnimationBuilder) ScrollFromTo(fromY, toY float32) *Animation {
	anim := &Animation{
		id:         newAnimationID(),
		widget:     b.widget,
		startTime:  time.Now(),
		duration:   b.duration,
		easing:     b.easing,
		loop:       b.loop,
		onComplete: b.onComplete,
		update: func(progress float64) {
			value := lerp(fromY, toY, float32(progress))
			b.widget.mu.Lock()
			b.widget.scrollY = value
			b.widget.dirtyMask |= DirtyScroll
			b.widget.mu.Unlock()

			if b.widget.tree != nil {
				b.widget.tree.notifyUpdate(b.widget, DirtyScroll)
			}
		},
	}

	b.registry.Add(anim)
	return anim
}

// ScrollToWidget animates scrolling to make a target widget visible within
// this scroll container. Returns nil if the widget is not a descendant or
// no scrolling is needed.
func ScrollToWidget(scrollContainer *Widget, target *Widget, registry *AnimationRegistry, cfg ScrollToConfig) *Animation {
	if scrollContainer == nil || target == nil || registry == nil {
		return nil
	}

	// Apply defaults
	if cfg.Duration == 0 {
		cfg.Duration = 250 * time.Millisecond
	}
	if cfg.Easing == nil {
		cfg.Easing = EaseOutCubic
	}
	if cfg.Padding == 0 {
		cfg.Padding = 20
	}

	// Calculate target scroll position
	targetScrollY, needsScroll := calculateScrollToWidget(scrollContainer, target, cfg.Padding, 0)
	if !needsScroll {
		return nil
	}

	builder := scrollContainer.Animate(registry).
		Duration(cfg.Duration).
		Easing(cfg.Easing)

	if cfg.OnComplete != nil {
		builder = builder.OnComplete(cfg.OnComplete)
	}

	return builder.ScrollToY(targetScrollY)
}

// ScrollToWidgetWithKeyboard is like ScrollToWidget but accounts for keyboard height.
// Use this for keyboard avoidance when a text input gains focus.
func ScrollToWidgetWithKeyboard(scrollContainer *Widget, target *Widget, registry *AnimationRegistry, cfg ScrollToConfig, keyboardHeight float32) *Animation {
	if scrollContainer == nil || target == nil || registry == nil {
		return nil
	}

	// Apply defaults
	if cfg.Duration == 0 {
		cfg.Duration = 250 * time.Millisecond
	}
	if cfg.Easing == nil {
		cfg.Easing = EaseOutCubic
	}
	if cfg.Padding == 0 {
		cfg.Padding = 20
	}

	// Calculate target scroll position accounting for keyboard
	targetScrollY, needsScroll := calculateScrollToWidget(scrollContainer, target, cfg.Padding, keyboardHeight)
	if !needsScroll {
		return nil
	}

	builder := scrollContainer.Animate(registry).
		Duration(cfg.Duration).
		Easing(cfg.Easing)

	if cfg.OnComplete != nil {
		builder = builder.OnComplete(cfg.OnComplete)
	}

	return builder.ScrollToY(targetScrollY)
}

// calculateScrollToWidget calculates the scroll position needed to make a widget visible.
// Returns the target scrollY and whether scrolling is needed.
func calculateScrollToWidget(scrollContainer *Widget, target *Widget, padding float32, keyboardHeight float32) (float32, bool) {
	// Get bounds
	targetBounds := target.ComputedBounds()
	containerBounds := scrollContainer.ComputedBounds()

	// Get current scroll position
	scrollContainer.mu.RLock()
	currentScrollY := scrollContainer.scrollY
	scrollContainer.mu.RUnlock()

	// Widget's position in container's content coordinate system
	// (ComputedBounds returns layout position, not screen position)
	widgetContentY := targetBounds.Y - containerBounds.Y
	widgetContentBottom := widgetContentY + targetBounds.Height

	// Calculate visible height (accounting for keyboard)
	visibleHeight := containerBounds.Height - keyboardHeight
	if visibleHeight < 0 {
		visibleHeight = 0
	}

	// Visible content area
	visibleContentTop := currentScrollY + padding
	visibleContentBottom := currentScrollY + visibleHeight - padding

	// Check if widget is already fully visible
	if widgetContentY >= visibleContentTop && widgetContentBottom <= visibleContentBottom {
		return currentScrollY, false
	}

	// Calculate target scroll position
	var targetScrollY float32

	if widgetContentBottom > visibleContentBottom {
		// Widget bottom is below visible area - scroll to bring it up
		targetScrollY = widgetContentBottom - visibleHeight + padding

		// Don't scroll so much that widget top goes above visible area
		maxScrollY := widgetContentY - padding
		if targetScrollY > maxScrollY {
			targetScrollY = maxScrollY
		}
	} else if widgetContentY < visibleContentTop {
		// Widget top is above visible area - scroll to bring it down
		targetScrollY = widgetContentY - padding
	} else {
		return currentScrollY, false
	}

	// Clamp to valid range
	if targetScrollY < 0 {
		targetScrollY = 0
	}

	return targetScrollY, true
}
