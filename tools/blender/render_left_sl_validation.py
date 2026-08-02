"""Render a side-view QA image for the separated left SL button."""

import os
from pathlib import Path

import bpy
from mathutils import Vector


output_path = Path(os.environ["VIBECON_SL_RENDER"]).expanduser().resolve()
scene = bpy.context.scene
scene.render.engine = "BLENDER_WORKBENCH"
scene.display.shading.light = "STUDIO"
scene.display.shading.color_type = "OBJECT"
scene.display.shading.show_shadows = True
scene.display.shading.show_cavity = True
scene.render.resolution_x = 520
scene.render.resolution_y = 720
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.filepath = str(output_path)
scene.world.color = (0.008, 0.018, 0.016)

for obj in list(scene.objects):
    if obj.type in {"CAMERA", "LIGHT"}:
        bpy.data.objects.remove(obj, do_unlink=True)
    elif obj.type == "MESH":
        obj.hide_render = not obj.name.startswith("SM_JoyCon_L_") and not obj.name.startswith("SM_JoyCon_left_")
        if obj.name == "SM_JoyCon_left_Button_SL":
            obj.color = (0.25, 1.0, 0.45, 1.0)
        elif "StickAssembly" in obj.name:
            obj.color = (0.05, 0.08, 0.08, 1.0)
        else:
            obj.color = (0.02, 0.55, 0.9, 1.0)


def point_at(obj, target):
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()


target = (0.0, -1.285, 0.0)
camera_data = bpy.data.cameras.new("CAM_Left_SL_QA")
camera = bpy.data.objects.new("CAM_Left_SL_QA", camera_data)
scene.collection.objects.link(camera)
camera.location = (0.0, 3.0, 0.0)
camera_data.type = "ORTHO"
camera_data.ortho_scale = 1.55
point_at(camera, target)
scene.camera = camera

output_path.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.render.render(write_still=True)
print(f"VIBECON_LEFT_SL_RENDER_READY path={output_path}")
