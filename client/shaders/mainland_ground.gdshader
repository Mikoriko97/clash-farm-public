// Mainland ground: biome weights arrive in the vertex colour (R meadow, G forest,
// B rock, A sand) baked by tools/make_mainland.gd; steep slopes turn to rock and
// the shoreline to wet sand. Same noise break-up and wetness as island_ground.
// Compatibility renderer friendly: no screen/depth textures.
shader_type spatial;
render_mode cull_back, diffuse_burley, specular_schlick_ggx;

uniform sampler2D noise_tex : filter_linear_mipmap, repeat_enable;
uniform vec3 grass_a : source_color = vec3(0.21, 0.41, 0.16);
uniform vec3 grass_b : source_color = vec3(0.30, 0.52, 0.21);
uniform vec3 forest_floor : source_color = vec3(0.16, 0.30, 0.12);
uniform vec3 sand : source_color = vec3(0.82, 0.72, 0.48);
uniform vec3 rock : source_color = vec3(0.40, 0.35, 0.30);
uniform vec3 snow : source_color = vec3(0.90, 0.92, 0.95);
uniform float noise_scale = 0.045;
/// Сухі жовтуваті плями трави (за крупним шумом).
uniform vec3 dry_color : source_color = vec3(0.52, 0.5, 0.22);
/// Above this height (m) the rock whitens.
uniform float snow_line = 42.0;
/// 0 dry .. 1 soaked (Weather): darker, glossier ground.
uniform float wetness : hint_range(0.0, 1.0) = 0.0;
/// Лише в редакторі (addons/flora_paint): маска зон поверх землі — червоне крони,
/// зелене кущі, синє покрив, жовте деталі; 0.5 (як є) не тонує.
uniform sampler2D paint_tex : filter_linear, repeat_disable;
uniform bool paint_show = false;
uniform vec2 paint_origin = vec2(-512.0);
uniform float paint_size = 1024.0;

varying vec3 v_world;
varying vec4 v_biome;
varying float v_up;

void vertex() {
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_biome = COLOR;
	v_up = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz).y;
}

void fragment() {
	float n1 = texture(noise_tex, v_world.xz * noise_scale).r;
	float n2 = texture(noise_tex, v_world.xz * noise_scale * 7.0).r;
	float n3 = texture(noise_tex, v_world.xz * noise_scale * 0.22 + vec2(31.0, 17.0)).r;
	vec3 grass = mix(grass_a, grass_b, smoothstep(0.3, 0.7, n1)) * (0.86 + 0.28 * n2) * (0.9 + 0.2 * n3);
	grass = mix(grass, dry_color * (0.9 + 0.2 * n2), smoothstep(0.58, 0.8, n3 * 0.65 + n1 * 0.35) * 0.55);
	vec3 wood = mix(forest_floor, grass_a, smoothstep(0.35, 0.65, n1)) * (0.9 + 0.2 * n2);
	float total = max(v_biome.r + v_biome.g + v_biome.b + v_biome.a, 0.001);
	vec3 col = (grass * v_biome.r + wood * v_biome.g + rock * (0.85 + 0.3 * n2) * v_biome.b + sand * v_biome.a) / total;
	// Cliffs and the high ridges read as stone whatever the biome map says.
	float steep = 1.0 - smoothstep(0.62, 0.82, v_up);
	col = mix(col, rock * (0.8 + 0.35 * n2), steep);
	float high = smoothstep(snow_line - 4.0, snow_line + 6.0, v_world.y + (n1 - 0.5) * 6.0) * smoothstep(0.55, 0.9, v_up);
	col = mix(col, snow, high);
	// Under water and on the shore: darker, wetter sand.
	float shore = 1.0 - smoothstep(-0.2, 0.9, v_world.y);
	col = mix(col, sand * 0.75, shore * 0.8);
	float wet = max(wetness, shore);
	float puddle = smoothstep(0.62, 0.85, n1) * (1.0 - v_biome.a) * wetness;
	ALBEDO = col * (1.0 - 0.14 * wet - 0.18 * puddle);
	if (paint_show) {
		vec4 m = texture(paint_tex, (v_world.xz - paint_origin) / paint_size) * 2.0 - 1.0;
		vec3 tint = vec3(1.0, 0.15, 0.1) * m.r + vec3(0.1, 1.0, 0.2) * m.g + vec3(0.15, 0.3, 1.0) * m.b + vec3(1.0, 0.9, 0.1) * m.a;
		ALBEDO = mix(ALBEDO, clamp(ALBEDO + tint * 0.6, 0.0, 1.0), 0.8);
	}
	ROUGHNESS = 0.95 - 0.15 * wet - 0.5 * puddle;
	SPECULAR = 0.08 + 0.05 * wet + 0.3 * puddle;
}
