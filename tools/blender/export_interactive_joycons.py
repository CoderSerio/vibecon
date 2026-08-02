"""Export development Joy-Cons with independently named interactive parts."""

import os
from pathlib import Path

import bpy


output_dir = Path(os.environ["VIBECON_INTERACTIVE_OUTPUT"]).expanduser().resolve()
output_dir.mkdir(parents=True, exist_ok=True)


def export_group(objects, path):
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
    )


left = [
    obj
    for obj in bpy.context.scene.objects
    if obj.type == "MESH" and (obj.name == "SM_JoyCon_L_Body" or obj.name.startswith("SM_JoyCon_left_"))
]
right = [
    obj
    for obj in bpy.context.scene.objects
    if obj.type == "MESH" and (obj.name == "SM_JoyCon_R_Body" or obj.name.startswith("SM_JoyCon_right_"))
]
if not left or not right:
    raise RuntimeError(f"Missing Joy-Con export groups: left={len(left)} right={len(right)}")

left_path = output_dir / "joycon-left.interactive.glb"
right_path = output_dir / "joycon-right.interactive.glb"
export_group(left, left_path)
export_group(right, right_path)
print(
    "VIBECON_INTERACTIVE_EXPORT_READY "
    f"left_objects={len(left)} right_objects={len(right)} left={left_path} right={right_path}"
)
