// Grass blades drawn by a MultiMesh (scripts/grass_field.gd): dark at the root,
// light at the tip, swaying with a slow wind. Double-sided, unlit-ish so it blends
// with the ground colour instead of casting noisy shading.
shader_type spatial;
render_mode cull_disabled, diffuse_lambert, specular_disabled, shadows_disabled;

uniform vec3 root_color : source_color = vec3(0.24, 0.46, 0.16);
uniform vec3 tip_color : source_color = vec3(0.55, 0.78, 0.30);
uniform float wind_strength = 0.18;
uniform float wind_speed = 1.3;

varying float v_h;

void vertex() {
	vec3 world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	// UV.y is 0 at the tip and 1 at the root for a QuadMesh; use vertex height instead.
	v_h = clamp(VERTEX.y / 0.5, 0.0, 1.0);
	float phase = world.x * 0.35 + world.z * 0.27;
	float sway = sin(TIME * wind_speed + phase) * 0.6 + sin(TIME * wind_speed * 2.3 + phase * 1.7) * 0.4;
	VERTEX.x += sway * wind_strength * v_h * v_h;
	VERTEX.z += sway * wind_strength * 0.4 * v_h * v_h;
}

void fragment() {
	// Both faces lit like the ground under them: an up-facing normal in view space.
	NORMAL = normalize((VIEW_MATRIX * vec4(0.0, 1.0, 0.0, 0.0)).xyz);
	ALBEDO = mix(root_color, tip_color, v_h);
	ROUGHNESS = 1.0;
}
