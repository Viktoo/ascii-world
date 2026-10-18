// A bottlenose dolphin, lying along +z with its belly near y = 0: a sleek
// grey torpedo, pale underneath, a short beak, a curved dorsal fin, two
// flippers and a wide fluke that beats up and down as it swims (k.f).
// Look: a = shade. Roles: s4 = lean (dive or rise), s5 = head (nod),
// s7 = spread (the fluke's beat, wider).
export const meta = { name: "dolphin", bounds: [0.85, 0.9, 1.55], tags: ["body"],
  props: { mass: 250, alive: 1, burns: 0.05, wet: 1 },
  body: { height: 0.62, eye: 0.38, radius: 0.45, reach: 0.5, grip: [0, 0.27, 1.45],
          roles: ["lean", "head", "spread"], gait: "slither", flies: false, arms: false,
          look: { shade: [0.3, 0.7] } } };

export function sdf(x, y, z, k) {
  const lean = clamp(k.s4, -0.3, 0.6);
  const p = rotX(x, y - 0.32, z, lean);
  const beat = (0.09 + clamp(k.s7, -1, 1) * 0.04) * sin(k.f - p.z * 2.2);
  const w = clamp(-p.z, 0, 1.4);
  const py = p.y - beat * w;
  // The head nods about its neck at z = 0.55.
  const nod = clamp(k.s5, -1, 1) * 0.25 * step(0.55, p.z);
  const h = rotX(p.x, py, p.z - 0.55, nod);
  const hy = h.y;
  const hz = h.z + 0.55;
  const front = ellipsoid(p.x, hy, hz - 0.15, 0.3, 0.29, 0.75);
  const back = cone(p.x, py, p.z, 0, 0, -1.2, 0, 0.01, 0.1, 0.07, 0.28);
  const body = smoothUnion(front, back, 0.2);
  const beak = capsule(p.x, hy, hz, 0, -0.03, 0.75, 0, -0.06, 1.12, 0.065);
  const melon = sphere(p.x, hy - 0.07, hz - 0.62, 0.2);
  const fin = triangle(p.x, py, p.z, 0, 0.22, 0.05, 0, 0.5, -0.32, 0, 0.2, -0.38, 0.025);
  const fl = triangle(abs(p.x), py, p.z, 0.2, -0.12, 0.35, 0.52, -0.24, 0.05, 0.22, -0.14, 0.12, 0.02);
  const sp = 0.38 + clamp(k.s7, -1, 1) * 0.06;
  const fluke = triangle(abs(p.x), py, p.z, 0, 0, -1.15, sp, 0.02, -1.5, 0.05, 0, -1.4, 0.022);
  return min(smoothUnion(smoothUnion(body, beak, 0.06), melon, 0.08), min(min(fin, fl), fluke));
}

export function color(x, y, z, k) {
  const v = 0.35 + clamp(k.a, 0, 1) * 0.3;
  if (y < 0.22 + 0.03 * sin(z * 3.0)) return rgb(214, 216, 220);
  return hsv(0.58, 0.08, v) * (0.92 + 0.08 * noise3(x * 4, y * 4, z * 4));
}

export function parts(x, y, z, k) {
  if (x < 0.5) return vec3(0, 0.26, 1.45);
  if (x < 1.5) return vec3(0, 0.26, 1.45);
  if (x < 2.5) return vec3(0, 0.36, 0.95);
  return vec3(0, 0.3, 0.2);
}
