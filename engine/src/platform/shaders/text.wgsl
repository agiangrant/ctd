// Text rendering shader for glyph atlas
//
// This shader samples from a glyph atlas texture and applies vertex colors
// to render text with proper alpha blending.

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
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

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the atlas texture
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, input.tex_coords);

    // Use the atlas alpha as a mask, multiply by vertex color
    // Atlas contains white text, so we use its alpha channel
    let text_alpha = atlas_sample.a;
    let final_color = vec4<f32>(input.color.rgb, input.color.a * text_alpha);

    return final_color;
}
