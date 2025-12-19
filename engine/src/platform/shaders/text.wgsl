// Text rendering shader for glyph atlas
//
// This shader samples from a glyph atlas texture and applies vertex colors
// to render text with proper alpha blending.
//
// For regular text: texture is white on transparent, vertex color provides the tint
// For emojis: texture contains actual colors, use_texture_color flag tells shader to use them

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) use_texture_color: f32, // 1.0 = use texture RGB (emoji), 0.0 = use vertex color (text)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) use_texture_color: f32,
}

@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert from screen coordinates to NDC (-1 to 1)
    // Assuming input position is already in NDC or will be converted by the application
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.tex_coords = input.tex_coords;
    output.color = input.color;
    output.use_texture_color = input.use_texture_color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the atlas texture
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, input.tex_coords);

    // For emojis (use_texture_color > 0.5): use texture RGB directly
    // For regular text: use vertex color RGB (tinting white text)
    let is_emoji = input.use_texture_color > 0.5;
    let final_rgb = select(input.color.rgb, atlas_sample.rgb, is_emoji);

    // Use atlas alpha as mask, modulated by vertex alpha
    let text_alpha = atlas_sample.a;
    let final_color = vec4<f32>(final_rgb, input.color.a * text_alpha);

    return final_color;
}
