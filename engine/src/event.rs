//! Event system - platform-agnostic event handling
//!
//! Events flow from platform -> Rust -> Go
//! Rust handles hit testing and event routing
//! Go handles application logic

use crate::widget::WidgetId;
use serde::{Deserialize, Serialize};

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// Keyboard key (simplified, will expand later)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Enter,
    Escape,
    Backspace,
    Tab,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Shift,
    Control,
    Alt,
    Meta,
    Unknown,
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Platform-agnostic event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// Mouse moved to position (x, y)
    MouseMove {
        x: f32,
        y: f32,
        widget: Option<WidgetId>,
    },

    /// Mouse button pressed
    MouseDown {
        x: f32,
        y: f32,
        button: MouseButton,
        widget: Option<WidgetId>,
    },

    /// Mouse button released
    MouseUp {
        x: f32,
        y: f32,
        button: MouseButton,
        widget: Option<WidgetId>,
    },

    /// Mouse wheel scrolled
    MouseWheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        widget: Option<WidgetId>,
    },

    /// Key pressed
    KeyDown {
        key: Key,
        modifiers: Modifiers,
    },

    /// Key released
    KeyUp {
        key: Key,
        modifiers: Modifiers,
    },

    /// Text input (for composed characters, emoji, etc.)
    TextInput {
        text: String,
    },

    /// Widget gained focus
    FocusGained {
        widget: WidgetId,
    },

    /// Widget lost focus
    FocusLost {
        widget: WidgetId,
    },

    /// Window resized
    WindowResize {
        width: u32,
        height: u32,
    },

    /// Window close requested
    WindowClose,

    /// Application should quit
    Quit,
}

/// Batch of events (sent in a single FFI call)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventBatch {
    pub events: Vec<Event>,
    pub frame_number: u64,
}

impl EventBatch {
    pub fn new(frame_number: u64) -> Self {
        Self {
            events: Vec::new(),
            frame_number,
        }
    }

    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Event dispatcher - handles event routing and hit testing
pub struct EventDispatcher {
    /// Current event batch
    current_batch: EventBatch,
    /// Frame counter
    frame_number: u64,
    /// Currently hovered widget
    hovered_widget: Option<WidgetId>,
    /// Currently focused widget
    focused_widget: Option<WidgetId>,
    /// Widget being pressed (for click detection)
    pressed_widget: Option<WidgetId>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            current_batch: EventBatch::new(0),
            frame_number: 0,
            hovered_widget: None,
            focused_widget: None,
            pressed_widget: None,
        }
    }

    /// Start a new frame
    pub fn begin_frame(&mut self) {
        self.frame_number += 1;
        self.current_batch = EventBatch::new(self.frame_number);
    }

    /// Add an event to the current batch
    pub fn push_event(&mut self, event: Event) {
        // Update internal state based on event
        match &event {
            Event::MouseMove { widget, .. } => {
                if self.hovered_widget != *widget {
                    self.hovered_widget = *widget;
                }
            }
            Event::MouseDown { widget, .. } => {
                self.pressed_widget = *widget;
            }
            Event::MouseUp { widget, .. } => {
                // Generate click event if released on same widget
                if self.pressed_widget == *widget && widget.is_some() {
                    // Click is implicit from MouseDown + MouseUp on same widget
                }
                self.pressed_widget = None;
            }
            Event::FocusGained { widget } => {
                self.focused_widget = Some(*widget);
            }
            Event::FocusLost { .. } => {
                self.focused_widget = None;
            }
            _ => {}
        }

        self.current_batch.push(event);
    }

    /// Get the current event batch (for sending to Go)
    pub fn current_batch(&self) -> &EventBatch {
        &self.current_batch
    }

    /// Take the current batch and reset
    pub fn take_batch(&mut self) -> EventBatch {
        let batch = std::mem::replace(
            &mut self.current_batch,
            EventBatch::new(self.frame_number),
        );
        batch
    }

    /// Get currently hovered widget
    pub fn hovered_widget(&self) -> Option<WidgetId> {
        self.hovered_widget
    }

    /// Get currently focused widget
    pub fn focused_widget(&self) -> Option<WidgetId> {
        self.focused_widget
    }

    /// Set focused widget
    pub fn set_focused_widget(&mut self, widget: Option<WidgetId>) {
        if self.focused_widget != widget {
            if let Some(old_widget) = self.focused_widget {
                self.push_event(Event::FocusLost { widget: old_widget });
            }
            if let Some(new_widget) = widget {
                self.push_event(Event::FocusGained { widget: new_widget });
            }
            self.focused_widget = widget;
        }
    }

    /// Current frame number
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_batch() {
        let mut batch = EventBatch::new(1);
        assert!(batch.is_empty());

        batch.push(Event::WindowClose);
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn test_event_dispatcher() {
        let mut dispatcher = EventDispatcher::new();
        dispatcher.begin_frame();

        dispatcher.push_event(Event::WindowClose);
        assert_eq!(dispatcher.current_batch().len(), 1);

        let batch = dispatcher.take_batch();
        assert_eq!(batch.len(), 1);
        assert_eq!(dispatcher.current_batch().len(), 0);
    }

    #[test]
    fn test_focus_tracking() {
        let mut dispatcher = EventDispatcher::new();
        assert_eq!(dispatcher.focused_widget(), None);

        let widget_id = WidgetId::from(slotmap::KeyData::from_ffi(1));
        dispatcher.set_focused_widget(Some(widget_id));
        assert_eq!(dispatcher.focused_widget(), Some(widget_id));
    }
}
