shader_type spatial;
render_mode cull_disabled, depth_draw_opaque, diffuse_burley, specular_disabled;

uniform vec3 color : source_color = vec3(0.2, 0.48, 0.14);
uniform vec3 color2 : source_color = vec3(0.58, 0.82, 0.28);
uniform sampler2D noise;
uniform float noiseScale = 20.0;
uniform float wind_strength = 0.08;
uniform float wind_speed = 1.1;

varying vec3 worldPos;

void vertex() {
	worldPos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	float bend = UV.y * UV.y;
	float wind = sin(TIME * wind_speed + worldPos.x * 0.85 + worldPos.z * 0.55);
	VERTEX.x += wind * bend * wind_strength;
	VERTEX.z += cos(TIME * wind_speed * 0.8 + worldPos.z * 0.7) * bend * wind_strength * 0.55;
}

void fragment() {
	vec3 noiseLevel = texture(noise, worldPos.xz / noiseScale).rgb;
	float grade = clamp(UV.y * 0.72 + noiseLevel.r * 0.28, 0.0, 1.0);
	ALBEDO = mix(color, color2, grade);
	ROUGHNESS = 0.88;
	RIM = 0.18;
	RIM_TINT = 0.4;
	if (!FRONT_FACING) {
		NORMAL = -NORMAL;
	}
}
