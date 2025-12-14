# Text Rendering FFI Design

## Architecture Flow

```
┌─────────────────────────────────────────────────────────────┐
│ Go Application Code                                         │
│                                                              │
│  widget.Text("Hello")                                       │
│    .Font("sans")           ← Tailwind-style API             │
│    .Size("text-lg")                                         │
│    .Weight("font-bold")                                     │
│    .Color("text-blue-500")                                  │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ↓
┌─────────────────────────────────────────────────────────────┐
│ Go Codegen / Theme Resolution                               │
│                                                              │
│  "text-lg"      → 18.0 px (from theme)                      │
│  "font-bold"    → 700 (weight number)                       │
│  "sans"         → "San Francisco" (macOS system font)       │
│  "text-blue-500" → 0x3B82F6FF (resolved color)              │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ↓
┌─────────────────────────────────────────────────────────────┐
│ Go Render Batch (in memory)                                 │
│                                                              │
│  []RenderCommand{                                           │
│    DrawText{                                                │
│      X: 100.0,                                              │
│      Y: 100.0,                                              │
│      Text: "Hello",                                         │
│      FontFamily: "San Francisco",  ← Resolved               │
│      FontWeight: 700,               ← Exact number          │
│      FontSize: 18.0,                ← Exact px              │
│      Color: 0x3B82F6FF,             ← Exact RGBA            │
│      MaxWidth: 400.0,               ← Exact px              │
│      LineHeight: 1.5,               ← Exact multiplier      │
│      LetterSpacing: 0.0,            ← Exact em              │
│      Alignment: AlignLeft,          ← Exact enum            │
│    },                                                        │
│    // ... more commands                                     │
│  }                                                           │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ↓ FFI Call (single batch)
┌─────────────────────────────────────────────────────────────┐
│ Rust FFI Layer (ffi.rs)                                     │
│                                                              │
│  #[no_mangle]                                               │
│  pub extern "C" fn engine_render_batch(                     │
│      commands_ptr: *const RenderCommand,                    │
│      commands_len: usize                                    │
│  )                                                           │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ↓
┌─────────────────────────────────────────────────────────────┐
│ Rust Engine (render.rs, platform/metal.rs)                 │
│                                                              │
│  • Load font: "San Francisco", weight 700, size 18px       │
│  • Shape text: "Hello" → positioned glyphs                 │
│  • Render glyphs to atlas                                   │
│  • Generate textured quads                                  │
│  • Draw to GPU                                              │
└─────────────────────────────────────────────────────────────┘
```

## FFI Command Structure

### Text Rendering Command (C-compatible)

```rust
// engine/src/ffi.rs

#[repr(C)]
pub struct DrawTextCommand {
    // Position
    pub x: f32,
    pub y: f32,

    // Text content
    pub text_ptr: *const u8,
    pub text_len: usize,

    // Font (exact values, already resolved)
    pub font_family_ptr: *const u8,
    pub font_family_len: usize,
    pub font_weight: u16,        // 100-900
    pub font_style: u8,          // 0 = Normal, 1 = Italic
    pub font_size: f32,          // Points (px on web, pt on native)

    // Color
    pub color: u32,              // 0xRRGGBBAA

    // Layout (exact values, already resolved)
    pub max_width: f32,          // 0.0 = no constraint
    pub max_height: f32,         // 0.0 = no constraint
    pub line_height: f32,        // Multiplier (1.5 = 150% of font size)
    pub letter_spacing: f32,     // em units
    pub word_spacing: f32,       // em units

    // Alignment (encoded as u8)
    pub alignment: u8,           // 0=Left, 1=Center, 2=Right, 3=Justify
    pub vertical_align: u8,      // 0=Top, 1=Middle, 2=Bottom, 3=Baseline

    // Behavior (encoded as u8)
    pub word_break: u8,          // 0=Normal, 1=BreakAll, 2=KeepAll, 3=BreakWord
    pub overflow: u8,            // 0=Clip, 1=Ellipsis, 2=Wrap
    pub white_space: u8,         // 0=Normal, 1=NoWrap, 2=Pre, 3=PreWrap
}

#[repr(C)]
pub struct DrawRectCommand {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: u32,
    pub border_radius: f32,
}

#[repr(C)]
#[repr(u8)]
pub enum RenderCommandType {
    DrawRect = 0,
    DrawText = 1,
    PushClip = 2,
    PopClip = 3,
    SetOpacity = 4,
    Clear = 5,
}

#[repr(C)]
pub struct RenderCommand {
    pub cmd_type: RenderCommandType,
    pub data: RenderCommandData,
}

#[repr(C)]
pub union RenderCommandData {
    pub draw_rect: DrawRectCommand,
    pub draw_text: DrawTextCommand,
    // ... other command types
}
```

## Go Side (Codegen Output)

### Go Structures

```go
// internal/render/command.go

type DrawTextCommand struct {
    X float32
    Y float32

    // Text content
    Text string

    // Font (resolved from theme)
    FontFamily    string  // "San Francisco", "Roboto", "Segoe UI"
    FontWeight    uint16  // 100-900
    FontStyle     uint8   // 0 = Normal, 1 = Italic
    FontSize      float32 // Exact px value

    // Color (resolved from theme)
    Color uint32 // 0xRRGGBBAA

    // Layout (resolved from theme/classes)
    MaxWidth       float32 // Exact px value, 0 = no constraint
    MaxHeight      float32
    LineHeight     float32 // Multiplier (1.5)
    LetterSpacing  float32 // em units (0.05)
    WordSpacing    float32

    // Alignment
    Alignment      uint8 // 0=Left, 1=Center, 2=Right, 3=Justify
    VerticalAlign  uint8

    // Behavior
    WordBreak   uint8 // 0=Normal, 1=BreakAll, 2=KeepAll, 3=BreakWord
    Overflow    uint8 // 0=Clip, 1=Ellipsis, 2=Wrap
    WhiteSpace  uint8 // 0=Normal, 1=NoWrap, 2=Pre, 3=PreWrap
}

type RenderCommand struct {
    Type CommandType
    Data interface{} // DrawTextCommand, DrawRectCommand, etc.
}
```

### Theme Configuration (Go)

```go
// theme/config.go

type Theme struct {
    FontSizes map[string]float32
    FontWeights map[string]uint16
    FontFamilies map[string]string
    LineHeights map[string]float32
    LetterSpacing map[string]float32
    // ... colors, spacing, etc.
}

var DefaultTheme = Theme{
    FontSizes: map[string]float32{
        "text-xs":   12.0,
        "text-sm":   14.0,
        "text-base": 16.0,
        "text-lg":   18.0,
        "text-xl":   20.0,
        "text-2xl":  24.0,
        "text-3xl":  30.0,
        "text-4xl":  36.0,
        "text-5xl":  48.0,
        "text-6xl":  60.0,
        "text-7xl":  72.0,
        "text-8xl":  96.0,
        "text-9xl":  128.0,
    },

    FontWeights: map[string]uint16{
        "font-thin":       100,
        "font-extralight": 200,
        "font-light":      300,
        "font-normal":     400,
        "font-medium":     500,
        "font-semibold":   600,
        "font-bold":       700,
        "font-extrabold":  800,
        "font-black":      900,
    },

    FontFamilies: map[string]string{
        // Platform-specific defaults
        "sans":  getPlatformSansFont(),  // "San Francisco" on macOS
        "serif": getPlatformSerifFont(), // "Times New Roman"
        "mono":  getPlatformMonoFont(),  // "SF Mono" on macOS
    },

    LineHeights: map[string]float32{
        "leading-none":    1.0,
        "leading-tight":   1.25,
        "leading-snug":    1.375,
        "leading-normal":  1.5,
        "leading-relaxed": 1.625,
        "leading-loose":   2.0,
    },

    LetterSpacing: map[string]float32{
        "tracking-tighter": -0.05,
        "tracking-tight":   -0.025,
        "tracking-normal":  0.0,
        "tracking-wide":    0.025,
        "tracking-wider":   0.05,
        "tracking-widest":  0.1,
    },
}

func getPlatformSansFont() string {
    switch runtime.GOOS {
    case "darwin":
        return "San Francisco"
    case "windows":
        return "Segoe UI"
    case "linux":
        return "Ubuntu"
    case "android":
        return "Roboto"
    default:
        return "sans-serif"
    }
}
```

### Widget API → Resolved Command (Go)

```go
// Example codegen output

func exampleWidget() Widget {
    return widget.Text("Hello, World!").
        Font("sans").
        Size("text-lg").
        Weight("font-bold").
        Color("text-blue-500").
        MaxWidth(400).
        Align("text-center").
        Leading("leading-relaxed")
}

// Codegen produces:
func exampleWidget_Render(ctx *RenderContext) []RenderCommand {
    theme := ctx.Theme

    return []RenderCommand{
        {
            Type: CommandDrawText,
            Data: DrawTextCommand{
                X: 100.0,
                Y: 100.0,
                Text: "Hello, World!",

                // Resolved from theme
                FontFamily: theme.FontFamilies["sans"],      // "San Francisco"
                FontWeight: theme.FontWeights["font-bold"],  // 700
                FontStyle: 0,                                 // Normal
                FontSize: theme.FontSizes["text-lg"],        // 18.0

                // Resolved from theme
                Color: theme.Colors["text-blue-500"],        // 0x3B82F6FF

                // Resolved layout
                MaxWidth: 400.0,                              // Explicit value
                LineHeight: theme.LineHeights["leading-relaxed"], // 1.625
                LetterSpacing: 0.0,                           // Default

                // Resolved alignment
                Alignment: AlignCenter,                       // 1

                // Defaults
                WordBreak: WordBreakNormal,
                Overflow: OverflowWrap,
                WhiteSpace: WhiteSpaceNormal,
            },
        },
    }
}
```

## Rust Side (Engine Implementation)

### Receiving FFI Commands

```rust
// engine/src/ffi.rs

#[no_mangle]
pub extern "C" fn engine_render_batch(
    commands_ptr: *const RenderCommand,
    commands_len: usize,
) -> i32 {
    // Safety: Trust Go to provide valid pointers
    let commands = unsafe {
        std::slice::from_raw_parts(commands_ptr, commands_len)
    };

    let mut render_commands = Vec::new();

    for cmd in commands {
        match cmd.cmd_type {
            RenderCommandType::DrawText => {
                let text_cmd = unsafe { &cmd.data.draw_text };

                // Convert C strings to Rust
                let text = unsafe {
                    let slice = std::slice::from_raw_parts(
                        text_cmd.text_ptr,
                        text_cmd.text_len
                    );
                    std::str::from_utf8_unchecked(slice).to_string()
                };

                let font_family = unsafe {
                    let slice = std::slice::from_raw_parts(
                        text_cmd.font_family_ptr,
                        text_cmd.font_family_len
                    );
                    std::str::from_utf8_unchecked(slice).to_string()
                };

                render_commands.push(crate::render::RenderCommand::DrawText {
                    x: text_cmd.x,
                    y: text_cmd.y,
                    text,
                    font: FontDescriptor {
                        family: font_family,
                        weight: text_cmd.font_weight,
                        style: text_cmd.font_style.into(),
                        size: text_cmd.font_size,
                    },
                    color: text_cmd.color,
                    layout: TextLayoutConfig {
                        max_width: if text_cmd.max_width > 0.0 {
                            Some(text_cmd.max_width)
                        } else {
                            None
                        },
                        line_height: text_cmd.line_height,
                        letter_spacing: text_cmd.letter_spacing,
                        alignment: text_cmd.alignment.into(),
                        // ... etc
                    },
                });
            },

            RenderCommandType::DrawRect => {
                let rect_cmd = unsafe { &cmd.data.draw_rect };
                render_commands.push(crate::render::RenderCommand::DrawRect {
                    x: rect_cmd.x,
                    y: rect_cmd.y,
                    width: rect_cmd.width,
                    height: rect_cmd.height,
                    color: rect_cmd.color,
                    border_radius: rect_cmd.border_radius,
                });
            },

            // ... other command types
        }
    }

    // Execute rendering
    if let Err(e) = GLOBAL_ENGINE.execute_commands(&render_commands) {
        eprintln!("Render error: {}", e);
        return -1;
    }

    0 // Success
}
```

### Internal Rendering (No Tailwind)

```rust
// engine/src/render.rs

pub struct FontDescriptor {
    pub family: String,      // Exact name: "San Francisco", not "sans"
    pub weight: u16,         // Exact number: 700, not "bold"
    pub style: FontStyle,    // Enum value
    pub size: f32,           // Exact px value: 18.0
}

pub struct TextLayoutConfig {
    pub max_width: Option<f32>,      // Exact px or None
    pub line_height: f32,            // Exact multiplier: 1.5
    pub letter_spacing: f32,         // Exact em: 0.05
    pub alignment: TextAlign,        // Enum value
    // ... all exact values
}

pub enum RenderCommand {
    DrawText {
        x: f32,
        y: f32,
        text: String,
        font: FontDescriptor,         // All resolved
        color: u32,
        layout: TextLayoutConfig,     // All resolved
    },
    // ...
}
```

## Benefits of This Design

✅ **Go owns Tailwind mapping** - Theme resolution happens once at codegen
✅ **Rust receives exact values** - No string parsing, no theme lookup
✅ **FFI is simple** - Just numbers and strings, no complex structures
✅ **Performance** - No runtime resolution of classes
✅ **Type safety** - Go enforces theme constraints at compile time
✅ **Platform-specific** - Go can choose different fonts per platform
✅ **Batching friendly** - All commands have resolved values ready to render

## Example Full Flow

```go
// 1. Go Widget Code
widget.Text("Hello").
    Size("text-lg").
    Weight("font-bold")

// 2. Codegen resolves (compile time or runtime)
DrawTextCommand{
    FontSize: 18.0,    // from theme["text-lg"]
    FontWeight: 700,   // from theme["font-bold"]
}

// 3. FFI sends exact values
engine_render_batch([
    DrawText{font_size: 18.0, font_weight: 700, ...}
])

// 4. Rust loads exact font
font_manager.load_font("San Francisco", 700, Normal, 18.0)

// 5. Rust shapes and renders
text_shaper.shape(text, font) → glyphs
glyph_atlas.render(glyphs) → GPU
```

## Summary

- **Go**: Handles Tailwind → exact values conversion via codegen/theme
- **FFI**: Passes exact, resolved values (no strings like "text-lg")
- **Rust**: Receives concrete rendering instructions, no interpretation needed

This is the optimal design for performance and separation of concerns!
