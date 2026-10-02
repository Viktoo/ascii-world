export const meta = { name: "folly", bounds: [4, 5, 4], tags: ["landmark"], note: "exercise every feature" };

const R = 2.5;
const H = R * 1.2;

export function sdf(x, y, z, k) {
  const q = rotY(x, y, z, 0.3 + k.seed);
  let d = roundBox(q.x, q.y + 3, q.z, R, 0.4, R, 0.1);
  for (let i = 0; i < 6; i++) {
    const a = i * PI / 3;
    const px = cos(a) * 2.0;
    const pz = Math.sin(a) * 2.0;
    d = union(d, capsule(q.x - px, q.y + 3, q.z - pz, H, 0.2));
  }
  const roof = cappedCone(q.x, q.y - 1.5, q.z, 0.8, 2.8, 0.2);
  d = smoothUnion(d, roof, 0.2);
  d = subtract(d, sphere(q.x, q.y - 1.0, q.z, 0.7));
  d = min(d, torus(q.x, q.y + 3.2, q.z, 3.0, 0.15), ellipsoid(q.x, q.y - 2.6, q.z, 0.3, 0.6, 0.3));
  if (d > 10 || !(d < 100)) {
    d = 10;
  }
  return d + noise3(x * 4, y * 4, z * 4) * 0.01;
}

export function color(x, y, z, k) {
  const stone = mix(rgb(200, 190, 170), rgb(150, 140, 130), noise3(x, y, z));
  const t = clamp(y / 5, 0, 1);
  if (y > 0.6) {
    return hsv(0.02 + hash(k.seed) * 0.05, 0.6, 0.7);
  } else if (abs(y + 3) < 0.5) {
    return stone * 0.8;
  }
  return mix(stone, rgb(90, 120, 80), t * 0.2);
}
