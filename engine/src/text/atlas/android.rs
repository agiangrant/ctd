//! Android glyph rasterization using Android Canvas via JNI
//!
//! Renders glyphs to RGBA bitmaps using Android's Paint and Canvas APIs.
//! This provides native system font support with proper i18n and text shaping.

#![cfg(target_os = "android")]

use super::{GlyphBitmap, GlyphRasterizer};
use crate::text::{FontDescriptor, FontSource, FontStyle};
use jni::objects::{GlobalRef, JObject, JValue};
use jni::JNIEnv;
use std::collections::HashMap;

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

// Get JavaVM from platform module
fn get_java_vm() -> Option<&'static jni::JavaVM> {
    unsafe { super::super::super::platform::android::JAVA_VM.as_ref() }
}

/// Android glyph rasterizer using Canvas/Paint via JNI
pub struct AndroidGlyphRasterizer {
    /// Cache of created Paint objects (font name + style -> GlobalRef<Paint>)
    paint_cache: HashMap<String, GlobalRef>,
    /// Cache of loaded Typeface objects for bundled fonts (path -> GlobalRef<Typeface>)
    typeface_cache: HashMap<String, GlobalRef>,
    /// Global reference to a reusable Bitmap for rasterization
    /// We create this lazily and resize as needed
    bitmap_ref: Option<GlobalRef>,
    /// Global reference to Canvas wrapping the bitmap
    canvas_ref: Option<GlobalRef>,
    /// Current bitmap dimensions
    bitmap_size: (i32, i32),
    /// Cached class references
    paint_class: Option<GlobalRef>,
    bitmap_class: Option<GlobalRef>,
    bitmap_config_class: Option<GlobalRef>,
    canvas_class: Option<GlobalRef>,
    rect_class: Option<GlobalRef>,
    typeface_class: Option<GlobalRef>,
    file_class: Option<GlobalRef>,
}

impl AndroidGlyphRasterizer {
    pub fn new() -> Self {
        Self {
            paint_cache: HashMap::new(),
            typeface_cache: HashMap::new(),
            bitmap_ref: None,
            canvas_ref: None,
            bitmap_size: (0, 0),
            paint_class: None,
            bitmap_class: None,
            bitmap_config_class: None,
            canvas_class: None,
            rect_class: None,
            typeface_class: None,
            file_class: None,
        }
    }

    /// Initialize JNI class references (called lazily on first use)
    fn init_classes(&mut self, env: &mut JNIEnv) -> bool {
        if self.paint_class.is_some() {
            return true; // Already initialized
        }

        log::info!("AndroidGlyphRasterizer: initializing JNI classes");

        // Find and cache class references
        let paint = match env.find_class("android/graphics/Paint") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(e) => {
                    log::error!("Failed to create global ref for Paint: {:?}", e);
                    return false;
                }
            },
            Err(e) => {
                log::error!("Failed to find Paint class: {:?}", e);
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_describe();
                    let _ = env.exception_clear();
                }
                return false;
            }
        };

        let bitmap = match env.find_class("android/graphics/Bitmap") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        let bitmap_config = match env.find_class("android/graphics/Bitmap$Config") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(e) => {
                    log::error!("Failed to create global ref for Bitmap$Config: {:?}", e);
                    return false;
                }
            },
            Err(e) => {
                log::error!("Failed to find Bitmap$Config class: {:?}", e);
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_describe();
                    let _ = env.exception_clear();
                }
                return false;
            }
        };

        let canvas = match env.find_class("android/graphics/Canvas") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        let rect = match env.find_class("android/graphics/Rect") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        let typeface = match env.find_class("android/graphics/Typeface") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        let file = match env.find_class("java/io/File") {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global) => global,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        self.paint_class = Some(paint);
        self.bitmap_class = Some(bitmap);
        self.bitmap_config_class = Some(bitmap_config);
        self.canvas_class = Some(canvas);
        self.rect_class = Some(rect);
        self.typeface_class = Some(typeface);
        self.file_class = Some(file);

        log::info!("AndroidGlyphRasterizer: JNI classes initialized successfully");
        true
    }

    /// Create a Paint object configured for the given font
    fn create_paint(&mut self, env: &mut JNIEnv, font: &FontDescriptor) -> Option<GlobalRef> {
        let cache_key = font.cache_key();

        // Check cache
        if let Some(paint) = self.paint_cache.get(&cache_key) {
            return Some(paint.clone());
        }

        // Ensure classes are initialized
        if !self.init_classes(env) {
            return None;
        }

        let paint_class = self.paint_class.as_ref()?;
        let _typeface_class = self.typeface_class.as_ref()?;

        // Create new Paint with ANTI_ALIAS_FLAG (0x01)
        // Use the cached class reference instead of string-based lookup
        let paint_jclass = unsafe { jni::objects::JClass::from_raw(paint_class.as_raw()) };
        let paint = env
            .new_object(&paint_jclass, "(I)V", &[JValue::Int(0x01)])
            .ok()?;

        // Set text size
        env.call_method(&paint, "setTextSize", "(F)V", &[JValue::Float(font.size)])
            .ok()?;

        // Set color to white (we'll use the alpha channel for the glyph shape)
        env.call_method(&paint, "setColor", "(I)V", &[JValue::Int(0xFFFFFFFFu32 as i32)])
            .ok()?;

        // Create and set Typeface
        let typeface = self.create_typeface(env, font)?;
        env.call_method(&paint, "setTypeface", "(Landroid/graphics/Typeface;)Landroid/graphics/Typeface;", &[JValue::Object(&typeface)])
            .ok()?;

        // Create global reference and cache it
        let paint_global = env.new_global_ref(&paint).ok()?;
        self.paint_cache.insert(cache_key, paint_global.clone());

        Some(paint_global)
    }

    /// Create a Typeface for the given font descriptor
    fn create_typeface<'a>(&mut self, env: &mut JNIEnv<'a>, font: &FontDescriptor) -> Option<JObject<'a>> {
        let typeface_class = self.typeface_class.as_ref()?;
        let typeface_jclass = unsafe { jni::objects::JClass::from_raw(typeface_class.as_raw()) };

        match &font.source {
            FontSource::Bundled(path) => {
                // For bundled fonts, use Typeface.createFromFile()
                self.create_typeface_from_file(env, path)
            }
            FontSource::System(name) | FontSource::Memory { name, .. } => {
                // For system fonts, use Typeface.create(family, style)
                let family_name = name.as_str();
                let j_family = env.new_string(family_name).ok()?;

                // Determine Android typeface style
                // Android Typeface styles: NORMAL=0, BOLD=1, ITALIC=2, BOLD_ITALIC=3
                let style = match (font.weight >= 600, font.style) {
                    (true, FontStyle::Italic) => 3,  // BOLD_ITALIC
                    (true, _) => 1,                   // BOLD
                    (false, FontStyle::Italic) => 2, // ITALIC
                    (false, _) => 0,                  // NORMAL
                };

                let typeface = env
                    .call_static_method(
                        &typeface_jclass,
                        "create",
                        "(Ljava/lang/String;I)Landroid/graphics/Typeface;",
                        &[JValue::Object(&j_family), JValue::Int(style)],
                    )
                    .ok()?
                    .l()
                    .ok()?;

                Some(typeface)
            }
        }
    }

    /// Create a Typeface from a bundled font file
    fn create_typeface_from_file<'a>(&mut self, env: &mut JNIEnv<'a>, path: &str) -> Option<JObject<'a>> {
        // Check if we have a cached typeface for this path
        if let Some(cached) = self.typeface_cache.get(path) {
            // Return a local reference from the global reference
            let local = env.new_local_ref(cached.as_obj()).ok()?;
            return Some(local);
        }

        log::info!("Loading bundled font: {}", path);

        // Try loading from Android assets first (this is where bundled fonts should be)
        if let Some(typeface) = self.create_typeface_from_assets(env, path) {
            // Cache the typeface
            if let Ok(global) = env.new_global_ref(&typeface) {
                log::info!("Successfully loaded bundled font from assets: {}", path);
                self.typeface_cache.insert(path.to_string(), global);
            }
            return Some(typeface);
        }

        // Fall back to filesystem (for development/testing)
        if let Some(typeface) = self.create_typeface_from_filesystem(env, path) {
            if let Ok(global) = env.new_global_ref(&typeface) {
                log::info!("Successfully loaded bundled font from filesystem: {}", path);
                self.typeface_cache.insert(path.to_string(), global);
            }
            return Some(typeface);
        }

        log::error!("Failed to load bundled font: {} - falling back to system font", path);
        self.create_fallback_typeface(env)
    }

    /// Try to create a Typeface from Android assets folder
    fn create_typeface_from_assets<'a>(&self, env: &mut JNIEnv<'a>, path: &str) -> Option<JObject<'a>> {
        let typeface_class = self.typeface_class.as_ref()?;

        // Get the activity to access AssetManager
        let activity_ptr = super::super::super::platform::android::get_activity_ptr();
        if activity_ptr.is_null() {
            log::warn!("Activity not available for asset loading");
            return None;
        }

        // Wrap activity pointer (don't let JNI delete it)
        let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

        // Get AssetManager: context.getAssets()
        let asset_manager = env
            .call_method(&*activity, "getAssets", "()Landroid/content/res/AssetManager;", &[])
            .ok()?
            .l()
            .ok()?;

        // Create Typeface from asset: Typeface.createFromAsset(AssetManager, String path)
        let j_path = env.new_string(path).ok()?;
        let typeface_jclass = unsafe { jni::objects::JClass::from_raw(typeface_class.as_raw()) };

        match env.call_static_method(
            &typeface_jclass,
            "createFromAsset",
            "(Landroid/content/res/AssetManager;Ljava/lang/String;)Landroid/graphics/Typeface;",
            &[JValue::Object(&asset_manager), JValue::Object(&j_path)],
        ) {
            Ok(result) => {
                if env.exception_check().unwrap_or(false) {
                    // Asset not found - this is expected if font isn't in assets
                    let _ = env.exception_clear();
                    log::info!("Font not found in assets: {}", path);
                    return None;
                }
                result.l().ok()
            }
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                None
            }
        }
    }

    /// Try to create a Typeface from filesystem
    fn create_typeface_from_filesystem<'a>(&self, env: &mut JNIEnv<'a>, path: &str) -> Option<JObject<'a>> {
        let typeface_class = self.typeface_class.as_ref()?;
        let file_class = self.file_class.as_ref()?;

        // Resolve the font path - try multiple locations
        let resolved_path = self.resolve_font_path(path);

        // Create a File object for the font path
        let file_jclass = unsafe { jni::objects::JClass::from_raw(file_class.as_raw()) };
        let j_path = env.new_string(&resolved_path).ok()?;
        let file_obj = env
            .new_object(&file_jclass, "(Ljava/lang/String;)V", &[JValue::Object(&j_path)])
            .ok()?;

        // Check if file exists
        let exists = env
            .call_method(&file_obj, "exists", "()Z", &[])
            .ok()?
            .z()
            .ok()?;

        if !exists {
            log::info!("Font file not found on filesystem: {}", resolved_path);
            return None;
        }

        // Create Typeface from file: Typeface.createFromFile(File file)
        let typeface_jclass = unsafe { jni::objects::JClass::from_raw(typeface_class.as_raw()) };
        match env.call_static_method(
            &typeface_jclass,
            "createFromFile",
            "(Ljava/io/File;)Landroid/graphics/Typeface;",
            &[JValue::Object(&file_obj)],
        ) {
            Ok(result) => {
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                    return None;
                }
                match result.l() {
                    Ok(typeface) if !typeface.is_null() => Some(typeface),
                    _ => None,
                }
            }
            Err(e) => {
                log::error!("Typeface.createFromFile failed: {:?}", e);
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                None
            }
        }
    }

    /// Resolve font path - try multiple locations
    fn resolve_font_path(&self, path: &str) -> String {
        use std::path::Path;

        // If path is absolute, use it directly
        if Path::new(path).is_absolute() {
            return path.to_string();
        }

        // Try the app's internal files directory first (this is where bundled assets should be)
        // This is set during android_main from context.getFilesDir()
        if let Some(files_dir) = super::super::super::platform::android::get_app_files_dir() {
            let files_path = Path::new(files_dir).join(path);
            log::info!("Checking font path in files dir: {}", files_path.display());
            if files_path.exists() {
                return files_path.to_string_lossy().to_string();
            }

            // Also try just the filename in the files directory
            // (in case the font was copied without preserving directory structure)
            if let Some(filename) = Path::new(path).file_name() {
                let flat_path = Path::new(files_dir).join(filename);
                if flat_path.exists() {
                    return flat_path.to_string_lossy().to_string();
                }
            }
        }

        // Try relative to current working directory
        if let Ok(cwd) = std::env::current_dir() {
            let full_path = cwd.join(path);
            if full_path.exists() {
                return full_path.to_string_lossy().to_string();
            }
        }

        // Try relative to executable directory
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_relative = exe_dir.join(path);
                if exe_relative.exists() {
                    return exe_relative.to_string_lossy().to_string();
                }
            }
        }

        // Return the original path if nothing else works
        path.to_string()
    }

    /// Create a fallback typeface (sans-serif)
    fn create_fallback_typeface<'a>(&self, env: &mut JNIEnv<'a>) -> Option<JObject<'a>> {
        let typeface_class = self.typeface_class.as_ref()?;
        let typeface_jclass = unsafe { jni::objects::JClass::from_raw(typeface_class.as_raw()) };

        let j_family = env.new_string("sans-serif").ok()?;
        let typeface = env
            .call_static_method(
                &typeface_jclass,
                "create",
                "(Ljava/lang/String;I)Landroid/graphics/Typeface;",
                &[JValue::Object(&j_family), JValue::Int(0)], // NORMAL style
            )
            .ok()?
            .l()
            .ok()?;

        Some(typeface)
    }

    /// Ensure bitmap is large enough for the given dimensions
    fn ensure_bitmap(&mut self, env: &mut JNIEnv, width: i32, height: i32) -> bool {
        // Check if current bitmap is large enough
        if let Some(ref _bitmap) = self.bitmap_ref {
            if self.bitmap_size.0 >= width && self.bitmap_size.1 >= height {
                return true;
            }
        }

        // Ensure classes are initialized
        if !self.init_classes(env) {
            return false;
        }

        let bitmap_class = match self.bitmap_class.as_ref() {
            Some(c) => c,
            None => return false,
        };
        let bitmap_config_class = match self.bitmap_config_class.as_ref() {
            Some(c) => c,
            None => return false,
        };
        let canvas_class = match self.canvas_class.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // Round up to reasonable size (at least 128x128, power of 2-ish)
        let new_width = (width.max(128) as u32).next_power_of_two().min(2048) as i32;
        let new_height = (height.max(128) as u32).next_power_of_two().min(2048) as i32;

        // Get Bitmap.Config.ARGB_8888 using cached class reference
        let config_jclass = unsafe { jni::objects::JClass::from_raw(bitmap_config_class.as_raw()) };
        let argb_8888 = match env.get_static_field(
            &config_jclass,
            "ARGB_8888",
            "Landroid/graphics/Bitmap$Config;",
        ) {
            Ok(field) => match field.l() {
                Ok(obj) => obj,
                Err(e) => {
                    log::error!("Failed to get ARGB_8888 object: {:?}", e);
                    return false;
                }
            },
            Err(e) => {
                log::error!("Failed to get ARGB_8888 field: {:?}", e);
                return false;
            }
        };

        // Create bitmap: Bitmap.createBitmap(width, height, Config) using cached class
        let bitmap_jclass = unsafe { jni::objects::JClass::from_raw(bitmap_class.as_raw()) };
        let bitmap = match env.call_static_method(
            &bitmap_jclass,
            "createBitmap",
            "(IILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;",
            &[
                JValue::Int(new_width),
                JValue::Int(new_height),
                JValue::Object(&argb_8888),
            ],
        ) {
            Ok(result) => match result.l() {
                Ok(obj) => obj,
                Err(e) => {
                    log::error!("Failed to get bitmap object: {:?}", e);
                    return false;
                }
            },
            Err(e) => {
                log::error!("Failed to call Bitmap.createBitmap: {:?}", e);
                return false;
            }
        };

        // Create canvas wrapping the bitmap using cached class
        let canvas_jclass = unsafe { jni::objects::JClass::from_raw(canvas_class.as_raw()) };
        let canvas = match env.new_object(
            &canvas_jclass,
            "(Landroid/graphics/Bitmap;)V",
            &[JValue::Object(&bitmap)],
        ) {
            Ok(obj) => obj,
            Err(e) => {
                log::error!("Failed to create Canvas: {:?}", e);
                return false;
            }
        };

        // Create global references
        let bitmap_global = match env.new_global_ref(&bitmap) {
            Ok(g) => g,
            Err(_) => return false,
        };
        let canvas_global = match env.new_global_ref(&canvas) {
            Ok(g) => g,
            Err(_) => return false,
        };

        self.bitmap_ref = Some(bitmap_global);
        self.canvas_ref = Some(canvas_global);
        self.bitmap_size = (new_width, new_height);

        true
    }

    /// Measure text bounds using Paint.getTextBounds
    fn measure_text_bounds(
        &mut self,
        env: &mut JNIEnv,
        paint: &GlobalRef,
        text: &str,
    ) -> Option<(i32, i32, i32, i32)> {
        // Create a Rect to receive bounds using cached class
        let rect_class = self.rect_class.as_ref()?;
        let rect_jclass = unsafe { jni::objects::JClass::from_raw(rect_class.as_raw()) };
        let rect = env.new_object(&rect_jclass, "()V", &[]).ok()?;

        // Convert text to Java string
        let j_text = env.new_string(text).ok()?;

        // Get Java string length (UTF-16 code units), NOT Rust's text.len() which is UTF-8 bytes
        // For emojis like "🎤" (4 bytes in UTF-8, 2 code units in UTF-16), we need the Java length
        let j_text_len = env.call_method(&j_text, "length", "()I", &[]).ok()?.i().ok()? as i32;

        // Call Paint.getTextBounds(String text, int start, int end, Rect bounds)
        env.call_method(
            paint,
            "getTextBounds",
            "(Ljava/lang/String;IILandroid/graphics/Rect;)V",
            &[
                JValue::Object(&j_text),
                JValue::Int(0),
                JValue::Int(j_text_len),
                JValue::Object(&rect),
            ],
        )
        .ok()?;

        // Get rect fields
        let left = env.get_field(&rect, "left", "I").ok()?.i().ok()?;
        let top = env.get_field(&rect, "top", "I").ok()?.i().ok()?;
        let right = env.get_field(&rect, "right", "I").ok()?.i().ok()?;
        let bottom = env.get_field(&rect, "bottom", "I").ok()?.i().ok()?;

        Some((left, top, right, bottom))
    }

    /// Measure text advance width
    fn measure_text_advance(&self, env: &mut JNIEnv, paint: &GlobalRef, text: &str) -> Option<f32> {
        let j_text = env.new_string(text).ok()?;

        let advance = env
            .call_method(
                paint,
                "measureText",
                "(Ljava/lang/String;)F",
                &[JValue::Object(&j_text)],
            )
            .ok()?
            .f()
            .ok()?;

        Some(advance)
    }

    /// Get font metrics
    fn get_font_metrics(&self, env: &mut JNIEnv, paint: &GlobalRef) -> Option<(f32, f32)> {
        // Get Paint.FontMetrics
        let metrics = env
            .call_method(
                paint,
                "getFontMetrics",
                "()Landroid/graphics/Paint$FontMetrics;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;

        let ascent = env.get_field(&metrics, "ascent", "F").ok()?.f().ok()?;
        let descent = env.get_field(&metrics, "descent", "F").ok()?.f().ok()?;

        // ascent is negative (distance above baseline), descent is positive
        Some((-ascent, descent))
    }
}

impl Default for AndroidGlyphRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphRasterizer for AndroidGlyphRasterizer {
    fn rasterize_glyph(&mut self, character: char, font: &FontDescriptor) -> Option<GlyphBitmap> {
        let vm = get_java_vm()?;
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                log::error!("Failed to attach JNI thread: {:?}", e);
                return None;
            }
        };

        // Check for JNI exceptions before proceeding
        if env.exception_check().unwrap_or(true) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
            log::error!("JNI exception pending before rasterize_glyph");
            return None;
        }

        // Create/get Paint for this font
        let paint = self.create_paint(&mut env, font)?;

        // Convert character to string for measurement
        let text = character.to_string();

        // Measure text bounds
        let (left, top, right, bottom) = self.measure_text_bounds(&mut env, &paint, &text)?;

        // Handle whitespace and zero-width characters
        let width = (right - left).max(1);
        let height = (bottom - top).max(1);

        // Get advance width (may be different from bounds width)
        let advance = self.measure_text_advance(&mut env, &paint, &text)?;

        // Handle pure whitespace (no visible pixels)
        if character.is_whitespace() {
            return Some(GlyphBitmap {
                data: vec![0u8; 4], // 1x1 transparent pixel
                width: 1,
                height: 1,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
            });
        }

        // Add padding for anti-aliasing
        let padding = 2;
        let bitmap_width = width + padding * 2;
        let bitmap_height = height + padding * 2;

        // Ensure bitmap is large enough
        if !self.ensure_bitmap(&mut env, bitmap_width, bitmap_height) {
            return None;
        }

        let bitmap = self.bitmap_ref.as_ref()?;
        let canvas = self.canvas_ref.as_ref()?;

        // Clear bitmap (fill with transparent black)
        env.call_method(
            bitmap,
            "eraseColor",
            "(I)V",
            &[JValue::Int(0)], // TRANSPARENT = 0x00000000
        )
        .ok()?;

        // Calculate draw position
        // The baseline is at y=0 in Canvas coordinates
        // top is negative (above baseline), bottom is positive (below baseline)
        let draw_x = (padding - left) as f32;
        let draw_y = (padding - top) as f32;

        // Draw text
        let paint_obj: &JObject = paint.as_ref();
        let char_is_emoji = is_emoji(character);

        // For color emoji fonts (CBDT/CBLC tables), the paint color can affect rendering.
        // We don't modify the color here - the shader handles color detection.
        // If emojis appear washed out, the issue is in how Android composites them.

        let j_text = env.new_string(&text).ok()?;
        env.call_method(
            canvas,
            "drawText",
            "(Ljava/lang/String;FFLandroid/graphics/Paint;)V",
            &[
                JValue::Object(&j_text),
                JValue::Float(draw_x),
                JValue::Float(draw_y),
                JValue::Object(paint_obj),
            ],
        )
        .ok()?;

        // Log emoji pixel data for debugging
        if char_is_emoji {
            log::debug!("Rasterizing emoji '{}' (U+{:04X})", character, character as u32);
        }

        // Extract pixels from bitmap
        let pixel_count = (bitmap_width * bitmap_height) as usize;
        let mut pixels = vec![0i32; pixel_count];

        // Create int array for getPixels
        let j_pixels = env.new_int_array(pixel_count as i32).ok()?;

        // Call Bitmap.getPixels(int[] pixels, int offset, int stride, int x, int y, int width, int height)
        env.call_method(
            bitmap,
            "getPixels",
            "([IIIIIII)V",
            &[
                JValue::Object(&j_pixels),
                JValue::Int(0),                      // offset
                JValue::Int(bitmap_width),           // stride
                JValue::Int(0),                      // x
                JValue::Int(0),                      // y
                JValue::Int(bitmap_width),           // width
                JValue::Int(bitmap_height),          // height
            ],
        )
        .ok()?;

        // Copy from Java array
        env.get_int_array_region(&j_pixels, 0, &mut pixels).ok()?;

        // Convert ARGB_8888 to RGBA (our expected format)
        let mut rgba_data = Vec::with_capacity(pixel_count * 4);
        for pixel in pixels {
            let a = ((pixel >> 24) & 0xFF) as u8;
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;

            // Output as RGBA with premultiplied alpha
            // Since we draw white text, RGB will be the alpha value
            rgba_data.push(r);
            rgba_data.push(g);
            rgba_data.push(b);
            rgba_data.push(a);
        }

        // bearing_y is the distance from the baseline to the top of the glyph bitmap
        // - top is negative (e.g., -30 means glyph extends 30 pixels above baseline)
        // - padding adds extra space around the glyph
        // So bearing_y = -top + padding = distance from baseline to top of bitmap
        let bearing_y = (-top) as f32 + padding as f32;

        // Debug logging for emoji measurements
        if char_is_emoji {
            log::info!(
                "EMOJI GLYPH '{}' (U+{:04X}): bounds=({},{},{},{}), size={}x{}, advance={:.1}, bearing=({:.1},{:.1})",
                character, character as u32,
                left, top, right, bottom,
                bitmap_width, bitmap_height,
                advance,
                left as f32 - padding as f32, bearing_y
            );
        }

        Some(GlyphBitmap {
            data: rgba_data,
            width: bitmap_width as u32,
            height: bitmap_height as u32,
            bearing_x: left as f32 - padding as f32,
            bearing_y,
            advance,
        })
    }
}

impl AndroidGlyphRasterizer {
    /// Rasterize a grapheme cluster (which may be multiple Unicode code points)
    /// This handles emoji sequences like 🎤️ (microphone + variation selector)
    pub fn rasterize_grapheme(&mut self, grapheme: &str, font: &FontDescriptor) -> Option<GlyphBitmap> {
        // For single-char graphemes, use the standard path
        let mut chars = grapheme.chars();
        let first_char = chars.next()?;

        // If it's a single character, use the standard rasterize_glyph
        if chars.next().is_none() {
            return self.rasterize_glyph(first_char, font);
        }

        // Multi-character grapheme - rasterize the whole string
        let vm = get_java_vm()?;
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                log::error!("Failed to attach JNI thread for grapheme: {:?}", e);
                return None;
            }
        };

        if env.exception_check().unwrap_or(true) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
            log::error!("JNI exception pending before rasterize_grapheme");
            return None;
        }

        let paint = self.create_paint(&mut env, font)?;

        // Measure the full grapheme
        let (left, top, right, bottom) = self.measure_text_bounds(&mut env, &paint, grapheme)?;
        let width = (right - left).max(1);
        let height = (bottom - top).max(1);
        let advance = self.measure_text_advance(&mut env, &paint, grapheme)?;

        log::info!(
            "GRAPHEME '{}' ({} chars): bounds=({},{},{},{}), advance={:.1}",
            grapheme, grapheme.chars().count(),
            left, top, right, bottom, advance
        );

        let padding = 2;
        let bitmap_width = width + padding * 2;
        let bitmap_height = height + padding * 2;

        if !self.ensure_bitmap(&mut env, bitmap_width, bitmap_height) {
            return None;
        }

        let bitmap = self.bitmap_ref.as_ref()?;
        let canvas = self.canvas_ref.as_ref()?;

        env.call_method(bitmap, "eraseColor", "(I)V", &[JValue::Int(0)]).ok()?;

        let draw_x = (padding - left) as f32;
        let draw_y = (padding - top) as f32;

        let paint_obj: &JObject = paint.as_ref();
        let j_text = env.new_string(grapheme).ok()?;
        env.call_method(
            canvas,
            "drawText",
            "(Ljava/lang/String;FFLandroid/graphics/Paint;)V",
            &[
                JValue::Object(&j_text),
                JValue::Float(draw_x),
                JValue::Float(draw_y),
                JValue::Object(paint_obj),
            ],
        ).ok()?;

        // Extract pixels
        let pixel_count = (bitmap_width * bitmap_height) as usize;
        let mut pixels = vec![0i32; pixel_count];
        let j_pixels = env.new_int_array(pixel_count as i32).ok()?;

        env.call_method(
            bitmap,
            "getPixels",
            "([IIIIIII)V",
            &[
                JValue::Object(&j_pixels),
                JValue::Int(0),
                JValue::Int(bitmap_width),
                JValue::Int(0),
                JValue::Int(0),
                JValue::Int(bitmap_width),
                JValue::Int(bitmap_height),
            ],
        ).ok()?;

        env.get_int_array_region(&j_pixels, 0, &mut pixels).ok()?;

        // Convert ARGB to RGBA
        let mut rgba_data = Vec::with_capacity(pixel_count * 4);
        for pixel in pixels {
            let a = ((pixel >> 24) & 0xFF) as u8;
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            rgba_data.push(r);
            rgba_data.push(g);
            rgba_data.push(b);
            rgba_data.push(a);
        }

        let bearing_y = (-top) as f32 + padding as f32;

        Some(GlyphBitmap {
            data: rgba_data,
            width: bitmap_width as u32,
            height: bitmap_height as u32,
            bearing_x: left as f32 - padding as f32,
            bearing_y,
            advance,
        })
    }
}

/// Measure text width (for layout purposes)
pub fn measure_text_width(text: &str, font: &FontDescriptor) -> Option<f32> {
    let vm = get_java_vm()?;
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            log::error!("measure_text_width: failed to attach thread: {:?}", e);
            return None;
        }
    };

    let mut rasterizer = AndroidGlyphRasterizer::new();
    let paint = match rasterizer.create_paint(&mut env, font) {
        Some(p) => p,
        None => {
            log::error!("measure_text_width: create_paint failed for text='{}' font_size={}", text, font.size);
            return None;
        }
    };

    let result = rasterizer.measure_text_advance(&mut env, &paint, text);
    if result.is_none() {
        log::error!("measure_text_width: measure_text_advance failed for text='{}'", text);
    }
    result
}

/// Measure text to a specific cursor position (for text editing)
pub fn measure_text_to_cursor(text: &str, cursor_index: usize, font: &FontDescriptor) -> Option<f32> {
    if cursor_index == 0 {
        return Some(0.0);
    }

    let substring: String = text.chars().take(cursor_index).collect();
    measure_text_width(&substring, font)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require an Android environment with JNI
    // They will be skipped when running on other platforms

    #[test]
    #[ignore] // Requires Android JNI environment
    fn test_rasterizer_creation() {
        let rasterizer = AndroidGlyphRasterizer::new();
        assert!(rasterizer.paint_cache.is_empty());
    }
}
