// Built-in fallen stick: something to pick up, throw, or burn.
export const meta = { name: "stick", bounds: [0.5, 0.08, 0.12], tags: ["builtin", "stick", "small", "nonsolid"], props: { mass: 0.4, burns: 0.9, fuel: 0.6, bounce: 0.25 } };

export function sdf(x, y, z, k) {
  const bend = (hash(k.seed) - 0.5) * 0.08;
  const main = capsule(x, y, z, -0.44, 0, -bend, 0.44, 0, bend, 0.03);
  const twig = capsule(x, y, z, 0.1, 0, bend * 0.2, 0.28, 0.02, 0.09, 0.016);
  return min(main, twig);
}

export function color(x, y, z, k) {
  return mix(rgb(92, 66, 44), rgb(128, 98, 66), noise3(x * 9, y * 9, z * 9));
}
