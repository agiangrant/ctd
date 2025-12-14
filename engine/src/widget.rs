//! Widget system - mode-agnostic widget tree management
//!
//! Performance considerations:
//! - SlotMap for O(1) lookups and cache-friendly iteration
//! - Minimal allocations during tree traversal
//! - Efficient dirty tracking for retained mode

use crate::layout::LayoutNodeId;
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};

new_key_type! {
    /// Unique identifier for widgets
    pub struct WidgetId;
}

/// Widget type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetKind {
    /// Container widgets
    VStack,
    HStack,
    ZStack,
    Container,
    ScrollView,

    /// Text widgets
    Text,
    Heading,
    Label,

    /// Input widgets
    Button,
    TextField,
    TextArea,
    Checkbox,
    Radio,
    Slider,

    /// Custom widget (for game or app-specific widgets)
    Custom(String),
}

/// Widget state flags
#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetState {
    pub hovered: bool,
    pub focused: bool,
    pub active: bool,
    pub disabled: bool,
    pub visible: bool,
}

impl WidgetState {
    pub fn new() -> Self {
        Self {
            hovered: false,
            focused: false,
            active: false,
            disabled: false,
            visible: true,
        }
    }
}

/// Widget data (properties that can be set from Go)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetData {
    /// Widget type
    pub kind: WidgetKind,
    /// Class string for styling
    pub classes: String,
    /// Text content (for text widgets)
    pub text: Option<String>,
    /// Custom data (JSON blob for app-specific data)
    pub custom_data: Option<String>,
}

/// Widget node in the tree
pub struct Widget {
    /// Widget data
    pub data: WidgetData,
    /// Parent widget
    pub parent: Option<WidgetId>,
    /// Child widgets
    pub children: Vec<WidgetId>,
    /// Associated layout node
    pub layout_node: Option<LayoutNodeId>,
    /// Widget state
    pub state: WidgetState,
    /// Dirty flag (needs re-render)
    pub dirty: bool,
    /// Generation counter for change detection
    pub generation: u64,
}

impl Widget {
    pub fn new(kind: WidgetKind) -> Self {
        Self {
            data: WidgetData {
                kind,
                classes: String::new(),
                text: None,
                custom_data: None,
            },
            parent: None,
            children: Vec::new(),
            layout_node: None,
            state: WidgetState::new(),
            dirty: true,
            generation: 0,
        }
    }

    /// Mark this widget as dirty
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.generation += 1;
    }
}

/// Widget tree - central data structure for the widget system
pub struct WidgetTree {
    /// Widget storage
    widgets: SlotMap<WidgetId, Widget>,
    /// Root widget
    root: Option<WidgetId>,
    /// Current generation (for change tracking)
    generation: u64,
}

impl WidgetTree {
    pub fn new() -> Self {
        Self {
            widgets: SlotMap::with_key(),
            root: None,
            generation: 0,
        }
    }

    /// Create a new widget
    pub fn create_widget(&mut self, kind: WidgetKind) -> WidgetId {
        self.widgets.insert(Widget::new(kind))
    }

    /// Get a widget by ID
    pub fn get_widget(&self, id: WidgetId) -> Option<&Widget> {
        self.widgets.get(id)
    }

    /// Get a mutable widget by ID
    pub fn get_widget_mut(&mut self, id: WidgetId) -> Option<&mut Widget> {
        self.widgets.get_mut(id)
    }

    /// Remove a widget and all its children
    pub fn remove_widget(&mut self, id: WidgetId) {
        // Collect children first to avoid borrow issues
        let children = self.widgets.get(id)
            .map(|w| w.children.clone())
            .unwrap_or_default();

        // Recursively remove children
        for child_id in children {
            self.remove_widget(child_id);
        }

        // Remove the widget itself
        self.widgets.remove(id);
    }

    /// Add a child widget to a parent
    pub fn add_child(&mut self, parent_id: WidgetId, child_id: WidgetId) {
        // Set parent on child
        if let Some(child) = self.widgets.get_mut(child_id) {
            child.parent = Some(parent_id);
        }

        // Add to parent's children
        if let Some(parent) = self.widgets.get_mut(parent_id) {
            parent.children.push(child_id);
            parent.mark_dirty();
        }
    }

    /// Remove a child from its parent
    pub fn remove_child(&mut self, parent_id: WidgetId, child_id: WidgetId) {
        if let Some(parent) = self.widgets.get_mut(parent_id) {
            parent.children.retain(|&id| id != child_id);
            parent.mark_dirty();
        }

        if let Some(child) = self.widgets.get_mut(child_id) {
            child.parent = None;
        }
    }

    /// Set the root widget
    pub fn set_root(&mut self, id: WidgetId) {
        self.root = Some(id);
    }

    /// Get the root widget ID
    pub fn root(&self) -> Option<WidgetId> {
        self.root
    }

    /// Mark a widget and all its ancestors as dirty
    pub fn mark_dirty(&mut self, id: WidgetId) {
        if let Some(widget) = self.widgets.get_mut(id) {
            widget.mark_dirty();

            // Mark parent as dirty too
            if let Some(parent_id) = widget.parent {
                self.mark_dirty(parent_id);
            }
        }
    }

    /// Clear the entire tree
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.root = None;
        self.generation += 1;
    }

    /// Increment generation counter
    pub fn increment_generation(&mut self) {
        self.generation += 1;
    }

    /// Get current generation
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Iterate over all widgets (depth-first)
    pub fn iter_depth_first(&self) -> DepthFirstIterator<'_> {
        DepthFirstIterator::new(self)
    }

    /// Count total widgets in tree
    pub fn widget_count(&self) -> usize {
        self.widgets.len()
    }
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Depth-first iterator for widget tree
pub struct DepthFirstIterator<'a> {
    tree: &'a WidgetTree,
    stack: Vec<WidgetId>,
}

impl<'a> DepthFirstIterator<'a> {
    fn new(tree: &'a WidgetTree) -> Self {
        let stack = tree.root.into_iter().collect();
        Self { tree, stack }
    }
}

impl<'a> Iterator for DepthFirstIterator<'a> {
    type Item = (WidgetId, &'a Widget);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let widget = self.tree.widgets.get(id)?;

        // Push children onto stack (in reverse order for left-to-right traversal)
        for &child_id in widget.children.iter().rev() {
            self.stack.push(child_id);
        }

        Some((id, widget))
    }
}

/// Delta update for retained mode (only changed widgets)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDelta {
    /// Widgets to add or update
    pub updates: Vec<(WidgetId, WidgetData)>,
    /// Widgets to remove
    pub removals: Vec<WidgetId>,
    /// New parent-child relationships
    pub reparenting: Vec<(WidgetId, WidgetId)>,
}

impl WidgetDelta {
    pub fn new() -> Self {
        Self {
            updates: Vec::new(),
            removals: Vec::new(),
            reparenting: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.updates.is_empty() && self.removals.is_empty() && self.reparenting.is_empty()
    }
}

impl Default for WidgetDelta {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_widget() {
        let mut tree = WidgetTree::new();
        let widget_id = tree.create_widget(WidgetKind::Button);
        assert!(tree.get_widget(widget_id).is_some());
    }

    #[test]
    fn test_parent_child_relationship() {
        let mut tree = WidgetTree::new();
        let parent_id = tree.create_widget(WidgetKind::VStack);
        let child_id = tree.create_widget(WidgetKind::Text);

        tree.add_child(parent_id, child_id);

        let parent = tree.get_widget(parent_id).unwrap();
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0], child_id);

        let child = tree.get_widget(child_id).unwrap();
        assert_eq!(child.parent, Some(parent_id));
    }

    #[test]
    fn test_remove_widget() {
        let mut tree = WidgetTree::new();
        let parent_id = tree.create_widget(WidgetKind::VStack);
        let child_id = tree.create_widget(WidgetKind::Text);

        tree.add_child(parent_id, child_id);
        tree.remove_widget(parent_id);

        assert!(tree.get_widget(parent_id).is_none());
        assert!(tree.get_widget(child_id).is_none());
    }

    #[test]
    fn test_depth_first_iteration() {
        let mut tree = WidgetTree::new();
        let root = tree.create_widget(WidgetKind::VStack);
        let child1 = tree.create_widget(WidgetKind::Text);
        let child2 = tree.create_widget(WidgetKind::Button);

        tree.set_root(root);
        tree.add_child(root, child1);
        tree.add_child(root, child2);

        let ids: Vec<WidgetId> = tree.iter_depth_first().map(|(id, _)| id).collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], root);
    }
}
