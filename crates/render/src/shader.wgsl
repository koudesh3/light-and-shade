// p = ambient, strength, sheen, mode (0 lit, 1 normals, 2 original)
// q.x = 1 to shade in linear light, q.y = how strongly normals show over the lit painting
struct Globals { xf: vec4f, light: vec4f, p: vec4f, q: vec4f };
struct Quad { rect: vec4f };

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var smp: sampler;
@group(1) @binding(0) var<uniform> r: Quad;
@group(1) @binding(1) var ct: texture_2d<f32>;
@group(1) @binding(2) var nt: texture_2d<f32>;

struct VsOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };

@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
  var q = array<vec2f, 6>(vec2f(0, 0), vec2f(1, 0), vec2f(0, 1), vec2f(0, 1), vec2f(1, 0), vec2f(1, 1));
  let uv = q[i];
  let p = r.rect.xy + uv * r.rect.zw;
  var o: VsOut;
  o.pos = vec4f(p.x * g.xf.x + g.xf.z, p.y * g.xf.y + g.xf.w, 0.0, 1.0);
  o.uv = uv;
  return o;
}

fn to_linear(c: vec3f) -> vec3f {
  return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
}
fn to_srgb(c: vec3f) -> vec3f {
  return select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3f(0.0031308));
}

@fragment fn fs(v: VsOut) -> @location(0) vec4f {
  let c = textureSample(ct, smp, v.uv);
  let nr = textureSampleLevel(nt, smp, v.uv, 0.0).rgb;
  if (c.a < 0.002) { discard; }
  let albedo = c.rgb / c.a;
  var col = albedo;
  if (g.p.w < 1.5) {
    let n = normalize(nr * 2.0 - 1.0);
    let l = normalize(g.light.xyz);
    let d = max(dot(n, l), 0.0);
    let h = normalize(l + vec3f(0.0, 0.0, 1.0));
    let s = pow(max(dot(n, h), 0.0), 28.0) * g.p.z;
    let linear = g.q.x > 0.5;
    let base = select(albedo, to_linear(albedo), linear);
    let shaded = clamp(base * (g.p.x + g.p.y * d) + vec3f(s), vec3f(0.0), vec3f(1.0));
    col = select(shaded, to_srgb(shaded), linear);
    if (g.p.w > 0.5) {
      col = mix(col, nr, g.q.y);
    }
  }
  // Neighbouring layers both fade out along a shared edge. Tightening the fade stops
  // their two half-covered pixels from letting the paper through as a hairline.
  let a = min(c.a * 2.0, 1.0);
  return vec4f(clamp(col, vec3f(0.0), vec3f(1.0)) * a, a);
}
