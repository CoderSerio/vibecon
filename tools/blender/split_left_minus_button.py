"""Split the left minus button from the welded Joy-Con body."""
import os
from pathlib import Path
import bpy

path = Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).resolve()
if "development" not in path.parts:
    raise RuntimeError("Refusing to edit outside development model")
body = bpy.data.objects.get("SM_JoyCon_L_Body")
name = "SM_JoyCon_left_Button_Minus"
if body is None:
    raise RuntimeError("Missing SM_JoyCon_L_Body")
if bpy.data.objects.get(name) is not None:
    raise RuntimeError(f"{name} already exists")

# Independently measured from the front/shoulder view. This encloses the
# complete raised minus control while excluding the surrounding shell.
bounds = {"x": (0.085, 0.093), "y": (-1.200, -1.135), "z": (0.490, 0.525)}
selected = []
for polygon in body.data.polygons:
    center = body.matrix_world @ polygon.center
    polygon.select = all(
        bounds[axis][0] <= getattr(center, axis) <= bounds[axis][1]
        for axis in ("x", "y", "z")
    )
    if polygon.select:
        selected.append(polygon.index)
if len(selected) < 8:
    raise RuntimeError(f"Minus selector found only {len(selected)} faces")

bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.separate(type="SELECTED")
bpy.ops.object.mode_set(mode="OBJECT")
parts = [obj for obj in bpy.context.selected_objects if obj != body and obj.type == "MESH"]
if len(parts) != 1:
    raise RuntimeError(f"Expected one minus object, found {len(parts)}")
button = parts[0]
button.name = name
button.data.name = "GEO_JoyCon_left_Button_Minus"
if "LJUV" not in button.data.uv_layers:
    raise RuntimeError("Minus button lost LJUV")
button.data.uv_layers.active = button.data.uv_layers["LJUV"]
for layer in button.data.uv_layers:
    layer.active_render = layer.name == "LJUV"
bpy.ops.wm.save_as_mainfile(filepath=str(path))
print(f"VIBECON_LEFT_MINUS_SPLIT_READY faces={len(selected)} vertices={len(button.data.vertices)} path={path}")
