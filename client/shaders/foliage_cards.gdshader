// Листя картками з паку Simple Trees (docs/ASSETS_NATURE.md): атлас з маскою
// прозорості, двобічні картки, вітер. Compatibility/WebGL: прозорість через
// ALPHA_SCISSOR (непрозорий прохід із discard, без сортування); щоб далекі
// картки не «лисіли» на мипмапах, альфа підсилюється за оцінкою рівня мипа з
// похідних UV. Вітер — та сама формула, що в nature_wind.gdshader (кора дерев
// Simple Trees стоїть на ньому з тими ж параметрами), тож листя не відривається
// від гілок; поверх — легке тремтіння листя.
//
// Тіні (скарга власника 2026-09-15: тіні під новими деревами мерехтіли): листя
// НЕ кидає тінь від сонця. Прохід тіні направленого світла ортографічний
// (PROJECTION_MATRIX[3][3] == 1), і там картки згортаються в точку. Тінь крони
// дає `shadow_crown` — гладка закрита крона з тим самим вітром, лише для тіней.
// Відтінок екземпляра приходить у COLOR (MultiMesh use_colors; поодинокі дерева — білий).
shader_type spatial;
render_mode cull_disabled, specular_disabled;

uniform sampler2D albedo_tex : source_color, filter_linear_mipmap, repeat_disable;
uniform vec3 tint : source_color = vec3(1.0);
uniform float alpha_cut : hint_range(0.05, 0.95) = 0.5;
/// Наскільки зростає альфа на кожен рівень мипа (зберігає густоту крони здалеку).
uniform float mip_coverage : hint_range(0.0, 1.0) = 0.3;
/// Затемнення зворотного боку картки.
uniform float backface_shade : hint_range(0.3, 1.0) = 0.8;
uniform float roughness : hint_range(0.0, 1.0) = 0.9;
uniform float height_ref = 8.0;
uniform float wind_strength = 0.18;
uniform float wind_speed = 0.8;
uniform float flutter = 0.02;
/// Скільки тіні отримує листя (0 — жодної, 1 — повна): гладка крона затінює і власне листя,
/// повна тінь робила крону надто темною; частка лишає обʼєм.
uniform float shadow_strength : hint_range(0.0, 1.0) = 0.9;
/// Мʼякий перехід світло–тінь на картках (wrap-освітлення).
uniform float wrap : hint_range(0.0, 1.0) = 0.15;

void vertex() {
	if (PROJECTION_MATRIX[3][3] > 0.5) {
		// Directional shadow pass: no leaf shadows (shadow_crown casts them).
		VERTEX = vec3(0.0);
	} else {
		vec3 anchor = MODEL_MATRIX[3].xyz;
		float w = clamp(VERTEX.y / height_ref, 0.0, 1.0);
		w *= w;
		float phase = anchor.x * 0.21 + anchor.z * 0.17;
		float t = TIME * wind_speed;
		float sway = sin(t + phase) * 0.6 + sin(t * 2.17 + phase * 1.9) * 0.3 + sin(t * 0.37 + phase * 0.5) * 0.4;
		float leaf = sin(t * 5.3 + VERTEX.x * 2.1 + VERTEX.z * 1.7 + phase) * flutter * w;
		VERTEX.x += sway * wind_strength * w + leaf;
		VERTEX.z += sway * wind_strength * 0.45 * w - leaf * 0.5;
	}
}

void fragment() {
	vec4 tex = texture(albedo_tex, UV);
	vec2 px = UV * vec2(textureSize(albedo_tex, 0));
	vec2 dx = dFdx(px);
	vec2 dy = dFdy(px);
	float mip = max(0.0, 0.5 * log2(max(dot(dx, dx), dot(dy, dy))));
	ALBEDO = tex.rgb * tint * COLOR.rgb * (FRONT_FACING ? 1.0 : backface_shade);
	ALPHA = tex.a * (1.0 + mip * mip_coverage);
	ALPHA_SCISSOR_THRESHOLD = alpha_cut;
	ROUGHNESS = roughness;
	SPECULAR = 0.15;
	if (!FRONT_FACING) {
		NORMAL = -NORMAL;
	}
}

void light() {
	float ndl = clamp((dot(NORMAL, LIGHT) + wrap) / (1.0 + wrap), 0.0, 1.0);
	// Shadow part of ATTENUATION only counts `shadow_strength`: the smooth crown shades its
	// own leaves, and full shadow turned the whole crown dark.
	float shade = mix(1.0, ATTENUATION, shadow_strength);
	DIFFUSE_LIGHT += LIGHT_COLOR * ndl * shade / PI;
}
