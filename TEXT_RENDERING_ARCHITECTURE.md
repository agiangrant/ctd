# Text Rendering Architecture

## Overview

Text rendering is a **first-class citizen** in Centered, with Tailwind-style utilities for fonts, sizing, spacing, and layout.

## Requirements

### 1. Font System - Flexible & Customizable

**Font Descriptor** - Complete font specification:
```rust
pub struct FontDescriptor {
    pub family: FontFamily,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub size: f32,  // Points
}

pub enum FontFamily {
    System,           // San Francisco on macOS, Roboto on Android, etc.
    SansSerif,        // Generic sans-serif
    Serif,            // Generic serif
    Monospace,        // Generic monospace
    Custom(String),   // Custom font name
}

pub enum FontWeight {
    Thin,        // 100
    ExtraLight,  // 200
    Light,       // 300
    Normal,      // 400
    Medium,      // 500
    SemiBold,    // 600
    Bold,        // 700
    ExtraBold,   // 800
    Black,       // 900
}

pub enum FontStyle {
    Normal,
    Italic,
}
```

### 2. Text Layout - Smart Line Breaking

**Layout Configuration**:
```rust
pub struct TextLayoutConfig {
    // Constraints
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub max_lines: Option<usize>,

    // Spacing
    pub line_height: LineHeight,
    pub letter_spacing: f32,
    pub word_spacing: f32,

    // Alignment
    pub alignment: TextAlign,
    pub vertical_align: VerticalAlign,

    // Breaking
    pub word_break: WordBreak,
    pub overflow: TextOverflow,
    pub white_space: WhiteSpace,
}

pub enum LineHeight {
    Normal,          // 1.5x font size (default)
    Tight,           // 1.25x
    Loose,           // 2x
    Custom(f32),     // Explicit multiplier
}

pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

pub enum WordBreak {
    Normal,      // Break at word boundaries
    BreakAll,    // Break anywhere
    KeepAll,     // No breaks (CJK)
    BreakWord,   // Break long words if needed
}

pub enum TextOverflow {
    Clip,        // Cut off
    Ellipsis,    // Add "..."
    Wrap,        // Wrap to next line
}

pub enum WhiteSpace {
    Normal,      // Collapse whitespace, wrap
    NoWrap,      // Collapse whitespace, no wrap
    Pre,         // Preserve whitespace, no wrap
    PreWrap,     // Preserve whitespace, wrap
}
```

### 3. Tailwind-Style Text Utilities

Map Tailwind classes to our system:

#### Font Family
```
font-sans     → FontFamily::SansSerif
font-serif    → FontFamily::Serif
font-mono     → FontFamily::Monospace
```

#### Font Size (Tailwind scale)
```
text-xs       → 12px (0.75rem)
text-sm       → 14px (0.875rem)
text-base     → 16px (1rem)
text-lg       → 18px (1.125rem)
text-xl       → 20px (1.25rem)
text-2xl      → 24px (1.5rem)
text-3xl      → 30px (1.875rem)
text-4xl      → 36px (2.25rem)
text-5xl      → 48px (3rem)
text-6xl      → 60px (3.75rem)
text-7xl      → 72px (4.5rem)
text-8xl      → 96px (6rem)
text-9xl      → 128px (8rem)
```

#### Font Weight
```
font-thin       → FontWeight::Thin (100)
font-extralight → FontWeight::ExtraLight (200)
font-light      → FontWeight::Light (300)
font-normal     → FontWeight::Normal (400)
font-medium     → FontWeight::Medium (500)
font-semibold   → FontWeight::SemiBold (600)
font-bold       → FontWeight::Bold (700)
font-extrabold  → FontWeight::ExtraBold (800)
font-black      → FontWeight::Black (900)
```

#### Line Height
```
leading-none    → 1.0
leading-tight   → 1.25
leading-snug    → 1.375
leading-normal  → 1.5
leading-relaxed → 1.625
leading-loose   → 2.0
```

#### Letter Spacing
```
tracking-tighter → -0.05em
tracking-tight   → -0.025em
tracking-normal  → 0em
tracking-wide    → 0.025em
tracking-wider   → 0.05em
tracking-widest  → 0.1em
```

#### Text Alignment
```
text-left    → TextAlign::Left
text-center  → TextAlign::Center
text-right   → TextAlign::Right
text-justify → TextAlign::Justify
```

#### Text Overflow
```
truncate      → TextOverflow::Ellipsis + WhiteSpace::NoWrap
text-ellipsis → TextOverflow::Ellipsis
text-clip     → TextOverflow::Clip
```

#### Word Break
```
break-normal → WordBreak::Normal
break-words  → WordBreak::BreakWord
break-all    → WordBreak::BreakAll
```

## Architecture Components

### 1. Font Manager (Platform-Specific)

**macOS** - Use Core Text:
```rust
pub struct FontManager {
    font_cache: HashMap<FontDescriptor, CTFont>,
}

impl FontManager {
    pub fn load_font(&mut self, descriptor: &FontDescriptor) -> CTFont {
        // Cache fonts to avoid re-loading
        // Use CTFontCreateWithName, CTFontCreateWithFontDescriptor
    }

    pub fn get_glyph_info(&self, font: &CTFont, text: &str) -> Vec<GlyphInfo> {
        // Use CTLine, CTRun for text shaping
        // Returns positioned glyphs with metrics
    }
}
```

**Other platforms**:
- **iOS**: Core Text (same as macOS)
- **Android**: FreeType + HarfBuzz
- **Linux**: FreeType + HarfBuzz
- **Windows**: DirectWrite

### 2. Text Shaper

Converts text + font → positioned glyphs:

```rust
pub struct TextShaper {
    font_manager: FontManager,
}

pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub lines: Vec<TextLine>,
    pub bounds: Rect,
}

pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub position: Point,
    pub advance: f32,
}

pub struct TextLine {
    pub glyphs: Range<usize>,  // Index into ShapedText.glyphs
    pub baseline: f32,
    pub width: f32,
    pub height: f32,
}

impl TextShaper {
    pub fn shape(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        config: &TextLayoutConfig,
    ) -> ShapedText {
        // 1. Load font from cache
        // 2. Shape text into glyphs (CTLine/HarfBuzz)
        // 3. Apply line breaking based on config.max_width
        // 4. Apply alignment
        // 5. Apply letter/word spacing
        // 6. Calculate bounds
    }
}
```

### 3. Glyph Atlas (GPU Texture)

Cache rendered glyphs in a texture:

```rust
pub struct GlyphAtlas {
    pub texture: MetalTexture,  // 2048x2048 or larger
    pub allocator: AtlasAllocator,
    pub cache: HashMap<GlyphKey, GlyphRegion>,
}

#[derive(Hash, Eq, PartialEq)]
pub struct GlyphKey {
    pub font: FontDescriptor,
    pub glyph_id: u32,
}

pub struct GlyphRegion {
    pub uv_rect: Rect,      // UV coordinates in atlas
    pub metrics: GlyphMetrics,
}

pub struct GlyphMetrics {
    pub bearing: Point,     // Offset from baseline
    pub size: Size,         // Glyph dimensions
    pub advance: f32,       // Horizontal advance
}

impl GlyphAtlas {
    pub fn get_or_render(
        &mut self,
        glyph_key: GlyphKey,
        font_manager: &FontManager,
    ) -> &GlyphRegion {
        // If not in cache:
        // 1. Render glyph to bitmap (Core Text)
        // 2. Allocate space in atlas
        // 3. Upload to GPU texture
        // 4. Store in cache
    }
}
```

### 4. Text Renderer (Metal)

Render text using glyph atlas:

```rust
impl MetalBackend {
    pub fn draw_text(
        &mut self,
        shaped_text: &ShapedText,
        position: Point,
        color: Color,
    ) -> Result<(), PlatformError> {
        // For each glyph:
        // 1. Get glyph region from atlas
        // 2. Create textured quad with UV coordinates
        // 3. Add to vertex buffer
        // 4. Batch all glyphs into single draw call

        // Use textured pipeline (different from rect pipeline)
        encoder.set_render_pipeline_state(&self.text_pipeline);
        encoder.set_texture(0, &self.glyph_atlas.texture);
        encoder.draw_primitives(...);
    }
}
```

**Text Shader (MSL)**:
```metal
struct TextVertex {
    float2 position [[attribute(0)]];
    float2 texCoord [[attribute(1)]];
    float4 color [[attribute(2)]];
};

vertex TextVertexOut text_vertex(
    TextVertex in [[stage_in]],
    constant float2& viewportSize [[buffer(1)]]
) {
    // Same coordinate transform as rect shader
    TextVertexOut out;
    float2 clipPosition = (in.position / viewportSize) * 2.0 - 1.0;
    clipPosition.y = -clipPosition.y;
    out.position = float4(clipPosition, 0.0, 1.0);
    out.texCoord = in.texCoord;
    out.color = in.color;
    return out;
}

fragment float4 text_fragment(
    TextVertexOut in [[stage_in]],
    texture2d<float> glyphTexture [[texture(0)]]
) {
    constexpr sampler textureSampler(filter::linear);
    float alpha = glyphTexture.sample(textureSampler, in.texCoord).r;
    return float4(in.color.rgb, in.color.a * alpha);
}
```

## Rendering Pipeline Integration

### Updated RenderCommand

```rust
pub enum RenderCommand {
    DrawRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: u32,
        border_radius: f32,
    },

    DrawText {
        x: f32,
        y: f32,
        text: String,
        font: FontDescriptor,
        color: u32,
        layout: TextLayoutConfig,
    },

    // ... other commands
}
```

### High-Level API Usage

```rust
// Simple text
RenderCommand::DrawText {
    x: 100.0,
    y: 100.0,
    text: "Hello, World!".to_string(),
    font: FontDescriptor {
        family: FontFamily::System,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
        size: 16.0,
    },
    color: 0x000000FF,
    layout: TextLayoutConfig::default(),
}

// Tailwind-style: "text-2xl font-bold text-center"
RenderCommand::DrawText {
    x: 0.0,
    y: 50.0,
    text: "Centered Title".to_string(),
    font: FontDescriptor {
        family: FontFamily::System,
        weight: FontWeight::Bold,
        style: FontStyle::Normal,
        size: 24.0,  // text-2xl
    },
    color: 0x000000FF,
    layout: TextLayoutConfig {
        max_width: Some(800.0),
        alignment: TextAlign::Center,
        ..Default::default()
    },
}

// Multi-line with word wrap: "text-base leading-relaxed break-words"
RenderCommand::DrawText {
    x: 50.0,
    y: 200.0,
    text: "Long paragraph text that should wrap...".to_string(),
    font: FontDescriptor {
        family: FontFamily::System,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
        size: 16.0,  // text-base
    },
    color: 0x333333FF,
    layout: TextLayoutConfig {
        max_width: Some(400.0),
        line_height: LineHeight::Loose,  // leading-relaxed (1.625)
        word_break: WordBreak::BreakWord,  // break-words
        ..Default::default()
    },
}
```

## Implementation Plan

### Phase 1: Core Text Structures (Engine)
1. Define `FontDescriptor`, `TextLayoutConfig`, etc. in `engine/src/style.rs`
2. Update `RenderCommand::DrawText` to use full descriptor
3. Add Tailwind mapping utilities

### Phase 2: Font Manager (Platform-Specific)
1. Implement `FontManager` for macOS using Core Text
2. Font loading and caching
3. Basic glyph metrics

### Phase 3: Text Shaping
1. Implement `TextShaper` using Core Text (CTLine, CTRun)
2. Line breaking algorithm
3. Alignment and spacing

### Phase 4: Glyph Atlas
1. Create atlas texture (2048x2048)
2. Atlas allocator (bin packing)
3. Glyph rendering to bitmap
4. Upload to GPU texture

### Phase 5: Metal Text Renderer
1. Create text rendering pipeline (separate from rect pipeline)
2. Text vertex shader with UV coordinates
3. Text fragment shader with texture sampling
4. Batch text rendering

### Phase 6: Go Integration
1. FFI for font descriptors
2. FFI for text layout config
3. Widget text styling API

## Performance Considerations

### Caching Strategy
- **Font cache**: Keep loaded fonts in memory (CTFont objects)
- **Glyph atlas**: 2048x2048 texture = ~4,000 unique glyphs at 24px
- **Shaped text cache**: Cache shaped text for static strings
- **Layout cache**: Cache layouts for fixed-width containers

### Optimization Targets
- **Glyph atlas upload**: Batch multiple glyphs per frame
- **Draw call batching**: Single draw for all text in frame
- **SDF rendering** (future): Sharp text at any scale with single glyph per character

### Memory Budget
- Font cache: ~1-2 MB per loaded font
- Glyph atlas: 16 MB for RGBA 2048x2048
- Shaped text cache: ~1 KB per cached string
- **Total**: ~20-50 MB for typical app

## Testing Strategy

### Test Cases
1. **Font loading**: All weights, styles, families
2. **Line breaking**: Various widths, long words, CJK
3. **Alignment**: Left, center, right, justify
4. **Overflow**: Clip, ellipsis, wrap
5. **Performance**: 1000+ glyphs at 60fps
6. **Memory**: Atlas eviction, cache limits

### Test Text Samples
- English: "The quick brown fox..."
- CJK: "日本語のテキスト"
- Arabic: "النص العربي" (RTL support - future)
- Emoji: "Hello 👋 World 🌍" (color emoji - future)
- Long words: "Supercalifragilisticexpialidocious"
- Numbers: "1234567890"

## Future Enhancements

1. **SDF (Signed Distance Field) rendering** - Sharp text at any scale
2. **RTL (Right-to-Left) support** - Arabic, Hebrew
3. **Ligatures** - "fi", "fl" ligatures
4. **Color emoji** - Full emoji rendering
5. **Text effects** - Shadow, outline, gradient
6. **Rich text** - Mixed fonts/colors in single block
7. **Text selection** - Interactive text
8. **Font subsetting** - Reduce font file sizes

## API Examples (Go)

```go
// Simple text
widget.Text("Hello").
    Font("sans").
    Size("text-lg").
    Weight("font-bold").
    Color("text-blue-500")

// Multi-line paragraph
widget.Text(longText).
    Font("serif").
    Size("text-base").
    Leading("leading-relaxed").
    MaxWidth(400).
    Align("text-justify").
    Break("break-words")

// Custom font
widget.Text("Code Sample").
    Font("JetBrains Mono").
    Size("text-sm").
    Weight("font-medium").
    Tracking("tracking-wide")
```

## Summary

This architecture provides:
- ✅ **Multiple customizable fonts** (family, weight, style, size)
- ✅ **Smart line breaking** (word wrap, character wrap, max width)
- ✅ **Tailwind-first design** (all Tailwind text utilities supported)
- ✅ **Performance-optimized** (glyph atlas, batching, caching)
- ✅ **Cross-platform ready** (Core Text, FreeType, DirectWrite)
- ✅ **Production-ready** (comprehensive layout control)

Next step: Implement Phase 1 (Core Text Structures) in the engine.
