// Контактна тінь під деревами, кущами й валунами (docs/MAINLAND.md §9): мʼяка
// темна пляма з альфою на площині над ґрунтом — заміна SSAO, якого в
// Compatibility нема; обʼєкти перестають «висіти» над травою. Звичайний
// alpha-бленд (blend_mul у WebGL малював чорні квадрати), без запису глибини,
// без тіней; площина 1 × 1 м, масштабується екземпляром.
shader_type spatial;
render_mode blend_mix, unshaded, cull_disabled, depth_draw_never, shadows_disabled;

uniform vec3 tint : source_color = vec3(0.05, 0.08, 0.04);
uniform float strength : hint_range(0.0, 1.0) = 0.5;
uniform float softness : hint_range(0.05, 1.0) = 0.8;

void fragment() {
	float d = length(UV - vec2(0.5)) * 2.0;
	ALBEDO = tint;
	ALPHA = smoothstep(1.0, 1.0 - softness, d) * strength;
}
