// Soft vignette overlay (PostFX layer). Pure alpha darkening: no screen texture,
// so it costs nothing on the Compatibility renderer and never touches input.
shader_type canvas_item;
render_mode blend_mix, unshaded;

uniform float strength : hint_range(0.0, 1.0) = 0.26;
uniform float inner : hint_range(0.0, 1.5) = 0.62;
uniform float softness : hint_range(0.1, 2.0) = 0.9;
uniform vec3 tint : source_color = vec3(0.05, 0.06, 0.09);

void fragment() {
	vec2 uv = UV * 2.0 - 1.0;
	float d = length(uv * vec2(1.0, 0.85));
	float v = smoothstep(inner, inner + softness, d);
	COLOR = vec4(tint, v * strength);
}
