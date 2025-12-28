# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CTD is a cross-platform native GUI framework combining Rust's performance with Go's development velocity. It features a Tailwind-inspired styling system and supports both immediate mode and retained mode rendering. The project is in early development (Phase 1: Foundation).

**Key Philosophy**: Platform-appropriate UX with shared core technology. Mobile and desktop are separate applications using the same Rust engine, not a single compromised codebase.

## Build Commands

The project uses Task (Taskfile.yml) for build automation.

### Core Development (Rust Engine)

```bash
task build              # Build Rust engine (debug)
task build:release      # Build Rust engine (optimized)
task test               # Run Rust tests
task test:watch         # Run tests in watch mode
task fmt                # Format Rust code
task fmt:check          # Check formatting
task clippy             # Run Clippy linter (fails on warnings)
task ci                 # Run all CI checks (fmt, clippy, test, build)
task dev                # Watch and rebuild on changes
task generate           # Generate Tailwind classes from theme.toml
```

### Running a Single Rust Test

```bash
cd engine && cargo test test_name           # Run specific test
cd engine && cargo test module::test_name   # Run test in module
cd engine && cargo test -- --nocapture      # Show println! output
```

### Go Commands

```bash
go test -v ./tw/                      # Run Tailwind parser tests
go test -v ./tw/ -run TestName        # Run specific Go test
go test -bench=. -benchmem ./tw/      # Run benchmarks
go build -o bin/example ./examples/demo/  # Build example (requires Rust engine)
```

### Platform Builds

```bash
# iOS
task ios:build          # Build for iOS device (aarch64-apple-ios)
task ios:build:sim      # Build for iOS simulator
task ios:run            # Build and run on simulator

# Android
task android:build      # Build for Android device (aarch64-linux-android)
task android:run        # Build, deploy, and run on device/emulator

# Web/WASM
task web:build          # Build Rust + Go for WebAssembly
task web:serve          # Serve at http://localhost:8080
```

### CTD CLI Tool

The CLI (`cmd/ctd/`) provides project management commands:

```bash
go run ./cmd/ctd init           # Initialize new project with ctd.toml
go run ./cmd/ctd dev            # Development with hot reload
go run ./cmd/ctd generate       # Generate tw/generated.go from theme.toml
go run ./cmd/ctd build-macos    # Build for macOS (--universal for universal binary)
go run ./cmd/ctd build-ios --simulator  # Build for iOS
go run ./cmd/ctd run-ios        # Build and run on iOS simulator
go run ./cmd/ctd create-ios     # Create Xcode project from ctd.toml
go run ./cmd/ctd create-android # Create Android Studio project
```

## Architecture

### Two-Layer Design

**Rust Engine (`engine/`)**: Platform abstraction, rendering backends, widget primitives, layout engine (flexbox), style system (Tailwind utilities), event dispatch, audio/video playback. Designed for zero-cost abstractions and minimal FFI overhead.

**Go Framework**: Idiomatic Go API with Tailwind-first styling, widget composition, FFI bindings. Located in:
- `internal/ffi/ffi.go`: CGO bindings to Rust engine (platform-specific: `ffi_unix.go`, `ffi_windows.go`, `ffi_js.go`)
- Root package (`ctd`): Widget system with layout, events, animation, and state management
- `tw/`: Complete Tailwind CSS parser with 2,117+ utility classes and arbitrary value support
- `cmd/ctd/`: CLI tool for project initialization, builds, and hot reload

**Supported Platforms**: macOS, iOS, Android, Linux, Windows, Web/WASM (see `engine/src/platform/`)

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
import "github.com/agiangrant/ctd/tw"

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

### Widget System (Root Package)

**Status**: Fully implemented with layout engine, event dispatch, and text input widgets.

The widget system provides a declarative widget API with automatic layout and state management:

**Core Files**:
- `widget.go` - Widget struct, builders, property accessors
- `layout.go` - Flexbox layout engine (Go implementation)
- `event_dispatch.go` - Mouse/keyboard event routing to widgets
- `loop.go` - Event loop integration, frame rendering
- `text_input.go` - TextField and TextArea implementations
- `tailwind.go` - Style property extraction from Tailwind classes
- `animation.go` - Animation system with easing functions and AnimationRegistry
- `scroll.go` - Scroll animation utilities for keyboard avoidance and navigation

**Widget Kinds**:
- Layout: `VStack`, `HStack`, `ZStack`, `Container`, `Spacer`
- Text: `Text`, `Heading`, `Label`
- Input: `Button`, `TextField`, `TextArea`, `Checkbox`, `Radio`, `Slider`
- Custom: Extensible via `KindCustom`

**Layout System**:
The Go-side layout engine (`layout.go`) implements flexbox:
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

**Scroll Animation Utilities** (`scroll.go`):
- `ScrollToWidget()` - Animate scroll to make a widget visible
- `ScrollToWidgetWithKeyboard()` - Same but accounts for keyboard height
- `ScrollToY()` - Animate to a specific scroll position (via AnimationBuilder)
- Configurable duration, easing, and padding
- Integrates with AnimationRegistry for 60fps mode
- Reusable for keyboard avoidance, anchor navigation, programmatic scrolling

**Usage Example**:
```go
import "github.com/agiangrant/ctd"

func buildUI() *ctd.Widget {
    return ctd.VStack(
        ctd.HStack(
            ctd.Text("Welcome").Class("text-2xl font-bold text-white"),
            ctd.Spacer(),
            ctd.Button("Settings").OnClick(func() { /* ... */ }),
        ).Class("px-4 py-2 bg-gray-800"),

        ctd.Container(
            ctd.TextField().
                Placeholder("Enter your name").
                Class("w-full px-3 py-2 bg-gray-700 rounded"),
        ).Class("p-4"),
    ).Class("flex-1 bg-gray-900")
}

func main() {
    ctd.Run(ctd.AppConfig{
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

**Platform-specific implementations**:
- `platform/macos.rs` - macOS windowing, dialogs, notifications
- `platform/ios.rs` - iOS UIKit integration, software keyboard, touch
- `platform/android.rs` - Android native activity, JNI bridge
- `platform/linux/` - X11/Wayland, GTK dialogs, system tray, D-Bus portals
- `platform/windows/` - Win32, system tray, window styling
- `platform/web.rs` - WASM/WebGL via web-sys

**wgpu Backend Features**:
- Rectangles with rounded corners, borders, linear gradients
- Soft shadows with blur and offset
- Text rendering via platform-specific APIs (Core Text/DirectWrite/FreeType)
- Multi-line text with word wrapping and ellipsis
- Image rendering (PNG, JPEG) with texture management
- Sprite sheets with source rect support
- HiDPI scaling (logical pixel coordinate system)
- Clipping regions (scissor rects)

### Audio/Video (`engine/src/audio/`, `engine/src/video/`)

Platform-specific audio/video implementations:
- **macOS**: AVFoundation for playback, Core Audio for capture
- **iOS**: AVAudioSession, camera via AVCaptureSession
- **Linux**: GStreamer for video, CPAL/Rodio for audio, V4L2 for camera
- **Windows**: WASAPI for audio, Media Foundation for video
- **Android**: MediaPlayer, AudioTrack, camera via JNI

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

The low-level FFI API is in `internal/ffi/ffi.go`. For most applications, use the higher-level `ctd` package instead. The FFI API provides immediate mode rendering with a callback-based event loop:

```go
import "github.com/agiangrant/ctd/internal/ffi"

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

## Architectural Decisions

### FFI vs Shared Memory
**Decision**: Use FFI for queries (text measurement, font metrics), shared memory for bulk data (frame commands).

- FFI calls are appropriate for occasional queries (e.g., measuring text width)
- Text measurement via FFI with caching: measure once per text/font change, cache result
- `ffi.MeasureTextWidth()` calls Rust's `centered_measure_text_width()` via CGO
- Results cached on Widget struct (`textWidth`, `textWidthDirty`)
- Measurement function swappable via `SetMeasureTextWidthFunc()` for testing
