"""Split the two welded right Joy-Con rail buttons into interactive nodes."""

import os
from pathlib import Path

import bpy


development_path = Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).expanduser().resolve()
if "development" not in development_path.parts:
    raise RuntimeError("Refusing to edit a blend outside the development directory")

body = bpy.data.objects.get("SM_JoyCon_R_Body")
if body is None:
    raise RuntimeError("Missing SM_JoyCon_R_Body")

# These bounds were measured independently on the right source mesh. In the
# rail view, SR is the upper cap (+Z) and SL is the lower cap (-Z); this is not
# derived by mirroring the left Joy-Con because the source topology differs.
parts = {
    "SR": {"x": (-0.024, 0.017), "y": (1.0475, 1.0615), "z": (0.245, 0.346)},
    "SL": {"x": (-0.024, 0.017), "y": (1.0475, 1.0615), "z": (-0.276, -0.176)},
}

for label, bounds in parts.items():
    part_name = f"SM_JoyCon_right_Button_{label}"
    if bpy.data.objects.get(part_name) is not None:
        print(f"VIBECON_RIGHT_RAIL_SKIP part={part_name} reason=already_split")
        continue

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

    if len(selected_indices) < 30:
        raise RuntimeError(
            f"{label} selector found too few faces ({len(selected_indices)}); refusing to separate"
        )

    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    bpy.context.view_layer.objects.active = body
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.separate(type="SELECTED")
    bpy.ops.object.mode_set(mode="OBJECT")

    separated = [
        obj for obj in bpy.context.selected_objects if obj != body and obj.type == "MESH"
    ]
    if len(separated) != 1:
        raise RuntimeError(f"Expected one separated {label} object, found {len(separated)}")

    button = separated[0]
    button.name = part_name
    button.data.name = f"GEO_JoyCon_right_Button_{label}"
    if "RJUV" not in button.data.uv_layers:
        raise RuntimeError(f"Separated right {label} button lost its RJUV map")
    button.data.uv_layers.active = button.data.uv_layers["RJUV"]
    for layer in button.data.uv_layers:
        layer.active_render = layer.name == "RJUV"

    bpy.context.view_layer.objects.active = button
    button.select_set(True)
    bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
    print(
        "VIBECON_RIGHT_RAIL_READY "
        f"part={part_name} faces={len(selected_indices)} vertices={len(button.data.vertices)}"
    )

bpy.ops.wm.save_as_mainfile(filepath=str(development_path))
print(f"VIBECON_RIGHT_RAIL_SAVED path={development_path}")
