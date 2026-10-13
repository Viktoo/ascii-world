// The window compositor: the world scaled up to fill the view, the screen's
// cells over it (backgrounds and block/box shapes drawn exactly), then text.

struct U {
    screen: vec2<f32>,
    cell: vec2<f32>,
    grid: vec2<u32>,
    view_rows: u32,
    has_world: u32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var world: texture_2d<f32>;
@group(0) @binding(2) var smooth_s: sampler;
// Per cell: background (a = flags: 1 shows the world beneath), foreground (a = shape).
@group(0) @binding(3) var cell_bg: texture_2d<f32>;
@group(0) @binding(4) var cell_fg: texture_2d<f32>;
@group(0) @binding(5) var atlas: texture_2d<f32>;

struct Full {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> Full {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Full;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

fn box_line(l: vec2<f32>, up: bool, down: bool, left: bool, right: bool, t: f32) -> f32 {
    let cx = abs(l.x - 0.5) * u.cell.x;
    let cy = abs(l.y - 0.5) * u.cell.y;
    var c = 0.0;
    if (cx <= t && ((up && l.y <= 0.5) || (down && l.y >= 0.5))) { c = 1.0; }
    if (cy <= t && ((left && l.x <= 0.5) || (right && l.x >= 0.5))) { c = 1.0; }
    return c;
}

fn double_line(l: vec2<f32>, horiz: bool) -> f32 {
    let t = max(1.0, u.cell.y * 0.05);
    let d = max(2.0, u.cell.y * 0.12);
    var o: f32;
    if (horiz) { o = abs(abs(l.y - 0.5) * u.cell.y - d); } else { o = abs(abs(l.x - 0.5) * u.cell.x - d * 0.6); }
    return select(0.0, 1.0, o <= t * 0.5 + 0.25);
}

// How much of the cell's foreground a shape covers at `l` (0..1 within the cell).
fn shape_cover(s: u32, l: vec2<f32>) -> f32 {
    if (s == 0u) { return 0.0; }
    if (s < 16u) {
        let bit = select(0u, 1u, l.x >= 0.5) + select(0u, 2u, l.y >= 0.5);
        return select(0.0, 1.0, ((s >> bit) & 1u) == 1u);
    }
    let thin = max(0.5, u.cell.y * 0.035);
    let heavy = max(1.0, u.cell.y * 0.07);
    switch s {
        case 16u: { return 0.25; }
        case 17u: { return 0.5; }
        case 18u: { return 0.75; }
        case 19u: { return box_line(l, false, false, true, true, thin); }
        case 20u: { return box_line(l, true, true, false, false, thin); }
        case 21u: { return box_line(l, false, true, false, true, thin); }
        case 22u: { return box_line(l, false, true, true, false, thin); }
        case 23u: { return box_line(l, true, false, false, true, thin); }
        case 24u: { return box_line(l, true, false, true, false, thin); }
        case 25u: { return box_line(l, true, true, false, true, thin); }
        case 26u: { return box_line(l, true, true, true, false, thin); }
        case 27u: { return box_line(l, false, true, true, true, thin); }
        case 28u: { return box_line(l, true, false, true, true, thin); }
        case 29u: { return box_line(l, true, true, true, true, thin); }
        case 30u: { return box_line(l, false, false, true, true, heavy); }
        case 31u: { return double_line(l, true); }
        case 32u: { return double_line(l, false); }
        case 33u: { return max(double_line(l, true) * select(0.0, 1.0, l.x >= 0.4), double_line(l, false) * select(0.0, 1.0, l.y >= 0.4)); }
        case 34u: { return max(double_line(l, true) * select(0.0, 1.0, l.x <= 0.6), double_line(l, false) * select(0.0, 1.0, l.y >= 0.4)); }
        case 35u: { return max(double_line(l, true) * select(0.0, 1.0, l.x >= 0.4), double_line(l, false) * select(0.0, 1.0, l.y <= 0.6)); }
        case 36u: { return max(double_line(l, true) * select(0.0, 1.0, l.x <= 0.6), double_line(l, false) * select(0.0, 1.0, l.y <= 0.6)); }
        default: {}
    }
    // Lower eighths ▁..▇ (40..46) and left eighths ▉..▏ (47..53).
    if (s >= 40u && s <= 46u) {
        return select(0.0, 1.0, 1.0 - l.y <= f32(s - 39u) / 8.0);
    }
    if (s >= 47u && s <= 53u) {
        return select(0.0, 1.0, l.x <= f32(54u - s) / 8.0);
    }
    return 0.0;
}

// Catmull-Rom upscaling in nine bilinear taps: sharper than bilinear alone.
fn world_at(uv: vec2<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(world));
    let sp = uv * size;
    let tc = floor(sp - 0.5) + 0.5;
    let f = sp - tc;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let t0 = (tc - 1.0) / size;
    let t3 = (tc + 2.0) / size;
    let t12 = (tc + w2 / w12) / size;
    var c = vec3<f32>(0.0);
    c += textureSampleLevel(world, smooth_s, vec2(t0.x, t0.y), 0.0).rgb * w0.x * w0.y;
    c += textureSampleLevel(world, smooth_s, vec2(t12.x, t0.y), 0.0).rgb * w12.x * w0.y;
    c += textureSampleLevel(world, smooth_s, vec2(t3.x, t0.y), 0.0).rgb * w3.x * w0.y;
    c += textureSampleLevel(world, smooth_s, vec2(t0.x, t12.y), 0.0).rgb * w0.x * w12.y;
    c += textureSampleLevel(world, smooth_s, vec2(t12.x, t12.y), 0.0).rgb * w12.x * w12.y;
    c += textureSampleLevel(world, smooth_s, vec2(t3.x, t12.y), 0.0).rgb * w3.x * w12.y;
    c += textureSampleLevel(world, smooth_s, vec2(t0.x, t3.y), 0.0).rgb * w0.x * w3.y;
    c += textureSampleLevel(world, smooth_s, vec2(t12.x, t3.y), 0.0).rgb * w12.x * w3.y;
    c += textureSampleLevel(world, smooth_s, vec2(t3.x, t3.y), 0.0).rgb * w3.x * w3.y;
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// Edges smoothed along their direction (FXAA, measured in world texels), so a
// small frame scaled up shows soft slopes rather than stairs.
fn world_smooth(uv: vec2<f32>) -> vec3<f32> {
    let px = 1.0 / vec2<f32>(textureDimensions(world));
    let m = world_at(uv);
    let nw = luma(textureSampleLevel(world, smooth_s, uv + vec2(-1.0, -1.0) * px, 0.0).rgb);
    let ne = luma(textureSampleLevel(world, smooth_s, uv + vec2(1.0, -1.0) * px, 0.0).rgb);
    let sw = luma(textureSampleLevel(world, smooth_s, uv + vec2(-1.0, 1.0) * px, 0.0).rgb);
    let se = luma(textureSampleLevel(world, smooth_s, uv + vec2(1.0, 1.0) * px, 0.0).rgb);
    let lm = luma(m);
    let lo = min(lm, min(min(nw, ne), min(sw, se)));
    let hi = max(lm, max(max(nw, ne), max(sw, se)));
    if (hi - lo < max(0.04, hi * 0.12)) {
        return m;
    }
    var dir = vec2<f32>(-((nw + ne) - (sw + se)), (nw + sw) - (ne + se));
    let reduce = max((nw + ne + sw + se) * 0.25 * (1.0 / 8.0), 1.0 / 128.0);
    let k = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
    dir = clamp(dir * k, vec2<f32>(-6.0), vec2<f32>(6.0)) * px;
    let a = 0.5 * (textureSampleLevel(world, smooth_s, uv + dir * (1.0 / 3.0 - 0.5), 0.0).rgb + textureSampleLevel(world, smooth_s, uv + dir * (2.0 / 3.0 - 0.5), 0.0).rgb);
    let b = a * 0.5 + 0.25 * (textureSampleLevel(world, smooth_s, uv - dir * 0.5, 0.0).rgb + textureSampleLevel(world, smooth_s, uv + dir * 0.5, 0.0).rgb);
    let lb = luma(b);
    if (lb < lo || lb > hi) {
        return a;
    }
    return b;
}

@fragment
fn fs_grid(f: Full) -> @location(0) vec4<f32> {
    let gp = f.pos.xy / u.cell;
    let c = min(vec2<u32>(gp), u.grid - vec2<u32>(1u, 1u));
    let l = clamp(gp - vec2<f32>(c), vec2<f32>(0.0), vec2<f32>(1.0));
    let b = textureLoad(cell_bg, c, 0);
    let fg = textureLoad(cell_fg, c, 0);
    let flags = u32(round(b.a * 255.0));
    let shape = u32(round(fg.a * 255.0));
    var col = b.rgb;
    if ((flags & 1u) != 0u) {
        let view = vec2<f32>(f32(u.grid.x) * u.cell.x, f32(u.view_rows) * u.cell.y);
        var w = vec3<f32>(0.0);
        if (u.has_world == 1u) {
            w = world_smooth(f.pos.xy / view);
        }
        col = w * b.rgb;
    }
    col = mix(col, fg.rgb, shape_cover(shape, l));
    return vec4<f32>(col, 1.0);
}

struct Glyph {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct GlyphOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_text(@builtin(vertex_index) i: u32, g: Glyph) -> GlyphOut {
    var corners = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));
    let k = corners[i];
    let px = g.rect.xy + k * g.rect.zw;
    var o: GlyphOut;
    o.pos = vec4<f32>(px.x / u.screen.x * 2.0 - 1.0, 1.0 - px.y / u.screen.y * 2.0, 0.0, 1.0);
    o.uv = mix(g.uv.xy, g.uv.zw, k);
    o.color = g.color;
    return o;
}

@fragment
fn fs_text(g: GlyphOut) -> @location(0) vec4<f32> {
    let a = textureLoad(atlas, vec2<i32>(g.uv), 0).r;
    return vec4<f32>(g.color.rgb, g.color.a * a);
}
