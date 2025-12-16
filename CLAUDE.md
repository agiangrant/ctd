# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Centered is a cross-platform native GUI framework combining Rust's performance with Go's development velocity. It features a Tailwind-inspired styling system and supports both immediate mode and retained mode rendering. The project is in early development (Phase 1: Foundation).

**Key Philosophy**: Platform-appropriate UX with shared core technology. Mobile and desktop are separate applications using the same Rust engine, not a single compromised codebase.

## Build Commands

The project uses Task (Taskfile.yml) for build automation:

```bash
# Build Rust engine (debug)
task build

# Build Rust engine (release/optimized)
task build:release

# Run tests
task test

# Run tests in watch mode
task test:watch

# Format Rust code
task fmt

# Check formatting without modifying
task fmt:check

# Run Clippy linter (fails on warnings)
task clippy

# Check code without building
task check

# Run all CI checks (format, clippy, test, build)
task ci

# Development mode - watch and rebuild on changes
task dev

# Generate and open documentation
task doc

# Clean build artifacts
task clean

# Generate Tailwind utility classes from theme.toml
task generate

# Watch theme.toml and regenerate on changes
task generate:watch
```

The engine crate builds as lib, cdylib, and staticlib for FFI consumption by Go.

### Go Commands

```bash
# Run Go tests (widget API, Tailwind parser)
go test -v ./widget_test.go ./widget.go
go test -v ./tw/

# Run benchmarks
go test -bench=. -benchmem ./tw/
go test -bench=BenchmarkWidget -benchmem ./widget_test.go ./widget.go

# Build example application (requires Rust engine to be built first)
cd engine && cargo build --release && cd ..
go build -o bin/example ./examples/basic/

# Generate Tailwind utilities
go run tools/generate/main.go
```

## Architecture

### Two-Layer Design

**Rust Engine (`engine/`)**: Platform abstraction, rendering backends, widget primitives, layout engine (flexbox), style system (Tailwind utilities), event dispatch. Designed for zero-cost abstractions and minimal FFI overhead.

**Go Framework** (implemented): Idiomatic Go API with Tailwind-first styling, widget composition, FFI bindings. Located in:
- `internal/ffi/ffi.go`: CGO bindings to Rust engine
- `widget.go`: High-level widget API (VStack, HStack, Text, Button, etc.)
- `engine.go`: Engine lifecycle management and frame rendering
- `tw/`: Complete Tailwind CSS parser with 2,117+ utility classes and arbitrary value support
- `retained/`: Retained mode widget system with automatic layout, event dispatch, and state management

### Rendering Modes

The framework supports both rendering paradigms:

- **Immediate Mode**: Go builds command buffer/scene per frame, single FFI call per frame with batched data. Best for dev tools, games, interactive visualizations.
- **Retained Mode**: Go sends widget tree updates only on state changes, Rust maintains tree and renders independently. Best for traditional apps, battery efficiency, accessibility integration.

**Critical**: Widgets are mode-agnostic. The rendering backend adapts to the mode, not the widgets.

### FFI Strategy

Performance is critical:
- **One FFI call per frame maximum** (immediate mode)
- **FFI calls only on state changes** (retained mode)
- Batched updates and command buffers
- Currently uses JSON for serialization (can optimize to shared memory later)

Key FFI functions in `engine/src/ffi.rs`:
- `centered_app_run(config, callback)` - Run app with Rust-owned window and event loop
- `centered_app_request_exit()` - Request graceful app shutdown
- `centered_backend_load_image(data, len)` → texture_id (or negative error)
- `centered_backend_load_image_file(path)` → texture_id (or negative error)
- `centered_backend_unload_image(texture_id)` - Free GPU texture resources
- `centered_measure_text_width(text, font_name, font_size)` → width in pixels (for layout)

Legacy FFI (for Go-owned window mode, not currently used):
- `centered_engine_init(config_json)` → `EngineHandle`
- `centered_engine_submit_frame(handle, frame_json)` → events JSON
- `centered_free_string(ptr)` - free JSON strings returned by engine

Global backend storage uses `Mutex<Option<WgpuBackend>>` for FFI access.

### Go Tailwind Parser (`tw/`)

**Status**: Fully implemented with comprehensive test coverage.

The Tailwind parser (`tw/parser.go`) provides first-class support for Tailwind CSS semantics in Go:

**Features**:
- 2,117+ predefined utility classes matching Tailwind CSS v4
- Full variant support: state (`hover:`, `focus:`, `active:`, `disabled:`), breakpoints (`sm:`, `md:`, `lg:`, `xl:`, `2xl:`), dark mode (`dark:`)
- Arbitrary value syntax: `w-[33%]`, `h-[250px]`, `bg-[#1da1f2]`, `p-[2.5rem]`, `rotate-[17deg]`
- Zero duplication: Variants share base class objects via pointers
- Code generation from `theme.toml` for custom color palettes and spacing scales

**Supported Categories**:
- Layout: display (flex, grid, block, hidden), position (relative, absolute, fixed)
- Flexbox: direction, justify-content, align-items, gap, grow, shrink
- Grid: grid-cols, grid-rows, col-span, row-span
- Sizing: width, height, min/max variants (including arbitrary values)
- Spacing: padding, margin (all directional variants)
- Colors: 19 colors × 11 shades (text-*, bg-*, border-*)
- Typography: font-size (xs-9xl), font-weight, line-height, letter-spacing, text-align
- Borders: width, radius, color
- Effects: shadows (sm-2xl), opacity (0-100)
- Transforms: scale, rotate, translate (with negatives)
- Transitions: properties, duration (75-1000ms), timing functions
- Interactivity: cursor, overflow, z-index

**Performance**:
- Standard classes: ~1.2μs parse time
- With variants: ~1.3μs parse time
- Arbitrary values: ~2.4μs parse time
- Negligible overhead for real-world UIs

**Parser Output** (`ComputedStyles` struct):
```go
type ComputedStyles struct {
    Base StyleProperties      // Default state
    Hover, Focus, Active, Disabled StyleProperties  // Interactive states
    SM, MD, LG, XL, XXL StyleProperties  // Responsive breakpoints
    Dark struct {  // Dark mode variants
        Base, Hover, Focus, Active, Disabled StyleProperties
    }
}
```

**Code Generation** (`tools/generate/main.go`):
- Reads `theme.toml` for custom colors, spacing, font families
- Generates `tw/generated.go` with ClassMap and helper functions
- Run with `task generate` or `task generate:watch`

**Usage**:
```go
import "github.com/agiangrant/centered/tw"

// Parse Tailwind classes
styles := tw.ParseClasses("bg-blue-500 hover:bg-blue-600 text-white px-4 py-2 rounded-lg")

// Access computed properties
if styles.Base.BackgroundColor != nil {
    color := *styles.Base.BackgroundColor  // 0x3B82F6FF
}

// Arbitrary values work too
styles2 := tw.ParseClasses("w-[80%] h-[400px] bg-[#1da1f2]")
```

**Testing**: See `tw/parser_test.go`, `tw/advanced_test.go`, `tw/arbitrary_test.go`

### Retained Mode Widget System (`retained/`)

**Status**: Fully implemented with layout engine, event dispatch, and text input widgets.

The retained mode system provides a declarative widget API with automatic layout and state management:

**Core Files**:
- `retained/widget.go` - Widget struct, builders, property accessors
- `retained/layout.go` - Flexbox layout engine (Go implementation)
- `retained/event_dispatch.go` - Mouse/keyboard event routing to widgets
- `retained/loop.go` - Event loop integration, frame rendering
- `retained/text_input.go` - TextField and TextArea implementations
- `retained/tailwind.go` - Style property extraction from Tailwind classes
- `retained/animation.go` - Animation system with easing functions and AnimationRegistry
- `retained/scroll.go` - Scroll animation utilities for keyboard avoidance and navigation

**Widget Kinds**:
- Layout: `VStack`, `HStack`, `ZStack`, `Container`, `Spacer`
- Text: `Text`, `Heading`, `Label`
- Input: `Button`, `TextField`, `TextArea`, `Checkbox`, `Radio`, `Slider`
- Custom: Extensible via `KindCustom`

**Layout System**:
The Go-side layout engine (`retained/layout.go`) implements flexbox:
- `calculateLayout()` - Main entry point, processes widget tree
- `calculateIntrinsicSize()` - Computes minimum size based on content
- `layoutChildren()` - Distributes space among children (flex-grow/shrink)
- Supports: flex-direction, justify-content, align-items, gap, padding, margin, percentage widths

**Text Width Measurement**:
Text widgets use FFI to measure actual text width for proper layout:
```go
// Widget caches measured text width
type Widget struct {
    textWidth      float32 // Cached measured width
    textWidthDirty bool    // Invalidated when text/font changes
}

// SetText, SetFontSize, SetFontName mark width dirty
func (w *Widget) SetText(text string) {
    w.text = text
    w.textWidthDirty = true
}

// TextWidth() returns cached width, measuring via FFI if dirty
func (w *Widget) TextWidth() float32 {
    if w.textWidthDirty {
        w.textWidth = ffi.MeasureTextWidth(w.text, w.fontName, w.fontSize)
        w.textWidthDirty = false
    }
    return w.textWidth
}
```

The measurement function is swappable via `SetMeasureTextWidthFunc()` to support future shared memory architecture.

**Event Dispatch**:
- Hit testing: Finds widget at mouse coordinates
- Focus management: Tab navigation, click-to-focus (on mouse down for iOS keyboard compatibility)
- Hover tracking: Mouse enter/leave detection
- Keyboard routing: Key events to focused widget
- Drag detection: For text selection in TextArea, scroll gestures blur focused inputs
- Momentum scrolling: Inertial scroll after drag release

**TextField/TextArea Features**:
- ✅ Cursor positioning (click, arrow keys)
- ✅ Text selection (shift+arrows, drag)
- ✅ Copy/paste (Cmd+C/V/X)
- ✅ Undo/redo (Cmd+Z/Shift+Z)
- ✅ Word navigation (Opt+arrows)
- ✅ Password masking
- ✅ Read-only mode
- ✅ Character filtering
- ✅ Placeholder text with styling
- ✅ Multi-line with scrolling (TextArea)
- ✅ Word wrap with ellipsis
- ✅ Double/triple-click selection

**iOS Software Keyboard** (`engine/src/platform/ios.rs`):
- ✅ UIKeyInput protocol implementation for software keyboard input
- ✅ Hardware keyboard support via UIPress events
- ✅ Keyboard show/hide notifications (UIKeyboardWillShowNotification)
- ✅ Keyboard frame tracking for avoidance
- ✅ Focus triggers keyboard on touch down (required for iOS responder chain)
- ✅ Drag-to-scroll automatically blurs focused inputs and hides keyboard

**Scroll Animation Utilities** (`retained/scroll.go`):
- `ScrollToWidget()` - Animate scroll to make a widget visible
- `ScrollToWidgetWithKeyboard()` - Same but accounts for keyboard height
- `ScrollToY()` - Animate to a specific scroll position (via AnimationBuilder)
- Configurable duration, easing, and padding
- Integrates with AnimationRegistry for 60fps mode
- Reusable for keyboard avoidance, anchor navigation, programmatic scrolling

**Usage Example**:
```go
import "github.com/agiangrant/centered/retained"

func buildUI() *retained.Widget {
    return retained.VStack(
        retained.HStack(
            retained.Text("Welcome").Class("text-2xl font-bold text-white"),
            retained.Spacer(),
            retained.Button("Settings").OnClick(func() { /* ... */ }),
        ).Class("px-4 py-2 bg-gray-800"),

        retained.Container(
            retained.TextField().
                Placeholder("Enter your name").
                Class("w-full px-3 py-2 bg-gray-700 rounded"),
        ).Class("p-4"),
    ).Class("flex-1 bg-gray-900")
}

func main() {
    retained.Run(retained.AppConfig{
        Title: "My App",
        Width: 800, Height: 600,
    }, buildUI)
}
```

## Core Modules

### Widget System (`engine/src/widget.rs`)

- Uses `SlotMap<WidgetId, Widget>` for O(1) lookups and cache-friendly iteration
- Tree structure with parent/child relationships
- Dirty tracking for retained mode optimization
- Generation counter for change detection
- Depth-first iterator for tree traversal
- `WidgetDelta` for retained mode updates (only changed widgets)

Widget kinds: VStack, HStack, ZStack, Container, ScrollView, Text, Heading, Label, Button, TextField, TextArea, Checkbox, Radio, Slider, Custom(String)

State flags: hovered, focused, active, disabled, visible

### Style System (`engine/src/style.rs`)

Tailwind-inspired utilities compiled at startup into lookup tables for zero-cost runtime.

- `StyleSystem` parses TOML theme config and caches utility classes
- `ComputedStyle` stores resolved styles (colors, typography, borders, effects)
- Custom classes expand into component utilities
- Default Tailwind-like color palette (gray-50 to gray-900, blue/red/green-500/600)
- Default spacing scale: 0-96 (0.25rem increments)

Supported utilities:
- Colors: `text-*`, `bg-*`, `border-*`
- Typography: `text-{xs,sm,base,lg,xl,2xl,3xl,4xl}`, `font-{thin,light,normal,medium,semibold,bold}`
- Borders: `rounded`, `rounded-{sm,md,lg,xl,full}`
- Effects: `opacity-{0-100}`
- State modifiers: `hover:`, `focus:`, `active:` (TODO)
- Responsive: `sm:`, `md:`, `lg:` (TODO)

Theme configuration uses TOML with colors, spacing, and custom_classes maps.

### Layout Engine (`engine/src/layout.rs`)

- Uses `euclid` crate for SIMD-friendly geometry calculations
- `SlotMap<LayoutNodeId, LayoutNode>` for cache-friendly storage
- Dirty tracking to avoid unnecessary recalculations
- Layout algorithms: Flex (partially implemented), Grid (TODO), Absolute, Block

Layout constraints: width/height/min/max dimensions (Auto | Points | Percent), padding, margin

Flexbox properties: `FlexDirection`, `FlexWrap`, `JustifyContent`, `AlignItems`, flex_grow, flex_shrink, flex_basis

`ComputedLayout` stores position, size, content_size, dirty flag.

**Current implementation status**: Basic flexbox layout works for simple cases. Full flexbox algorithm (flex wrapping, multi-line, complex alignment) is TODO.

### Event System (`engine/src/event.rs`)

Platform-agnostic event handling:
- Mouse: MouseMove, MouseDown, MouseUp, MouseWheel (with widget hit testing)
- Keyboard: KeyDown, KeyUp, TextInput (with modifiers)
- Focus: FocusGained, FocusLost
- Window: WindowResize, WindowClose
- Application: Quit

`EventDispatcher` tracks hover/focus/pressed state and batches events for single FFI return. Events include optional `WidgetId` for hit-tested widget (hit testing not yet implemented).

`EventBatch` contains `Vec<Event>` and frame_number for synchronization.

### Rendering (`engine/src/render.rs`)

`Renderer` supports both modes via `RenderMode` enum.

Immediate mode uses `CommandBuffer` with `Vec<RenderCommand>`:
- DrawRect (with border_radius), DrawText, PushClip, PopClip, SetOpacity, Clear

### Platform Backends (`engine/src/platform/`)

**Status**: wgpu backend fully implemented for macOS and iOS. Other platforms scaffolded.

The primary rendering backend uses **wgpu** (`platform/wgpu_backend.rs`) which provides cross-platform GPU access via Metal (macOS/iOS), Vulkan (Linux/Android), and D3D12 (Windows).

**iOS Platform** (`platform/ios.rs`):
- Direct UIKit integration bypassing winit for proper iOS lifecycle
- CAMetalLayer-backed UIView with wgpu rendering
- Multi-touch support with gesture detection (tap vs drag)
- Software keyboard via UIKeyInput protocol conformance
- Hardware keyboard via UIPress event handling
- Keyboard frame notifications for avoidance animations
- Safe area insets support
- Device orientation handling
- App lifecycle events (suspend/resume)

**wgpu Backend Features** (fully implemented):
- ✅ Rectangles with rounded corners (per-corner radii)
- ✅ Borders with configurable width and color
- ✅ Linear gradients (horizontal, vertical, diagonal)
- ✅ Soft shadows with blur and offset
- ✅ Text rendering via Core Text (macOS) with glyph atlas caching
- ✅ Font weights (100-900), styles (normal/italic)
- ✅ Multi-line text with word wrapping
- ✅ Text overflow with ellipsis (single-line, multi-line, height-based)
- ✅ Letter spacing and word spacing
- ✅ Clipping regions (scissor rects)
- ✅ Image rendering (PNG, JPEG) with texture management
- ✅ Sprite sheets with source rect support
- ✅ HiDPI scaling (logical pixel coordinate system)

**Render Commands** (in `render.rs`):
```rust
enum RenderCommand {
    DrawRect { x, y, width, height, color, corner_radii, border, gradient },
    DrawText { x, y, text, font, color, layout },
    DrawImage { x, y, width, height, texture_id, source_rect },
    DrawShadow { x, y, width, height, blur, color, offset_x, offset_y, corner_radii },
    PushClip { x, y, width, height },
    PopClip,
    SetOpacity(f32),
    Clear { r, g, b, a },
}
```

Legacy platform-specific backends are scaffolded but not actively used:
- `platform/metal.rs` - Direct Metal (superseded by wgpu)
- `platform/vulkan_linux.rs`, `platform/vulkan_android.rs`
- `platform/d3d.rs` - Direct3D 12

See `PLATFORM_BACKENDS.md` for detailed platform documentation.

## Development Practices

### Code Quality

- **Clippy fails on warnings** (`-D warnings`): All clippy warnings must be fixed
- Format with `cargo fmt` before committing
- Run `task ci` before pushing to ensure all checks pass
- Write tests for core functionality (existing test coverage is minimal)

### Performance Considerations

- Style parsing happens at startup, runtime is just integer lookups
- Use SlotMap for O(1) widget/layout access with generational indices
- Dirty tracking prevents unnecessary recalculations
- FFI overhead minimized via batching

### Memory Management

- FFI allocates CString for JSON returns - Go must call `centered_free_string()`
- Widget tree uses SlotMap which reuses deleted slots (handles sparse IDs gracefully)
- Parent removal recursively removes all children

### Testing Strategy

All core modules have basic tests. Run with `task test` or `task test:watch`.

Example patterns:
- Widget lifecycle: create, add_child, remove_widget
- Style parsing: parse_classes returns expected ComputedStyle
- Event batching: push_event, take_batch

## Common Patterns

### Building UIs with the Go FFI API

The primary API is in `internal/ffi/ffi.go`. Applications use immediate mode rendering with a callback-based event loop:

```go
import "github.com/agiangrant/centered/internal/ffi"

func main() {
    config := ffi.DefaultAppConfig()
    config.Title = "My App"
    config.Width = 800
    config.Height = 600

    state := &AppState{}

    // Run blocks until window closes
    ffi.Run(config, func(event ffi.Event) ffi.FrameResponse {
        switch event.Type {
        case ffi.EventReady:
            // Load images on startup
            imageData, _ := os.ReadFile("icon.png")
            state.iconID, _ = ffi.LoadImage(imageData)
            return ffi.FrameResponse{RequestRedraw: true}

        case ffi.EventRedrawRequested:
            return renderFrame(state)

        case ffi.EventMouseMoved:
            state.mouseX, state.mouseY = event.Data1, event.Data2
            return ffi.FrameResponse{RequestRedraw: true}

        case ffi.EventMousePressed:
            // Handle click
            return ffi.FrameResponse{RequestRedraw: true}
        }
        return ffi.FrameResponse{}
    })
}

func renderFrame(state *AppState) ffi.FrameResponse {
    commands := []ffi.RenderCommand{
        ffi.Clear(26, 26, 38, 255),

        // Rounded rectangle with gradient
        ffi.RoundedRectWithGradient(100, 100, 200, 100,
            ffi.LinearGradientVertical(ffi.RGB(59, 130, 246), ffi.RGB(37, 99, 235)),
            12),

        // Text with custom font
        ffi.TextWithFont("Hello World", 100, 250,
            ffi.FontDescriptor{
                Source: ffi.FontSource{System: strPtr("system")},
                Weight: 600,
                Size:   24,
            }, ffi.RGB(255, 255, 255)),

        // Multi-line wrapped text with ellipsis
        ffi.TextWithLayout("Long text that wraps...", 100, 300,
            ffi.FontDescriptor{Size: 14},
            ffi.RGB(200, 200, 200),
            ffi.EllipsisTextLayout(200, 3)), // max 200px wide, 3 lines

        // Image
        ffi.Image(state.iconID, 100, 400, 64, 64),

        // Clipping region
        ffi.PushClip(50, 50, 100, 100),
        ffi.RoundedRect(0, 0, 200, 200, ffi.RGB(255, 0, 0), 0), // clipped
        ffi.PopClip(),
    }

    return ffi.FrameResponse{ImmediateCommands: commands}
}
```

**Available Render Commands:**
- `Clear(r, g, b, a)` - Clear screen
- `RoundedRect(x, y, w, h, color, radius)` - Rectangle with uniform corner radius
- `RoundedRectWithRadii(x, y, w, h, color, [4]radii)` - Per-corner radii
- `RoundedRectWithBorder(...)` - With border
- `RoundedRectWithGradient(...)` - With linear gradient
- `Shadow(x, y, w, h, blur, color, offsetX, offsetY, radius)` - Soft shadow
- `Text(text, x, y, size, color)` - Simple text
- `TextWithFont(text, x, y, font, color)` - With font descriptor
- `TextWithLayout(text, x, y, font, color, layout)` - With wrapping/ellipsis
- `Image(textureID, x, y, w, h)` - Draw loaded image
- `ImageWithSourceRect(...)` - Draw portion of image (sprite sheets)
- `Sprite(textureID, x, y, w, h, spriteX, spriteY, cols, rows)` - Grid-based sprites
- `PushClip(x, y, w, h)` / `PopClip()` - Clipping regions

**Image Loading:**
- `ffi.LoadImage([]byte)` - Load from PNG/JPEG bytes
- `ffi.LoadImageFile(path)` - Load from file path
- `ffi.UnloadImage(textureID)` - Free GPU resources

**Events:**
- `EventReady` - Window initialized (Data1/Data2 = logical width/height)
- `EventResized` - Window resized
- `EventRedrawRequested` - Time to render
- `EventMouseMoved` - Mouse position (Data1/Data2 = x/y in logical pixels)
- `EventMousePressed/Released` - Mouse buttons (Data1 = button index)
- `EventKeyPressed/Released` - Keyboard (Data1 = scancode)
- `EventCloseRequested` - Window close button clicked

### Adding a New Widget Kind

1. Add variant to `WidgetKind` enum in `widget.rs`
2. Document expected data in `WidgetData` (text, custom_data, etc.)
3. Add rendering logic (when platform backends are implemented)
4. Add tests for widget creation and properties

### Adding a New Style Utility

1. Add parsing logic in `StyleSystem::parse_utility_class()` in `style.rs`
2. Create corresponding `StyleRule` variant if needed
3. Update `apply_rule()` to handle new rule
4. Add test case in `test_parse_*_classes()`

### Adding a New Event Type

1. Add variant to `Event` enum in `event.rs`
2. Update `EventDispatcher::push_event()` if state tracking needed
3. Document event flow in FFI layer
4. Add test case

## Accessibility System (Planned)

Font scaling will respect OS preferences via scalar-based system. All text utilities multiply against base font size. Updates trigger automatic layout recalculation. Infrastructure should support accessibility metadata on widgets now, even if platform-specific APIs come later.

## Asset Management (Planned)

- Embedded assets (compile-time via `go:embed`): icons, fonts, config files
- External assets (runtime-loaded): videos, large images, user content
- Developer chooses approach based on binary size vs convenience tradeoff

## Internationalization (Planned)

TOML-based translation files with key/value pairs and variable substitution. RTL support via `direction = "rtl"` in locale metadata, which mirrors layouts and reverses text automatically.

## Architectural Decisions

### FFI vs Shared Memory
**Decision**: Use FFI for queries (text measurement, font metrics), shared memory for bulk data (frame commands).

**Rationale**:
- FFI calls are appropriate for occasional queries that need precise answers (e.g., measuring text width)
- Text measurement via FFI with caching is efficient: measure once per text/font change, cache result
- Even animated text at 60fps = 60 FFI calls/sec, which is negligible
- Shared memory will be used for frame command buffers where bulk data transfer matters
- Keeping layout in Go preserves immediate mode flexibility for game rendering

**Implementation**:
- `ffi.MeasureTextWidth()` calls Rust's `centered_measure_text_width()` via CGO
- Results cached on Widget struct (`textWidth`, `textWidthDirty`)
- Measurement function swappable via `SetMeasureTextWidthFunc()` for testing or future changes

## Open Questions

- Performance profiling and debugging tools?
- Hot reload implementation strategy?
- Plugin system for extending the framework?
- Full flexbox algorithm implementation priority?
