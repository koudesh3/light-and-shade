# Pothos painting project

From one phone photo of a gouache pothos painting to a layered, lit, animated plant.

## tools/ (open in a browser)
1. **1_trace_outline.html**: trace a shape over the photo. Used for the cutout outline, the paper holes, and all 16 leaves.
2. **2_leaf_order.html**: answer "which leaf is in front?" for each overlapping pair; it builds the back-to-front order.
3. **3_leaf_normals.html**: pick a facing direction from the normal-map sphere and paint it onto each leaf, with a lit preview.
4. **4_light_study.html**: all layers stacked with their normal maps under a light you can drag (WebGPU, WebGL2 fallback).
5. **5_pothos_in_a_breeze.html**: the living plant: sway, cursor wind, depth parallax, cast shadows, moving sun.

Tools 1–3 saved your work to Claude when opened as artifacts. Opened as plain files they still work, but use their Copy button to get your data out.

## data/
- `outline.json`: the cutout outline. `shapes.json`: outline plus the two paper holes.
- `leaves.json`: the 16 leaf polygons (index 0 = leaf 1), in full-res photo pixels (3024×4032).
- `pairs.json`, `leaf_order.json`: overlapping pairs, your answers, and the final order.
- `normal_strokes.json`: every normal-map stroke you painted.
All coordinates are in the original photo's pixels.

## output/
- `original.png`, `painting_on_white.png`
- `leaf_layers/`: one transparent PNG per layer (`z00_untraced` … `z16_leaf06`), `hidden/` masks, `normals/` maps (x right, y up, z toward viewer; color = normal × 0.5 + 0.5), `stacked_check.png`, `layers.json`
- Preview sheets: `leaf_map.jpg`, `layer_sheet.jpg`, `normals_check.jpg`, `z0_assign.jpg`

## scripts/
The Python that built the outputs, roughly in this order: `layers.py` (cut layers), `z0.py` (fold slivers into leaves), `nmaps.py` (rasterize normals), `alive_prep.py` (fill hidden areas, pivots, pack assets for the breeze page). `*_template.html` are the tool sources before the image data was embedded.
The scripts were run once in a working session and still contain absolute paths and some intermediate files (`frame.npy`, `z0_extra.npy`) from that session. Read them as a record of the method; rerunning them needs the paths updated.
