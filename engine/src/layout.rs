//! Layout engine - flexbox and grid layout calculations
//!
//! Performance considerations:
//! - Uses euclid for SIMD-friendly geometry calculations
//! - Dirty tracking to avoid unnecessary recalculations
//! - Cache-friendly data structures

use euclid::{Point2D, Rect, Size2D};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};

/// Unit type for layout space
pub struct LayoutSpace;

/// Type aliases for clarity
pub type LayoutPoint = Point2D<f32, LayoutSpace>;
pub type LayoutSize = Size2D<f32, LayoutSpace>;
pub type LayoutRect = Rect<f32, LayoutSpace>;

new_key_type! {
    /// Unique identifier for layout nodes
    pub struct LayoutNodeId;
}

/// Layout algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutAlgorithm {
    /// Flexbox layout
    Flex,
    /// Grid layout
    Grid,
    /// Absolute positioning
    Absolute,
    /// Block layout
    Block,
}

/// Flex direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// Flex wrap behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Justify content alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Align items alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    Baseline,
}

/// Dimension constraint
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Dimension {
    /// Undefined/auto
    Auto,
    /// Fixed size in pixels
    Points(f32),
    /// Percentage of parent
    Percent(f32),
}

impl Default for Dimension {
    fn default() -> Self {
        Self::Auto
    }
}

/// Layout constraints for a node
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct LayoutConstraints {
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,

    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,

    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
}

/// Computed layout result for a node
#[derive(Debug, Clone, Copy, Default)]
pub struct ComputedLayout {
    /// Position relative to parent
    pub position: LayoutPoint,
    /// Computed size
    pub size: LayoutSize,
    /// Content size (excluding padding/border)
    pub content_size: LayoutSize,
    /// Whether this layout is dirty and needs recalculation
    pub dirty: bool,
}

/// Layout node in the tree
pub struct LayoutNode {
    /// Parent node
    pub parent: Option<LayoutNodeId>,
    /// Child nodes
    pub children: Vec<LayoutNodeId>,
    /// Layout algorithm
    pub algorithm: LayoutAlgorithm,
    /// Constraints
    pub constraints: LayoutConstraints,
    /// Computed layout
    pub computed: ComputedLayout,

    // Flexbox-specific properties
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self {
            parent: None,
            children: Vec::new(),
            algorithm: LayoutAlgorithm::Flex,
            constraints: LayoutConstraints::default(),
            computed: ComputedLayout::default(),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
        }
    }
}

/// Main layout engine
pub struct LayoutEngine {
    /// Layout node storage using slotmap for cache-friendly access
    nodes: SlotMap<LayoutNodeId, LayoutNode>,
    /// Root node ID
    root: Option<LayoutNodeId>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            root: None,
        }
    }

    /// Create a new layout node
    pub fn create_node(&mut self) -> LayoutNodeId {
        self.nodes.insert(LayoutNode::default())
    }

    /// Get a node by ID
    pub fn get_node(&self, id: LayoutNodeId) -> Option<&LayoutNode> {
        self.nodes.get(id)
    }

    /// Get a mutable node by ID
    pub fn get_node_mut(&mut self, id: LayoutNodeId) -> Option<&mut LayoutNode> {
        self.nodes.get_mut(id)
    }

    /// Set the root node
    pub fn set_root(&mut self, id: LayoutNodeId) {
        self.root = Some(id);
    }

    /// Mark a node as dirty (needs layout recalculation)
    pub fn mark_dirty(&mut self, id: LayoutNodeId) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.computed.dirty = true;

            // Mark all ancestors as dirty too
            if let Some(parent_id) = node.parent {
                self.mark_dirty(parent_id);
            }
        }
    }

    /// Calculate layout for the entire tree
    pub fn calculate_layout(&mut self, available_width: f32, available_height: f32) {
        if let Some(root_id) = self.root {
            self.calculate_node_layout(root_id, available_width, available_height);
        }
    }

    /// Calculate layout for a specific node (recursive)
    fn calculate_node_layout(&mut self, node_id: LayoutNodeId, available_width: f32, available_height: f32) {
        // Get the node (we'll need to be careful with borrowing)
        let node = match self.nodes.get(node_id) {
            Some(n) => n,
            None => return,
        };

        // Skip if not dirty
        if !node.computed.dirty {
            return;
        }

        let algorithm = node.algorithm;
        let children: Vec<LayoutNodeId> = node.children.clone();

        // Calculate based on algorithm
        match algorithm {
            LayoutAlgorithm::Flex => {
                self.calculate_flex_layout(node_id, available_width, available_height);
            }
            LayoutAlgorithm::Block => {
                self.calculate_block_layout(node_id, available_width, available_height);
            }
            LayoutAlgorithm::Absolute => {
                self.calculate_absolute_layout(node_id);
            }
            LayoutAlgorithm::Grid => {
                // TODO: Implement grid layout
                self.calculate_block_layout(node_id, available_width, available_height);
            }
        }

        // Calculate layout for children
        for child_id in children {
            if let Some(parent) = self.nodes.get(node_id) {
                let content_width = parent.computed.content_size.width;
                let content_height = parent.computed.content_size.height;
                self.calculate_node_layout(child_id, content_width, content_height);
            }
        }

        // Mark as clean
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.computed.dirty = false;
        }
    }

    fn calculate_flex_layout(&mut self, node_id: LayoutNodeId, available_width: f32, available_height: f32) {
        // Basic flex layout implementation
        // TODO: Full flexbox algorithm implementation
        let node = match self.nodes.get_mut(node_id) {
            Some(n) => n,
            None => return,
        };

        // Calculate node size
        let width = match node.constraints.width {
            Dimension::Points(w) => w,
            Dimension::Percent(p) => available_width * p / 100.0,
            Dimension::Auto => available_width,
        };

        let height = match node.constraints.height {
            Dimension::Points(h) => h,
            Dimension::Percent(p) => available_height * p / 100.0,
            Dimension::Auto => available_height,
        };

        node.computed.size = LayoutSize::new(width, height);
        node.computed.content_size = LayoutSize::new(
            width - node.constraints.padding_left - node.constraints.padding_right,
            height - node.constraints.padding_top - node.constraints.padding_bottom,
        );
    }

    fn calculate_block_layout(&mut self, node_id: LayoutNodeId, available_width: f32, available_height: f32) {
        // Basic block layout implementation
        let node = match self.nodes.get_mut(node_id) {
            Some(n) => n,
            None => return,
        };

        let width = match node.constraints.width {
            Dimension::Points(w) => w,
            Dimension::Percent(p) => available_width * p / 100.0,
            Dimension::Auto => available_width,
        };

        let height = match node.constraints.height {
            Dimension::Points(h) => h,
            Dimension::Percent(p) => available_height * p / 100.0,
            Dimension::Auto => 0.0, // Will be calculated based on content
        };

        node.computed.size = LayoutSize::new(width, height);
        node.computed.content_size = LayoutSize::new(
            width - node.constraints.padding_left - node.constraints.padding_right,
            height - node.constraints.padding_top - node.constraints.padding_bottom,
        );
    }

    fn calculate_absolute_layout(&mut self, node_id: LayoutNodeId) {
        // Absolute positioning
        let node = match self.nodes.get_mut(node_id) {
            Some(n) => n,
            None => return,
        };

        let width = match node.constraints.width {
            Dimension::Points(w) => w,
            Dimension::Auto => 0.0,
            _ => 0.0,
        };

        let height = match node.constraints.height {
            Dimension::Points(h) => h,
            Dimension::Auto => 0.0,
            _ => 0.0,
        };

        node.computed.size = LayoutSize::new(width, height);
        node.computed.content_size = node.computed.size;
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node() {
        let mut engine = LayoutEngine::new();
        let node_id = engine.create_node();
        assert!(engine.get_node(node_id).is_some());
    }

    #[test]
    fn test_mark_dirty() {
        let mut engine = LayoutEngine::new();
        let node_id = engine.create_node();
        engine.mark_dirty(node_id);
        let node = engine.get_node(node_id).unwrap();
        assert!(node.computed.dirty);
    }
}
