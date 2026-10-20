// Stencil-then-cover shaders. Vertex positions arrive already in NDC.

struct Paint {
    kind: u32,        // 0 solid, 1 linear, 2 radial
    space: u32,       // 0 sRGB interpolation, 1 linear (stop colours are pre-decoded)
    spread: u32,      // 0 pad, 1 repeat, 2 reflect
    count: u32,       // number of stops in use
    color: vec4<f32>, // solid colour (straight alpha)
    geom: vec4<f32>,  // linear: start.xy, end.xy; radial: centre.xy, radius
    inv: vec4<f32>,   // device-to-user matrix: a, b, c, d
    inv_ef: vec4<f32>,// device-to-user translation: e, f
    offsets: array<vec4<f32>, 16>,
    colors: array<vec4<f32>, 16>,
};

@group(0) @binding(0) var<uniform> paint: Paint;
// Clip layer (or a 1x1 white texture when there is no clip).
@group(0) @binding(1) var clip_tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@location(0) p: vec2<f32>) -> VsOut {
    var o: VsOut;
    o.pos = vec4<f32>(p, 0.0, 1.0);
    return o;
}

@fragment
fn fs_empty() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

fn texel(tex: texture_2d<f32>, frag: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(tex));
    let p = clamp(vec2<i32>(floor(frag)), vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(tex, p, 0).r;
}

// Clip layer computation: parent layer value wherever the path winds.
@fragment
fn fs_clip(@builtin(position) frag: vec4<f32>) -> @location(0) f32 {
    return texel(clip_tex, frag.xy);
}

fn linear_to_srgb_vec(c: vec3<f32>) -> vec3<f32> {
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    let lo = c * 12.92;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn spread_t(t: f32) -> f32 {
    if paint.spread == 1u {
        return t - floor(t);
    }
    if paint.spread == 2u {
        let m = t - 2.0 * floor(t * 0.5);
        return select(m, 2.0 - m, m > 1.0);
    }
    return clamp(t, 0.0, 1.0);
}

// Mirrors tpt_color_stub::interpolate, including its boundary rules.
fn gradient(t: f32) -> vec4<f32> {
    let n = paint.count;
    if n == 0u {
        return vec4<f32>(0.0);
    }
    var c = paint.colors[n - 1u];
    if t <= paint.offsets[0].x {
        c = paint.colors[0];
    } else if t >= paint.offsets[n - 1u].x {
        c = paint.colors[n - 1u];
    } else {
        for (var i = 0u; i + 1u < n; i = i + 1u) {
            let o0 = paint.offsets[i].x;
            let o1 = paint.offsets[i + 1u].x;
            if t >= o0 && t <= o1 {
                let span = o1 - o0;
                if span <= 0.0 {
                    c = paint.colors[i + 1u];
                } else {
                    c = mix(paint.colors[i], paint.colors[i + 1u], (t - o0) / span);
                }
                break;
            }
        }
    }
    if paint.space == 1u {
        return vec4<f32>(linear_to_srgb_vec(c.rgb), c.a);
    }
    return c;
}

@fragment
fn fs_fill(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    var c = paint.color;
    if paint.kind != 0u {
        let p = frag.xy;
        let u = vec2<f32>(
            paint.inv.x * p.x + paint.inv.z * p.y + paint.inv_ef.x,
            paint.inv.y * p.x + paint.inv.w * p.y + paint.inv_ef.y,
        );
        var t = 0.0;
        if paint.kind == 1u {
            let s = paint.geom.xy;
            let d = paint.geom.zw - s;
            let len2 = dot(d, d);
            if len2 > 0.0 {
                t = dot(u - s, d) / len2;
            }
        } else {
            if paint.geom.z > 0.0 {
                t = distance(u, paint.geom.xy) / paint.geom.z;
            }
        }
        c = gradient(spread_t(t));
    }
    let a = c.a * texel(clip_tex, frag.xy);
    return vec4<f32>(c.rgb * a, a);
}
