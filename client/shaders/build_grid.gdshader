shader_type spatial;
render_mode unshaded, shadows_disabled, cull_disabled, blend_mix, depth_draw_never;

uniform float cell_size : hint_range(0.4, 8.0) = 1.6;
uniform vec4 line_color : source_color = vec4(1.0, 0.92, 0.45, 0.55);
uniform float line_width : hint_range(0.01, 0.2) = 0.045;

varying vec3 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec2 p = world_pos.xz / cell_size;
	vec2 f = abs(fract(p) - 0.5);
	float line = 1.0 - smoothstep(0.0, line_width, min(f.x, f.y));
	ALBEDO = line_color.rgb;
	ALPHA = line * line_color.a;
}
