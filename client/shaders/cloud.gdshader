shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back, diffuse_lambert_wrap, specular_disabled, shadows_disabled;

uniform vec4 albedo : source_color = vec4(0.97, 0.98, 1.0, 0.78);
uniform vec4 belly : source_color = vec4(0.78, 0.82, 0.9, 1.0);
uniform float edge_softness : hint_range(0.15, 1.2) = 0.62;
uniform float glow : hint_range(0.0, 0.6) = 0.16;

varying vec3 world_n;

void vertex() {
	world_n = (MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz;
}

void fragment() {
	float ndv = clamp(dot(normalize(NORMAL), normalize(VIEW)), 0.0, 1.0);
	float shell = smoothstep(0.02, edge_softness, ndv);
	float up = clamp(normalize(world_n).y * 0.55 + 0.45, 0.0, 1.0);
	vec3 col = mix(belly.rgb, albedo.rgb, up);
	ALBEDO = col;
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	ALPHA = albedo.a * pow(shell, 0.72);
	EMISSION = col * glow * shell;
	if (ALPHA < 0.035) {
		discard;
	}
}
