# Asset System Design

## Overview

The asset system handles loading, bundling, and managing resources (images, sprite sheets, fonts, audio) for both embedded and runtime assets.

## Asset Types

### 1. **Textures** (Images)
```rust
struct TextureAsset {
    id: u32,
    width: u32,
    height: u32,
    format: TextureFormat,  // RGBA8, etc.
    data: Vec<u8>,
}
```
- Loaded from PNG, JPEG, WebP
- GPU texture created on load
- Cached in texture atlas or standalone texture

### 2. **Sprite Sheets**
```rust
struct SpriteSheet {
    id: u32,
    texture_id: u32,
    sprites: Vec<SpriteDefinition>,
}

struct SpriteDefinition {
    index: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    // Optional metadata
    pivot_x: f32,  // For rotation center
    pivot_y: f32,
}
```
- Single texture with multiple sprites
- Metadata file (JSON/TOML) defines sprite locations
- Efficient batching (all sprites share one texture)

### 3. **Fonts**
```rust
struct FontAsset {
    id: u32,
    family: String,
    source: FontSource,
}

enum FontSource {
    System(String),           // "Helvetica Neue"
    Embedded(Vec<u8>),        // TTF/OTF data
    File(PathBuf),            // Path to TTF/OTF
}
```
- System fonts (already working)
- Embedded fonts (bundled with app)
- Runtime-loaded fonts

### 4. **Meshes** (for 3D/custom geometry)
```rust
struct MeshAsset {
    id: u32,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}
```
- Pre-built geometry
- Can be instanced for performance

## Asset Bundle Format

### Go Side (Build Time)
```go
type AssetBundle struct {
    Textures    map[string][]byte  // "button.png" → PNG data
    SpriteSheets map[string]SpriteSheetDef
    Fonts       map[string][]byte  // "custom.ttf" → TTF data
}

// Bundle everything into single file or embed in binary
func BuildAssetBundle(assets *AssetBundle) []byte {
    // Serialize to efficient binary format
    // Or use go:embed for compile-time embedding
}
```

### Rust Side (Runtime)
```rust
// FFI function to load asset bundle
pub extern "C" fn centered_assets_load_bundle(
    handle: EngineHandle,
    bundle_data: *const u8,
    bundle_len: usize,
) -> i32 {
    // Deserialize bundle
    // Create GPU textures
    // Cache for rendering
}

// FFI function to load individual asset
pub extern "C" fn centered_assets_load_texture(
    handle: EngineHandle,
    name: *const c_char,
    data: *const u8,
    len: usize,
) -> u32 {
    // Returns asset ID
}
```

## Loading Strategies

### 1. **Embedded Assets** (Compile-Time)
```go
//go:embed assets/*
var assetFS embed.FS

func main() {
    engine := centered.NewEngine(config)

    // Load all embedded assets
    btnImage, _ := assetFS.ReadFile("assets/button.png")
    btnID := engine.LoadTexture("button", btnImage)

    // Use in rendering
    engine.DrawImage(100, 100, 50, 50, btnID)
}
```

**Pros**: No runtime I/O, assets always available, single binary
**Cons**: Larger binary size
**Use case**: Icons, UI elements, core game assets

### 2. **Runtime Assets** (Load on Demand)
```go
func loadLevel(engine *Engine, levelName string) {
    data, _ := os.ReadFile(fmt.Sprintf("levels/%s.png", levelName))
    bgID := engine.LoadTexture("level_bg", data)
}
```

**Pros**: Smaller binary, dynamic content
**Cons**: Requires file I/O, can fail
**Use case**: Large images, user content, DLC

### 3. **Hybrid Approach**
```go
// Core assets embedded
//go:embed assets/core/*
var coreAssets embed.FS

// Optional content loaded at runtime
func init() {
    engine.LoadBundle(coreAssets)  // Always available
}

func loadOptional() {
    if exists("dlc/pack1") {
        engine.LoadBundleFromDisk("dlc/pack1")  // Optional
    }
}
```

## Sprite Sheet Example

### Metadata File (`sprites.json`)
```json
{
  "texture": "spritesheet.png",
  "sprites": [
    {
      "name": "player_idle",
      "index": 0,
      "x": 0, "y": 0,
      "width": 32, "height": 32,
      "pivot_x": 0.5, "pivot_y": 0.5
    },
    {
      "name": "player_run_0",
      "index": 1,
      "x": 32, "y": 0,
      "width": 32, "height": 32
    }
  ]
}
```

### Usage in Go
```go
sheetID := engine.LoadSpriteSheet("sprites.json", "spritesheet.png")

// Draw specific sprite
engine.DrawSprite(100, 100, 64, 64, sheetID, 0)  // player_idle

// Animation loop
for frame := 0; frame < 8; frame++ {
    engine.DrawSprite(x, y, w, h, sheetID, frame)
}
```

## Memory Management

### GPU Texture Lifecycle
```rust
struct TextureCache {
    textures: HashMap<u32, wgpu::Texture>,
    atlases: Vec<TextureAtlas>,
    ref_counts: HashMap<u32, usize>,
}

impl TextureCache {
    fn load(&mut self, id: u32, data: &[u8]) {
        // Decode image
        // Create wgpu texture
        // Cache in HashMap
    }

    fn unload(&mut self, id: u32) {
        // Decrement ref count
        // Free GPU memory if zero
    }
}
```

### Asset Unloading (Go → Rust FFI)
```rust
pub extern "C" fn centered_assets_unload(
    handle: EngineHandle,
    asset_id: u32,
) {
    // Remove from cache
    // Free GPU texture
}
```

## Performance Considerations

### Texture Atlas Packing
- **Small textures** (<256x256) → Pack into shared atlas
- **Large textures** (>512x512) → Standalone texture
- **Sprite sheets** → Already packed, use as-is

### Batching Strategy
```rust
// Sort draw calls by texture to minimize state changes
commands.sort_by_key(|cmd| match cmd {
    DrawImage { texture_id, .. } => *texture_id,
    DrawSprite { sprite_sheet_id, .. } => *sprite_sheet_id,
    _ => 0,
});
```

### Lazy Loading
```rust
// Don't create GPU texture until first use
struct LazyTexture {
    id: u32,
    data: Vec<u8>,
    gpu_texture: Option<wgpu::Texture>,
}
```

## Next Steps for Implementation

1. ✅ Define RenderCommand API (DONE)
2. **Implement DrawRect** (high-level, no assets needed)
3. **Implement DrawTriangles** (low-level, foundation)
4. **Add texture loading** (FFI + GPU upload)
5. **Implement DrawImage** (uses loaded textures)
6. **Add sprite sheet support** (metadata + rendering)
7. **Asset bundle format** (serialization)
8. **Go embedding with `go:embed`**

This design ensures:
- ✅ Single binary deployment (embedded assets)
- ✅ Efficient GPU memory usage (atlasing)
- ✅ High performance (batching, caching)
- ✅ Flexibility (runtime loading supported)
- ✅ Game-ready (sprite sheets, instancing)
