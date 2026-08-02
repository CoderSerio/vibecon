"""Split the welded left SL rail button into an interactive mesh node."""

import os
from pathlib import Path

import bpy


development_path = Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).expanduser().resolve()
if "development" not in development_path.parts:
    raise RuntimeError("Refusing to edit a blend outside the development directory")

body = bpy.data.objects.get("SM_JoyCon_L_Body")
part_name = "SM_JoyCon_left_Button_SL"
if body is None:
    raise RuntimeError("Missing SM_JoyCon_L_Body")
if bpy.data.objects.get(part_name) is not None:
    raise RuntimeError(f"{part_name} has already been split")

# Measured from the current packed development model. This box encloses the
# complete raised rectangular SL button: front face, bevel and side walls,
# while excluding the surrounding black rail.
bounds = {
    "x": (-0.0255, 0.0180),
    "y": (-1.0750, -1.0535),
    "z": (0.2435, 0.3475),
}

selected_indices = []
for polygon in body.data.polygons:
    center = body.matrix_world @ polygon.center
    selected = all(
        bounds[axis][0] <= getattr(center, axis) <= bounds[axis][1]
        for axis in ("x", "y", "z")
    )
    polygon.select = selected
    if selected:
        selected_indices.append(polygon.index)

if len(selected_indices) < 20:
    raise RuntimeError(
        f"SL selector found too few faces ({len(selected_indices)}); refusing to separate"
    )

bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.separate(type="SELECTED")
bpy.ops.object.mode_set(mode="OBJECT")

separated = [
    obj
    for obj in bpy.context.selected_objects
    if obj != body and obj.type == "MESH"
]
if len(separated) != 1:
    raise RuntimeError(f"Expected one separated SL object, found {len(separated)}")

button = separated[0]
button.name = part_name
button.data.name = "GEO_JoyCon_left_Button_SL"
if "LJUV" not in button.data.uv_layers:
    raise RuntimeError("Separated left SL button lost its LJUV map")
button.data.uv_layers.active = button.data.uv_layers["LJUV"]
for layer in button.data.uv_layers:
    layer.active_render = layer.name == "LJUV"

# The button presses toward the controller center along +Y in Blender space.
# Keep the pivot at its geometric center so Three.js can restore/offset it.
bpy.context.view_layer.objects.active = button
button.select_set(True)
bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")

bpy.ops.wm.save_as_mainfile(filepath=str(development_path))
print(
    "VIBECON_LEFT_SL_READY "
    f"faces={len(selected_indices)} vertices={len(button.data.vertices)} "
    f"body={body.name} part={button.name} path={development_path}"
)
