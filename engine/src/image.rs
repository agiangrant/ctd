//! Image loading and texture management
//!
//! This module handles loading images from files or memory, decoding them,
//! and managing GPU textures for rendering.

use std::collections::HashMap;
use std::error::Error;

/// A loaded image ready for GPU upload
pub struct LoadedImage {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// RGBA pixel data (4 bytes per pixel)
    pub data: Vec<u8>,
}

impl LoadedImage {
    /// Load an image from raw bytes (PNG, JPEG, etc.)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let img = image::load_from_memory(bytes)?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        Ok(Self {
            width,
            height,
            data: rgba.into_raw(),
        })
    }

    /// Load an image from a file path
    pub fn from_file(path: &str) -> Result<Self, Box<dyn Error>> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Create a solid color image (useful for placeholders)
    pub fn solid_color(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Self {
        let pixel_count = (width * height) as usize;
        let mut data = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            data.push(r);
            data.push(g);
            data.push(b);
            data.push(a);
        }
        Self { width, height, data }
    }
}

/// Information about a loaded texture
#[derive(Debug, Clone)]
pub struct TextureInfo {
    /// Texture ID for referencing
    pub id: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// Manages loaded textures and their GPU resources
pub struct TextureManager {
    /// Map from texture ID to texture info
    textures: HashMap<u32, TextureInfo>,
    /// Next texture ID to assign
    next_id: u32,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 1, // Start at 1, 0 can be "no texture"
        }
    }

    /// Register a new texture and return its ID
    pub fn register(&mut self, width: u32, height: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        self.textures.insert(id, TextureInfo { id, width, height });
        id
    }

    /// Get texture info by ID
    pub fn get(&self, id: u32) -> Option<&TextureInfo> {
        self.textures.get(&id)
    }

    /// Remove a texture
    pub fn remove(&mut self, id: u32) -> Option<TextureInfo> {
        self.textures.remove(&id)
    }
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid_color_image() {
        let img = LoadedImage::solid_color(2, 2, 255, 0, 0, 255);
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 16); // 4 pixels * 4 bytes
        assert_eq!(&img.data[0..4], &[255, 0, 0, 255]); // First pixel is red
    }

    #[test]
    fn test_texture_manager() {
        let mut manager = TextureManager::new();

        let id1 = manager.register(100, 100);
        let id2 = manager.register(200, 200);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let info = manager.get(id1).unwrap();
        assert_eq!(info.width, 100);
        assert_eq!(info.height, 100);
    }
}
