"""Create a non-destructive Blender workfile from a downloaded Joy-Con FBX.

Run with Blender, for example:
  VIBECON_FBX=/path/source.fbx VIBECON_TEXTURES=/path/textures \
  VIBECON_BLEND=/path/VibeCon_JoyCon_R_working.blend \
  Blender --background --factory-startup --python tools/blender/create_joycon_workfile.py
"""

import difflib
import json
import os
import re
from pathlib import Path

import bpy


def normalized(value: str) -> str:
    return re.sub(r"[^a-z0-9]", "", value.lower())


source = Path(os.environ["VIBECON_FBX"]).expanduser().resolve()
textures = Path(os.environ["VIBECON_TEXTURES"]).expanduser().resolve()
output = Path(os.environ["VIBECON_BLEND"]).expanduser().resolve()

if not source.is_file():
    raise RuntimeError(f"FBX source does not exist: {source}")
if not textures.is_dir():
    raise RuntimeError(f"Texture directory does not exist: {textures}")

bpy.ops.import_scene.fbx(filepath=str(source))
texture_files = [path for path in textures.iterdir() if path.is_file()]
texture_keys = {path: normalized(path.name) for path in texture_files}
resolved_images = []

for image in bpy.data.images:
    if not image.filepath:
        continue
    requested = normalized(Path(image.filepath).name)
    candidate = max(
        texture_files,
        key=lambda path: difflib.SequenceMatcher(None, requested, texture_keys[path]).ratio(),
        default=None,
    )
    if candidate is None:
        continue
    score = difflib.SequenceMatcher(None, requested, texture_keys[candidate]).ratio()
    if score < 0.70:
        continue
    image.filepath = str(candidate)
    image.reload()
    resolved_images.append({"image": image.name, "file": candidate.name, "score": round(score, 3)})

output.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.save_as_mainfile(filepath=str(output))

meshes = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
report = {
    "output": str(output),
    "mesh_count": len(meshes),
    "triangles": sum(sum(max(0, len(poly.vertices) - 2) for poly in obj.data.polygons) for obj in meshes),
    "resolved_images": resolved_images,
    "unresolved_images": [image.name for image in bpy.data.images if image.filepath and not Path(image.filepath).is_file()],
}
print("VIBECON_WORKFILE=" + json.dumps(report))
