"""Inspect polygon centers/normals around the left SL rail button."""

import bpy
from mathutils import Vector


body = bpy.data.objects.get("SM_JoyCon_L_Body")
if body is None:
    raise RuntimeError("Missing SM_JoyCon_L_Body")

world_corners = [body.matrix_world @ Vector(corner) for corner in body.bound_box]
center_z = sum(point.z for point in world_corners) / len(world_corners)
rail_y = max(point.y for point in world_corners)
target_z = center_z + 0.31

candidates = []
for polygon in body.data.polygons:
    center = body.matrix_world @ polygon.center
    normal = (body.matrix_world.to_3x3() @ polygon.normal).normalized()
    if center.y >= rail_y - 0.09 and abs(center.z - target_z) <= 0.09:
        candidates.append((center, normal, polygon.index, polygon.area))

print(
    "VIBECON_SIDE_REGION "
    f"rail_y={rail_y:.5f} target_z={target_z:.5f} faces={len(candidates)}"
)
for center, normal, index, area in sorted(
    candidates,
    key=lambda item: (-item[0].y, item[0].z, item[0].x),
):
    print(
        f"FACE index={index:04d} "
        f"center=({center.x:.5f},{center.y:.5f},{center.z:.5f}) "
        f"normal=({normal.x:.4f},{normal.y:.4f},{normal.z:.4f}) "
        f"area={area:.7f}"
    )
