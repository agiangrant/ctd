# Cross-Platform GUI Framework: Rust + Go Design Document

## Overview

A cross-platform native GUI framework combining Rust's performance and safety with Go's development velocity and simplicity. Features a Tailwind-inspired utility-first styling system that will be familiar to modern web developers.

## Core Architecture

### Division of Responsibilities

**Rust Engine (Core Layer)**
- Platform abstraction (window creation, input, native APIs)
- Rendering backends (wgpu, native platform renderers)
- Widget primitive implementations
- Layout engine (flexbox, grid, absolute positioning)
- Style system parsing and compilation
- Event dispatch system
- Support for both immediate and retained rendering modes

**Go Framework (Application Layer)**
- Idiomatic Go API for building UIs
- State management
- Widget composition
- Event handler registration
- Developer-facing ergonomics

## Rendering Modes

The framework supports both rendering paradigms, with widgets being mode-agnostic:

### Immediate Mode
- Go builds command buffer/scene description per frame
- Single FFI call per frame with batched data
- Rust processes and renders
- Best for: dev tools, games, interactive visualizations, prototyping

### Retained Mode
- Go sends widget tree updates only on state changes
- Rust maintains widget tree and renders independently
- FFI calls only when UI state changes
- Best for: traditional apps, battery efficiency, accessibility integration

**Key Design Decision:** Widgets don't know which mode they're being rendered in. The rendering backend adapts to the mode, not the widgets.

## FFI Strategy

### Performance Considerations
- **One FFI call per frame** maximum (immediate mode)
- **FFI calls only on state changes** (retained mode)
- Batched updates and command buffers
- Shared memory where possible for zero-copy operations

### Rust FFI Surface

```rust
// Initialization
fn init_engine(config: EngineConfig) -> EngineHandle
fn load_styles(handle: EngineHandle, styles: StyleData)

// Frame/Update submission
fn submit_frame(handle: EngineHandle, frame_data: *const FrameData)
fn submit_update(handle: EngineHandle, widget_delta: *const WidgetDelta)

// Event handling
fn poll_events(handle: EngineHandle) -> *const EventBatch
fn register_callback(handle: EngineHandle, callback: EventCallback)
```

## Tailwind-Inspired Styling System

### Why Tailwind-Style Utilities?

1. **Familiar to millions** - Developers already know this pattern
2. **Co-location** - Styling lives with widgets
3. **Composable** - No style conflicts
4. **Fast iteration** - No context switching
5. **IDE-friendly** - Autocomplete support

### Go Widget API

```go
// Basic usage
Text{
    Content: "Hello World",
    Class: "text-2xl font-bold text-gray-900 mb-4",
}

// Or with slice syntax
Button{
    Label: "Click Me",
    Class: []string{"bg-blue-500", "hover:bg-blue-600", "text-white", "px-4", "py-2", "rounded"},
    OnClick: handleClick,
}

// Using custom classes
Card{
    Class: "card shadow-xl",
    Children: []Widget{
        Text{Content: "Title", Class: "text-lg font-semibold"},
        Text{Content: "Description", Class: "text-gray-600"},
    },
}
```

### Theme Configuration (TOML)

```toml
# theme.toml

[colors]
brand-primary = "#FF6B35"
brand-secondary = "#004E89"
brand-accent = "#F77F00"
success = "#06D6A0"
warning = "#FFB627"
danger = "#EF476F"

[spacing]
# Extend default spacing scale
72 = 18.0
84 = 21.0
96 = 24.0

[custom-classes]
btn-primary = ["bg-brand-primary", "text-white", "px-4", "py-2", "rounded", "hover:bg-brand-secondary"]
btn-secondary = ["bg-gray-200", "text-gray-800", "px-4", "py-2", "rounded", "hover:bg-gray-300"]
card = ["bg-white", "shadow-lg", "rounded-lg", "p-6"]
card-dark = ["bg-gray-800", "text-white", "shadow-xl", "rounded-lg", "p-6"]
input = ["border", "border-gray-300", "rounded", "px-3", "py-2", "focus:border-brand-primary", "focus:ring-2", "focus:ring-brand-primary"]

[typography]
# Custom font families
sans = "Inter, system-ui, sans-serif"
mono = "JetBrains Mono, monospace"

[breakpoints]
# Responsive breakpoints (window sizes)
sm = 640
md = 768
lg = 1024
xl = 1280
"2xl" = 1536
```

### Loading Theme in Go

```go
package main

import "ui"

func main() {
    app := ui.NewApp(ui.Config{
        Mode: ui.Retained,
        ThemeFile: "theme.toml",
    })

    app.Run(func() ui.Widget {
        return ui.VStack{
            Class: "p-8 gap-4",
            Children: []ui.Widget{
                ui.Text{
                    Content: "Welcome",
                    Class: "text-4xl font-bold text-brand-primary",
                },
                ui.Button{
                    Label: "Get Started",
                    Class: "btn-primary",
                    OnClick: handleStart,
                },
            },
        }
    })
}
```

### Style Processing (Rust Side)

At startup, the Rust engine:
1. Parses the TOML theme configuration
2. Merges custom colors, spacing, and classes with defaults
3. Expands custom classes into their component utilities
4. Compiles everything into optimized lookup tables
5. Creates efficient rendering instructions

**Result:** Zero runtime cost for style abstractions. Styling is just integer lookups into pre-computed data.

### Supported Utility Categories

- **Layout**: flex, grid, block, inline, absolute, relative
- **Spacing**: p-*, m-*, gap-*, space-*
- **Sizing**: w-*, h-*, min-*, max-*
- **Typography**: text-*, font-*, leading-*, tracking-*
- **Colors**: text-*, bg-*, border-*
- **Borders**: border, border-*, rounded-*
- **Effects**: shadow-*, opacity-*
- **States**: hover:*, focus:*, active:*, disabled:*
- **Responsive**: sm:*, md:*, lg:*, xl:*

## Widget System

### Core Widgets

- **Layout**: VStack, HStack, ZStack, Grid, Spacer
- **Text**: Text, Heading, Label
- **Input**: Button, TextField, TextArea, Checkbox, Radio, Select, Slider
- **Containers**: Container, Card, Panel, ScrollView
- **Navigation**: TabView, NavBar, Sidebar
- **Feedback**: Alert, Toast, Modal, ProgressBar, Spinner
- **Media**: Image, Icon, Video (future)

### Widget Interface

Widgets are mode-agnostic and describe themselves declaratively:

```go
type Widget interface {
    // Internal methods for framework use
    // Developers compose widgets, don't implement this interface
}

// Example widget definitions
type Button struct {
    Label    string
    Class    string // or []string
    OnClick  func()
    Disabled bool
}

type VStack struct {
    Class    string
    Children []Widget
}

type Text struct {
    Content string
    Class   string
}
```

## Application Structure

### Minimal Example

```go
package main

import "ui"

func main() {
    app := ui.NewApp(ui.Config{
        Mode: ui.Retained,
        ThemeFile: "theme.toml",
        Window: ui.WindowConfig{
            Title: "My App",
            Width: 800,
            Height: 600,
        },
    })

    app.Run(func() ui.Widget {
        return ui.VStack{
            Class: "p-4 gap-2",
            Children: []ui.Widget{
                ui.Text{
                    Content: "Hello, World!",
                    Class: "text-2xl font-bold",
                },
                ui.Button{
                    Label: "Click Me",
                    Class: "btn-primary",
                    OnClick: func() {
                        println("Clicked!")
                    },
                },
            },
        }
    })
}
```

### Stateful Example

```go
package main

import "ui"

type AppState struct {
    count int
}

func (s *AppState) increment() {
    s.count++
}

func main() {
    state := &AppState{}
    
    app := ui.NewApp(ui.Config{
        Mode: ui.Retained,
        ThemeFile: "theme.toml",
    })

    app.Run(func() ui.Widget {
        return ui.VStack{
            Class: "p-8 gap-4 items-center",
            Children: []ui.Widget{
                ui.Text{
                    Content: fmt.Sprintf("Count: %d", state.count),
                    Class: "text-3xl font-mono",
                },
                ui.Button{
                    Label: "Increment",
                    Class: "btn-primary",
                    OnClick: state.increment,
                },
            },
        }
    })
}
```

## Development Roadmap

### Phase 1: Foundation
- [ ] Rust engine core architecture
- [ ] Basic rendering backend (single platform)
- [ ] FFI layer design and implementation
- [ ] Go bindings generation
- [ ] Style system parser and compiler
- [ ] Core layout engine (flexbox)

### Phase 2: Widget Library
- [ ] Essential widgets (Text, Button, Container, VStack, HStack)
- [ ] Input widgets (TextField, Checkbox, etc.)
- [ ] Event handling system
- [ ] Both rendering modes functional

### Phase 3: Styling System
- [ ] Complete Tailwind-inspired utility set
- [ ] TOML theme configuration
- [ ] Custom class support
- [ ] State variants (hover, focus, etc.)
- [ ] Responsive utilities

### Phase 4: Platform Support
- [ ] macOS native integration
- [ ] Windows native integration
- [ ] Linux native integration
- [ ] Platform-specific optimizations

### Phase 5: Polish & Ecosystem
- [ ] Documentation and examples
- [ ] Hot reload in development
- [ ] Debugging tools
- [ ] Performance profiling
- [ ] Community widget library

## Design Principles

1. **Performance First**: Native speed, minimal overhead, respect user hardware
2. **Developer Experience**: Familiar patterns, fast iteration, great tooling
3. **Mode Agnostic**: Widgets work in both immediate and retained modes
4. **Zero Cost Abstractions**: Style utilities compile to optimal code
5. **Platform Integration**: Feel native on each OS
6. **Sustainable**: No megacorp ownership, open governance
7. **Pragmatic**: Choose appropriate technology for each layer

## Why This Will Win

- **Familiar to web developers** (Tailwind styling, declarative UI)
- **Better than Electron** (native performance, smaller binaries, less resource usage)
- **Simpler than Rust-only** (Go development velocity for app logic)
- **More flexible than Flutter** (not Dart-locked, supports multiple paradigms)
- **Not corporate-controlled** (unlike React Native, Flutter, Kotlin)
- **Actual native code** (unlike JavaScript bridges)
- **Cross-platform without compromise** (shared styling, platform-appropriate rendering)

## Animation System

Animations are implemented as public API functions that can be called directly or invoked via utility classes. **Both approaches produce identical results** - the choice is based on team familiarity and preference, not capability.

### Public API Approach

```go
// Direct function calls for animation control
ui.Transition(widgetID, ui.Property.Opacity, 300*time.Millisecond, ui.Ease.Out)
ui.Transform(widgetID, ui.Scale(1.05))
ui.Opacity(widgetID, 0.5)

// Complex animation sequences
ui.AnimationSequence(
    ui.Parallel(
        ui.Transition(id, ui.Property.Opacity, 200*time.Millisecond, ui.Ease.In),
        ui.Transition(id, ui.Property.Transform, 200*time.Millisecond, ui.Ease.Out),
    ),
    ui.Delay(100*time.Millisecond),
    ui.Transition(id, ui.Property.Scale, 300*time.Millisecond, ui.Ease.Spring),
)
```

### Utility Class Approach

```go
// Declarative utilities that compile to the same API calls
Button{
    Label: "Click Me",
    Class: "transition-opacity duration-300 hover:scale-105",
    OnClick: handler,
}
```

### Configuration (TOML)

```toml
[animations]
# Global animation settings
duration-fast = 150    # milliseconds
duration-normal = 300
duration-slow = 500

# Easing curves
ease-default = "ease-out"
ease-spring = "spring(1.0, 100, 10)"

[animations.presets]
fade = { property = "opacity", duration = "normal", ease = "default" }
slide-in = { property = "transform", from = "translateX(-100%)", duration = "normal" }
scale-up = { property = "scale", to = 1.05, duration = "fast", ease = "spring" }
```

### Design Philosophy

The utility classes are syntactic sugar - they parse into the same public API functions. This means:

- **No duplicate implementation** - one animation system, two syntaxes
- **Equivalent power** - utilities can do anything the API can (within reason)
- **Easy extension** - new API functions can get corresponding utilities
- **Team choice** - use what fits your workflow

**When to use utilities:**
- Rapid prototyping
- Simple state transitions (hover, focus)
- Team familiar with Tailwind/CSS animations
- Preference for declarative style

**When to use API:**
- Complex animation sequences
- Dynamic animations based on runtime data
- Programmatic control needed
- Preference for explicit Go code

Both approaches compile to the same Rust animation engine, producing identical performance and visual results.

## Accessibility System

### Font Scaling

Centered respects user accessibility preferences through a scalar-based system that automatically adjusts all text sizes proportionally.

```toml
[accessibility]
# Base font size in pixels (default: 16)
base-font-size = 16

# Respect operating system font size preferences
respect-system-font-size = true

# Manual scale factor (1.0 = normal, 1.5 = 150%, etc.)
# Only used if respect-system-font-size is false
scale-factor = 1.0
```

**How it works:**
- All text utilities (`text-sm`, `text-base`, `text-lg`, etc.) multiply against the base font size
- When accessibility settings change, the Rust engine recalculates all text sizing
- Layouts automatically adapt to new text dimensions
- **No application code changes required**

**Example:**
- Default: `text-base` = 16px
- User increases accessibility: `base-font-size` becomes 20px
- Now: `text-base` = 20px, `text-sm` = 17.5px, `text-lg` = 22.5px
- Layout engine re-flows to accommodate larger text

### Dynamic Updates

Changes to accessibility settings trigger automatic updates:
1. User adjusts system font size preference
2. OS notifies the application
3. Rust engine updates base font size scalar
4. Layout recalculation triggered
5. UI re-renders with new sizes

This ensures accessibility is built-in, not an afterthought that developers must implement manually.

## Asset Management

Assets are handled differently based on size and use case:

### Embedded Assets (Compile-time)
**For:** Icons, small images, fonts, configuration files
**Why:** Fast access, no file I/O, single binary distribution

```go
//go:embed assets/icons/*.png
//go:embed assets/fonts/*.ttf
var embeddedAssets embed.FS

ui.LoadEmbeddedAssets(embeddedAssets)
```

### External Assets (Runtime-loaded)
**For:** Videos, large images, user-generated content, downloadable resources
**Why:** Keeps binary size reasonable, reduces constant RAM usage

```go
Image{
    Source: ui.File("./assets/hero-image.jpg"),
    Class: "w-full h-auto",
}

Video{
    Source: ui.File("./media/tutorial.mp4"),
    Class: "aspect-video",
}
```

### Use Case: Games
Games may want everything embedded for distribution simplicity. This is supported - the developer chooses based on their needs.

## Internationalization & Localization

Simple, pragmatic approach using TOML files with key/value pairs and variable substitution.

### Translation Files

```toml
# locales/en.toml
[app]
title = "My Application"
welcome = "Welcome, {name}!"
item-count = "You have {count} items"

[buttons]
save = "Save"
cancel = "Cancel"
submit = "Submit"

# locales/es.toml
[app]
title = "Mi Aplicación"
welcome = "¡Bienvenido, {name}!"
item-count = "Tienes {count} artículos"

[buttons]
save = "Guardar"
cancel = "Cancelar"
submit = "Enviar"
```

### Usage in Go

```go
// Initialize with locale
i18n := ui.LoadLocale("locales/en.toml")

// Simple key lookup
Text{
    Content: i18n.T("buttons.save"),
    Class: "text-lg",
}

// With variable substitution
Text{
    Content: i18n.T("app.welcome", map[string]any{
        "name": userName,
    }),
}

Text{
    Content: i18n.T("app.item-count", map[string]any{
        "count": itemCount,
    }),
}

// Change locale at runtime
i18n.SetLocale("locales/es.toml")
```

### Right-to-Left (RTL) Support

Text direction is automatically handled based on locale configuration:

```toml
# locales/ar.toml
[_meta]
direction = "rtl"  # or "ltr" (default)

[app]
title = "تطبيقي"
```

When RTL is enabled:
- Text rendering direction reverses
- Layout engine mirrors flex/grid layouts appropriately
- Padding/margin utilities respect direction (e.g., `pl-4` becomes logical start padding)
- Developers write layout-agnostic code, framework handles directionality

## Roadmap Decisions

### Accessibility
**Decision:** Build infrastructure now (widget parameters for accessibility metadata), implement platform-specific APIs (screen readers, high contrast, etc.) in later phases. Leave room in the architecture but don't block initial development.

### Testing Framework
**Decision:** Design for testability from the start (widget tree inspection, event simulation), but full testing framework comes after core functionality is stable.

### WASM Target
**Decision:** WASM compilation is a supported target. Centered apps should be able to compile to web if needed, making it a true universal framework.

### Package Management
**Decision:** Standard Go modules for widget libraries. No custom package system needed - leverage existing ecosystem.

## Platform Strategy: Mobile vs Desktop

### Philosophy

**Centered supports both mobile and desktop, but as separate applications, not a single codebase trying to be everything.**

Mobile and desktop are fundamentally different:
- **Input methods:** Touch vs mouse/keyboard
- **Screen sizes:** 6" vs 27"
- **Interaction patterns:** Swipe gestures vs hover states
- **Navigation:** Bottom tabs vs sidebars
- **Context:** On-the-go vs stationary

Attempting to write one application for both leads to:
- Endless conditional logic (`if mobile { ... } else { ... }`)
- Compromised UX on every platform
- Confusing codebases ("which code path is this?")
- Maintenance nightmares

**The Centered approach:** One framework, separate applications.

### Module Structure

```
centered/
├── core/           # Shared Rust engine (rendering, layout, events)
├── styling/        # Shared style system (Tailwind utilities, TOML parser)
├── desktop/        # Desktop-specific widgets and platform integration
│   ├── widgets/    # Window, MenuBar, ContextMenu, Sidebar, etc.
│   └── platform/   # macOS, Windows, Linux integration
└── mobile/         # Mobile-specific widgets and platform integration
    ├── widgets/    # BottomSheet, SwipeView, TabBar, etc.
    └── platform/   # iOS, Android integration
```

### Usage Pattern

**Desktop Application:**
```go
// github.com/mycompany/myapp-desktop
import "github.com/centered/desktop"

func main() {
    app := desktop.NewApp(desktop.Config{
        Window: desktop.WindowConfig{
            Title: "MyApp",
            Width: 1200,
            Height: 800,
        },
    })
    
    app.Run(func() desktop.Widget {
        return desktop.HStack{
            Sidebar{},      // Always visible
            MainContent{},
            DetailsPanel{}, // Side panel
        }
    })
}
```

**Mobile Application:**
```go
// github.com/mycompany/myapp-mobile
import "github.com/centered/mobile"

func main() {
    app := mobile.NewApp(mobile.Config{
        Orientation: mobile.Portrait,
    })
    
    app.Run(func() mobile.Widget {
        return mobile.VStack{
            MobileHeader{},
            ScrollView{
                Content: MainContent{},
            },
            TabBar{}, // Bottom navigation
        }
    })
}
```

**Shared Business Logic:**
```go
// github.com/mycompany/myapp-core
package core

// Share API clients, data models, business rules
// Import into both desktop and mobile apps
```

### Widget Philosophy

Rather than one Button widget trying to be a superhero:

**Desktop Button:**
- Hover states
- Right-click support
- Keyboard focus indicators
- Tooltip support
- Cursor changes

**Mobile Button:**
- Touch ripple effect
- Larger hit targets (44pt minimum)
- Press and hold gestures
- Haptic feedback
- No hover states

Each widget is optimized for its platform, not compromised for cross-platform compatibility.

### Benefits

1. **Platform-appropriate UX** - Desktop feels like desktop, mobile feels like mobile
2. **Clean codebases** - No platform conditionals scattered everywhere
3. **Independent evolution** - Desktop widgets can add features without breaking mobile
4. **Easier testing** - Test one platform at a time
5. **Shared foundation** - Core engine, styling, business logic still reused

### When to Share Code

**Always share:**
- Core Rust engine (rendering, layout, events)
- Style system and theme configuration
- Business logic and data models
- API clients and networking
- Utility functions

**Never share:**
- UI layout code
- Navigation patterns
- Widget trees
- Platform-specific integrations

## Open Questions

- Performance profiling and debugging tools?
- Hot reload implementation strategy?
- Plugin system for extending the framework?
