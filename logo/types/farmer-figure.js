// An orchardist, 1.8 m tall, standing easy: a woven straw hat with a dark
// band, a red-and-black plaid flannel shirt with its sleeves rolled to the
// elbow, a leather belt, faded blue jeans, brown work boots. His left hand
// holds a wicker basket of red apples by its handle; his right hangs loose.
// Faces +z, stands on y = 0.
export const meta = { name: "farmer figure", bounds: [0.6, 2.0, 0.6], tags: ["figure", "body"],
  body: { height: 1.8, eye: 1.68, radius: 0.35, reach: 2.2, grip: [0.3, 1.0, 0.3],
          roles: ["head"], gait: "biped", arms: false,
          look: { height: [1.5, 2.0] } } };

export function sdf(x, y, z0, k) {
  let z = z0;
  const far = length3(x * 0.8, (y - 0.95) * 0.5, z) - 0.5;
  if (far > 0.3) return far;
  const ax = abs(x);
  // Legs in jeans: thigh to knee to ankle, a touch apart.
  const thigh = capsule(ax, y, z, 0.095, 0.93, 0.0, 0.1, 0.5, 0.02, 0.072);
  const calf = capsule(ax, y, z, 0.1, 0.5, 0.02, 0.105, 0.11, -0.005, 0.054);
  // Jeans flare a little over the boot.
  const cuff = cappedCone(ax - 0.105, y - 0.17, z + 0.0, 0.07, 0.062, 0.055);
  let d = smoothUnion(thigh, min(calf, cuff), 0.05);
  // Work boots: a rounded toe, a heel, a thick sole.
  const boot = smoothUnion(roundBox(ax - 0.108, y - 0.06, z - 0.045, 0.048, 0.055, 0.12, 0.035), capsule(ax, y, z, 0.106, 0.06, -0.01, 0.106, 0.16, -0.01, 0.058), 0.03);
  d = min(d, min(boot, roundBox(ax - 0.108, y - 0.012, z - 0.045, 0.054, 0.012, 0.13, 0.008)));
  // Seat and hips, then the waist and the chest, the shirt loose over them.
  const hips = ellipsoid(x, y - 0.96, z + 0.005, 0.165, 0.11, 0.105);
  const waist = ellipsoid(x, y - 1.1, z, 0.152, 0.14, 0.1);
  const chest = ellipsoid(x, y - 1.31, z - 0.005, 0.175, 0.16, 0.112);
  let torso = smoothUnion(smoothUnion(hips, waist, 0.08), chest, 0.1);
  // Shoulders slope from the neck to the arm.
  torso = smoothUnion(torso, capsule(ax, y, z, 0.05, 1.495, -0.01, 0.185, 1.445, -0.01, 0.06), 0.06);
  // A belt standing just proud of the jeans.
  torso = min(torso, max(torso - 0.008, abs(y - 1.0) - 0.022));
  // Collar points either side of the open neck.
  const collar = triangle(ax, y, z, 0.035, 1.5, 0.07, 0.085, 1.47, 0.085, 0.07, 1.42, 0.1, 0.006);
  torso = min(torso, collar);
  d = smoothUnion(d, torso, 0.04);
  // Neck and head: skull, jaw, ears, nose, brow. The head nods.
  const nod = clamp(k.s5, -1, 1) * 0.04 * smoothstep(1.45, 1.6, y);
  z = z - nod;
  const neck = capsule(x, y, z, 0, 1.45, -0.005, 0, 1.6, 0.0, 0.056);
  const skull = ellipsoid(x, y - 1.69, z + 0.005, 0.078, 0.1, 0.097);
  const jaw = ellipsoid(x, y - 1.615, z - 0.022, 0.066, 0.058, 0.07);
  let head = smoothUnion(skull, jaw, 0.04);
  head = smoothUnion(head, ellipsoid(ax - 0.078, y - 1.675, z + 0.005, 0.013, 0.03, 0.02), 0.01);
  const nose = capsule(x, y, z, 0, 1.69, 0.088, 0, 1.652, 0.108, 0.013);
  head = smoothUnion(head, nose, 0.015);
  head = smoothUnion(head, capsule(ax, y, z, 0.012, 1.708, 0.085, 0.05, 1.71, 0.074, 0.011), 0.012);
  // Eye sockets set under the brow.
  head = smoothSubtract(head, ellipsoid(ax - 0.032, y - 1.688, z - 0.093, 0.018, 0.009, 0.012), 0.006);
  d = smoothUnion(d, smoothUnion(neck, head, 0.03), 0.02);
  // Hat: a crown with a dent in the top, a wide brim curling up at the sides.
  const crown = roundBox(x, y - 1.82, z + 0.005, 0.088, 0.06, 0.1, 0.035);
  const dent = ellipsoid(x, y - 1.885, z + 0.005, 0.03, 0.02, 0.07);
  const r = length2(x, z + 0.005);
  const brim = max(abs(y - 1.77 - r * r * 0.25 + 0.012 * smoothstep(0.1, 0.22, r)) - 0.006, r - 0.25);
  d = min(d, max(min(crown, brim), -dent));
  // Arms: upper arm in the sleeve, rolled cuff at the elbow, bare forearm.
  // Right (−x) hangs easy, a little bent; left (+x) holds the basket.
  const rUp = capsule(x, y, z, -0.19, 1.43, -0.01, -0.215, 1.15, -0.03, 0.05);
  const rFo = capsule(x, y, z, -0.215, 1.15, -0.03, -0.235, 0.89, 0.02, 0.038);
  const lUp = capsule(x, y, z, 0.19, 1.43, -0.01, 0.225, 1.15, -0.02, 0.05);
  const lFo = capsule(x, y, z, 0.225, 1.15, -0.02, 0.255, 0.9, 0.03, 0.038);
  const rRoll = capsule(x, y, z, -0.211, 1.19, -0.027, -0.217, 1.14, -0.027, 0.054);
  const lRoll = capsule(x, y, z, 0.221, 1.19, -0.018, 0.227, 1.14, -0.018, 0.054);
  let arms = min(min(rUp, rFo), min(lUp, lFo));
  arms = min(arms, min(rRoll, lRoll));
  // Hands: a palm and curled fingers; the left closed round the handle.
  const rHand = smoothUnion(ellipsoid(x + 0.238, y - 0.83, z - 0.03, 0.026, 0.05, 0.042), capsule(x, y, z, -0.228, 0.86, 0.06, -0.232, 0.81, 0.06, 0.014), 0.015);
  const lHand = smoothUnion(ellipsoid(x - 0.258, y - 0.85, z - 0.04, 0.03, 0.045, 0.045), ellipsoid(x - 0.258, y - 0.825, z - 0.06, 0.035, 0.03, 0.035), 0.015);
  arms = smoothUnion(arms, min(rHand, lHand), 0.02);
  d = smoothUnion(d, arms, 0.03);
  // The basket: a round wicker body hung from an arched handle, apples heaped in it.
  const bx = x - 0.29;
  const by = y - 0.5;
  const bz = z - 0.05;
  const basket = max(cappedCone(bx, by, bz, 0.12, 0.14, 0.175), -cappedCone(bx, by - 0.02, bz, 0.12, 0.125, 0.162));
  const rim = torus(bx, by - 0.12, bz, 0.172, 0.014);
  const handle = max(torus(rotX(bx, by - 0.12, bz, 1.5708).x, rotX(bx, by - 0.12, bz, 1.5708).y, rotX(bx, by - 0.12, bz, 1.5708).z, 0.17, 0.013), -by + 0.1);
  let b = min(min(basket, rim), handle);
  for (let i = 0; i < 7; i++) {
    const a = i * 2.39996;
    const rr = i == 0 ? 0.0 : 0.09;
    b = min(b, sphere(bx - cos(a) * rr, by - 0.1 - (i == 0 ? 0.045 : 0.0), bz - sin(a) * rr, 0.052));
  }
  return min(d, b);
}

export function color(x, y, z, k) {
  const ax = abs(x);
  const n = noise3(x * 40, y * 40, z * 40);
  const skin = rgb(206, 150, 116);
  // Basket and apples.
  const bx = x - 0.29;
  const by = y - 0.5;
  const bz = z - 0.05;
  if (length3(bx, by - 0.08, bz) < 0.27 && x > 0.12 && y < 0.8) {
    if (by > 0.1 && by < 0.21 && length2(bx, bz) < 0.15) {
      // An apple: red over yellow-green, a pale shine on the top.
      const s = noise3(x * 25, y * 25, z * 25);
      return mix(rgb(150, 18, 20), rgb(214, 54, 34), s) * (0.85 + 0.25 * step(0.72, noise3(x * 60, y * 60, z * 60)));
    }
    const ang = atan2(bz, bx);
    const weave = step(0.5, fract(ang * 9.0 + step(0.5, fract(by * 30.0)) * 0.5));
    return mix(rgb(150, 104, 52), rgb(196, 150, 86), weave * 0.7 + n * 0.3);
  }
  // Straw hat: a crossing weave, a dark band round the crown.
  if (y > 1.755 && length2(x, z + 0.005) < 0.26) {
    if (y > 1.77 && y < 1.8 && length2(x, z + 0.005) < 0.11) return rgb(58, 38, 28);
    const r = length2(x, z + 0.005);
    const w = step(0.5, fract(r * 70.0 + step(0.5, fract(atan2(z, x) * 30.0)) * 0.5));
    return mix(rgb(196, 160, 96), rgb(232, 204, 140), w * 0.6 + n * 0.4);
  }
  // Head and neck: skin, hair showing under the hat, stubble, eyes, brows.
  if ((y > 1.56 && ax < 0.11 && z > -0.12) || (y > 1.44 && length2(x, z) < 0.062)) {
    if (y > 1.71 && z < 0.06) return mix(rgb(96, 64, 40), rgb(120, 84, 52), n);
    if (y > 1.74 && z < 0.085) return mix(rgb(96, 64, 40), rgb(120, 84, 52), n);
    if (z > 0.06 && abs(ax - 0.032) < 0.02 && abs(y - 1.704) < 0.005) return rgb(92, 62, 40);
    if (z > 0.07 && abs(ax - 0.032) < 0.008 && abs(y - 1.688) < 0.006) return ax < 0.03 || ax > 0.036 ? rgb(236, 230, 220) : rgb(54, 70, 60);
    if (z > 0.03 && y < 1.645 && y > 1.57) return mix(skin, rgb(120, 90, 70), 0.18 + 0.1 * n);
    if (z > 0.08 && abs(y - 1.628) < 0.004 && ax < 0.022) return rgb(150, 86, 76);
    return skin * (0.92 + 0.1 * n) + rgb(18, 0, 0) * smoothstep(0.03, 0.06, ax) * step(1.62, y) * step(y, 1.67);
  }
  // Forearms and hands below the rolled sleeves.
  if (ax > 0.17 && y < 1.15 && y > 0.75) {
    if (abs(y - 1.165) < 0.03) {
      // The rolled cuff shows the shirt's paler inside.
      return mix(rgb(176, 70, 60), rgb(150, 50, 44), n);
    }
    return skin * (0.88 + 0.12 * n);
  }
  // Belt and buckle.
  if (abs(y - 1.0) < 0.024 && ax < 0.2) {
    if (ax < 0.028 && z > 0.06) return rgb(190, 170, 110);
    return mix(rgb(70, 42, 26), rgb(96, 60, 36), n);
  }
  // Boots.
  if (y < 0.17) {
    if (y < 0.025) return rgb(40, 30, 24);
    return mix(rgb(86, 52, 30), rgb(118, 76, 44), noise3(x * 20, y * 20, z * 20)) * (0.85 + 0.15 * step(0.5, fract(y * 40.0)));
  }
  // Jeans: a blue twill, faded at the thighs and knees, side seams and pockets.
  if (y < 1.0) {
    const twill = step(0.5, fract((x + y) * 160.0 + z * 160.0));
    const fade = smoothstep(0.2, 0.8, noise3(x * 6, y * 4, z * 6)) * (smoothstep(0.06, 0.0, abs(y - 0.5)) * 0.5 + smoothstep(0.1, 0.0, abs(y - 0.78)) * 0.4);
    let c = mix(rgb(44, 66, 104), rgb(60, 86, 130), twill * 0.5 + n * 0.3);
    c = mix(c, rgb(126, 150, 184), fade);
    const seam = abs(z) < 0.01 && ax > 0.12;
    const fly = ax < 0.006 && z > 0 && y > 0.86;
    if (seam || fly) return rgb(170, 130, 70);
    return c;
  }
  // The flannel shirt: red and black plaid with thin yellow lines, a placket
  // and buttons down the front.
  if (z > 0.08 && ax < 0.012) {
    if (fract(y * 11.0) < 0.15) return rgb(240, 230, 210);
    return rgb(120, 26, 24);
  }
  const u = ax * 9.0 + (z < 0 ? 0.5 : 0.0);
  const v = y * 9.0;
  const bandU = step(0.5, fract(u));
  const bandV = step(0.5, fract(v));
  const thin = step(0.965, fract(u * 2.0 + 0.25)) + step(0.965, fract(v * 2.0 + 0.25));
  // The flannel's soft twill.
  const tw = step(0.5, fract((u + v) * 14.0));
  let c = rgb(172, 34, 30);
  if (bandU + bandV > 1.5) c = rgb(36, 20, 20);
  else if (bandU + bandV > 0.5) c = rgb(104, 24, 22);
  if (thin > 0.5) c = mix(c, rgb(200, 160, 70), 0.55);
  return c * (0.86 + 0.08 * tw + 0.12 * n);
}
