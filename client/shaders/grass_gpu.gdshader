// Трава, розставлена у вершинному шейдері (етап 2 плану, docs/PLAN.md): форк
// grass_quad.gdshader (BinbunGrass, пак власника) без жодної роботи CPU на пучок.
// MultiMesh несе N одиничних трансформів, а кожен пучок сам рахує своє місце з
// INSTANCE_ID: сітка `side × side` клітинок `cell_m` навколо `anchor` (сітка
// прилипає до світових клітинок, тож при русі пучки не пливуть, лише зникають
// на дальньому краю й зʼявляються на ближньому), зсув/поворот/масштаб — хеш
// світової клітинки. Висота — з карти висот материка (texelFetch + білінійна
// інтерполяція, як `MainlandTerrain.height_at`), дозвіл рости — з маски:
// mask_mode 0 = ваги біомів материка (R лука, G ліс, B скеля, A пісок),
// mask_mode 1 = маска зайнятості острова (R > 0.5 — вільно). Кола-винятки
// (бруківка площі) і затухання з відстанню зменшують масштаб до нуля, а
// нульовий пучок растеризатор не малює.
shader_type spatial;
render_mode blend_mix, depth_prepass_alpha;

group_uniforms Placement;
uniform vec2 anchor = vec2(0.0);
uniform int side = 300;
uniform float cell_m = 0.25;
/// Частка радіуса, з якої трава рідшає до нуля на краю; 1.0 — без затухання (острів:
/// якір у центрі, і затухання малювало круглу пляму).
uniform float fade_start : hint_range(0.0, 1.0) = 0.55;
uniform float scale_min = 0.8;
uniform float scale_max = 1.3;
uniform bool use_height = true;
uniform sampler2D height_tex : filter_nearest, repeat_disable;
uniform sampler2D mask_tex : filter_nearest, repeat_disable;
uniform int mask_mode = 0;
/// Нижній лівий кут карт (x, z) і їх розмір у метрах.
uniform vec2 map_origin = vec2(-512.0);
uniform float map_size = 1024.0;
/// Не рости нижче цієї висоти (море).
uniform float min_height = 0.5;
/// Кола без трави: (x, z, радіус, мʼякий край).
uniform vec4 exclusions[4];
uniform int exclusion_count = 0;

group_uniforms Island;
/// Форма острова як у island_ground.gdshader: заокруглений прямокутник, за краєм
/// земля спускається у воду, а трава рідшає в пісок тим самим рваним краєм з шуму.
uniform vec2 island_centre = vec2(0.0);
uniform vec2 half_extent = vec2(24.0, 24.0);
uniform float corner_radius = 7.0;
uniform float edge_width = 5.0;
uniform float edge_drop = 5.0;
uniform sampler2D edge_noise : filter_linear_mipmap, repeat_enable;
uniform float noise_scale = 0.045;

group_uniforms Look;
uniform bool billboard = true;
uniform sampler2D shape_texture;
uniform sampler2D shape_atlas;
uniform bool use_atlas = false;
uniform sampler2D noise_texture;
uniform sampler2D color_gradient;
uniform float random_variation : hint_range(0.0, 1.0, 0.001) = 0.002;
uniform sampler2D wind_texture;
uniform vec2 wind_velocity;
uniform float alpha_cut_start : hint_range(0.0, 1.0, 0.05) = 0.1;
uniform float alpha_cut_end : hint_range(0.0, 1.0, 0.05) = 0.9;
uniform int alpha_mode : hint_enum("Smooth", "Dithered", "Cut") = 0;

varying flat int id;
varying vec3 world_pos;

float bayer4(vec2 frag) {
	int x = int(mod(frag.x, 4.0));
	int y = int(mod(frag.y, 4.0));
	int index = x + y * 4;
	int matrix[16] = int[](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
	return float(matrix[index]) / 16.0;
}

vec2 atlas_uv(vec2 uv, int index, int size) {
	int tile_count = size * size;
	int i = index % tile_count;
	float x = float(i % size);
	float y = float(i / size);
	vec2 tile_size = 1.0 / vec2(float(size));
	return uv * tile_size + (vec2(x, y) * tile_size);
}

float randomf(int index, int seed) {
	return sin(float(index + seed)) * 0.5 + 0.5;
}

// Хеш світової клітинки: три незалежні числа 0..1 (зсув x, зсув z, масштаб/поворот).
vec3 cell_hash(vec2 cell) {
	vec3 p = fract(vec3(cell.xyx) * vec3(0.1031, 0.1030, 0.0973));
	p += dot(p, p.yxz + 33.33);
	return fract((p.xxy + p.yzz) * p.zyx);
}

float sd_rounded_box(vec2 p, vec2 b, float r) {
	vec2 q = abs(p) - b + vec2(r);
	return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Земля острова під точкою (той самий спуск, що у island_ground.gdshader) і
// частка піску 0..1 на рваному краю; трава лишається там, де піску ще нема.
vec2 island_ground(vec2 xz) {
	float d = sd_rounded_box(xz - island_centre, half_extent, corner_radius);
	float t = clamp(d / edge_width, 0.0, 1.0);
	float y = -(t * t * edge_drop + max(d - edge_width, 0.0) * 1.5);
	float n1 = textureLod(edge_noise, xz * noise_scale, 0.0).r;
	float n2 = textureLod(edge_noise, xz * noise_scale * 7.0, 0.0).r;
	float sandy = smoothstep(0.0, 0.18, t + (n2 - 0.5) * 0.14 + (n1 - 0.5) * 0.06);
	return vec2(y, sandy);
}

float height_at(vec2 xz) {
	vec2 f = xz - map_origin;
	ivec2 p = ivec2(floor(f));
	vec2 t = fract(f);
	int n = int(map_size) - 1;
	ivec2 p0 = clamp(p, ivec2(0), ivec2(n));
	ivec2 p1 = clamp(p + ivec2(1), ivec2(0), ivec2(n));
	float h00 = texelFetch(height_tex, ivec2(p0.x, p0.y), 0).r;
	float h10 = texelFetch(height_tex, ivec2(p1.x, p0.y), 0).r;
	float h01 = texelFetch(height_tex, ivec2(p0.x, p1.y), 0).r;
	float h11 = texelFetch(height_tex, ivec2(p1.x, p1.y), 0).r;
	return mix(mix(h00, h10, t.x), mix(h01, h11, t.x), t.y);
}

// 1 — можна рости, 0 — ні (біом, море, зайнятість).
float allowed(vec2 xz, float h) {
	vec2 f = xz - map_origin;
	int n = int(map_size) - 1;
	ivec2 p = clamp(ivec2(floor(f)), ivec2(0), ivec2(n));
	vec4 m = texelFetch(mask_tex, p, 0);
	if (mask_mode == 0) {
		bool ok = (m.r + m.g >= 0.45) && (m.b <= 0.3) && (m.a <= 0.5) && (h >= min_height);
		return ok ? 1.0 : 0.0;
	}
	return m.r > 0.5 ? 1.0 : 0.0;
}

float keep_outside(vec2 xz) {
	float keep = 1.0;
	for (int i = 0; i < 4; i++) {
		if (i >= exclusion_count) {
			break;
		}
		vec4 c = exclusions[i];
		float d = distance(xz, c.xy) - c.z;
		keep = min(keep, clamp(d / max(c.w, 0.01), 0.0, 1.0));
	}
	return keep;
}

float wind_noise() {
	return texture(wind_texture, (world_pos.xz * 0.03) - (vec2(TIME * 0.01) * wind_velocity)).r;
}

vec3 wind(vec2 uv) {
	float w = wind_noise();
	float affect = pow(1.0 - uv.y, 2.0);
	return vec3(w * wind_velocity.x, 0.0, w * wind_velocity.y) * 0.25 * affect;
}

void vertex() {
	int ix = INSTANCE_ID % side;
	int iz = INSTANCE_ID / side;
	float half_side = float(side) * 0.5;
	// Integer cell indices: hashing floor(pos / cell) flickered when the division
	// landed on 2.9999 for one cell and 3.0 for the next frame.
	ivec2 cell_i = ivec2(floor(anchor / cell_m)) - ivec2(side / 2) + ivec2(ix, iz);
	vec3 h = cell_hash(vec2(cell_i));
	// Atlas tile and colour bump keyed by the world cell too, not by the instance:
	// otherwise every blade changed its look each time the grid stepped (strobing).
	id = (cell_i.x * 7919 + cell_i.y * 104729) & 0x7fffffff;
	vec2 xz = vec2(cell_i) * cell_m + h.xy * cell_m;
	float y = use_height ? height_at(xz) : 0.0;
	float shore = 1.0;
	if (mask_mode == 1) {
		vec2 g = island_ground(xz);
		y = g.x;
		shore = 1.0 - smoothstep(0.15, 0.45, g.y);
	}
	float radius = half_side * cell_m;
	float dist = distance(xz, anchor) / max(radius, 0.01);
	float fade = fade_start >= 1.0 ? 1.0 : 1.0 - smoothstep(fade_start, 1.0, dist);
	float scale = mix(scale_min, scale_max, h.z) * fade * shore * keep_outside(xz) * allowed(xz, y);
	vec3 origin = vec3(xz.x, y, xz.y);
	world_pos = origin + VERTEX * scale;
	VERTEX *= scale;
	if (billboard) {
		mat4 billboard_matrix = mat4(
				MAIN_CAM_INV_VIEW_MATRIX[0],
				MAIN_CAM_INV_VIEW_MATRIX[1],
				MAIN_CAM_INV_VIEW_MATRIX[2],
				vec4(origin, 1.0));
		MODELVIEW_MATRIX = VIEW_MATRIX * billboard_matrix;
		VERTEX += (VIEW_MATRIX * vec4(wind(UV), 0.0)).xyz * scale;
	} else {
		float yaw = h.z * 6.2831853;
		float c = cos(yaw);
		float s = sin(yaw);
		VERTEX = vec3(VERTEX.x * c - VERTEX.z * s, VERTEX.y, VERTEX.x * s + VERTEX.z * c) + origin + wind(UV) * scale;
	}
	NORMAL = vec3(0.0, 1.0, 0.0);
}

void fragment() {
	float shape_value = texture(shape_texture, UV).r;
	if (use_atlas) {
		shape_value = texture(shape_atlas, atlas_uv(UV, id, 2)).r;
	}
	float noise_value = texture(noise_texture, world_pos.xz * 0.1).r;
	if (randomf(id, 1) < random_variation) {
		noise_value += 0.4;
	}
	vec4 color = texture(color_gradient, vec2(noise_value, 0.0));
	ALBEDO = color.rgb;
	float alpha = shape_value;
	if (alpha_mode == 0) {
		ALPHA = alpha;
	} else {
		alpha = clamp((shape_value - alpha_cut_start) / (alpha_cut_end - alpha_cut_start), 0.0, 1.0);
		if (alpha_mode == 1) {
			alpha = step(bayer4(FRAGCOORD.xy) + 0.01, alpha);
		}
		if (alpha < 0.1) {
			discard;
		}
	}
}

void light() {
	float ndotl = clamp(dot(LIGHT, NORMAL), 0.0, 1.0) * ATTENUATION;
	DIFFUSE_LIGHT += ndotl * LIGHT_COLOR;
}
