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
    pat: vec4<f32>,    // pattern: image-from-user matrix a, b, c, d
    pat_ef: vec4<f32>, // pattern: e, f, image width, image height
    crop: vec4<f32>,   // drawn image: source x, y, width, height (width 0 means no crop)
};

@group(0) @binding(0) var<uniform> paint: Paint;
// Clip layer (or a 1x1 white texture when there is no clip).
@group(0) @binding(1) var clip_tex: texture_2d<f32>;
// The pattern's image, as premultiplied floats (a 1x1 stand-in when no pattern is used).
@group(0) @binding(2) var pat_tex: texture_2d<f32>;

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

// The pattern's colour at this pixel, straight alpha. The image repeats in its own pixels.
fn pattern_sample(frag: vec2<f32>) -> vec4<f32> {
    let u = vec2<f32>(
        paint.inv.x * frag.x + paint.inv.z * frag.y + paint.inv_ef.x,
        paint.inv.y * frag.x + paint.inv.w * frag.y + paint.inv_ef.y,
    );
    let size = paint.pat_ef.zw;
    let q = vec2<f32>(
        paint.pat.x * u.x + paint.pat.z * u.y + paint.pat_ef.x,
        paint.pat.y * u.x + paint.pat.w * u.y + paint.pat_ef.y,
    );
    let cropped = paint.crop.z > 0.0;
    if cropped {
        // A drawn image: nothing outside its source rectangle, and no repeat.
        if q.x < paint.crop.x || q.x >= paint.crop.x + paint.crop.z
            || q.y < paint.crop.y || q.y >= paint.crop.y + paint.crop.w {
            return vec4<f32>(0.0);
        }
    }
    // Pattern: the image repeats. Drawn image: the sample is the pixel the point falls in.
    var at = floor(q - floor(q / size) * size);
    if cropped {
        at = floor(q);
    }
    let dims = vec2<i32>(textureDimensions(pat_tex));
    let p = textureLoad(pat_tex, clamp(vec2<i32>(at), vec2<i32>(0), dims - vec2<i32>(1)), 0);
    if p.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(p.rgb / p.a, p.a);
}

// Straight colour of the paint at this pixel, with the clip's coverage in its alpha.
fn paint_color(frag: vec2<f32>) -> vec4<f32> {
    var c = paint.color;
    if paint.kind == 3u {
        c = pattern_sample(frag);
    } else if paint.kind != 0u {
        let u = vec2<f32>(
            paint.inv.x * frag.x + paint.inv.z * frag.y + paint.inv_ef.x,
            paint.inv.y * frag.x + paint.inv.w * frag.y + paint.inv_ef.y,
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
    return vec4<f32>(c.rgb, c.a * texel(clip_tex, frag));
}

@fragment
fn fs_fill(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let c = paint_color(frag.xy);
    return vec4<f32>(c.rgb * c.a, c.a);
}

// Blended fills. The canvas under the fill is a copy, and the blend mode is an index.
@group(0) @binding(5) var dst_tex: texture_2d<f32>;
@group(0) @binding(3) var<uniform> blend_mode: vec4<u32>;

fn luminosity(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}

// Pulls an out-of-range colour back into [0, 1] while keeping its luminosity (ClipColor).
fn clip_color(c: vec3<f32>) -> vec3<f32> {
    let l = luminosity(c);
    let n = min(min(c.r, c.g), c.b);
    let x = max(max(c.r, c.g), c.b);
    var o = c;
    if n < 0.0 && l - n > 0.0 {
        o = vec3<f32>(l) + (o - vec3<f32>(l)) * l / (l - n);
    }
    if x > 1.0 && x - l > 0.0 {
        o = vec3<f32>(l) + (o - vec3<f32>(l)) * (1.0 - l) / (x - l);
    }
    return o;
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    return clip_color(c + vec3<f32>(l - luminosity(c)));
}

fn saturation(c: vec3<f32>) -> f32 {
    return max(max(c.r, c.g), c.b) - min(min(c.r, c.g), c.b);
}

// Rescales the colour to spread `s`, keeping the order of its channels (SetSat).
fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let cmax = max(max(c.r, c.g), c.b);
    let cmin = min(min(c.r, c.g), c.b);
    if cmax > cmin {
        return (c - vec3<f32>(cmin)) * s / (cmax - cmin);
    }
    return vec3<f32>(0.0);
}

// The separable modes, one channel at a time. Modes 1 to 7 in `BlendMode` order.
fn separable(mode: u32, b: f32, s: f32) -> f32 {
    if mode == 1u {
        return b * s;
    }
    if mode == 2u {
        return b + s - b * s;
    }
    if mode == 3u {
        // Overlay is hard light with the roles of backdrop and source swapped.
        if b <= 0.5 {
            return s * 2.0 * b;
        }
        let t = 2.0 * b - 1.0;
        return s + t - s * t;
    }
    if mode == 4u {
        return min(b, s);
    }
    if mode == 5u {
        return max(b, s);
    }
    if mode == 6u {
        return abs(b - s);
    }
    return b + s - 2.0 * b * s;
}

// B(backdrop, source) of the W3C spec, on unpremultiplied colours.
fn blend_rgb(mode: u32, cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    if mode == 8u {
        return set_lum(set_sat(cs, saturation(cb)), luminosity(cb));
    }
    if mode == 9u {
        return set_lum(set_sat(cb, saturation(cs)), luminosity(cb));
    }
    if mode == 10u {
        return set_lum(cs, luminosity(cb));
    }
    if mode == 11u {
        return set_lum(cb, luminosity(cs));
    }
    return vec3<f32>(
        separable(mode, cb.r, cs.r),
        separable(mode, cb.g, cs.g),
        separable(mode, cb.b, cs.b),
    );
}

// Premultiplied source-over with a blend: Cr = (1 - ab) Cs + (1 - as) Cb + as ab B(cb, cs).
// Only used for non-normal modes, so the blend pipeline replaces the target outright.
// Blends one straight-colour source pixel `c` over the canvas copy at `at`.
fn blend_over(at: vec2<f32>, c: vec4<f32>) -> vec4<f32> {
    let as_ = c.a;
    let d = textureLoad(dst_tex, vec2<i32>(floor(at)), 0);
    let ab = d.a;
    var cb = vec3<f32>(0.0);
    if ab > 0.0 {
        cb = d.rgb / ab;
    }
    let b = blend_rgb(blend_mode.x, cb, c.rgb);
    let out_rgb = (1.0 - ab) * (c.rgb * as_) + (1.0 - as_) * d.rgb + as_ * ab * b;
    return vec4<f32>(out_rgb, as_ + ab * (1.0 - as_));
}

@fragment
fn fs_fill_blend(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return blend_over(frag.xy, paint_color(frag.xy));
}

// Groups: a finished layer is composited onto what is below, scaled by its alpha. The output is
// premultiplied, and the pipeline blends it source-over.
@group(0) @binding(6) var group_tex: texture_2d<f32>;
@group(0) @binding(7) var<uniform> group_alpha: vec4<f32>;

@fragment
fn fs_composite(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = textureLoad(group_tex, vec2<i32>(floor(frag.xy)), 0);
    return p * group_alpha.x;
}

// Blurred shadows. A blur pass reads `group_tex` (the mask, or the scratch copy of it) and
// averages `2r + 1` pixels along `blur_p.yz`, with zero outside the canvas. `blur_p` is
// (r, dx, dy, unused).
@group(0) @binding(8) var<uniform> blur_p: vec4<i32>;

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(frag.xy));
    let dims = vec2<i32>(textureDimensions(group_tex));
    let r = blur_p.x;
    let d = blur_p.yz;
    var sum = vec4<f32>(0.0);
    for (var k = -r; k <= r; k = k + 1) {
        let q = p + d * k;
        if q.x >= 0 && q.y >= 0 && q.x < dims.x && q.y < dims.y {
            sum = sum + textureLoad(group_tex, q, 0);
        }
    }
    return sum / f32(2 * r + 1);
}

// The paint, with its alpha scaled by the blurred mask at this pixel (the mask's alpha).
fn masked_paint(at: vec2<f32>) -> vec4<f32> {
    var c = paint_color(at);
    c.a = c.a * textureLoad(group_tex, vec2<i32>(floor(at)), 0).a;
    return c;
}

@fragment
fn fs_mask_fill(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let c = masked_paint(frag.xy);
    return vec4<f32>(c.rgb * c.a, c.a);
}

@fragment
fn fs_mask_fill_blend(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return blend_over(frag.xy, masked_paint(frag.xy));
}
