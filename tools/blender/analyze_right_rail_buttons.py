"""Audit texture-colored face clusters on the right Joy-Con rail."""

import bpy
from mathutils import Vector


body = bpy.data.objects.get("SM_JoyCon_R_Body")
if body is None:
    raise RuntimeError("Missing SM_JoyCon_R_Body")

uv_layer = body.data.uv_layers.get("RJUV")
image = bpy.data.images.get("R_Colour.png")
if uv_layer is None or image is None:
    raise RuntimeError("Missing right Joy-Con UV map or color texture")

width, height = image.size
pixels = list(image.pixels)


def sample(u, v):
    x = max(0, min(width - 1, int((u % 1.0) * width)))
    y = max(0, min(height - 1, int((v % 1.0) * height)))
    offset = (y * width + x) * 4
    return tuple(pixels[offset + channel] for channel in range(3))


corners = [body.matrix_world @ Vector(corner) for corner in body.bound_box]
print(
    "VIBECON_RIGHT_RAIL_BOUNDS "
    + " ".join(
        f"{axis}=({min(getattr(p, axis) for p in corners):.5f},"
        f"{max(getattr(p, axis) for p in corners):.5f})"
        for axis in ("x", "y", "z")
    )
)

# The rail is the narrow rear plane. Report saturated red faces on either Y
# extreme so the source model, rather than a mirrored-left assumption, tells
# us which side contains the physical SL/SR caps.
for rail_name, predicate in (
    ("min_y", lambda center: center.y < min(p.y for p in corners) + 0.035),
    ("max_y", lambda center: center.y > max(p.y for p in corners) - 0.035),
):
    candidates = []
    for polygon in body.data.polygons:
        center = body.matrix_world @ polygon.center
        if not predicate(center):
            continue
        colors = []
        for loop_index in polygon.loop_indices:
            uv = uv_layer.data[loop_index].uv
            colors.append(sample(uv.x, uv.y))
        red_score = max(red - max(green, blue) for red, green, blue in colors)
        if red_score > 0.18:
            candidates.append((polygon.index, center, red_score))
    print(f"VIBECON_RIGHT_RAIL_CANDIDATES side={rail_name} faces={len(candidates)}")
    for index, center, score in sorted(candidates, key=lambda item: item[1].z):
        print(
            f"FACE index={index:04d} center=({center.x:.5f},{center.y:.5f},{center.z:.5f}) "
            f"red_score={score:.3f}"
        )
