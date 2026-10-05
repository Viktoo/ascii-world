export const meta = { name: "test crate", bounds: [0.65, 0.4, 0.65], tags: ["building"] };
export function sdf(x, y, z, k) { return box(x, y, z, 0.6, 0.35, 0.6); }
export function color(x, y, z, k) { return rgb(150, 110, 70); }
