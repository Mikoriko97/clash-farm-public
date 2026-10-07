shader_type sky;

group_uniforms Sky;
uniform vec4 sky_day : source_color = vec4(0.24, 0.54, 0.90, 1.0);
uniform vec4 horizon_day : source_color = vec4(0.76, 0.88, 0.96, 1.0);
uniform vec4 horizon_sunset : source_color = vec4(1.0, 0.52, 0.24, 1.0);
uniform vec4 sky_sunset : source_color = vec4(0.18, 0.22, 0.45, 1.0);
uniform vec4 horizon_night : source_color = vec4(0.06, 0.08, 0.16, 1.0);
uniform vec4 sky_night : source_color = vec4(0.02, 0.03, 0.08, 1.0);
uniform float day_night_mix : hint_range(-1.0, 1.0, 0.01) = 0.8;
uniform bool use_directional_light = false;
uniform float horizon_exponent = 2.2;
uniform vec3 sun_dir = vec3(0.15, 0.75, 0.40);

group_uniforms Clouds;
uniform sampler2D cloud_tex_01;
uniform sampler2D cloud_tex_02;
uniform vec4 cloud_color : source_color = vec4(0.96, 0.97, 1.0, 1.0);
uniform vec2 cloud_tiling = vec2(1.1);
uniform vec2 wind_speed = vec2(0.35, 0.12);
uniform float cloud_density : hint_range(0.0, 2.0, 0.01) = 0.38;
uniform float cloud_coverage : hint_range(0.0, 1.0, 0.01) = 0.28;
uniform float cloud_shape_exponent = 2.2;
uniform float cloud_occlude_exponent = 1.2;
uniform float storm : hint_range(0.0, 1.0, 0.01) = 0.0;
uniform float lightning : hint_range(0.0, 2.0, 0.01) = 0.0;
uniform vec3 lightning_dir = vec3(0.32, 0.62, -0.28);

group_uniforms Sun;
uniform float sun_scale = 0.028;
uniform float sun_strength = 8.0;
uniform vec4 sun_color : source_color = vec4(1.0, 0.94, 0.65, 1.0);

group_uniforms Moon;
uniform float moon_scale = 0.018;
uniform float moon_strength = 2.4;
uniform vec4 moon_color : source_color = vec4(0.85, 0.90, 1.0, 1.0);

float hash12(vec2 p) {
	vec3 p3 = fract(vec3(p.xyx) * 0.1031);
	p3 += dot(p3, p3.yzx + 33.33);
	return fract((p3.x + p3.y) * p3.z);
}

vec2 sky_uv(vec3 dir, float scale) {
	float y = max(dir.y + 0.18, 0.06);
	return (dir.xz / y) * scale;
}

float sample_clouds(vec3 dir) {
	if (dir.y < -0.02) {
		return 0.0;
	}
	vec2 wind = wind_speed * TIME * 0.012;
	vec2 uv1 = sky_uv(dir, cloud_tiling.x * 0.48) + wind;
	vec2 uv2 = sky_uv(dir, cloud_tiling.y * 0.86) + wind * 0.42 + vec2(17.2, 9.1);
	vec2 uv3 = sky_uv(dir, 0.20) + wind * 0.16 + vec2(3.7, 8.4);
	float n1 = texture(cloud_tex_01, uv1).r;
	float n2 = texture(cloud_tex_02, uv2).r;
	float n3 = texture(cloud_tex_01, uv3).r;
	float n = mix(n1, n1 * n2, 0.38);
	float cover = clamp(cloud_coverage, 0.0, 1.0);
	float dens = clamp(cloud_density, 0.05, 1.8);
	cover = clamp(cover + (dens - 0.38) * 0.18, 0.0, 1.0);
	float lo = mix(0.70, 0.08, cover);
	float hi = mix(0.94, 0.36, cover);
	float puffs = smoothstep(lo, hi, n);
	float blanket = smoothstep(mix(0.58, 0.16, cover), mix(0.88, 0.48, cover), n3);
	float c = max(puffs, blanket * smoothstep(0.28, 0.78, cover));
	c = mix(c, max(c, cover * (0.38 + n * 0.62)), smoothstep(0.42, 0.88, cover));
	c = pow(clamp(c, 0.0, 1.0), mix(cloud_shape_exponent, 1.05, cover));
	float horizon_fade = smoothstep(-0.02, 0.12, dir.y);
	return c * horizon_fade;
}

float starfield(vec3 dir) {
	if (dir.y < 0.04) {
		return 0.0;
	}
	vec3 p = dir * 280.0;
	vec3 ip = floor(p);
	vec3 fp = fract(p) - vec3(0.5);
	float h = hash12(ip.xy + vec2(ip.z * 37.17, ip.z * 13.91));
	if (h < 0.92) {
		return 0.0;
	}
	vec2 offset = (vec2(fract(h * 123.45), fract(h * 678.91)) - vec2(0.5)) * 0.5;
	float d = length(fp.xy - offset);
	float star_size = mix(0.08, 0.18, fract(h * 43.21));
	float star = smoothstep(star_size, 0.0, d);
	float twinkle = 0.65 + 0.35 * sin(TIME * (2.0 + h * 4.0) + h * 60.0);
	return star * twinkle * smoothstep(0.05, 0.22, dir.y);
}

void sky() {
	vec3 dir = normalize(EYEDIR);
	vec3 sun = normalize(sun_dir);
	if (use_directional_light) {
		sun = normalize(-LIGHT0_DIRECTION);
	}

	float sun_y = sun.y;
	float day_k = smoothstep(-0.08, 0.32, sun_y);
	float dusk_k = exp(-pow(sun_y * 4.5, 2.0));
	float night_k = 1.0 - day_k;

	vec3 zenith = mix(sky_night.rgb, sky_day.rgb, day_k);
	zenith = mix(zenith, sky_sunset.rgb, dusk_k * (1.0 - day_k * 0.35));
	vec3 horiz = mix(horizon_night.rgb, horizon_day.rgb, day_k);
	horiz = mix(horiz, horizon_sunset.rgb, dusk_k);

	float storm_k = clamp(storm, 0.0, 1.0);
	zenith = mix(zenith, vec3(0.10, 0.12, 0.16), storm_k);
	horiz = mix(horiz, vec3(0.20, 0.22, 0.26), storm_k);

	float hmix = pow(1.0 - clamp(dir.y, 0.0, 1.0), horizon_exponent);
	vec3 col = mix(zenith, horiz, hmix);

	float clouds = sample_clouds(dir);
	vec3 ccol = mix(cloud_color.rgb * 0.28, cloud_color.rgb, day_k);
	ccol = mix(ccol, cloud_color.rgb * vec3(0.55, 0.42, 0.38), dusk_k * 0.65);
	vec3 storm_cloud = vec3(0.13, 0.14, 0.17) + vec3(0.04, 0.045, 0.055) * clouds;
	ccol = mix(ccol, storm_cloud, storm_k);
	float cover_blend = mix(0.58, 0.97, clamp(cloud_coverage, 0.0, 1.0));
	col = mix(col, ccol, clouds * cover_blend);

	float occlude = pow(clouds, mix(cloud_occlude_exponent, 0.55, storm_k));
	float sun_mu = clamp(dot(dir, sun), 0.0, 1.0);
	float sun_core = smoothstep(1.0 - sun_scale, 1.0, sun_mu);
	float sun_halo = pow(sun_mu, 24.0) * 0.35 + pow(sun_mu, 6.0) * 0.06;
	float sun_vis = (1.0 - occlude) * smoothstep(-0.12, 0.02, sun_y);
	sun_vis *= (1.0 - storm_k * 0.96) * (1.0 - clamp(cloud_coverage, 0.0, 1.0) * 0.72);
	vec3 sun_add = sun_color.rgb * sun_strength * (sun_core * 1.3 + sun_halo) * sun_vis;

	vec3 moon = normalize(-sun);
	float moon_mu = clamp(dot(dir, moon), 0.0, 1.0);
	float moon_core = smoothstep(1.0 - moon_scale, 1.0, moon_mu);
	float moon_halo = pow(moon_mu, 32.0) * 0.25;
	float moon_vis = (1.0 - occlude) * night_k * smoothstep(-0.05, 0.15, moon.y) * (1.0 - storm_k * 0.85);
	vec3 moon_add = moon_color.rgb * moon_strength * (moon_core * 1.2 + moon_halo) * moon_vis;

	float stars = starfield(dir) * night_k * (1.0 - clouds) * 1.6;
	col += sun_add + moon_add + vec3(stars);

	float bolt = pow(max(dot(dir, normalize(lightning_dir)), 0.0), mix(64.0, 14.0, clamp(lightning, 0.0, 1.0)));
	float flash = clamp(lightning, 0.0, 2.0);
	col += vec3(0.70, 0.80, 1.0) * flash * (0.07 + clouds * 0.28 + bolt * 1.55);
	col = mix(col, vec3(0.62, 0.70, 0.88), flash * 0.18);

	if (dir.y < 0.0) {
		float g = pow(clamp(-dir.y, 0.0, 1.0), 0.65);
		vec3 ground = mix(horiz * 0.35, vec3(0.02, 0.03, 0.05), g);
		col = mix(col, ground, smoothstep(0.0, 0.08, -dir.y));
	}

	COLOR = col;
}
