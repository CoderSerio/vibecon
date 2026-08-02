"""Render a front-view QA image for the separated left minus button."""
import os
from pathlib import Path
import bpy
from mathutils import Vector

output = Path(os.environ["VIBECON_MINUS_RENDER"]).resolve()
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
scene.render.filepath = str(output)
for obj in list(scene.objects):
    if obj.type in {"CAMERA", "LIGHT"}:
        bpy.data.objects.remove(obj, do_unlink=True)
    elif obj.type == "MESH":
        obj.hide_render = not (obj.name.startswith("SM_JoyCon_L_") or obj.name.startswith("SM_JoyCon_left_"))
        obj.color = (0.25, 1.0, 0.45, 1.0) if obj.name == "SM_JoyCon_left_Button_Minus" else (0.02, 0.55, 0.9, 1.0)

def point_at(obj, target):
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()

camera_data = bpy.data.cameras.new("CAM_Left_Minus_QA")
camera = bpy.data.objects.new("CAM_Left_Minus_QA", camera_data)
scene.collection.objects.link(camera)
camera.location = (3.0, -1.285, 0.12)
camera_data.type = "ORTHO"
camera_data.ortho_scale = 1.45
point_at(camera, (0.0, -1.285, 0.05))
scene.camera = camera
output.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.render.render(write_still=True)
print(f"VIBECON_LEFT_MINUS_RENDER_READY path={output}")
