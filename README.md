# CTD

A cross-platform native GUI framework combining Rust's performance with Go's development velocity.

## Features

- **Tailwind-inspired styling** — Use familiar class names like `bg-blue-500`, `px-4`, `rounded-lg`
- **Cross-platform** — macOS, iOS, Android, Linux, Windows, and Web/WASM from a single codebase
- **Native rendering** — GPU-accelerated via wgpu (Metal, Vulkan, D3D12, WebGPU)
- **Flexible rendering modes** — Immediate mode for games, retained mode for apps
- **Rich widget library** — Layouts, buttons, text inputs, checkboxes, sliders, and more
- **Animation system** — Built-in easing functions with automatic 60 FPS mode switching
- **Audio/Video** — Platform-native playback and capture

## Quick Start

### Prerequisites

- [Go 1.21+](https://go.dev/dl/)
- [Rust 1.70+](https://rustup.rs/)

### Create a New Project

```bash
# Install the CTD CLI
go install github.com/agiangrant/ctd/cmd/ctd@latest

# Create a new project
mkdir myapp && cd myapp
ctd init

# Start development with hot reload
ctd dev
```

The `ctd init` command creates:
- `main.go` — Application entry point
- `ctd.toml` — Project configuration (app info, permissions, platform settings)
- `theme.toml` — Custom colors and spacing for Tailwind classes

### CLI Commands

```bash
ctd init              # Initialize a new project
ctd dev               # Run with hot reload
ctd build             # Build for current platform
ctd build --release   # Build optimized release

# Mobile
ctd create-ios        # Create Xcode project
ctd create-android    # Create Android Studio project
ctd run-ios           # Build and run on iOS simulator
ctd run-android       # Build and run on Android emulator
```

## Hello World

```go
package main

import (
    "runtime"
    "github.com/agiangrant/ctd"
    "github.com/agiangrant/ctd/internal/ffi"
)

func init() {
    runtime.LockOSThread()
}

func main() {
    loop := ctd.NewLoop(ctd.DefaultLoopConfig())

    root := ctd.VStack("bg-gray-900 p-8 gap-4",
        ctd.Text("Hello, CTD!", "text-white text-2xl"),
        ctd.Button("Click me", "bg-blue-500 text-white px-4 py-2 rounded").
            OnClick(func(e *ctd.MouseEvent) {
                println("Button clicked!")
            }),
    )

    loop.Tree().SetRoot(root)

    loop.Run(ffi.AppConfig{
        Title:  "My App",
        Width:  800,
        Height: 600,
    })
}
```

## Styling with Tailwind Classes

CTD uses a Tailwind-inspired styling system. Apply styles using familiar class names:

```go
// Layout
ctd.VStack("flex flex-col gap-4 p-4", ...)
ctd.HStack("flex flex-row justify-between items-center", ...)

// Colors (19 colors × 11 shades)
ctd.Container("bg-blue-500 text-white border-gray-300")

// Sizing
ctd.Container("w-full h-[200px] min-w-[100px]")

// Typography
ctd.Text("Hello", "text-2xl font-bold text-gray-100")

// Effects
ctd.Container("rounded-lg shadow-lg opacity-80")

// State variants
ctd.Button("bg-blue-500 hover:bg-blue-600 active:bg-blue-700")

// Animations
ctd.Container("animate-pulse")
ctd.Container("animate-bounce")
ctd.Container("animate-[pulse_500ms_elastic]")  // Custom timing
```

## Widgets

### Layout
```go
ctd.VStack(classes, children...)      // Vertical stack
ctd.HStack(classes, children...)      // Horizontal stack
ctd.ZStack(classes, children...)      // Overlapping layers
ctd.Container(classes)                // Generic container
ctd.ScrollView(classes, children...)  // Scrollable container
ctd.Flex(classes, children...)        // Flex container (respects flex-direction)
```

### Text & Buttons
```go
ctd.Text("Content", "text-white text-lg")
ctd.Button("Label", "bg-blue-500 px-4 py-2").OnClick(func(e *ctd.MouseEvent) { ... })
```

### Form Controls
```go
ctd.TextField("placeholder", "classes")
ctd.Checkbox("Label", "").OnChange(func(v any) { ... })
ctd.Toggle("").OnChange(func(v any) { ... })
ctd.Radio("Label", "groupName", "").OnChange(func(v any) { ... })
ctd.Slider("").SetSliderRange(0, 100).OnChange(func(v any) { ... })
ctd.Select("Choose...", "").SetSelectOptions(options)
```

### Media
```go
ctd.Image(textureID, "classes")           // From pre-loaded texture
ctd.ImageFromSource("path/or/url", "")    // Auto-loads from file or URL
ctd.Video("path/or/url", "")
ctd.Audio("path/or/url", "")
ctd.Camera("")                            // Video capture
ctd.Microphone("")                        // Audio capture
```

### System
```go
ctd.Clipboard()                           // System clipboard access
ctd.FilePicker()                          // Native file dialogs
ctd.TrayIcon()                            // System tray (desktop only)
```

## Animation

### Class-Based Animations
```go
// Built-in animations
ctd.Container("bg-blue-500 animate-pulse")
ctd.Container("bg-green-500 animate-bounce")
ctd.Container("bg-purple-500 animate-spin")
ctd.Container("bg-red-500 animate-ping")

// Initialize on startup
loop.InitAnimations()
```

### Programmatic Animations
```go
widget.Animate(loop.Animations()).
    Duration(200 * time.Millisecond).
    Easing(ctd.EaseOutBack).
    Size(120, 120)

// Available easing functions:
// EaseLinear, EaseInOutCubic, EaseOutBack, EaseOutElastic, EaseOutBounce
```

## Platform Builds

```bash
# Desktop
task build              # Current platform (debug)
task build:release      # Current platform (optimized)

# iOS
task ios:build          # Device
task ios:build:sim      # Simulator
task ios:run            # Run on simulator

# Android
task android:build      # Device
task android:run        # Run on device/emulator

# Web
task web:build          # Build WASM
task web:serve          # Serve at localhost:8080
```

## Project Structure

```
ctd/
├── engine/                 # Rust rendering engine
│   └── src/
│       ├── platform/       # Platform backends (macOS, iOS, Android, etc.)
│       ├── audio/          # Audio playback/capture
│       ├── video/          # Video playback/capture
│       └── text/           # Text rendering (Core Text, DirectWrite, FreeType)
├── internal/ffi/           # Go ↔ Rust FFI bindings
├── tw/                     # Tailwind CSS parser
├── cmd/ctd/                # CLI tool
├── examples/               # Example applications
├── theme.toml              # Custom colors and spacing
└── Taskfile.yml            # Build automation
```

## Architecture

CTD uses a two-layer architecture:

1. **Rust Engine** — Handles GPU rendering, platform APIs, audio/video, and text shaping
2. **Go Framework** — Provides the widget API, layout engine, and application logic

Communication uses FFI with JSON serialization, optimized to one call per frame. The Go side handles layout computation and widget tree management, while Rust handles rendering and platform-specific operations.

## Examples

The `examples/` directory contains working demos:

| Example | Description |
|---------|-------------|
| `tailwind` | Tailwind classes and animations |
| `controls` | Form widgets (checkbox, radio, slider, select) |
| `textinput` | Text fields and text areas |
| `flexbox` | Flexbox layout system |
| `layout` | Layout patterns |
| `responsive` | Responsive breakpoints |
| `darkmode` | Light/dark mode switching |
| `audio` | Audio playback |
| `video` | Video playback |
| `images` | Image loading and display |
| `media_input` | Camera and microphone capture |
| `clipboard` | System clipboard access |
| `frameless` | Frameless window styling |
| `ios_demo` | iOS-specific features |
| `android_demo` | Android-specific features |
| `web_demo` | WebAssembly demo |

Run any example:
```bash
task build && go run ./examples/tailwind
```

## Contributing

### Prerequisites

- [Task](https://taskfile.dev/) (build automation)

### Setup

```bash
git clone https://github.com/agiangrant/ctd.git
cd ctd
task build
```

### Development Commands

```bash
task build          # Build Rust engine (debug)
task build:release  # Build optimized
task dev            # Watch mode (rebuild on changes)
task test           # Run Rust tests
task ci             # Run all CI checks (fmt, clippy, test, build)
task fmt            # Format Rust code
go test ./tw/       # Run Go tests
```

### Running Examples

```bash
task build && go run ./examples/tailwind
```

## Status

CTD is in active development. The core rendering engine, widget system, and Tailwind parser are functional across macOS, iOS, Android, Linux, Windows, and Web platforms.

## License

MIT
