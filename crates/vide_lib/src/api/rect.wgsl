struct TransformUniform {
    matrix: mat4x4<f32>;
};
[[group(0), binding(0)]] var<uniform> transform: TransformUniform;

struct VertexInput {
    [[location(0)]] position: vec2<f32>;
    [[location(1)]] uv: vec2<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = vec3<f32>(0.3, 0.6, 0.9);
    out.clip_position = transform.matrix * vec4<f32>(model.position, 0.0, 1.0);
    return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}