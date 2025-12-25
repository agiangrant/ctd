//! Windows glyph rasterization using DirectWrite
//!
//! Renders glyphs to RGBA bitmaps using DirectWrite GDI interop.
//! Supports font weights and styles via DirectWrite font creation APIs.
//! Bundled fonts are loaded using AddFontResourceExW for private process access.

use super::{GlyphBitmap, GlyphRasterizer};
use crate::text::{FontDescriptor, FontSource, FontStyle};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, COLORREF, RECT};
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;

/// FR_PRIVATE flag for AddFontResourceExW - font is only available to this process
const FR_PRIVATE: u32 = 0x10;

/// Global COM initialization flag
static COM_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Ensure COM is initialized (required for DirectWrite)
fn ensure_com_initialized() {
    COM_INITIALIZED.get_or_init(|| {
        unsafe {
            // Try to initialize COM - if already initialized, that's fine
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        true
    });
}

/// Check if a character is an emoji (should render with native colors, not white)
fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        // Emoticons
        0x1F600..=0x1F64F |
        // Miscellaneous Symbols and Pictographs
        0x1F300..=0x1F5FF |
        // Transport and Map Symbols
        0x1F680..=0x1F6FF |
        // Supplemental Symbols and Pictographs
        0x1F900..=0x1F9FF |
        // Symbols and Pictographs Extended-A
        0x1FA00..=0x1FA6F |
        // Symbols and Pictographs Extended-B
        0x1FA70..=0x1FAFF |
        // Dingbats
        0x2700..=0x27BF |
        // Miscellaneous Symbols
        0x2600..=0x26FF |
        // Regional Indicator Symbols (flags)
        0x1F1E0..=0x1F1FF |
        // Skin tone modifiers
        0x1F3FB..=0x1F3FF |
        // Additional common emoji ranges
        0x1F400..=0x1F4FF |
        // Musical/activity symbols that are often emoji
        0x1F3A0..=0x1F3FF
    )
}

/// Map CSS font weight (100-900) to DirectWrite font weight
fn map_weight_to_dwrite(weight: u16) -> DWRITE_FONT_WEIGHT {
    // In windows crate v0.58+, font weights are numeric values wrapped in DWRITE_FONT_WEIGHT
    DWRITE_FONT_WEIGHT(match weight {
        0..=149 => 100,   // Thin
        150..=249 => 200, // Extra Light
        250..=349 => 300, // Light
        350..=449 => 400, // Regular
        450..=549 => 500, // Medium
        550..=649 => 600, // Semi Bold
        650..=749 => 700, // Bold
        750..=849 => 800, // Extra Bold
        _ => 900,         // Black
    })
}

/// Cached font file data for bundled fonts
struct LoadedFontFile {
    #[allow(dead_code)]
    font_file: IDWriteFontFile,
    font_face: IDWriteFontFace,
}

/// Information about a loaded bundled font
struct BundledFontInfo {
    /// The font family name to use for rendering
    font_name: String,
    /// The resolved absolute path to the font file
    resolved_path: String,
    /// Whether the font was successfully added to the process
    loaded: bool,
}

/// Windows glyph rasterizer using DirectWrite for metrics and GDI for rendering
pub struct WindowsGlyphRasterizer {
    /// DirectWrite factory (for text metrics)
    dwrite_factory: IDWriteFactory,
    /// Cache of loaded font files from file paths (for DirectWrite font faces)
    loaded_fonts: HashMap<String, LoadedFontFile>,
    /// Cache of bundled font paths to their family names
    bundled_fonts: HashMap<String, BundledFontInfo>,
}

// External function for adding fonts
#[link(name = "gdi32")]
extern "system" {
    fn AddFontResourceExW(name: PCWSTR, fl: u32, res: *mut std::ffi::c_void) -> i32;
}

impl WindowsGlyphRasterizer {
    pub fn new() -> Self {
        ensure_com_initialized();

        unsafe {
            // Create DirectWrite factory for text metrics
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .expect("Failed to create DirectWrite factory");

            Self {
                dwrite_factory,
                loaded_fonts: HashMap::new(),
                bundled_fonts: HashMap::new(),
            }
        }
    }

    /// Load a bundled font and return its family name
    fn load_bundled_font(&mut self, path: &str) -> Option<String> {
        // Check cache first
        if let Some(info) = self.bundled_fonts.get(path) {
            if info.loaded {
                return Some(info.font_name.clone());
            } else {
                return None;
            }
        }

        // Resolve the path
        let resolved_path = self.resolve_font_path(path)?;

        unsafe {
            // Add the font to this process using AddFontResourceExW (for GDI)
            let wide_path: Vec<u16> = resolved_path.encode_utf16().chain(std::iter::once(0)).collect();
            let result = AddFontResourceExW(
                PCWSTR::from_raw(wide_path.as_ptr()),
                FR_PRIVATE,
                std::ptr::null_mut(),
            );

            if result == 0 {
                eprintln!("Failed to add font resource: {}", resolved_path);
                self.bundled_fonts.insert(path.to_string(), BundledFontInfo {
                    font_name: String::new(),
                    resolved_path: String::new(),
                    loaded: false,
                });
                return None;
            }

            // Extract font family name from the font file
            let font_name = self.get_font_family_name(&resolved_path).unwrap_or_else(|| {
                // Fall back to using filename without extension as font name
                Path::new(&resolved_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Segoe UI")
                    .to_string()
            });

            eprintln!("Loaded bundled font '{}' as '{}'", path, font_name);

            // Verify the font is actually available in GDI with this name
            let verified_name = self.verify_gdi_font_name(&font_name, &resolved_path);
            if verified_name != font_name {
                eprintln!("Font name corrected from '{}' to '{}'", font_name, verified_name);
            }

            // Also load the font into DirectWrite for proper metrics
            self.load_font_from_file(&resolved_path);

            self.bundled_fonts.insert(path.to_string(), BundledFontInfo {
                font_name: verified_name.clone(),
                resolved_path: resolved_path.clone(),
                loaded: true,
            });

            Some(verified_name)
        }
    }

    /// Verify that GDI can find the font and return the actual name it uses
    fn verify_gdi_font_name(&self, expected_name: &str, font_path: &str) -> String {
        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);
            ReleaseDC(None, screen_dc);

            let mut face_name = [0u16; 64];

            // Helper to check if a font name works
            let check_font_name = |dc: HDC, name: &str| -> Option<String> {
                let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                let font = CreateFontW(
                    -16, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0,
                    PCWSTR::from_raw(wide_name.as_ptr()),
                );
                let old = SelectObject(dc, font);

                let mut face = [0u16; 64];
                let len = GetTextFaceW(dc, Some(&mut face[..]));
                let actual = if len > 0 {
                    String::from_utf16_lossy(&face[..len as usize]).trim_matches('\0').to_string()
                } else {
                    String::new()
                };

                SelectObject(dc, old);
                let _ = DeleteObject(font);

                if actual.eq_ignore_ascii_case(name) {
                    Some(name.to_string())
                } else {
                    None
                }
            };

            // Try expected name first
            if let Some(name) = check_font_name(mem_dc, expected_name) {
                let _ = DeleteDC(mem_dc);
                return name;
            }

            // Get what GDI actually selected for logging
            let wide_name: Vec<u16> = expected_name.encode_utf16().chain(std::iter::once(0)).collect();
            let font = CreateFontW(
                -16, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0,
                PCWSTR::from_raw(wide_name.as_ptr()),
            );
            let old = SelectObject(mem_dc, font);
            let len = GetTextFaceW(mem_dc, Some(&mut face_name[..]));
            let fallback_name = if len > 0 {
                String::from_utf16_lossy(&face_name[..len as usize]).trim_matches('\0').to_string()
            } else {
                "Segoe UI".to_string()
            };
            SelectObject(mem_dc, old);
            let _ = DeleteObject(font);

            eprintln!("GDI selected '{}' instead of '{}'", fallback_name, expected_name);

            // Try alternative names from the font file
            if let Ok(data) = std::fs::read(font_path) {
                // Try nameID=16 (Typographic Family Name)
                if let Some(alt_name) = self.parse_font_name_with_id(&data, 16) {
                    if let Some(name) = check_font_name(mem_dc, &alt_name) {
                        eprintln!("Using typographic family name: '{}'", name);
                        let _ = DeleteDC(mem_dc);
                        return name;
                    }
                }

                // Try nameID=4 (Full Font Name)
                if let Some(alt_name) = self.parse_font_name_with_id(&data, 4) {
                    if let Some(name) = check_font_name(mem_dc, &alt_name) {
                        eprintln!("Using full font name: '{}'", name);
                        let _ = DeleteDC(mem_dc);
                        return name;
                    }
                }
            }

            let _ = DeleteDC(mem_dc);

            // Fall back to what GDI actually selected (might be a fallback font)
            fallback_name
        }
    }

    /// Get the resolved path for a bundled font
    fn get_bundled_font_path(&self, path: &str) -> Option<String> {
        self.bundled_fonts.get(path).map(|info| info.resolved_path.clone())
    }

    /// Resolve a font path to an absolute path
    fn resolve_font_path(&self, path: &str) -> Option<String> {
        if Path::new(path).is_absolute() && Path::new(path).exists() {
            return Some(path.to_string());
        }

        // Try relative to current working directory
        if let Ok(cwd) = std::env::current_dir() {
            let full_path = cwd.join(path);
            if full_path.exists() {
                return Some(full_path.to_string_lossy().to_string());
            }
        }

        // Try relative to executable directory
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_relative = exe_dir.join(path);
                if exe_relative.exists() {
                    return Some(exe_relative.to_string_lossy().to_string());
                }
            }
        }

        eprintln!("Font file not found: {}", path);
        None
    }

    /// Get the font family name from a font file by reading the TrueType name table
    /// This extracts the actual internal family name from the font's metadata
    fn get_font_family_name(&self, path: &str) -> Option<String> {
        // Read the font file and parse the name table
        let data = std::fs::read(path).ok()?;
        self.parse_font_family_name(&data)
    }

    /// Parse the font family name from TrueType/OpenType font data
    fn parse_font_family_name(&self, data: &[u8]) -> Option<String> {
        if data.len() < 12 {
            return None;
        }

        // Check for TrueType/OpenType signature
        let sfnt_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if sfnt_version != 0x00010000 && sfnt_version != 0x4F54544F {
            // Not TrueType (0x00010000) or OpenType/CFF (OTTO)
            return None;
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;

        // Find the 'name' table
        for i in 0..num_tables {
            let offset = 12 + i * 16;
            if offset + 16 > data.len() {
                break;
            }

            let tag = &data[offset..offset + 4];
            if tag == b"name" {
                let table_offset = u32::from_be_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]) as usize;

                return self.parse_name_table(data, table_offset);
            }
        }

        None
    }

    /// Parse the name table to extract the font family name
    fn parse_name_table(&self, data: &[u8], table_offset: usize) -> Option<String> {
        if table_offset + 6 > data.len() {
            return None;
        }

        let name_data = &data[table_offset..];
        if name_data.len() < 6 {
            return None;
        }

        // Name table header
        let count = u16::from_be_bytes([name_data[2], name_data[3]]) as usize;
        let string_offset = u16::from_be_bytes([name_data[4], name_data[5]]) as usize;

        // Look for name ID 1 (Font Family) or 4 (Full Name)
        // Prefer platform 3 (Windows), encoding 1 (Unicode BMP)
        let mut family_name: Option<String> = None;

        for i in 0..count {
            let record_offset = 6 + i * 12;
            if record_offset + 12 > name_data.len() {
                break;
            }

            let platform_id = u16::from_be_bytes([name_data[record_offset], name_data[record_offset + 1]]);
            let encoding_id = u16::from_be_bytes([name_data[record_offset + 2], name_data[record_offset + 3]]);
            let _language_id = u16::from_be_bytes([name_data[record_offset + 4], name_data[record_offset + 5]]);
            let name_id = u16::from_be_bytes([name_data[record_offset + 6], name_data[record_offset + 7]]);
            let length = u16::from_be_bytes([name_data[record_offset + 8], name_data[record_offset + 9]]) as usize;
            let offset = u16::from_be_bytes([name_data[record_offset + 10], name_data[record_offset + 11]]) as usize;

            // Name ID 1 = Font Family Name
            if name_id == 1 {
                let str_start = string_offset + offset;
                if str_start + length <= name_data.len() {
                    let str_data = &name_data[str_start..str_start + length];

                    // Platform 3 (Windows) with encoding 1 (Unicode BMP) - UTF-16BE
                    if platform_id == 3 && encoding_id == 1 {
                        let utf16: Vec<u16> = str_data
                            .chunks(2)
                            .filter_map(|chunk| {
                                if chunk.len() == 2 {
                                    Some(u16::from_be_bytes([chunk[0], chunk[1]]))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if let Ok(s) = String::from_utf16(&utf16) {
                            return Some(s);
                        }
                    }
                    // Platform 1 (Macintosh) with encoding 0 (Roman) - ASCII/MacRoman
                    else if platform_id == 1 && encoding_id == 0 && family_name.is_none() {
                        if let Ok(s) = String::from_utf8(str_data.to_vec()) {
                            family_name = Some(s);
                        }
                    }
                }
            }
        }

        family_name
    }

    /// Load a font from a file path
    fn load_font_from_file(&mut self, path: &str) -> Option<&LoadedFontFile> {
        // Check cache first
        if self.loaded_fonts.contains_key(path) {
            return self.loaded_fonts.get(path);
        }

        // Resolve the path
        let resolved_path = if Path::new(path).is_absolute() {
            path.to_string()
        } else {
            // Try relative to current working directory
            if let Ok(cwd) = std::env::current_dir() {
                let full_path = cwd.join(path);
                if full_path.exists() {
                    full_path.to_string_lossy().to_string()
                } else {
                    // Try relative to executable directory
                    if let Ok(exe_path) = std::env::current_exe() {
                        if let Some(exe_dir) = exe_path.parent() {
                            let exe_relative = exe_dir.join(path);
                            if exe_relative.exists() {
                                exe_relative.to_string_lossy().to_string()
                            } else {
                                eprintln!("Font file not found: {} (tried {} and {})",
                                    path,
                                    full_path.display(),
                                    exe_relative.display());
                                return None;
                            }
                        } else {
                            eprintln!("Font file not found: {}", full_path.display());
                            return None;
                        }
                    } else {
                        eprintln!("Font file not found: {}", full_path.display());
                        return None;
                    }
                }
            } else {
                path.to_string()
            }
        };

        unsafe {
            // Convert path to wide string
            let wide_path: Vec<u16> = resolved_path.encode_utf16().chain(std::iter::once(0)).collect();

            // Create font file from path
            let font_file: IDWriteFontFile = match self.dwrite_factory.CreateFontFileReference(
                PCWSTR::from_raw(wide_path.as_ptr()),
                None,
            ) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Failed to create font file reference for {}: {:?}", resolved_path, e);
                    return None;
                }
            };

            // Check if the font is supported
            let mut is_supported = BOOL::default();
            let mut file_type = DWRITE_FONT_FILE_TYPE_UNKNOWN;
            let mut face_type = DWRITE_FONT_FACE_TYPE_UNKNOWN;
            let mut num_faces = 0u32;

            if font_file.Analyze(
                &mut is_supported as *mut _,
                &mut file_type as *mut _,
                Some(&mut face_type as *mut _),
                &mut num_faces as *mut _,
            ).is_err() {
                eprintln!("Failed to analyze font file: {}", resolved_path);
                return None;
            }

            if !is_supported.as_bool() {
                eprintln!("Unsupported font file format: {}", resolved_path);
                return None;
            }

            // Create font face from font file
            let font_files = [Some(font_file.clone())];
            let font_face: IDWriteFontFace = match self.dwrite_factory.CreateFontFace(
                face_type,
                &font_files,
                0,
                DWRITE_FONT_SIMULATIONS_NONE,
            ) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Failed to create font face for {}: {:?}", resolved_path, e);
                    return None;
                }
            };

            // Cache the loaded font
            let loaded = LoadedFontFile {
                font_file,
                font_face,
            };
            self.loaded_fonts.insert(path.to_string(), loaded);
            self.loaded_fonts.get(path)
        }
    }

    /// Create a text format with the specified font settings
    fn create_text_format(&mut self, font: &FontDescriptor) -> Option<IDWriteTextFormat> {
        let size = font.size;
        let weight = map_weight_to_dwrite(font.weight);
        let style = if font.style == FontStyle::Italic {
            DWRITE_FONT_STYLE_ITALIC
        } else {
            DWRITE_FONT_STYLE_NORMAL
        };

        // Handle bundled fonts - load them first to get the font family name
        let font_name = match &font.source {
            FontSource::System(name) => {
                if name == "system" || name.is_empty() {
                    "Segoe UI".to_string() // Windows system font
                } else {
                    name.clone()
                }
            }
            FontSource::Bundled(path) => {
                // Load the bundled font and get its family name
                self.load_bundled_font(path).unwrap_or_else(|| "Segoe UI".to_string())
            }
            FontSource::Memory { name, .. } => name.clone(),
        };

        unsafe {
            let wide_name: Vec<u16> = font_name.encode_utf16().chain(std::iter::once(0)).collect();
            let locale: Vec<u16> = "en-us".encode_utf16().chain(std::iter::once(0)).collect();

            match self.dwrite_factory.CreateTextFormat(
                PCWSTR::from_raw(wide_name.as_ptr()),
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR::from_raw(locale.as_ptr()),
            ) {
                Ok(format) => Some(format),
                Err(e) => {
                    eprintln!("Failed to create text format for {}: {:?}", font_name, e);
                    // Fall back to Segoe UI
                    let fallback: Vec<u16> = "Segoe UI".encode_utf16().chain(std::iter::once(0)).collect();
                    self.dwrite_factory.CreateTextFormat(
                        PCWSTR::from_raw(fallback.as_ptr()),
                        None,
                        weight,
                        style,
                        DWRITE_FONT_STRETCH_NORMAL,
                        size,
                        PCWSTR::from_raw(locale.as_ptr()),
                    ).ok()
                }
            }
        }
    }

    /// Create a text layout for measuring and drawing
    fn create_text_layout(&mut self, text: &str, font: &FontDescriptor) -> Option<IDWriteTextLayout> {
        let text_format = self.create_text_format(font)?;

        unsafe {
            let wide_text: Vec<u16> = text.encode_utf16().collect();

            match self.dwrite_factory.CreateTextLayout(
                &wide_text,
                &text_format,
                10000.0, // Max width (large value for single-line measurement)
                10000.0, // Max height
            ) {
                Ok(layout) => Some(layout),
                Err(e) => {
                    eprintln!("Failed to create text layout: {:?}", e);
                    None
                }
            }
        }
    }

    /// Create a GDI font with consistent parameters for both measurement and rendering
    fn create_gdi_font(&self, font_name: &str, font: &FontDescriptor) -> HFONT {
        unsafe {
            let wide_name: Vec<u16> = font_name.encode_utf16().chain(std::iter::once(0)).collect();
            CreateFontW(
                -(font.size.round() as i32), // Round to nearest integer for consistency
                0, // Width (0 = default aspect ratio)
                0, // Escapement
                0, // Orientation
                font.weight as i32, // Weight (100-900)
                if font.style == FontStyle::Italic { 1 } else { 0 }, // Italic
                0, // Underline
                0, // StrikeOut
                1, // CharSet (DEFAULT_CHARSET)
                0, // OutPrecision (OUT_DEFAULT_PRECIS)
                0, // ClipPrecision (CLIP_DEFAULT_PRECIS)
                5, // Quality (CLEARTYPE_QUALITY)
                0, // PitchAndFamily (DEFAULT_PITCH | FF_DONTCARE)
                PCWSTR::from_raw(wide_name.as_ptr()),
            )
        }
    }

    /// Measure text width using GDI
    fn measure_with_gdi(&self, text: &str, font_name: &str, font: &FontDescriptor) -> f32 {
        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);
            ReleaseDC(None, screen_dc);

            // Create GDI font using shared function for consistency
            let gdi_font = self.create_gdi_font(font_name, font);
            let old_font = SelectObject(mem_dc, gdi_font);

            // Use GetTextExtentPoint32W for string measurement
            let wide_text: Vec<u16> = text.encode_utf16().collect();
            let mut size = windows::Win32::Foundation::SIZE::default();
            let _ = GetTextExtentPoint32W(mem_dc, &wide_text, &mut size);

            // Cleanup
            SelectObject(mem_dc, old_font);
            let _ = DeleteObject(gdi_font);
            let _ = DeleteDC(mem_dc);

            size.cx as f32
        }
    }

    /// Parse a specific name ID from the font's name table
    fn parse_font_name_with_id(&self, data: &[u8], target_name_id: u16) -> Option<String> {
        if data.len() < 12 {
            return None;
        }

        // Check for TrueType/OpenType signature
        let sfnt_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if sfnt_version != 0x00010000 && sfnt_version != 0x4F54544F {
            return None;
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;

        // Find the 'name' table
        for i in 0..num_tables {
            let offset = 12 + i * 16;
            if offset + 16 > data.len() {
                break;
            }

            let tag = &data[offset..offset + 4];
            if tag == b"name" {
                let table_offset = u32::from_be_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]) as usize;

                return self.parse_name_table_with_id(data, table_offset, target_name_id);
            }
        }

        None
    }

    /// Parse the name table to extract a specific name ID
    fn parse_name_table_with_id(&self, data: &[u8], table_offset: usize, target_name_id: u16) -> Option<String> {
        if table_offset + 6 > data.len() {
            return None;
        }

        let name_data = &data[table_offset..];
        if name_data.len() < 6 {
            return None;
        }

        let count = u16::from_be_bytes([name_data[2], name_data[3]]) as usize;
        let string_offset = u16::from_be_bytes([name_data[4], name_data[5]]) as usize;

        let mut fallback_name: Option<String> = None;

        for i in 0..count {
            let record_offset = 6 + i * 12;
            if record_offset + 12 > name_data.len() {
                break;
            }

            let platform_id = u16::from_be_bytes([name_data[record_offset], name_data[record_offset + 1]]);
            let encoding_id = u16::from_be_bytes([name_data[record_offset + 2], name_data[record_offset + 3]]);
            let name_id = u16::from_be_bytes([name_data[record_offset + 6], name_data[record_offset + 7]]);
            let length = u16::from_be_bytes([name_data[record_offset + 8], name_data[record_offset + 9]]) as usize;
            let offset = u16::from_be_bytes([name_data[record_offset + 10], name_data[record_offset + 11]]) as usize;

            if name_id == target_name_id {
                let str_start = string_offset + offset;
                if str_start + length <= name_data.len() {
                    let str_data = &name_data[str_start..str_start + length];

                    // Platform 3 (Windows) with encoding 1 (Unicode BMP) - UTF-16BE
                    if platform_id == 3 && encoding_id == 1 {
                        let utf16: Vec<u16> = str_data
                            .chunks(2)
                            .filter_map(|chunk| {
                                if chunk.len() == 2 {
                                    Some(u16::from_be_bytes([chunk[0], chunk[1]]))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if let Ok(s) = String::from_utf16(&utf16) {
                            return Some(s);
                        }
                    }
                    // Platform 1 (Macintosh) with encoding 0 (Roman) - ASCII/MacRoman
                    else if platform_id == 1 && encoding_id == 0 && fallback_name.is_none() {
                        if let Ok(s) = String::from_utf8(str_data.to_vec()) {
                            fallback_name = Some(s);
                        }
                    }
                }
            }
        }

        fallback_name
    }

    /// Get font metrics (ascent, descent) for a given font descriptor.
    /// Returns (ascent, descent) in pixels. Both values are positive.
    /// Height = ascent + descent.
    pub fn get_font_metrics(&mut self, font: &FontDescriptor) -> (f32, f32) {
        // Ensure bundled font is loaded first
        if let FontSource::Bundled(path) = &font.source {
            let _ = self.load_bundled_font(path);
        }

        // Try to get metrics from DirectWrite for bundled fonts
        if let FontSource::Bundled(path) = &font.source {
            if let Some(resolved_path) = self.get_bundled_font_path(path) {
                if let Some(loaded) = self.loaded_fonts.get(&resolved_path) {
                    unsafe {
                        let mut metrics = DWRITE_FONT_METRICS::default();
                        loaded.font_face.GetMetrics(&mut metrics);

                        let design_units_per_em = metrics.designUnitsPerEm as f32;
                        let scale = font.size / design_units_per_em;

                        let ascent = metrics.ascent as f32 * scale;
                        let descent = metrics.descent as f32 * scale;

                        return (ascent, descent);
                    }
                }
            }
        }

        // Fall back to DirectWrite text format for system fonts
        if let Some(text_format) = self.create_text_format(font) {
            unsafe {
                // Create a text layout to get metrics
                let test_text: Vec<u16> = "Hg".encode_utf16().collect();
                if let Ok(layout) = self.dwrite_factory.CreateTextLayout(
                    &test_text,
                    &text_format,
                    1000.0,
                    1000.0,
                ) {
                    let mut line_count = 0u32;
                    let _ = layout.GetLineMetrics(None, &mut line_count);

                    if line_count > 0 {
                        let mut line_metrics = vec![DWRITE_LINE_METRICS::default(); line_count as usize];
                        if layout.GetLineMetrics(Some(&mut line_metrics), &mut line_count).is_ok() {
                            let baseline = line_metrics[0].baseline;
                            let height = line_metrics[0].height;
                            let descent = height - baseline;
                            return (baseline, descent);
                        }
                    }
                }
            }
        }

        // Fallback: assume standard ratio
        let ascent = font.size * 0.8;
        let descent = font.size * 0.2;
        (ascent, descent)
    }

    /// Measure the width of a string (fast path, no rasterization)
    /// Uses GDI for all fonts to ensure consistency with GDI rendering
    pub fn measure_string(&mut self, text: &str, font: &FontDescriptor) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        // Get the font name to use for GDI measurement (verified during load_bundled_font)
        let font_name = match &font.source {
            FontSource::Bundled(path) => {
                // Ensure the font is loaded first - returns verified name
                match self.load_bundled_font(path) {
                    Some(name) => name,
                    None => "Segoe UI".to_string(),
                }
            }
            FontSource::System(name) => {
                if name == "system" || name.is_empty() {
                    "Segoe UI".to_string()
                } else {
                    name.clone()
                }
            }
            FontSource::Memory { name, .. } => name.clone(),
        };

        // Use GDI for all fonts to match rendering
        self.measure_with_gdi(text, &font_name, font)
    }

    /// Render text to a bitmap using GDI and extract RGBA data
    fn render_to_bitmap(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        _is_emoji: bool,
    ) -> Option<(Vec<u8>, u32, u32, f32, f32, f32)> {
        // CRITICAL: Use measure_string for the advance to ensure consistency
        // between layout measurement and glyph rendering
        let advance = self.measure_string(text, font);

        // Get dimensions for the bitmap
        let (width, height, ascent, descent) = match &font.source {
            FontSource::Bundled(path) => {
                // Ensure the font is loaded first
                if self.load_bundled_font(path).is_none() {
                    return None;
                }

                // Get height metrics from DirectWrite font face
                let resolved_path = self.get_bundled_font_path(path)?;
                if let Some(loaded) = self.loaded_fonts.get(&resolved_path) {
                    let font_face = &loaded.font_face;

                    unsafe {
                        let mut font_metrics = DWRITE_FONT_METRICS::default();
                        font_face.GetMetrics(&mut font_metrics);

                        let design_units_per_em = font_metrics.designUnitsPerEm as f32;
                        let scale = font.size / design_units_per_em;

                        let face_ascent = font_metrics.ascent as f32 * scale;
                        let face_descent = font_metrics.descent as f32 * scale;
                        let line_height = face_ascent + face_descent;

                        (advance.ceil() as u32, line_height.ceil() as u32, face_ascent, face_descent)
                    }
                } else {
                    let (_, height, ascent, descent, _) = self.measure_with_layout(text, font)?;
                    (advance.ceil() as u32, height, ascent, descent)
                }
            }
            _ => {
                // Get height metrics from DirectWrite
                let (_, height, ascent, descent, _) = self.measure_with_layout(text, font)?;
                (advance.ceil() as u32, height, ascent, descent)
            }
        };

        unsafe {
            // Add generous padding for proper rendering (decorative fonts may have swashes)
            let padding = 8u32;
            let bitmap_width = width + padding * 2;
            let bitmap_height = height + padding * 2;

            if bitmap_width == 0 || bitmap_height == 0 {
                return Some((Vec::new(), 0, 0, 0.0, 0.0, advance));
            }

            // Create a memory DC
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);
            ReleaseDC(None, screen_dc);

            // Create a DIB section for 32-bit color
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bitmap_width as i32,
                    biHeight: -(bitmap_height as i32), // Top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0, // BI_RGB
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(
                mem_dc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            );

            let dib = match dib {
                Ok(d) if !d.is_invalid() && !bits_ptr.is_null() => d,
                _ => {
                    let _ = DeleteDC(mem_dc);
                    return None;
                }
            };

            // Select the DIB into the DC
            let old_bitmap = SelectObject(mem_dc, dib);

            // Clear to black
            let brush = CreateSolidBrush(COLORREF(0x00000000));
            let rect = RECT {
                left: 0,
                top: 0,
                right: bitmap_width as i32,
                bottom: bitmap_height as i32,
            };
            FillRect(mem_dc, &rect, brush);
            let _ = DeleteObject(brush);

            // Create GDI font matching DirectWrite settings
            // For bundled fonts, we need to get the font family name we loaded earlier
            let font_name = match &font.source {
                FontSource::System(name) if name != "system" && !name.is_empty() => name.clone(),
                FontSource::Bundled(path) => {
                    // Check if we have the font cached, otherwise try to load it
                    if let Some(info) = self.bundled_fonts.get(path.as_str()) {
                        if info.loaded {
                            info.font_name.clone()
                        } else {
                            "Segoe UI".to_string()
                        }
                    } else {
                        // Font should have been loaded by create_text_format already
                        "Segoe UI".to_string()
                    }
                }
                _ => "Segoe UI".to_string(),
            };

            // Use shared font creation for consistency with measurement
            let gdi_font = self.create_gdi_font(&font_name, font);

            let old_font = SelectObject(mem_dc, gdi_font);

            // Set text color to white on black background
            SetTextColor(mem_dc, COLORREF(0x00FFFFFF));
            SetBkMode(mem_dc, TRANSPARENT);

            // Draw text
            let wide_text: Vec<u16> = text.encode_utf16().collect();
            TextOutW(mem_dc, padding as i32, padding as i32, &wide_text);

            // Restore and cleanup GDI objects
            SelectObject(mem_dc, old_font);
            let _ = DeleteObject(gdi_font);

            // Extract RGBA data from the DIB
            let bits = std::slice::from_raw_parts(
                bits_ptr as *const u8,
                (bitmap_width * bitmap_height * 4) as usize,
            );

            let mut rgba_data = vec![0u8; (bitmap_width * bitmap_height * 4) as usize];

            // Convert BGRA to RGBA and use luminance as alpha for proper antialiasing
            for i in (0..bits.len()).step_by(4) {
                let b = bits[i];
                let g = bits[i + 1];
                let r = bits[i + 2];

                // Use luminance as alpha (text is white on black)
                let alpha = ((r as u32 + g as u32 + b as u32) / 3) as u8;

                // Output white with computed alpha
                rgba_data[i] = 255;     // R
                rgba_data[i + 1] = 255; // G
                rgba_data[i + 2] = 255; // B
                rgba_data[i + 3] = alpha;
            }

            // Cleanup
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(dib);
            let _ = DeleteDC(mem_dc);

            Some((
                rgba_data,
                bitmap_width,
                bitmap_height,
                ascent,
                descent,
                advance,
            ))
        }
    }

    /// Measure text using DirectWrite layout (for system fonts)
    fn measure_with_layout(&mut self, text: &str, font: &FontDescriptor) -> Option<(u32, u32, f32, f32, f32)> {
        let layout = self.create_text_layout(text, font)?;

        unsafe {
            // Get metrics to determine bitmap size
            let mut metrics = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut metrics).is_err() {
                return None;
            }

            // Get line metrics for ascent/descent
            let mut line_count = 0u32;
            let _ = layout.GetLineMetrics(None, &mut line_count);

            let mut line_metrics_vec = vec![DWRITE_LINE_METRICS::default(); line_count as usize];
            if line_count > 0 {
                let _ = layout.GetLineMetrics(Some(&mut line_metrics_vec), &mut line_count);
            }

            let ascent = if !line_metrics_vec.is_empty() {
                line_metrics_vec[0].baseline
            } else {
                metrics.height * 0.8 // Estimate
            };
            let descent = metrics.height - ascent;

            let width = metrics.width.ceil() as u32;
            let height = metrics.height.ceil() as u32;

            Some((width, height, ascent, descent, metrics.width))
        }
    }
}

impl GlyphRasterizer for WindowsGlyphRasterizer {
    fn rasterize_glyph(
        &mut self,
        character: char,
        font: &FontDescriptor,
    ) -> Option<GlyphBitmap> {
        // Handle whitespace characters - no visual glyph needed
        if character.is_whitespace() {
            let char_string = character.to_string();
            let advance = self.measure_string(&char_string, font);

            return Some(GlyphBitmap {
                data: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
            });
        }

        let char_string = character.to_string();
        let char_is_emoji = is_emoji(character);

        let (data, width, height, ascent, _descent, advance) =
            self.render_to_bitmap(&char_string, font, char_is_emoji)?;

        if width == 0 || height == 0 {
            return Some(GlyphBitmap {
                data: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
            });
        }

        // Padding must match what's used in render_to_bitmap
        let padding = 8.0f32;

        Some(GlyphBitmap {
            data,
            width,
            height,
            bearing_x: -padding,
            bearing_y: ascent + padding,
            advance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterizer_creation() {
        let rasterizer = WindowsGlyphRasterizer::new();
        let _ = rasterizer;
    }

    #[test]
    fn test_rasterize_glyph() {
        let mut rasterizer = WindowsGlyphRasterizer::new();
        let font = FontDescriptor::system("Segoe UI", 400, FontStyle::Normal, 16.0);
        let bitmap = rasterizer.rasterize_glyph('A', &font);

        assert!(bitmap.is_some());
        let bitmap = bitmap.unwrap();

        // Should have non-zero dimensions
        assert!(bitmap.width > 0);
        assert!(bitmap.height > 0);

        // Data should be RGBA (4 bytes per pixel)
        assert_eq!(bitmap.data.len(), (bitmap.width * bitmap.height * 4) as usize);
    }

    #[test]
    fn test_rasterize_whitespace() {
        let mut rasterizer = WindowsGlyphRasterizer::new();
        let font = FontDescriptor::system("Segoe UI", 400, FontStyle::Normal, 16.0);
        let bitmap = rasterizer.rasterize_glyph(' ', &font);

        assert!(bitmap.is_some());
        let bitmap = bitmap.unwrap();

        // Whitespace should have zero dimensions
        assert_eq!(bitmap.width, 0);
        assert_eq!(bitmap.height, 0);
    }

    #[test]
    fn test_rasterize_different_weights() {
        let mut rasterizer = WindowsGlyphRasterizer::new();

        // Regular weight
        let regular = FontDescriptor::system("Segoe UI", 400, FontStyle::Normal, 16.0);
        let regular_bitmap = rasterizer.rasterize_glyph('A', &regular);
        assert!(regular_bitmap.is_some());

        // Bold weight
        let bold = FontDescriptor::system("Segoe UI", 700, FontStyle::Normal, 16.0);
        let bold_bitmap = rasterizer.rasterize_glyph('A', &bold);
        assert!(bold_bitmap.is_some());

        let regular_b = regular_bitmap.unwrap();
        let bold_b = bold_bitmap.unwrap();

        assert!(regular_b.width > 0);
        assert!(bold_b.width > 0);
    }

    #[test]
    fn test_measure_string() {
        let mut rasterizer = WindowsGlyphRasterizer::new();
        let font = FontDescriptor::system("Segoe UI", 400, FontStyle::Normal, 16.0);

        let width = rasterizer.measure_string("Hello", &font);
        assert!(width > 0.0);

        let empty_width = rasterizer.measure_string("", &font);
        assert_eq!(empty_width, 0.0);
    }
}
