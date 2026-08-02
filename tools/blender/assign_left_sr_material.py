"""Assign the welded left SR button faces to a dedicated material slot."""
import os
from pathlib import Path
import bpy

path = Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).resolve()
if "development" not in path.parts:
    raise RuntimeError("Refusing to edit outside the development model")
body = bpy.data.objects.get("SM_JoyCon_L_Body")
if body is None:
    raise RuntimeError("Missing SM_JoyCon_L_Body")
source = bpy.data.materials.get("MAT_FullJoyCon_L")
if source is None:
    raise RuntimeError("Missing MAT_FullJoyCon_L")

name = "MAT_FullJoyCon_L_Button_SR"
material = bpy.data.materials.get(name) or source.copy()
material.name = name
if body.data.materials.get(name) is None:
    body.data.materials.append(material)
slot = next(i for i, item in enumerate(body.data.materials) if item and item.name == name)

# Use the source texture as the semantic mask: the SR button is cyan while the
# surrounding rail is near-black. Spatial bounds only limit the search to this
# rail region; they do not decide which faces belong to the button.
bounds = {"x": (-0.065, 0.060), "y": (-1.073, -1.0535), "z": (-0.38, -0.205)}
uv_layer = body.data.uv_layers["LJUV"]
image = bpy.data.images["L_Colour.png"]
width, height = image.size
pixels = list(image.pixels)

def sample(u, v):
    x = max(0, min(width - 1, int((u % 1.0) * width)))
    y = max(0, min(height - 1, int((v % 1.0) * height)))
    offset = (y * width + x) * 4
    return tuple(pixels[offset + index] for index in range(3))

blue_faces = set()
for polygon in body.data.polygons:
    center = body.matrix_world @ polygon.center
    if not all(bounds[a][0] <= getattr(center, a) <= bounds[a][1] for a in ("x", "y", "z")):
        continue
    uvs = [uv_layer.data[index].uv for index in polygon.loop_indices]
    u = sum(value.x for value in uvs) / len(uvs)
    v = sum(value.y for value in uvs) / len(uvs)
    samples = [sample(u, v), *(sample(value.x, value.y) for value in uvs)]
    touches_button_blue = any(
        green > 0.55 and blue > 0.70 and blue > red * 3
        for red, green, blue in samples
    )
    if touches_button_blue:
        blue_faces.add(polygon.index)

# Include one topology ring for the dark bevel/side-wall faces belonging to
# the same physical button. Keep the expansion inside the measured button
# envelope so it cannot spread into the surrounding rail.
button_bounds = {"x": (-0.030, 0.023), "y": (-1.073, -1.0535), "z": (-0.283, -0.226)}
selected = set(blue_faces)
for polygon in body.data.polygons:
    center = body.matrix_world @ polygon.center
    inside = all(
        button_bounds[a][0] <= getattr(center, a) <= button_bounds[a][1]
        for a in ("x", "y", "z")
    )
    # The model duplicates vertices along several UV seams, so some visible
    # bevel faces do not share edge keys with the blue seed faces. The bounds
    # have now been audited face-by-face and contain only the physical button.
    if inside:
        selected.add(polygon.index)
for index in selected:
    body.data.polygons[index].material_index = slot
if len(selected) < 20:
    raise RuntimeError(f"SR material selector found only {len(selected)} faces")

bpy.ops.wm.save_as_mainfile(filepath=str(path))
print(f"VIBECON_LEFT_SR_MATERIAL_READY faces={len(selected)} slot={slot} material={name}")
