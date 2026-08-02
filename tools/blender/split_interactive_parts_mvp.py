"""Split one interactive surface from the development Joy-Con mesh.

MVP scope: the raised front/side faces of the left analogue stick. Run only
against a development copy; the baseline asset must remain untouched.
"""

import os
from pathlib import Path

import bpy
from mathutils import Vector


development_path = Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).expanduser().resolve()
if "development" not in development_path.parts:
    raise RuntimeError("Refusing to edit a blend outside the development directory")

body = bpy.data.objects.get("SM_JoyCon_L_Body")
if body is None:
    raise RuntimeError("Missing SM_JoyCon_L_Body")
if bpy.data.objects.get("SM_JoyCon_left_StickAssembly") is not None:
    raise RuntimeError("Left StickAssembly has already been split in this development file")

center_y = sum((body.matrix_world @ Vector(corner)).y for corner in body.bound_box) / 8
target_y = center_y
target_z = 0.36
radius = 0.15
minimum_front_x = 0.10

selected_indices = []
for polygon in body.data.polygons:
    center = body.matrix_world @ polygon.center
    radial = ((center.y - target_y) ** 2 + (center.z - target_z) ** 2) ** 0.5
    polygon.select = radial <= radius and center.x >= minimum_front_x
    if polygon.select:
        selected_indices.append(polygon.index)

if not selected_indices:
    raise RuntimeError("Spatial selector did not find any StickCap faces")

bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.separate(type="SELECTED")
bpy.ops.object.mode_set(mode="OBJECT")

separated = [obj for obj in bpy.context.selected_objects if obj != body and obj.type == "MESH"]
if len(separated) != 1:
    raise RuntimeError(f"Expected one separated StickCap object, found {len(separated)}")

stick = separated[0]
stick.name = "SM_JoyCon_left_StickAssembly"
stick.data.name = "GEO_JoyCon_left_StickAssembly"
if "LJUV" not in stick.data.uv_layers:
    raise RuntimeError("Separated left StickCap lost its LJUV map")
stick.data.uv_layers.active = stick.data.uv_layers["LJUV"]
for layer in stick.data.uv_layers:
    layer.active_render = layer.name == "LJUV"
bpy.context.view_layer.objects.active = stick
stick.select_set(True)

# The cap and stem form one rigid assembly. Pivot at the stem base, not at the
# cap's geometric center, so the whole stick tilts like the physical mechanism.
world_points = [stick.matrix_world @ vertex.co for vertex in stick.data.vertices]
minimum_x = min(point.x for point in world_points)
maximum_x = max(point.x for point in world_points)
top_points = [point for point in world_points if point.x >= maximum_x - 0.015]
bpy.context.scene.cursor.location = (
    minimum_x,
    sum(point.y for point in top_points) / len(top_points),
    sum(point.z for point in top_points) / len(top_points),
)
bpy.ops.object.origin_set(type="ORIGIN_CURSOR")

# Preserve the clean production material. Object colors are only for Workbench
# QA screenshots and do not change the exported PBR appearance.
body.color = (0.01, 0.55, 0.85, 1.0)
stick.color = (0.25, 1.0, 0.45, 1.0)

bpy.ops.wm.save_as_mainfile(filepath=str(development_path))
print(
    "VIBECON_INTERACTIVE_MVP_READY "
    f"faces={len(selected_indices)} body={body.name} part={stick.name} "
    f"pivot={tuple(round(value, 5) for value in stick.location)} path={development_path}"
)
