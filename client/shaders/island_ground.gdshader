// Island ground: a flat top that rolls off into sand and rock at the rounded-square
// edge (vertex displacement), grass colour broken up by two noise octaves.
// Compatibility renderer friendly: no screen/depth textures.
shader_type spatial;
render_mode cull_back, diffuse_burley, specular_schlick_ggx;

uniform sampler2D noise_tex : filter_linear_mipmap, repeat_enable;
uniform vec3 grass_a : source_color = vec3(0.30, 0.56, 0.20);
uniform vec3 grass_b : source_color = vec3(0.45, 0.68, 0.25);
uniform vec3 sand : source_color = vec3(0.86, 0.78, 0.55);
uniform vec3 rock : source_color = vec3(0.40, 0.35, 0.30);
/// Half extents of the flat top (m) around the mesh origin; IslandShape keeps the
/// mesh centred on the server's rectangle and the collision box in step.
uniform vec2 half_extent = vec2(24.0, 24.0);
uniform float corner_radius = 7.0;
/// How far (m) past the flat top the ground keeps going down, and how deep.
uniform float edge_width = 5.0;
uniform float edge_drop = 5.0;
uniform float noise_scale = 0.045;
/// Сухі жовтуваті плями трави (за крупним шумом).
uniform vec3 dry_color : source_color = vec3(0.52, 0.5, 0.22);
/// 0 dry .. 1 soaked (Weather): darker, glossier ground.
uniform float wetness : hint_range(0.0, 1.0) = 0.0;

varying float v_edge;
varying vec3 v_world;

float sd_rounded_box(vec2 p, vec2 b, float r) {
	vec2 q = abs(p) - b + vec2(r);
	return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

void vertex() {
	float d = sd_rounded_box(VERTEX.xz, half_extent, corner_radius);
	float t = clamp(d / edge_width, 0.0, 1.0);
	// Past the slope the seabed keeps falling (same curve as water_shore.gdshader),
	// so no flat shelf ever shows through the water.
	VERTEX.y -= t * t * edge_drop + max(d - edge_width, 0.0) * 1.5;
	// Tilt the normal down the slope so the rim shades as a cliff, not a floor.
	vec2 dir = normalize(VERTEX.xz + vec2(0.0001));
	float slope = 2.0 * t * edge_drop / edge_width;
	NORMAL = normalize(vec3(-dir.x * slope, 1.0, -dir.y * slope));
	v_edge = t;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	float n1 = texture(noise_tex, v_world.xz * noise_scale).r;
	float n2 = texture(noise_tex, v_world.xz * noise_scale * 7.0).r;
	float n3 = texture(noise_tex, v_world.xz * noise_scale * 0.22 + vec2(31.0, 17.0)).r;
	vec3 grass = mix(grass_a, grass_b, smoothstep(0.3, 0.7, n1)) * (0.86 + 0.28 * n2) * (0.9 + 0.2 * n3);
	grass = mix(grass, dry_color * (0.9 + 0.2 * n2), smoothstep(0.58, 0.8, n3 * 0.65 + n1 * 0.35) * 0.55);
	// Grass thins into sand with a ragged edge; below the waterline the sand is wet and dark.
	float sandy = smoothstep(0.0, 0.18, v_edge + (n2 - 0.5) * 0.14 + (n1 - 0.5) * 0.06);
	float wet_sand = smoothstep(0.22, 0.36, v_edge);
	float rocky = smoothstep(0.42, 0.8, v_edge);
	vec3 col = mix(grass, sand, sandy);
	col = mix(col, sand * 0.68, wet_sand);
	col = mix(col, rock * (0.85 + 0.3 * n2), rocky);
	// Wet ground: a mild, matte darkening everywhere; only the hollows of the coarse noise
	// collect puddles that turn glossy, so rain never reads as a flooded mirror.
	// Under water the seabed fades into the deep-water colour: the mesh rim vanishes.
	float under = smoothstep(-0.4, -3.0, v_world.y);
	col = mix(col, vec3(0.05, 0.14, 0.26), under * 0.9);
	float puddle = smoothstep(0.62, 0.85, n1) * (1.0 - sandy) * wetness;
	ALBEDO = col * (1.0 - 0.14 * wetness - 0.18 * puddle);
	ROUGHNESS = 0.95 - 0.15 * wetness - 0.5 * puddle;
	SPECULAR = 0.08 + 0.05 * wetness + 0.3 * puddle;
}
