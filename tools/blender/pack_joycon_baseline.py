"""Save a self-contained baseline containing only images used by Joy-Con materials."""

import os
from pathlib import Path

import bpy


output_path = Path(os.environ["VIBECON_BASELINE_BLEND"]).expanduser().resolve()
used_images = set()
used_materials = {
    material
    for obj in bpy.context.scene.objects
    if obj.type == "MESH"
    for material in obj.data.materials
    if material is not None
}

for material in used_materials:
    if not material.use_nodes:
        continue
    for node in material.node_tree.nodes:
        if node.type == "TEX_IMAGE" and node.image is not None:
            used_images.add(node.image)

for image in list(bpy.data.images):
    if image not in used_images:
        bpy.data.images.remove(image)

for image in used_images:
    image.pack()

output_path.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.save_as_mainfile(filepath=str(output_path))
print(
    "VIBECON_BASELINE_PACKED "
    f"materials={len(used_materials)} images={len(used_images)} path={output_path}"
)
