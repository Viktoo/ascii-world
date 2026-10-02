# Plan: editing things where you touch them

## Why

You should be able to punch a hole in a hut, add a stick to its wall, or put
a steering wheel on a boat, and see the world change in that spot. Before this,
the interpreter could only change props and state, or replace a whole thing with
a newly written type. That was slow, and it put the change in the wrong place.

What the code told us:

- **Every shape lives in one big shader.** Adding a type rebuilds all of it on
  the committer thread: about 3 s at 50 types and 4.5 s at 69, growing with each
  new type. Edits that each make a new type don't scale.
- **The hit point was thrown away.** Picking found the exact spot on a surface,
  but actions never carried it.
- **Every SDF goes through `inst_sdf`.** That means per-instance changes can be
  data instead of code.

## The primitives

There is one way in: say what you do after `/`, pointing at the spot with the
middle of the view. The interpreter decides from the words whether it changes
that thing or makes something new (its `make` effect hands off to the
placement planner that `/create` used to be). Each change goes into that
thing's history.

| Primitive | What it is | Cost |
|---|---|---|
| **Reshape** | The builder edits this thing's current code. It gets the change, the touched spot in the code's own coordinates, its cuts (which get baked in) and, optionally, **the held thing's code to work into it** (`"with": "held"`, which uses the held thing up). | LLM call plus a shader rebuild |
| **Cut** | A fast preview for taking pieces out: up to 4 per thing (sphere or cube), subtracted in the shader and on the CPU, showing at once. The next reshape bakes them into the code. | Data: no recompile |
| **State / props** | As before. | Free |

"Punch a hole" becomes a cut. "Add the stick to this wall" (with the stick in
hand) and "make the roof a dome" become reshapes.

Combining things means they stop being separate: a wheel added to a boat is
part of the boat's shape. "Use the wheel" still goes through the interpreter,
and "take the stick off" is just another reshape. One rewrite verb is simpler
than special cases, and it gets better as LLMs get cheaper.

## Status: built

- **Hit point:** `Action::Use` and `Action::Do` take `at`, filled from the
  pointed spot. The interpreter is told `touched_at` (a local point, the
  surface normal, and how high up it is). Characters' plans, which carry no
  spot, use the point where a line from their eyes to the thing's centre hits
  its surface (`Sim::touch_point`).
- **Cuts:** `GpuInst` grew from 128 to 192 bytes, to hold `cuts`.
  - `shape_sdf` in `scene.wgsl` and `GpuInst::sdf` on the CPU apply them, so
    rendering, physics, picking and placement all agree.
  - When there are too many cuts, the new one merges with the nearest.
- **Reshape** (`src/sim/shape.rs`): `Request::EditType` and `Cmd::EditType`
  reach `edit_item_type` in `brain.rs` (it uses `prompts::edit_task`).
  - The new type is named as the interpreter asked, so the next "add a chimney"
    on another hut reuses it.
  - Until the new type arrives, the thing keeps its old shape, and the held
    ingredient stays in hand. If the build fails, nothing is used up.
- **History:** `Thing.shape` (cuts and the last 16 edits) is saved in
  `thing_shapes`. It is shown in F2 inspect and given to the interpreter.
- **Tests:**
  - `a_cut_takes_a_piece_out_where_it_was_touched`
  - `the_interpreter_cuts_and_reshapes_at_the_spot_touched` (covers the
    cut, a chimney, and the stick added to a wall)
  - `cuts_agree_on_gpu_and_cpu`
  - `render_edited_hut_png` (ignored; renders an image to look at)

## Not yet

- **An instant preview for "add".** Today the change shows only once the new
  shape arrives. You could show the held thing at the spot right away, as a
  cut does for holes.

- **Shader builds off the committer thread.** A reshape still blocks other
  commits for the length of a full rebuild. Fix: build the pipeline in the
  background, keep drawing with the old one, then swap.
- **Baked shapes (the long-term fix).** Run an edited thing's code once into a
  small 3D distance grid (around 48³) in a texture. The shader samples the grid,
  so edits never trigger a recompile, and CPU collision samples the same grid.
  Hand-written types stay as code. Build this once reshapes are common enough
  that the type count hurts.
- **Only live things can be edited.** A thing joins the live layer
  first, which `liven` already does when it is touched.
- **The undo rule stays the same.** Like physics and fire, edits to live things
  are not versioned.
