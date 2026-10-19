// The gazebo on the green, the heart of the village: an eight-sided bandstand
// on a raised floor skirted with white lattice, three steps up at the front.
// Slender white posts carry arched gingerbread brackets and a frieze under a
// flared roof of red shingles; over it a little open lantern-cupola with its
// own roof, a gilded ball and a weathervane. Turned balusters run between the
// posts, benches round the inside, a lantern hangs from the ceiling where the
// fiddler plays. Its steps face +z.
export const meta = {
  name: "gazebo",
  bounds: [3.9, 6.6, 3.9],
  tags: ["building", "landmark", "shelter"],
  props: { mass: 8000, burns: 0.6, light: 1 },
  anchors: [
    { kind: "seat", at: [-2.4, 1.1, 0], face: 90 },
    { kind: "seat", at: [2.4, 1.1, 0], face: 270 },
    { kind: "seat", at: [0, 1.1, -2.4], face: 0 },
    { kind: "seat", at: [-1.7, 1.1, -1.7], face: 45 },
    { kind: "seat", at: [1.7, 1.1, -1.7], face: 315 },
    { kind: "light", at: [0, 2.9, 0] },
    { kind: "stairs", at: [0, 0, 3.75], to: [0, 0.6, 2.6], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  // Fold round the eight sides: u out through the middle of a side, v along it.
  const a = atan2(x, z);
  const sec = floor(a / 0.7854 + 0.5) * 0.7854;
  const u = x * sin(sec) + z * cos(sec);
  const v = x * cos(sec) - z * sin(sec);
  // …and round the eight corners, for the posts.
  const sc = (floor(a / 0.7854) + 0.5) * 0.7854;
  const uc = x * sin(sc) + z * cos(sc);
  const vc = x * cos(sc) - z * sin(sc);
  const R = 3.0;
  const front = z > 2.2 && abs(x) < 1.0;
  // The raised floor, its lattice skirt, the steps.
  const floor0 = max(u - R, abs(y - 0.3) - 0.3);
  const steps = min(max(max(abs(x) - 0.95, abs(z - 3.25) - 0.25), abs(y - 0.1) - 0.1), max(max(abs(x) - 0.95, abs(z - 3.05) - 0.25), abs(y - 0.3) - 0.1));
  const rails = max(max(abs(abs(x) - 0.98) - 0.03, abs(z - 3.2) - 0.35), abs(y - 0.9 + (z - 3.0) * 0.6) - 0.03);
  // Posts at the corners, a cap and a foot to each.
  const pr = R / 0.9239 - 0.12;
  const post = box(uc - pr, y - 2.1, vc, 0.07, 1.5, 0.07);
  const postcap = box(uc - pr, y - 3.48, vc, 0.11, 0.06, 0.11);
  const postfoot = box(uc - pr, y - 0.68, vc, 0.1, 0.08, 0.1);
  // Balusters between the posts, a top and a bottom rail; open at the front.
  const bv = v - 0.16 * floor(v / 0.16 + 0.5);
  const turn = 0.022 + 0.012 * sin((y - 0.6) * 9.0);
  const balusters = max(max(length2(u - R + 0.12, bv) - turn, abs(y - 1.0) - 0.38), abs(v) - 1.1);
  const toprail = box(u - R + 0.12, y - 1.42, v, 0.06, 0.035, 1.2);
  const botrail = box(u - R + 0.12, y - 0.66, v, 0.04, 0.03, 1.2);
  const railing = front ? 10.0 : min(min(balusters, toprail), botrail);
  // Arched gingerbread brackets and the frieze under the eaves.
  const arch = max(box(u - R + 0.12, y - 3.05, v, 0.025, 0.4, 1.18), -(length2(v, y - 2.35) - 1.12));
  const fretv = v - 0.24 * floor(v / 0.24 + 0.5);
  const fret = max(arch, -max(length2(fretv, y - 3.15) - 0.06, abs(u - R + 0.12) - 0.1));
  const frieze = box(u - R + 0.1, y - 3.55, v, 0.06, 0.11, 1.3);
  // The flared roof, a cupola on it with open arches, its own roof, the finial.
  const roof = max(max(u - 3.55 + (y - 3.62) * 1.25 - max(3.9 - y, 0.0) * 0.6, 3.62 - y), y - 5.2);
  const drum = max(max(u - 0.62, abs(y - 5.3) - 0.35), -max(max(u - 0.7, -u + 0.4), max(abs(v) - 0.16, abs(y - 5.32) - 0.2)));
  const croof = max(max(u - 0.85 + (y - 5.62) * 0.95, 5.62 - y), y - 6.25);
  const ball = sphere(x, y - 6.32, z, 0.09);
  const vane = min(capsule(x, y, z, 0, 6.3, 0, 0, 6.58, 0, 0.012), min(box(x - 0.02, y - 6.5, z, 0.3, 0.012, 0.012), triangle(x, y, z, 0.32, 6.5, 0, 0.22, 6.56, 0, 0.22, 6.44, 0, 0.008)));
  const ceiling = max(u - 3.0, abs(y - 3.62) - 0.02);
  const lantern = min(capsule(x, y, z, 0, 3.6, 0, 0, 3.0, 0, 0.01), sphere(x, y - 2.9, z, 0.13));
  const benches = max(box(u - 2.58, y - 1.05, v, 0.18, 0.035, 0.95), -box(x, y - 1.0, z - 2.6, 1.0, 0.5, 0.8));
  return min(min(min(floor0, steps), min(min(post, postcap), postfoot)), min(min(min(railing, rails), min(fret, frieze)), min(min(min(roof, drum), min(croof, ball)), min(min(vane, ceiling), min(lantern, benches)))));
}

export function color(x, y, z, k) {
  const a = atan2(x, z);
  const sec = floor(a / 0.7854 + 0.5) * 0.7854;
  const u = x * sin(sec) + z * cos(sec);
  const v = x * cos(sec) - z * sin(sec);
  const red = mix(rgb(160, 40, 34), rgb(190, 58, 44), noise3(x * 4, y * 4, z * 4));
  // The roofs: rows of shingles, each row's ends staggered.
  if ((y > 3.66 && y < 5.2 && u > 0.6) || (y > 5.64 && y < 6.25)) {
    const row = floor(y * 7.0);
    const sh = fract(v * 4.0 + row * 0.5) < 0.08 || fract(y * 7.0) < 0.12;
    return red * (sh ? 0.78 : 1.0) * (0.92 + 0.16 * hash(row, floor(v * 4.0 + row * 0.5)));
  }
  if (y > 6.25) return glow(rgb(220, 176, 60), 0.15);
  if (abs(y - 2.9) < 0.14 && length2(x, z) < 0.15) return glow(rgb(255, 214, 140), 0.8);
  // The lattice skirt under the floor.
  if (y < 0.56 && u > 2.95 && !(z > 2.2 && abs(x) < 1.0)) {
    const l = step(0.86, fract((v + y) * 5.0)) + step(0.86, fract((v - y) * 5.0));
    return l > 0.0 ? rgb(246, 244, 236) : rgb(60, 56, 50);
  }
  if (y < 0.61 && u < 2.98) return fract(v * 6.0) < 0.06 ? rgb(130, 110, 90) : mix(rgb(176, 156, 126), rgb(196, 176, 146), noise3(x * 3, y * 3, z * 9));
  if (y < 0.42 && z > 2.8) return rgb(176, 156, 126);
  if (abs(y - 3.62) < 0.03 && u < 2.98) return fract(v * 3.0) < 0.1 ? rgb(200, 190, 170) : rgb(236, 230, 214);
  if (abs(y - 1.05) < 0.05 && u > 2.3 && u < 2.8) return rgb(150, 110, 70);
  if (abs(y - 3.55) < 0.12 && u > 3.0) return rgb(60, 100, 70);
  return rgb(246, 244, 238) * (0.95 + 0.05 * noise3(x * 6, y * 6, z * 6));
}
