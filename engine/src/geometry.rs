//! Geometry generation for UI primitives
//!
//! This module generates vertices and indices for common UI shapes:
//! - Rectangles with optional rounded corners
//! - Borders (stroked rectangles)
//! - Circles and ellipses
//! - Lines with thickness
//! - Gradient fills (linear, radial)
//!
//! All geometry is generated in screen-space coordinates.

use crate::render::{Gradient, GradientStop, Vertex};
use std::f32::consts::PI;

/// Number of segments to use for each rounded corner
const CORNER_SEGMENTS: usize = 8;

/// Generate vertices and indices for a rectangle with optional rounded corners
///
/// # Arguments
/// * `x`, `y` - Top-left position in screen coordinates
/// * `width`, `height` - Size in pixels
/// * `color` - RGBA color as u32 (0xRRGGBBAA)
/// * `radii` - Corner radii [top-left, top-right, bottom-right, bottom-left]
///
/// # Returns
/// (vertices, indices) for rendering with DrawTriangles
pub fn rounded_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: u32,
    radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    // Clamp radii to half the smallest dimension
    let max_radius = (width.min(height)) / 2.0;
    let radii = [
        radii[0].min(max_radius),
        radii[1].min(max_radius),
        radii[2].min(max_radius),
        radii[3].min(max_radius),
    ];

    // Check if we have any rounded corners
    let has_rounded = radii.iter().any(|&r| r > 0.5);

    if !has_rounded {
        // Simple rectangle - 4 vertices, 2 triangles
        return simple_rect(x, y, width, height, color);
    }

    // Generate rounded rectangle
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Unpack color to RGBA floats
    let rgba = color_to_rgba(color);

    // Center point for fan triangulation
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let center_idx = 0u16;
    vertices.push(Vertex {
        position: [center_x, center_y, 0.0],
        texcoord: [0.5, 0.5],
        color: rgba,
    });

    // Generate corner arcs and edges
    // Going clockwise: top-left, top-right, bottom-right, bottom-left
    // In screen coordinates: Y increases downward, so angles work differently
    //
    // For each corner, we generate an arc from one edge direction to another:
    // - top-left: from pointing left (-x) to pointing up (-y), angles PI to PI/2
    // - top-right: from pointing up (-y) to pointing right (+x), angles PI/2 to 0
    // - bottom-right: from pointing right (+x) to pointing down (+y), angles 0 to -PI/2
    // - bottom-left: from pointing down (+y) to pointing left (-x), angles -PI/2 to -PI

    let corners = [
        // (corner_center_x, corner_center_y, start_angle, end_angle, radius)
        (x + radii[0], y + radii[0], PI, PI / 2.0, radii[0]),                    // top-left
        (x + width - radii[1], y + radii[1], PI / 2.0, 0.0, radii[1]),           // top-right
        (x + width - radii[2], y + height - radii[2], 0.0, -PI / 2.0, radii[2]), // bottom-right
        (x + radii[3], y + height - radii[3], -PI / 2.0, -PI, radii[3]),         // bottom-left
    ];

    // Corner positions for when radius is 0 (sharp corners)
    let sharp_corners = [
        (x, y),                         // top-left
        (x + width, y),                 // top-right
        (x + width, y + height),        // bottom-right
        (x, y + height),                // bottom-left
    ];

    for (corner_idx, &(cx, cy, start_angle, end_angle, radius)) in corners.iter().enumerate() {
        if radius > 0.5 {
            // Rounded corner - generate arc
            for i in 0..=CORNER_SEGMENTS {
                let t = i as f32 / CORNER_SEGMENTS as f32;
                let angle = start_angle + (end_angle - start_angle) * t;

                let px = cx + angle.cos() * radius;
                let py = cy - angle.sin() * radius;

                let u = (px - x) / width;
                let v = (py - y) / height;

                vertices.push(Vertex {
                    position: [px, py, 0.0],
                    texcoord: [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)],
                    color: rgba,
                });
            }

            // Create triangles from center to edge vertices (within corner arc)
            let start_vertex = 1 + corner_idx * (CORNER_SEGMENTS + 1);
            for i in 0..CORNER_SEGMENTS {
                let v1 = (start_vertex + i) as u16;
                let v2 = (start_vertex + i + 1) as u16;
                indices.push(center_idx);
                indices.push(v1);
                indices.push(v2);
            }
        } else {
            // Sharp corner - just one vertex at the corner point
            // But we still need CORNER_SEGMENTS + 1 vertices to keep indexing consistent
            let (sharp_x, sharp_y) = sharp_corners[corner_idx];
            let u = (sharp_x - x) / width;
            let v = (sharp_y - y) / height;

            for _ in 0..=CORNER_SEGMENTS {
                vertices.push(Vertex {
                    position: [sharp_x, sharp_y, 0.0],
                    texcoord: [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)],
                    color: rgba,
                });
            }
            // No triangles within sharp corner - all vertices are the same point
        }
    }

    // Connect corners with straight edges
    // Each corner has (CORNER_SEGMENTS + 1) vertices
    let verts_per_corner = CORNER_SEGMENTS + 1;

    // Helper to get vertex index at end of corner arc (last vertex of that corner)
    let corner_end = |corner: usize| -> u16 {
        (1 + corner * verts_per_corner + CORNER_SEGMENTS) as u16
    };

    // Helper to get vertex index at start of corner arc (first vertex of that corner)
    let corner_start = |corner: usize| -> u16 {
        (1 + corner * verts_per_corner) as u16
    };

    // Connect top-left end to top-right start (top edge)
    indices.push(center_idx);
    indices.push(corner_end(0));
    indices.push(corner_start(1));

    // Connect top-right end to bottom-right start (right edge)
    indices.push(center_idx);
    indices.push(corner_end(1));
    indices.push(corner_start(2));

    // Connect bottom-right end to bottom-left start (bottom edge)
    indices.push(center_idx);
    indices.push(corner_end(2));
    indices.push(corner_start(3));

    // Connect bottom-left end to top-left start (left edge)
    indices.push(center_idx);
    indices.push(corner_end(3));
    indices.push(corner_start(0));

    (vertices, indices)
}

/// Generate a simple rectangle without rounded corners
fn simple_rect(x: f32, y: f32, width: f32, height: f32, color: u32) -> (Vec<Vertex>, Vec<u16>) {
    let rgba = color_to_rgba(color);

    let vertices = vec![
        Vertex {
            position: [x, y, 0.0],
            texcoord: [0.0, 0.0],
            color: rgba,
        },
        Vertex {
            position: [x + width, y, 0.0],
            texcoord: [1.0, 0.0],
            color: rgba,
        },
        Vertex {
            position: [x, y + height, 0.0],
            texcoord: [0.0, 1.0],
            color: rgba,
        },
        Vertex {
            position: [x + width, y + height, 0.0],
            texcoord: [1.0, 1.0],
            color: rgba,
        },
    ];

    let indices = vec![0, 1, 2, 1, 3, 2];

    (vertices, indices)
}

/// Generate vertices and indices for a border (stroked rectangle)
///
/// # Arguments
/// * `x`, `y` - Top-left position
/// * `width`, `height` - Outer size
/// * `border_width` - Thickness of the border
/// * `color` - Border color
/// * `radii` - Corner radii [top-left, top-right, bottom-right, bottom-left]
pub fn border_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_width: f32,
    color: u32,
    radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    let rgba = color_to_rgba(color);

    // For now, generate as a simple frame (4 rectangles)
    // TODO: Proper rounded border with inner/outer arcs
    if radii.iter().all(|&r| r < 0.5) {
        return simple_border(x, y, width, height, border_width, rgba);
    }

    // For rounded borders, we need to generate inner and outer arcs
    rounded_border(x, y, width, height, border_width, rgba, radii)
}

/// Simple rectangular border (no rounded corners)
fn simple_border(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_width: f32,
    rgba: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let bw = border_width;

    // Top edge
    add_rect_vertices(&mut vertices, &mut indices, x, y, width, bw, rgba);
    // Bottom edge
    add_rect_vertices(&mut vertices, &mut indices, x, y + height - bw, width, bw, rgba);
    // Left edge (between top and bottom)
    add_rect_vertices(&mut vertices, &mut indices, x, y + bw, bw, height - 2.0 * bw, rgba);
    // Right edge (between top and bottom)
    add_rect_vertices(&mut vertices, &mut indices, x + width - bw, y + bw, bw, height - 2.0 * bw, rgba);

    (vertices, indices)
}

/// Rounded border with inner and outer arcs
fn rounded_border(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_width: f32,
    rgba: [f32; 4],
    radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let bw = border_width;
    let max_radius = (width.min(height)) / 2.0;
    let radii = [
        radii[0].min(max_radius),
        radii[1].min(max_radius),
        radii[2].min(max_radius),
        radii[3].min(max_radius),
    ];

    // Corner centers and angles - same as rounded_rect
    let corners = [
        (x + radii[0], y + radii[0], PI, PI / 2.0, radii[0]),                    // top-left
        (x + width - radii[1], y + radii[1], PI / 2.0, 0.0, radii[1]),           // top-right
        (x + width - radii[2], y + height - radii[2], 0.0, -PI / 2.0, radii[2]), // bottom-right
        (x + radii[3], y + height - radii[3], -PI / 2.0, -PI, radii[3]),         // bottom-left
    ];

    // Generate outer and inner vertices for each corner
    for (corner_idx, &(cx, cy, start_angle, end_angle, outer_radius)) in corners.iter().enumerate() {
        let inner_radius = (outer_radius - bw).max(0.0);
        let segments = CORNER_SEGMENTS;

        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            // Outer vertex
            let outer_x = cx + cos_a * outer_radius;
            let outer_y = cy - sin_a * outer_radius;
            vertices.push(Vertex {
                position: [outer_x, outer_y, 0.0],
                texcoord: [0.0, 0.0],
                color: rgba,
            });

            // Inner vertex
            let inner_x = cx + cos_a * inner_radius;
            let inner_y = cy - sin_a * inner_radius;
            vertices.push(Vertex {
                position: [inner_x, inner_y, 0.0],
                texcoord: [1.0, 1.0],
                color: rgba,
            });
        }

        // Create triangles between outer and inner arcs
        let start = (corner_idx * (CORNER_SEGMENTS + 1) * 2) as u16;
        for i in 0..segments as u16 {
            let outer1 = start + i * 2;
            let inner1 = start + i * 2 + 1;
            let outer2 = start + (i + 1) * 2;
            let inner2 = start + (i + 1) * 2 + 1;

            // Two triangles per segment
            indices.push(outer1);
            indices.push(outer2);
            indices.push(inner1);

            indices.push(inner1);
            indices.push(outer2);
            indices.push(inner2);
        }
    }

    // Connect corners with straight edge segments
    let verts_per_corner = (CORNER_SEGMENTS + 1) * 2;

    // Helper to get vertex indices at the end of a corner arc
    let corner_end = |corner: usize| -> (u16, u16) {
        let base = (corner * verts_per_corner + CORNER_SEGMENTS * 2) as u16;
        (base, base + 1) // (outer, inner)
    };

    let corner_start = |corner: usize| -> (u16, u16) {
        let base = (corner * verts_per_corner) as u16;
        (base, base + 1) // (outer, inner)
    };

    // Connect top-left end to top-right start (top edge)
    let (tl_outer, tl_inner) = corner_end(0);
    let (tr_outer, tr_inner) = corner_start(1);
    indices.extend_from_slice(&[tl_outer, tr_outer, tl_inner, tl_inner, tr_outer, tr_inner]);

    // Connect top-right end to bottom-right start (right edge)
    let (tr_outer, tr_inner) = corner_end(1);
    let (br_outer, br_inner) = corner_start(2);
    indices.extend_from_slice(&[tr_outer, br_outer, tr_inner, tr_inner, br_outer, br_inner]);

    // Connect bottom-right end to bottom-left start (bottom edge)
    let (br_outer, br_inner) = corner_end(2);
    let (bl_outer, bl_inner) = corner_start(3);
    indices.extend_from_slice(&[br_outer, bl_outer, br_inner, br_inner, bl_outer, bl_inner]);

    // Connect bottom-left end to top-left start (left edge)
    let (bl_outer, bl_inner) = corner_end(3);
    let (tl_outer, tl_inner) = corner_start(0);
    indices.extend_from_slice(&[bl_outer, tl_outer, bl_inner, bl_inner, tl_outer, tl_inner]);

    (vertices, indices)
}

/// Generate a circle
pub fn circle(
    cx: f32,
    cy: f32,
    radius: f32,
    color: u32,
    segments: usize,
) -> (Vec<Vertex>, Vec<u16>) {
    let rgba = color_to_rgba(color);
    let mut vertices = Vec::with_capacity(segments + 1);
    let mut indices = Vec::with_capacity(segments * 3);

    // Center vertex
    vertices.push(Vertex {
        position: [cx, cy, 0.0],
        texcoord: [0.5, 0.5],
        color: rgba,
    });

    // Edge vertices
    for i in 0..segments {
        let angle = 2.0 * PI * (i as f32 / segments as f32);
        let px = cx + angle.cos() * radius;
        let py = cy + angle.sin() * radius;

        let u = 0.5 + angle.cos() * 0.5;
        let v = 0.5 + angle.sin() * 0.5;

        vertices.push(Vertex {
            position: [px, py, 0.0],
            texcoord: [u, v],
            color: rgba,
        });
    }

    // Triangle fan from center
    for i in 0..segments {
        indices.push(0);
        indices.push((i + 1) as u16);
        indices.push(((i + 1) % segments + 1) as u16);
    }

    (vertices, indices)
}

/// Generate a line with thickness
pub fn line(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    thickness: f32,
    color: u32,
) -> (Vec<Vertex>, Vec<u16>) {
    let rgba = color_to_rgba(color);

    // Calculate perpendicular direction
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();

    if len < 0.001 {
        return (vec![], vec![]);
    }

    // Perpendicular unit vector
    let px = -dy / len * thickness * 0.5;
    let py = dx / len * thickness * 0.5;

    let vertices = vec![
        Vertex {
            position: [x1 - px, y1 - py, 0.0],
            texcoord: [0.0, 0.0],
            color: rgba,
        },
        Vertex {
            position: [x1 + px, y1 + py, 0.0],
            texcoord: [0.0, 1.0],
            color: rgba,
        },
        Vertex {
            position: [x2 - px, y2 - py, 0.0],
            texcoord: [1.0, 0.0],
            color: rgba,
        },
        Vertex {
            position: [x2 + px, y2 + py, 0.0],
            texcoord: [1.0, 1.0],
            color: rgba,
        },
    ];

    let indices = vec![0, 2, 1, 1, 2, 3];

    (vertices, indices)
}

/// Helper to add a simple rectangle's vertices and indices
fn add_rect_vertices(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rgba: [f32; 4],
) {
    let base = vertices.len() as u16;

    vertices.push(Vertex {
        position: [x, y, 0.0],
        texcoord: [0.0, 0.0],
        color: rgba,
    });
    vertices.push(Vertex {
        position: [x + width, y, 0.0],
        texcoord: [1.0, 0.0],
        color: rgba,
    });
    vertices.push(Vertex {
        position: [x, y + height, 0.0],
        texcoord: [0.0, 1.0],
        color: rgba,
    });
    vertices.push(Vertex {
        position: [x + width, y + height, 0.0],
        texcoord: [1.0, 1.0],
        color: rgba,
    });

    indices.extend_from_slice(&[
        base, base + 1, base + 2,
        base + 1, base + 3, base + 2,
    ]);
}

/// Convert u32 color (0xRRGGBBAA) to [f32; 4] RGBA
fn color_to_rgba(color: u32) -> [f32; 4] {
    [
        ((color >> 24) & 0xFF) as f32 / 255.0,
        ((color >> 16) & 0xFF) as f32 / 255.0,
        ((color >> 8) & 0xFF) as f32 / 255.0,
        (color & 0xFF) as f32 / 255.0,
    ]
}

// ===== Gradient Support =====

/// Generate vertices and indices for a rectangle with a gradient fill
///
/// # Arguments
/// * `x`, `y` - Top-left position in screen coordinates
/// * `width`, `height` - Size in pixels
/// * `gradient` - Gradient specification (linear or radial)
/// * `radii` - Corner radii [top-left, top-right, bottom-right, bottom-left]
///
/// # Returns
/// (vertices, indices) for rendering with DrawTriangles
pub fn gradient_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    gradient: &Gradient,
    radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    // Clamp radii to half the smallest dimension
    let max_radius = (width.min(height)) / 2.0;
    let radii = [
        radii[0].min(max_radius),
        radii[1].min(max_radius),
        radii[2].min(max_radius),
        radii[3].min(max_radius),
    ];

    // Check if we have any rounded corners
    let has_rounded = radii.iter().any(|&r| r > 0.5);

    if !has_rounded {
        // Simple rectangle with gradient
        return simple_gradient_rect(x, y, width, height, gradient);
    }

    // Generate rounded rectangle with gradient
    rounded_gradient_rect(x, y, width, height, gradient, radii)
}

/// Simple rectangle with gradient (no rounded corners)
fn simple_gradient_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    gradient: &Gradient,
) -> (Vec<Vertex>, Vec<u16>) {
    // For a simple rect, we can use 4 vertices with gradient-computed colors
    let vertices = vec![
        Vertex {
            position: [x, y, 0.0],
            texcoord: [0.0, 0.0],
            color: compute_gradient_color(gradient, 0.0, 0.0, width, height),
        },
        Vertex {
            position: [x + width, y, 0.0],
            texcoord: [1.0, 0.0],
            color: compute_gradient_color(gradient, width, 0.0, width, height),
        },
        Vertex {
            position: [x, y + height, 0.0],
            texcoord: [0.0, 1.0],
            color: compute_gradient_color(gradient, 0.0, height, width, height),
        },
        Vertex {
            position: [x + width, y + height, 0.0],
            texcoord: [1.0, 1.0],
            color: compute_gradient_color(gradient, width, height, width, height),
        },
    ];

    let indices = vec![0, 1, 2, 1, 3, 2];

    (vertices, indices)
}

/// Rounded rectangle with gradient fill
fn rounded_gradient_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    gradient: &Gradient,
    radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Center point for fan triangulation
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let center_idx = 0u16;

    // Center vertex gets the gradient color at center
    vertices.push(Vertex {
        position: [center_x, center_y, 0.0],
        texcoord: [0.5, 0.5],
        color: compute_gradient_color(gradient, width / 2.0, height / 2.0, width, height),
    });

    // Corner definitions - same as rounded_rect
    let corners = [
        (x + radii[0], y + radii[0], PI, PI / 2.0, radii[0]),                    // top-left
        (x + width - radii[1], y + radii[1], PI / 2.0, 0.0, radii[1]),           // top-right
        (x + width - radii[2], y + height - radii[2], 0.0, -PI / 2.0, radii[2]), // bottom-right
        (x + radii[3], y + height - radii[3], -PI / 2.0, -PI, radii[3]),         // bottom-left
    ];

    // Sharp corner positions for radius 0
    let sharp_corners = [
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ];

    for (corner_idx, &(cx, cy, start_angle, end_angle, radius)) in corners.iter().enumerate() {
        if radius > 0.5 {
            // Rounded corner - generate arc with gradient colors
            for i in 0..=CORNER_SEGMENTS {
                let t = i as f32 / CORNER_SEGMENTS as f32;
                let angle = start_angle + (end_angle - start_angle) * t;

                let px = cx + angle.cos() * radius;
                let py = cy - angle.sin() * radius;

                // Compute position relative to rect origin for gradient
                let rel_x = px - x;
                let rel_y = py - y;

                let u = rel_x / width;
                let v = rel_y / height;

                vertices.push(Vertex {
                    position: [px, py, 0.0],
                    texcoord: [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)],
                    color: compute_gradient_color(gradient, rel_x, rel_y, width, height),
                });
            }

            // Create triangles from center to edge vertices
            let start_vertex = 1 + corner_idx * (CORNER_SEGMENTS + 1);
            for i in 0..CORNER_SEGMENTS {
                let v1 = (start_vertex + i) as u16;
                let v2 = (start_vertex + i + 1) as u16;
                indices.push(center_idx);
                indices.push(v1);
                indices.push(v2);
            }
        } else {
            // Sharp corner
            let (sharp_x, sharp_y) = sharp_corners[corner_idx];
            let rel_x = sharp_x - x;
            let rel_y = sharp_y - y;
            let u = rel_x / width;
            let v = rel_y / height;
            let color = compute_gradient_color(gradient, rel_x, rel_y, width, height);

            for _ in 0..=CORNER_SEGMENTS {
                vertices.push(Vertex {
                    position: [sharp_x, sharp_y, 0.0],
                    texcoord: [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)],
                    color,
                });
            }
        }
    }

    // Connect corners with straight edges (same as rounded_rect)
    let verts_per_corner = CORNER_SEGMENTS + 1;

    let corner_end = |corner: usize| -> u16 {
        (1 + corner * verts_per_corner + CORNER_SEGMENTS) as u16
    };

    let corner_start = |corner: usize| -> u16 {
        (1 + corner * verts_per_corner) as u16
    };

    // Connect corners
    indices.push(center_idx);
    indices.push(corner_end(0));
    indices.push(corner_start(1));

    indices.push(center_idx);
    indices.push(corner_end(1));
    indices.push(corner_start(2));

    indices.push(center_idx);
    indices.push(corner_end(2));
    indices.push(corner_start(3));

    indices.push(center_idx);
    indices.push(corner_end(3));
    indices.push(corner_start(0));

    (vertices, indices)
}

/// Compute the gradient color at a given position within a rectangle
///
/// # Arguments
/// * `gradient` - The gradient specification
/// * `local_x`, `local_y` - Position relative to the rect's top-left
/// * `width`, `height` - Rectangle dimensions
fn compute_gradient_color(
    gradient: &Gradient,
    local_x: f32,
    local_y: f32,
    width: f32,
    height: f32,
) -> [f32; 4] {
    match gradient {
        Gradient::Linear { angle, stops } => {
            compute_linear_gradient_color(*angle, stops, local_x, local_y, width, height)
        }
        Gradient::Radial { center_x, center_y, stops } => {
            compute_radial_gradient_color(*center_x, *center_y, stops, local_x, local_y, width, height)
        }
    }
}

/// Compute color for linear gradient
fn compute_linear_gradient_color(
    angle_deg: f32,
    stops: &[GradientStop],
    local_x: f32,
    local_y: f32,
    width: f32,
    height: f32,
) -> [f32; 4] {
    if stops.is_empty() {
        return [1.0, 1.0, 1.0, 1.0]; // Default to white
    }
    if stops.len() == 1 {
        return color_to_rgba(stops[0].color);
    }

    // Convert angle to radians (CSS convention: 0 = up, 90 = right)
    // We adjust so 0 = right (like standard math), then apply CSS offset
    let angle_rad = (angle_deg - 90.0).to_radians();

    // Direction vector for the gradient
    let dir_x = angle_rad.cos();
    let dir_y = angle_rad.sin();

    // Normalize position to 0-1 range
    let norm_x = if width > 0.0 { local_x / width } else { 0.5 };
    let norm_y = if height > 0.0 { local_y / height } else { 0.5 };

    // Center the coordinate system
    let centered_x = norm_x - 0.5;
    let centered_y = norm_y - 0.5;

    // Project onto gradient direction
    // The gradient line runs from -0.5 to 0.5 in the gradient direction
    let projection = centered_x * dir_x + centered_y * dir_y;

    // Map from [-0.5, 0.5] to [0, 1]
    let t = (projection + 0.5).clamp(0.0, 1.0);

    interpolate_gradient_stops(stops, t)
}

/// Compute color for radial gradient
fn compute_radial_gradient_color(
    center_x: f32,
    center_y: f32,
    stops: &[GradientStop],
    local_x: f32,
    local_y: f32,
    width: f32,
    height: f32,
) -> [f32; 4] {
    if stops.is_empty() {
        return [1.0, 1.0, 1.0, 1.0];
    }
    if stops.len() == 1 {
        return color_to_rgba(stops[0].color);
    }

    // Normalize position to 0-1 range
    let norm_x = if width > 0.0 { local_x / width } else { 0.5 };
    let norm_y = if height > 0.0 { local_y / height } else { 0.5 };

    // Distance from center (normalized)
    let dx = norm_x - center_x;
    let dy = norm_y - center_y;

    // For a circular gradient that reaches the corners, max distance is ~0.707 from center
    // We scale so that distance 0.5 = edge of the inscribed circle
    let distance = (dx * dx + dy * dy).sqrt();

    // Map distance to gradient position (0 at center, 1 at edge/corner)
    // Using 0.707 (1/sqrt(2)) as the "full" distance for a square
    let t = (distance / 0.707).clamp(0.0, 1.0);

    interpolate_gradient_stops(stops, t)
}

// ===== Shadow Support =====

/// Minimum number of layers for shadows
const MIN_SHADOW_LAYERS: usize = 8;

/// Maximum number of layers (to prevent excessive geometry)
const MAX_SHADOW_LAYERS: usize = 32;

/// Calculate the number of shadow layers based on blur radius
/// Larger blur needs more layers for smooth transitions
fn shadow_layer_count(blur: f32) -> usize {
    // Use ~2 layers per pixel of blur, with min/max bounds
    let layers = (blur * 2.0).ceil() as usize;
    layers.clamp(MIN_SHADOW_LAYERS, MAX_SHADOW_LAYERS)
}

/// Smooth easing function for shadow alpha (ease-in-out cubic)
/// Provides smoother transitions than simple quadratic
fn shadow_ease(t: f32) -> f32 {
    // Cubic ease-in: starts slow, accelerates
    // This creates a more natural-looking soft shadow
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Generate vertices and indices for a soft shadow
///
/// Creates multiple concentric layers with decreasing opacity to simulate blur.
/// The shadow is rendered as expanded rounded rectangles with alpha falloff.
/// Layer count automatically scales with blur radius for consistent quality.
///
/// # Arguments
/// * `x`, `y` - Top-left position of the element casting the shadow
/// * `width`, `height` - Size of the element
/// * `blur` - Blur radius in pixels (larger = softer shadow)
/// * `color` - Shadow color (0xRRGGBBAA)
/// * `offset_x`, `offset_y` - Shadow offset from the element
/// * `corner_radii` - Corner radii of the element [top-left, top-right, bottom-right, bottom-left]
///
/// # Returns
/// (vertices, indices) for rendering with DrawTriangles
pub fn shadow_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    blur: f32,
    color: u32,
    offset_x: f32,
    offset_y: f32,
    corner_radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();

    // Adaptive layer count based on blur radius
    let num_layers = shadow_layer_count(blur);

    // Base shadow position (with offset)
    let shadow_x = x + offset_x;
    let shadow_y = y + offset_y;

    // Extract base color components
    let base_r = ((color >> 24) & 0xFF) as f32 / 255.0;
    let base_g = ((color >> 16) & 0xFF) as f32 / 255.0;
    let base_b = ((color >> 8) & 0xFF) as f32 / 255.0;
    let base_a = (color & 0xFF) as f32 / 255.0;

    // Scale alpha multiplier inversely with layer count to maintain consistent overall intensity
    // More layers = less alpha per layer, so total shadow doesn't get darker
    let alpha_multiplier = 3.0 / num_layers as f32;

    // Generate layers from outermost (most transparent) to innermost (most opaque)
    // This ensures proper alpha blending with back-to-front rendering
    for layer in 0..num_layers {
        // Layer 0 is outermost, layer (num_layers-1) is innermost
        let layer_t = layer as f32 / (num_layers - 1) as f32;

        // Expansion: outermost layer is fully expanded, innermost has no expansion
        let expansion = blur * (1.0 - layer_t);

        // Alpha: use smooth easing for natural-looking soft shadow
        // Outermost layer is very transparent, innermost is more opaque
        let alpha_factor = shadow_ease(layer_t);
        let layer_alpha = base_a * alpha_factor * alpha_multiplier;

        // Skip nearly invisible layers
        if layer_alpha < 0.005 {
            continue;
        }

        // Expanded rect position and size
        let layer_x = shadow_x - expansion;
        let layer_y = shadow_y - expansion;
        let layer_width = width + expansion * 2.0;
        let layer_height = height + expansion * 2.0;

        // Expand corner radii proportionally
        let layer_radii = [
            corner_radii[0] + expansion,
            corner_radii[1] + expansion,
            corner_radii[2] + expansion,
            corner_radii[3] + expansion,
        ];

        // Create color with adjusted alpha
        let layer_color = [base_r, base_g, base_b, layer_alpha];

        // Generate this layer's geometry
        let (layer_verts, layer_indices) = shadow_layer_rect(
            layer_x,
            layer_y,
            layer_width,
            layer_height,
            layer_color,
            layer_radii,
        );

        // Offset indices for this layer
        let base_idx = all_vertices.len() as u16;
        all_vertices.extend(layer_verts);
        all_indices.extend(layer_indices.iter().map(|i| i + base_idx));
    }

    (all_vertices, all_indices)
}

/// Generate a single shadow layer (rounded rect with specific color)
fn shadow_layer_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
    radii: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    // Clamp radii
    let max_radius = (width.min(height)) / 2.0;
    let radii = [
        radii[0].min(max_radius).max(0.0),
        radii[1].min(max_radius).max(0.0),
        radii[2].min(max_radius).max(0.0),
        radii[3].min(max_radius).max(0.0),
    ];

    let has_rounded = radii.iter().any(|&r| r > 0.5);

    if !has_rounded {
        // Simple rectangle
        let vertices = vec![
            Vertex { position: [x, y, 0.0], texcoord: [0.0, 0.0], color },
            Vertex { position: [x + width, y, 0.0], texcoord: [1.0, 0.0], color },
            Vertex { position: [x, y + height, 0.0], texcoord: [0.0, 1.0], color },
            Vertex { position: [x + width, y + height, 0.0], texcoord: [1.0, 1.0], color },
        ];
        let indices = vec![0, 1, 2, 1, 3, 2];
        return (vertices, indices);
    }

    // Rounded rectangle using same algorithm as rounded_rect but with pre-computed color
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let center_idx = 0u16;

    vertices.push(Vertex {
        position: [center_x, center_y, 0.0],
        texcoord: [0.5, 0.5],
        color,
    });

    let corners = [
        (x + radii[0], y + radii[0], PI, PI / 2.0, radii[0]),
        (x + width - radii[1], y + radii[1], PI / 2.0, 0.0, radii[1]),
        (x + width - radii[2], y + height - radii[2], 0.0, -PI / 2.0, radii[2]),
        (x + radii[3], y + height - radii[3], -PI / 2.0, -PI, radii[3]),
    ];

    let sharp_corners = [
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ];

    for (corner_idx, &(cx, cy, start_angle, end_angle, radius)) in corners.iter().enumerate() {
        if radius > 0.5 {
            for i in 0..=CORNER_SEGMENTS {
                let t = i as f32 / CORNER_SEGMENTS as f32;
                let angle = start_angle + (end_angle - start_angle) * t;

                let px = cx + angle.cos() * radius;
                let py = cy - angle.sin() * radius;

                let u = (px - x) / width;
                let v = (py - y) / height;

                vertices.push(Vertex {
                    position: [px, py, 0.0],
                    texcoord: [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)],
                    color,
                });
            }

            let start_vertex = 1 + corner_idx * (CORNER_SEGMENTS + 1);
            for i in 0..CORNER_SEGMENTS {
                let v1 = (start_vertex + i) as u16;
                let v2 = (start_vertex + i + 1) as u16;
                indices.push(center_idx);
                indices.push(v1);
                indices.push(v2);
            }
        } else {
            let (sharp_x, sharp_y) = sharp_corners[corner_idx];
            let u = (sharp_x - x) / width;
            let v = (sharp_y - y) / height;

            for _ in 0..=CORNER_SEGMENTS {
                vertices.push(Vertex {
                    position: [sharp_x, sharp_y, 0.0],
                    texcoord: [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)],
                    color,
                });
            }
        }
    }

    let verts_per_corner = CORNER_SEGMENTS + 1;

    let corner_end = |corner: usize| -> u16 {
        (1 + corner * verts_per_corner + CORNER_SEGMENTS) as u16
    };

    let corner_start = |corner: usize| -> u16 {
        (1 + corner * verts_per_corner) as u16
    };

    indices.push(center_idx);
    indices.push(corner_end(0));
    indices.push(corner_start(1));

    indices.push(center_idx);
    indices.push(corner_end(1));
    indices.push(corner_start(2));

    indices.push(center_idx);
    indices.push(corner_end(2));
    indices.push(corner_start(3));

    indices.push(center_idx);
    indices.push(corner_end(3));
    indices.push(corner_start(0));

    (vertices, indices)
}

/// Interpolate between gradient stops at position t (0.0 to 1.0)
fn interpolate_gradient_stops(stops: &[GradientStop], t: f32) -> [f32; 4] {
    // Find the two stops we're between
    let mut prev_stop = &stops[0];
    let mut next_stop = &stops[stops.len() - 1];

    for i in 0..stops.len() - 1 {
        if t >= stops[i].position && t <= stops[i + 1].position {
            prev_stop = &stops[i];
            next_stop = &stops[i + 1];
            break;
        }
    }

    // Handle edge cases
    if t <= prev_stop.position {
        return color_to_rgba(prev_stop.color);
    }
    if t >= next_stop.position {
        return color_to_rgba(next_stop.color);
    }

    // Interpolate between the two stops
    let range = next_stop.position - prev_stop.position;
    let local_t = if range > 0.0 {
        (t - prev_stop.position) / range
    } else {
        0.0
    };

    let c1 = color_to_rgba(prev_stop.color);
    let c2 = color_to_rgba(next_stop.color);

    [
        c1[0] + (c2[0] - c1[0]) * local_t,
        c1[1] + (c2[1] - c1[1]) * local_t,
        c1[2] + (c2[2] - c1[2]) * local_t,
        c1[3] + (c2[3] - c1[3]) * local_t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_rect() {
        let (verts, indices) = simple_rect(0.0, 0.0, 100.0, 50.0, 0xFF0000FF);
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_rounded_rect() {
        let (verts, indices) = rounded_rect(0.0, 0.0, 100.0, 50.0, 0xFF0000FF, [10.0, 10.0, 10.0, 10.0]);
        // 1 center + 4 corners * (CORNER_SEGMENTS + 1) vertices
        assert_eq!(verts.len(), 1 + 4 * (CORNER_SEGMENTS + 1));
        assert!(indices.len() > 0);
    }

    #[test]
    fn test_circle() {
        let (verts, indices) = circle(50.0, 50.0, 25.0, 0x00FF00FF, 16);
        assert_eq!(verts.len(), 17); // center + 16 edge
        assert_eq!(indices.len(), 48); // 16 triangles * 3
    }

    #[test]
    fn test_line() {
        let (verts, indices) = line(0.0, 0.0, 100.0, 0.0, 2.0, 0x0000FFFF);
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_color_conversion() {
        let rgba = color_to_rgba(0xFF8040C0);
        assert!((rgba[0] - 1.0).abs() < 0.01);      // R = 255
        assert!((rgba[1] - 0.502).abs() < 0.01);    // G = 128
        assert!((rgba[2] - 0.251).abs() < 0.01);    // B = 64
        assert!((rgba[3] - 0.753).abs() < 0.01);    // A = 192
    }
}
