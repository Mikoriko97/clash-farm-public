// Рослинність з паку (docs/ASSETS_NATURE.md): та сама палітра, що й у
// materials/nature_palette.tres, плюс вітер — верхівка гойдається сильніше за
// основу (вага за висотою вершини до `height_ref`), фаза — від положення
// екземпляра у світі (MODEL_MATRIX містить трансформ MultiMesh), тож сусідні
// дерева не махають синхронно; дрібне тремтіння листя поверх. Compatibility:
// без екранних текстур, двобічно, як оригінальний матеріал паку.
shader_type spatial;
render_mode cull_disabled, diffuse_burley, specular_schlick_ggx;

uniform sampler2D albedo_tex : source_color, filter_linear_mipmap, repeat_enable;
uniform float roughness : hint_range(0.0, 1.0) = 0.9;
uniform float specular : hint_range(0.0, 1.0) = 0.2;
/// Висота (м), від якої гойдання повне; стовбур під нею майже нерухомий.
uniform float height_ref = 8.0;
uniform float wind_strength = 0.25;
uniform float wind_speed = 0.9;
uniform float flutter = 0.03;
/// Множити колір на COLOR (відтінок екземпляра MultiMesh) — далекі крони Simple Trees.
uniform bool tint_by_color = false;

void vertex() {
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

void fragment() {
	ALBEDO = texture(albedo_tex, UV).rgb * (tint_by_color ? COLOR.rgb : vec3(1.0));
	ROUGHNESS = roughness;
	SPECULAR = specular;
}
