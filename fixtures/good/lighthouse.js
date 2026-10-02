export const meta = {
  name: "lighthouse",
  bounds: [3, 14, 3],          // half-extents in metres, local space
  tags: ["building", "landmark"],
};

// Signed distance in local space. Scalars only.
export function sdf(x, y, z, k) {            // k = instance params {seed, scale, ...}
  const tower = cappedCone(x, y - 6, z, 6, 1.6, 1.0);
  const lamp  = sphere(x, y - 12.5, z, 1.1);
  return smoothUnion(tower, lamp, 0.3);
}

// Called once per hit pixel.
export function color(x, y, z, k) {
  if (y > 11.5) return glow(rgb(255, 230, 160));
  return (floor(y / 2) % 2 == 0) ? rgb(220, 60, 50) : rgb(240, 240, 235);
}
