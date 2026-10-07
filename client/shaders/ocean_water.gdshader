shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back, diffuse_burley, specular_schlick_ggx;

uniform vec3 shallow_color : source_color = vec3(0.22, 0.58, 0.68);
uniform vec3 deep_color : source_color = vec3(0.04, 0.16, 0.32);
uniform float wave_speed : hint_range(0.05, 2.0) = 0.42;
uniform float wave_height : hint_range(0.0, 1.0) = 0.16;
uniform float roughness_val : hint_range(0.0, 1.0) = 0.07;

void vertex() {
	float w1 = sin(VERTEX.x * 0.21 + TIME * wave_speed);
	float w2 = cos(VERTEX.z * 0.17 + TIME * wave_speed * 0.76);
	VERTEX.y += (w1 * 0.62 + w2 * 0.38) * wave_height;
	NORMAL = normalize(vec3(-w1 * 0.18, 1.0, -w2 * 0.14));
}

void fragment() {
	float fresnel = pow(1.0 - clamp(dot(normalize(NORMAL), normalize(VIEW)), 0.0, 1.0), 4.0);
	ALBEDO = mix(deep_color, shallow_color, fresnel);
	ROUGHNESS = roughness_val;
	METALLIC = 0.05;
	SPECULAR = 0.35;
	RIM = 0.15;
	RIM_TINT = 0.3;
	EMISSION = shallow_color * fresnel * 0.03;
}
