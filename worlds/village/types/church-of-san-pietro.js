// San Pietro, Marinella's church: a nave of warm stone under a terracotta
// roof, its front striped in white marble and grey-green stone with a rose
// window over the arched door, pews and an altar inside, and a tall ochre
// campanile at its side with a bell in its open belfry and a tiled spire.
// Its door opens on the piazza (+z).
export const meta = {
  name: "church of san pietro",
  bounds: [9.0, 25.6, 9.5],
  tags: ["building", "landmark", "church"],
  props: { mass: 100000, burns: 0.1, light: 1 },
  sound: { hard: 1, dry: 0.4, ring: 0.6 },
  anchors: [
    { kind: "door", at: [0, 0, 9.2], face: 0, size: [2.0, 2.8] },
    { kind: "light", at: [0, 3.4, 9.6] },
    { kind: "light", at: [0, 2.0, -6.5] },
  ],
};

export function sdf(x, y, z, k) {
  const nave = box(x, y - 4.5, z, 5.0, 4.5, 9.0);
  const inner = box(x, y - 4.2, z, 4.6, 4.2, 8.6);
  const door = min(box(x, y - 1.4, z - 9.0, 1.0, 1.4, 0.7), max(length2(x, y - 2.8) - 1.0, abs(z - 9.0) - 0.7));
  const walls = max(nave, -min(inner, door));
  // The front rises into a pediment; the roof is pitched along the nave.
  const front = max(max(box(x, y - 6.2, z - 9.05, 5.3, 6.2, 0.35), (y - 12.4 + abs(x) * 0.62) * 0.85), -door);
  const roof = max(max(abs(x) - 5.5, abs(z) - 9.1), max((y - 11.9 + abs(x) * 0.6) * 0.85, 8.7 - y));
  const rose = max(length2(x, y - 7.2) - 1.15, abs(z - 9.45) - 0.12);
  const steps = box(x, y - 0.08, z - 9.6, 1.8, 0.08, 0.35);
  // Inside: pews either side of the aisle, the altar at the back.
  const pz = z - 1.3 * floor(z / 1.3 + 0.5);
  const pews = max(box(abs(x) - 2.4, y - 0.45, pz, 1.7, 0.45, 0.22), abs(z - 1.5) - 5.6);
  const altar = box(x, y - 0.55, z + 7.4, 1.5, 0.55, 0.6);
  // The campanile at its side.
  const cx = x - 7.1;
  const cz = z + 5.0;
  const shaft = box(cx, y - 10.5, cz, 1.6, 10.5, 1.6);
  const belfry = min(box(cx, y - 19.0, cz, 0.75, 1.1, 2.0), box(cx, y - 19.0, cz, 2.0, 1.1, 0.75));
  const tower = max(shaft, -belfry);
  const cornice = box(cx, y - 21.1, cz, 1.85, 0.14, 1.85);
  const spire = max((max(abs(cx), abs(cz)) - 1.7 + (y - 21.2) * 0.56) * 0.85, max(21.2 - y, y - 24.2));
  const bell = cappedCone(cx, y - 19.0, cz, 0.55, 0.62, 0.28);
  const link = box(x - 5.6, y - 3.0, z + 5.0, 0.7, 3.0, 1.2);
  // Crosses: on the front's peak, on the spire, and over the altar inside.
  const cross1 = min(box(x, y - 13.1, z - 9.05, 0.08, 0.75, 0.08), box(x, y - 13.4, z - 9.05, 0.38, 0.08, 0.08));
  const cross2 = min(box(cx, y - 24.75, cz, 0.06, 0.55, 0.06), box(cx, y - 24.95, cz, 0.28, 0.06, 0.06));
  const cross3 = min(box(x, y - 3.1, z + 8.55, 0.09, 0.85, 0.05), box(x, y - 3.45, z + 8.55, 0.5, 0.09, 0.05));
  const crosses = min(cross1, min(cross2, cross3));
  return min(min(min(min(walls, front), min(roof, rose)), min(min(steps, pews), min(altar, min(min(tower, cornice), min(spire, min(bell, link)))))), crosses);
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.4, y * 1.4, z * 1.4);
  // The crosses: wrought iron outside, gilded over the altar.
  if (y > 12.3 && abs(x) < 0.45 && abs(z - 9.05) < 0.12) return rgb(40, 40, 44);
  if (y > 24.15) return rgb(196, 160, 60);
  if (abs(z + 8.55) < 0.07 && abs(x) < 0.55 && y > 2.2 && y < 4.0) return glow(rgb(220, 180, 70), 0.3);
  const cx = x - 7.1;
  const cz = z + 5.0;
  if (x > 5.45 && abs(cz) < 1.9) {
    if (y > 21.15) return mix(rgb(178, 82, 46), rgb(204, 108, 64), n);
    if (y > 17.6 && y < 20.3 && max(abs(cx), abs(cz)) < 1.0) return rgb(196, 152, 60);
    if (abs(abs(cx) - 1.6) < 0.3 && abs(abs(cz) - 1.6) < 0.3) return rgb(240, 234, 220);
    if (abs(y - 17.6) < 0.15 || abs(y - 21.0) < 0.2) return rgb(240, 234, 220);
    return rgb(214, 160, 84) * (0.9 + 0.1 * n);
  }
  if (y > 8.7 && abs(z) < 9.15 && y > 11.9 - abs(x) * 0.6 - 0.4) {
    const tile = fract(x * 3.4) < 0.18 ? 0.82 : 1;
    return mix(rgb(176, 78, 44), rgb(206, 104, 60), n) * tile;
  }
  if (y < 0.18 && z > 9.2) return rgb(214, 206, 190);
  if (z > 8.65) {
    // The front: the rose window, the door's marble frame, the stripes.
    const r = length2(x, y - 7.2);
    if (r < 1.15) {
      if (r < 0.2 || fract(atan2(y - 7.2, x) * 1.91 + 0.5) < 0.12 || abs(r - 0.75) < 0.06) return rgb(236, 230, 214);
      return glow(hsv(fract(atan2(y - 7.2, x) * 0.32 + 0.6), 0.6, 0.65), 0.7);
    }
    if (r < 1.4) return rgb(244, 240, 230);
    if (abs(x) < 1.35 && y < 4.0 && length2(x, max(y - 2.8, 0)) > 1.0) return rgb(244, 240, 230);
    if (abs(x) < 1.0 && y < 3.8) return rgb(70, 46, 30);
    if (y > 12.4 - abs(x) * 0.62 - 0.3) return rgb(244, 240, 230);
    return fract(y * 0.9) < 0.5 ? rgb(238, 234, 222) : rgb(124, 138, 124);
  }
  // Tall arched windows down the nave's sides.
  if (abs(x) > 4.95 && y > 3.2 && y < 7.4) {
    const wz = z - 4.0 * floor(z / 4.0 + 0.5);
    if (abs(wz) < 0.5 && length2(wz, max(y - 6.9, 0)) < 0.5) return glow(mix(rgb(60, 70, 90), rgb(255, 200, 130), 0.3), 0.6);
  }
  if (abs(x) < 4.65 && abs(z) < 8.65 && y < 8.3) {
    if (y < 0.05) return rgb(196, 170, 140);
    if (y < 1.0 && abs(x) > 0.6 && abs(z + 7.4) > 0.7) return rgb(110, 72, 44);
    if (abs(z + 7.4) < 0.7 && abs(x) < 1.6) return y > 1.05 ? rgb(196, 160, 60) : rgb(236, 230, 214);
    return rgb(236, 226, 206);
  }
  if (abs(abs(x) - 5.0) < 0.25 && abs(abs(z) - 9.0) < 0.3) return rgb(240, 234, 220);
  return rgb(232, 214, 178) * (0.9 + 0.1 * n);
}
