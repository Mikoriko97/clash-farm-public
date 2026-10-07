// Стилізована вода з піною на березі без екранних текстур (docs/MAINLAND.md §9,
// Compatibility/WebGL): глибина рахується не з буфера глибини, а з форми дна —
// острів: SDF округлого квадрата зі спуском як у island_ground.gdshader
// (depth_mode 0), материк: карта висот терену (depth_mode 1). Мілке світліше,
// на березі — дві смуги піни, що накочуються, гребені хвиль пінисті, дві
// шарові normal-карти дають брижі й відблиск сонця, Френель підмішує колір
// неба під гострим кутом. Кольори surface/depth/foam крутить DayCycle.
shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_disabled, diffuse_burley, specular_schlick_ggx;

group_uniforms Colour;
uniform vec4 surface_color : source_color = vec4(0.16, 0.52, 0.66, 1.0);
uniform vec4 depth_color : source_color = vec4(0.04, 0.16, 0.36, 1.0);
uniform vec4 foam_color : source_color = vec4(0.95, 0.98, 1.0, 1.0);
uniform float depth_fade = 5.0;
uniform float alpha : hint_range(0.5, 1.0) = 0.93;

group_uniforms Waves;
uniform sampler2D wave_texture : repeat_enable, filter_linear_mipmap;
uniform sampler2D wave_normal_texture : hint_normal, repeat_enable, filter_linear_mipmap;
uniform sampler2D foam_texture : repeat_enable, filter_linear_mipmap;
uniform vec2 wave_scale = vec2(0.045, 0.045);
uniform vec2 wave_velocity = vec2(0.03, 0.02);
uniform float normal_strength : hint_range(0.0, 1.0) = 0.35;
uniform float crest_foam : hint_range(0.0, 1.0) = 0.35;

group_uniforms Shore;
uniform float foam_width = 1.8;
uniform float foam_speed = 1.2;
/// 0 — острів (SDF нижче), 1 — материк (карта висот).
uniform int depth_mode = 0;
/// Центр острова у світі: після розширення він зміщується (IslandShape).
uniform vec2 island_centre = vec2(0.0, 0.0);
uniform vec2 half_extent = vec2(24.0, 24.0);
uniform float corner_radius = 7.0;
uniform float edge_width = 5.0;
uniform float edge_drop = 5.0;
uniform sampler2D height_tex : filter_nearest, repeat_disable;
uniform vec2 map_origin = vec2(-512.0, -512.0);
uniform float map_size = 1024.0;

varying vec3 world_pos;

float sd_rounded_box(vec2 p, vec2 b, float r) {
	vec2 q = abs(p) - b + vec2(r);
	return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

float ground_height(vec2 p) {
	if (depth_mode == 0) {
		float d = sd_rounded_box(p - island_centre, half_extent, corner_radius);
		float t = clamp(d / edge_width, 0.0, 1.0);
		float y = -t * t * edge_drop;
		return y - max(d - edge_width, 0.0) * 1.5;
	}
	vec2 uv = (p - map_origin) / map_size;
	if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
		return -30.0;
	}
	return texture(height_tex, uv).r;
}

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec2 uv = world_pos.xz * wave_scale;
	vec2 flow = TIME * wave_velocity;
	float wave = texture(wave_texture, uv * 1.5 - flow).r;
	wave = mix(wave, texture(wave_texture, uv + flow * 0.6 - wave * 0.08).r, 0.5);
	vec3 n1 = texture(wave_normal_texture, uv * 1.5 - flow).rgb;
	vec3 n2 = texture(wave_normal_texture, uv * 0.7 + flow * 0.5 + vec2(0.37, 0.11)).rgb;
	NORMAL_MAP = normalize(mix(n1, n2, 0.5) * 2.0 - 1.0) * 0.5 + 0.5;
	NORMAL_MAP_DEPTH = normal_strength;

	float depth = max(world_pos.y - ground_height(world_pos.xz), 0.0);
	float shallow = exp(-depth / max(depth_fade, 0.01));
	vec3 col = mix(depth_color.rgb, surface_color.rgb, shallow);

	// Shore foam: a soft band plus a second line rolling onto the beach.
	float shore = 1.0 - smoothstep(0.0, foam_width, depth);
	float roll = 0.5 + 0.5 * sin(depth * 2.6 - TIME * foam_speed + wave * 2.0);
	float mask = texture(foam_texture, world_pos.xz * 0.35 + flow * 0.8).r;
	float foam = smoothstep(0.45, 0.85, shore * (0.55 + 0.45 * roll) + (mask - 0.5) * 0.35);
	foam = max(foam, smoothstep(0.78, 0.92, wave) * crest_foam * (1.0 - shallow * 0.5));
	foam = clamp(foam, 0.0, 1.0);

	float fres = pow(1.0 - clamp(dot(normalize(NORMAL), normalize(VIEW)), 0.0, 1.0), 3.0);
	col = mix(col, surface_color.rgb * 1.25, fres * 0.35);
	col = mix(col, surface_color.rgb * 1.08, smoothstep(0.55, 0.85, wave) * 0.25);
	ALBEDO = mix(col, foam_color.rgb, foam);
	ROUGHNESS = mix(0.14, 0.7, foam);
	SPECULAR = 0.55;
	// Only the first metre or so is see-through (sand under the water); deeper the
	// surface is solid, so nothing of the seabed mesh shows. Foam stays opaque.
	float see = 1.0 - smoothstep(0.2, 1.6, depth);
	ALPHA = mix(mix(alpha, 0.5, see), 1.0, foam);
}
