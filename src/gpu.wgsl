// rust-ui GPU renderer: every primitive is an instanced quad shaded with
// signed distance functions (rounded rects, borders, analytic shadows) or
// sampled from an atlas (glyphs, rasterized paths, color emoji).

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

struct Instance {
    @location(0) rect: vec4<f32>,       // quad bounds (physical px)
    @location(1) shape: vec4<f32>,      // shape rect x, y, w, h
    @location(2) radii: vec4<f32>,      // tl, tr, br, bl
    @location(3) border: vec4<f32>,     // top, right, bottom, left widths
    @location(4) color: vec4<f32>,      // straight-alpha sRGB
    @location(5) color2: vec4<f32>,     // gradient end color
    @location(6) extra: vec4<f32>,      // gradient p0.xy p1.xy | shadow: occluding box
    @location(7) uv: vec4<f32>,         // atlas x, y | gradient stops t0, t1
    @location(8) clip: vec4<f32>,       // clip rect x, y, w, h
    @location(9) clip_radii: vec4<f32>,
    @location(10) params: vec4<f32>,    // kind, flags, sigma, _
    @location(11) extra_radii: vec4<f32>, // shadow: occluding box radii
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) rect: vec4<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) radii: vec4<f32>,
    @location(3) @interpolate(flat) border: vec4<f32>,
    @location(4) @interpolate(flat) color: vec4<f32>,
    @location(5) @interpolate(flat) color2: vec4<f32>,
    @location(6) @interpolate(flat) extra: vec4<f32>,
    @location(7) @interpolate(flat) uv: vec4<f32>,
    @location(8) @interpolate(flat) clip: vec4<f32>,
    @location(9) @interpolate(flat) clip_radii: vec4<f32>,
    @location(10) @interpolate(flat) params: vec4<f32>,
    @location(11) @interpolate(flat) extra_radii: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var mask_atlas: texture_2d<f32>;
@group(0) @binding(2) var color_atlas: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = inst.rect.xy + corner * inst.rect.zw;
    let ndc = vec2<f32>(p.x / globals.viewport.x * 2.0 - 1.0, 1.0 - p.y / globals.viewport.y * 2.0);
    var o: VOut;
    o.pos = vec4<f32>(ndc, 0.0, 1.0);
    o.rect = inst.rect;
    o.shape = inst.shape;
    o.radii = inst.radii;
    o.border = inst.border;
    o.color = inst.color;
    o.color2 = inst.color2;
    o.extra = inst.extra;
    o.uv = inst.uv;
    o.clip = inst.clip;
    o.clip_radii = inst.clip_radii;
    o.params = inst.params;
    o.extra_radii = inst.extra_radii;
    return o;
}

// Signed distance to a rounded rect with per-corner radii (y points down).
fn sd_rrect(p: vec2<f32>, r: vec4<f32>, radii: vec4<f32>) -> f32 {
    let half = r.zw * 0.5;
    let c = r.xy + half;
    let q0 = p - c;
    let top_r = select(radii.x, radii.y, q0.x > 0.0);     // tl / tr
    let bottom_r = select(radii.w, radii.z, q0.x > 0.0);  // bl / br
    var rr = select(top_r, bottom_r, q0.y > 0.0);
    rr = min(rr, min(half.x, half.y));
    let q = abs(q0) - half + vec2<f32>(rr, rr);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - rr;
}

fn coverage(d: f32) -> f32 {
    return clamp(0.5 - d, 0.0, 1.0);
}

// Abramowitz–Stegun style erf approximation (as used by Evan Wallace / GPUI).
fn erf2(v: vec2<f32>) -> vec2<f32> {
    let s = sign(v);
    let a = abs(v);
    let r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
    let r2 = r1 * r1;
    return s - s / (r2 * r2);
}

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (2.5066283 * sigma);
}

fn blur_along_x(x: f32, y: f32, sigma: f32, corner: f32, half: vec2<f32>) -> f32 {
    let delta = min(half.y - corner - abs(y), 0.0);
    let curved = half.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    let integral = 0.5 + 0.5 * erf2((vec2<f32>(x, x) + vec2<f32>(-curved, curved)) * (0.70710678 / sigma));
    return integral.y - integral.x;
}

// Analytic blurred rounded rect: integrates a Gaussian over the shape.
fn shadow_alpha(p: vec2<f32>, shape: vec4<f32>, corner: f32, sigma: f32) -> f32 {
    let half = shape.zw * 0.5;
    let c = shape.xy + half;
    let q = p - c;
    let low = q.y - half.y;
    let high = q.y + half.y;
    let start = clamp(-3.0 * sigma, low, high);
    let end = clamp(3.0 * sigma, low, high);
    let step = (end - start) / 4.0;
    var y = start + step * 0.5;
    var a = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        a = a + blur_along_x(q.x, q.y - y, sigma, min(corner, min(half.x, half.y)), half) * gaussian(y, sigma) * step;
        y = y + step;
    }
    return a;
}

// DirectWrite-style contrast + gamma correction of glyph coverage
// (mirrors `text::correct_alpha`).
fn correct_alpha(a: f32, color: vec3<f32>) -> f32 {
    let luma = dot(color, vec3<f32>(0.25, 0.5, 0.25));
    let k = clamp(luma * -4.0 + 3.0, 0.0, 1.0);
    let c = a * (k + 1.0) / (a * k + 1.0);
    let f = dot(color, vec3<f32>(0.30, 0.59, 0.11));
    let g = vec4<f32>(0.1469, -0.8911, 1.4644, -0.3234) / 4.0;
    return clamp(c + c * (1.0 - c) * ((g.x * f + g.y) * c + (g.z * f + g.w)), 0.0, 1.0);
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    let p = v.pos.xy;
    let kind = u32(v.params.x + 0.5);
    let flags = u32(v.params.y + 0.5);
    var col = v.color;
    var a = 0.0;
    if kind == 0u {
        // Filled rounded rect (solid or 2-stop linear gradient).
        a = coverage(sd_rrect(p, v.shape, v.radii));
        if (flags & 1u) != 0u {
            let d = v.extra.zw - v.extra.xy;
            let t = clamp(dot(p - v.extra.xy, d) / max(dot(d, d), 1e-5), 0.0, 1.0);
            let tt = clamp((t - v.uv.x) / max(v.uv.y - v.uv.x, 1e-5), 0.0, 1.0);
            col = mix(v.color, v.color2, tt);
        }
    } else if kind == 1u {
        // Border: outer shape minus inner shape (per-side widths).
        let outer = coverage(sd_rrect(p, v.shape, v.radii));
        let b = v.border;
        let inner_rect = vec4<f32>(v.shape.x + b.w, v.shape.y + b.x, v.shape.z - b.w - b.y, v.shape.w - b.x - b.z);
        let m = max(max(b.x, b.y), max(b.z, b.w));
        let inner_radii = max(v.radii - vec4<f32>(m, m, m, m), vec4<f32>(0.0));
        var inner = 0.0;
        if inner_rect.z > 0.0 && inner_rect.w > 0.0 {
            inner = coverage(sd_rrect(p, inner_rect, inner_radii));
        }
        a = outer * (1.0 - inner);
    } else if kind == 2u {
        // Box shadow; never painted beneath the occluding box (CSS semantics).
        let corner = max(max(v.radii.x, v.radii.y), max(v.radii.z, v.radii.w));
        let sigma = max(v.params.z, 0.1);
        a = shadow_alpha(p, v.shape, corner, sigma);
        let occ = coverage(sd_rrect(p, v.extra, v.extra_radii));
        a = a * (1.0 - occ);
    } else if kind == 3u {
        // Mask sprite (glyphs, paths).
        let texel = vec2<i32>(floor(p - v.rect.xy) + v.uv.xy);
        let m = textureLoad(mask_atlas, texel, 0).r;
        if (flags & 2u) != 0u {
            a = correct_alpha(m, v.color.rgb);
        } else {
            a = m;
        }
    } else {
        // Color sprite (emoji), premultiplied in the atlas.
        let texel = vec2<i32>(floor(p - v.rect.xy) + v.uv.xy);
        let c = textureLoad(color_atlas, texel, 0);
        let clip_a = coverage(sd_rrect(p, v.clip, v.clip_radii));
        return c * v.color.a * clip_a;
    }
    a = a * coverage(sd_rrect(p, v.clip, v.clip_radii));
    let alpha = a * col.a;
    return vec4<f32>(col.rgb * alpha, alpha);
}
