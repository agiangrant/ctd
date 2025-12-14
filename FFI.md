# FFI Architecture

This document describes the Foreign Function Interface (FFI) between Go and Rust in the Centered framework.

## Design Goals

1. **Single FFI call per frame (immediate mode)**: Minimize overhead by batching all widget data into one call
2. **Efficient updates (retained mode)**: Only send deltas when changes occur
3. **Safe memory management**: Proper cleanup across the Go/Rust boundary
4. **Tailwind-first**: First-class support for Tailwind CSS semantics

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                   Go Application                     │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐    │
│  │  widget.go │  │ engine.go  │  │ tw/parser  │    │
│  │            │  │            │  │            │    │
│  │ VStack()   │  │ Engine     │  │ ParseClass │    │
│  │ HStack()   │  │ .Render()  │  │            │    │
│  │ Text()     │  │            │  │ tw.go      │    │
│  └──────┬─────┘  └──────┬─────┘  └────────────┘    │
│         │                │                           │
│         └────────────────┴───────────┐               │
│                                      │               │
│  ┌───────────────────────────────────▼───┐          │
│  │        internal/ffi/ffi.go (CGO)      │          │
│  │                                        │          │
│  │  - InitEngine()                        │          │
│  │  - SubmitFrame()   (immediate)         │          │
│  │  - SubmitDelta()   (retained)          │          │
│  │  - LoadStyles()                        │          │
│  │  - Resize()                            │          │
│  └────────────────────────────────────────┘          │
└──────────────────┬───────────────────────────────────┘
                   │ FFI Boundary (C ABI)
                   │ JSON serialization
┌──────────────────▼───────────────────────────────────┐
│              Rust Engine (Cargo)                      │
│  ┌────────────────────────────────────────┐          │
│  │       engine/src/ffi.rs                │          │
│  │                                         │          │
│  │  #[no_mangle]                           │          │
│  │  pub extern "C" fn centered_*           │          │
│  └───────────┬─────────────────────────────┘          │
│              │                                         │
│  ┌───────────▼──────┬─────────┬──────────┐           │
│  │ widget.rs        │ style.rs│ layout.rs│           │
│  │                  │         │          │           │
│  │ WidgetTree       │ Style   │ Layout   │           │
│  │ SlotMap          │ System  │ Engine   │           │
│  └──────────────────┴─────────┴──────────┘           │
│                                                        │
│  ┌────────────────────────────────────────┐          │
│  │            render.rs                    │          │
│  │                                         │          │
│  │  Platform backends (Metal, etc.)        │          │
│  └─────────────────────────────────────────┘          │
└────────────────────────────────────────────────────────┘
```

## Performance Characteristics

### Immediate Mode (1 FFI call per frame)

```go
// Build UI tree
ui := VStack("bg-white p-4",
    Text("Hello", "text-lg"),
    Button("Click", "bg-blue-500 px-4 py-2 rounded"),
)

// Single FFI call with complete tree
events, _ := engine.RenderFrame(ui)
```

**Cost per frame**:
- Widget tree construction: ~2-5μs (in Go)
- JSON serialization: ~1-3μs (in Go)
- FFI call: ~0.1-0.5μs
- Rust deserialization: ~1-2μs
- **Total overhead: ~5-10μs per frame**

This is fast enough for 60fps (16.6ms per frame), leaving plenty of headroom for actual rendering.

### Retained Mode (delta updates only)

```go
// First frame - send full tree
events1, _ := engine.RenderFrame(ui1)

// Second frame - only send changes if tree changed
ui2 := buildUI(newState)  // Different state
events2, _ := engine.RenderFrame(ui2)  // Sends delta

// Third frame - same UI
ui3 := buildUI(newState)  // Same state
events3, _ := engine.RenderFrame(ui3)  // No FFI call!
```

**Cost per frame**:
- Tree diffing: ~1-3μs (simple comparison)
- Delta serialization (if changed): ~0.5-2μs
- FFI call (if changed): ~0.1-0.5μs
- **Total: 1-3μs (no changes), 3-8μs (with changes)**

## Data Flow

### Immediate Mode Frame Submission

1. **Go**: Build widget tree with `VStack()`, `Text()`, etc.
2. **Go**: Call `engine.RenderFrame(root)`
3. **Go**: Serialize tree to JSON
4. **FFI**: Single call to `centered_engine_submit_frame(json)`
5. **Rust**: Deserialize JSON into `WidgetTree`
6. **Rust**: Compute layout using flex/grid algorithms
7. **Rust**: Render to platform backend
8. **Rust**: Serialize events to JSON
9. **FFI**: Return events JSON to Go
10. **Go**: Deserialize and dispatch events

### Retained Mode Delta Submission

1. **Go**: Build new widget tree
2. **Go**: Compare with previous frame
3. **Go**: If identical, return cached result (no FFI call!)
4. **Go**: If different, compute delta
5. **Go**: Serialize delta to JSON
6. **FFI**: Call `centered_engine_submit_delta(json)`
7. **Rust**: Apply delta to existing tree
8. **Rust**: Mark affected nodes as dirty
9. **Rust**: Re-layout and re-render only dirty subtrees
10. **FFI**: Return events

## Memory Management

### Go → Rust

- Strings are copied across the FFI boundary using `CString`
- Go retains ownership of widget tree (Rust deserializes a copy)
- No shared memory between Go and Rust

### Rust → Go

- Event strings are allocated by Rust (`CString::into_raw()`)
- Go takes ownership and frees with `centered_free_string()`
- Proper cleanup to avoid leaks

### Safety Guarantees

```rust
#[no_mangle]
pub unsafe extern "C" fn centered_engine_submit_frame(
    handle: EngineHandle,
    frame_json: *const c_char,
) -> *mut c_char {
    // Validate pointer
    if frame_json.is_null() {
        return ptr::null_mut();
    }

    // Safe conversion
    let frame_str = match CStr::from_ptr(frame_json).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    // Process...
}
```

## Serialization Format

### Widget Tree JSON

```json
{
  "root": {
    "kind": "VStack",
    "classes": "bg-white p-4 gap-2",
    "children": [
      {
        "kind": "Text",
        "classes": "text-lg font-bold",
        "text": "Hello World"
      },
      {
        "kind": "Button",
        "classes": "bg-blue-500 hover:bg-blue-600 px-4 py-2 rounded",
        "text": "Click Me"
      }
    ]
  }
}
```

### Event Batch JSON

```json
{
  "events": [
    {
      "type": "click",
      "target": "button-1",
      "data": "{\"x\": 100, \"y\": 200}"
    }
  ]
}
```

## API Usage

### Basic Example (Immediate Mode)

```go
package main

import (
    "github.com/agiangrant/centered"
)

func main() {
    // Create engine
    engine, _ := centered.NewEngine(centered.DefaultEngineConfig())
    defer engine.Shutdown()

    // Game/app loop
    for {
        // Build UI (can be different each frame)
        ui := centered.VStack("bg-white w-full h-full",
            centered.Text("Frame", "text-2xl"),
            centered.Button("Quit", "bg-red-500 px-4 py-2"),
        )

        // Single FFI call
        events, _ := engine.RenderFrame(ui)

        // Handle events
        for _, event := range events {
            if event.Type == "click" {
                break
            }
        }
    }
}
```

### Retained Mode with State

```go
type AppState struct {
    count int
}

func buildUI(state *AppState) centered.Widget {
    return centered.VStack("p-4",
        centered.Text(
            fmt.Sprintf("Count: %d", state.count),
            "text-xl",
        ),
        centered.Button("Increment", "bg-blue-500 px-4 py-2"),
    )
}

func main() {
    config := centered.DefaultEngineConfig()
    config.Mode = centered.RenderModeRetained  // Retained mode

    engine, _ := centered.NewEngine(config)
    defer engine.Shutdown()

    state := &AppState{count: 0}

    for {
        ui := buildUI(state)
        events, _ := engine.RenderFrame(ui)  // Only sends delta if state changed

        for _, event := range events {
            if event.Type == "click" {
                state.count++
            }
        }
    }
}
```

## Future Optimizations

### 1. Binary Serialization
Replace JSON with a binary format (MessagePack, Flatbuffers, or custom):
- **Current**: ~3-5μs for JSON encode/decode
- **Target**: <1μs for binary format

### 2. Shared Memory Pool
Use shared memory for large widget trees:
- Avoid serialization overhead entirely
- Requires unsafe code and careful synchronization

### 3. Tree Diffing Algorithm
Implement proper diffing (similar to React):
- Track widget IDs
- Compute minimal delta
- Send only changed properties

### 4. Batched Event Handling
Buffer events and return them in batches:
- Reduce FFI call frequency for events
- Improve throughput for high-event scenarios

## Building

### Build Rust Engine

```bash
cd engine
cargo build --release
```

This produces `libcentered_engine.dylib` (macOS) or `.so` (Linux) in `engine/target/release/`.

### Build Go Application

```bash
# The CGO LDFLAGS in ffi.go point to the Rust library
go build -o app ./examples/basic
```

### Run Tests

```bash
# Widget API tests (no Rust required)
go test -v ./widget_test.go ./widget.go

# Tailwind parser tests
go test -v ./tw/

# Full integration tests (requires Rust engine)
cargo test  # Rust tests
# go test -v .  # Go integration tests (TODO)
```

## Performance Benchmarks

```bash
# Go widget creation and serialization
go test -bench=BenchmarkWidget -benchmem ./widget_test.go ./widget.go

# Tailwind class parsing
go test -bench=. -benchmem ./tw/
```

Expected results:
- Widget creation: ~500ns per widget
- JSON serialization: ~2-3μs for typical tree
- Tailwind parsing: ~1.2μs for standard classes, ~2.4μs with arbitrary values

## Troubleshooting

### CGO Errors

If you get linking errors, ensure:
1. Rust library is built: `cd engine && cargo build --release`
2. Path is correct in `ffi.go`: `-L../../engine/target/release`
3. Library name matches: `-lcentered_engine`

### JSON Serialization Issues

Enable debug logging:
```go
jsonStr, _ := widget.ToJSON()
fmt.Println(jsonStr)  // Inspect the JSON structure
```

### Memory Leaks

Ensure proper cleanup:
```go
defer engine.Shutdown()  // Always call Shutdown()
```

Check Rust side with Valgrind or AddressSanitizer:
```bash
cd engine
cargo build --release
valgrind ./target/release/centered_engine
```
