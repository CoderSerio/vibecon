import os
from pathlib import Path
import bpy
from mathutils import Vector

output = Path(os.environ["VIBECON_L_RENDER"]).resolve()
isolate = os.environ.get("VIBECON_L_ISOLATE") == "1"
scene = bpy.context.scene
scene.render.engine = "BLENDER_WORKBENCH"
scene.display.shading.light = "STUDIO"
scene.display.shading.color_type = "OBJECT"
scene.display.shading.show_shadows = True
scene.display.shading.show_cavity = True
scene.render.resolution_x = 720
scene.render.resolution_y = 620
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.filepath = str(output)
for obj in list(scene.objects):
    if obj.type in {"CAMERA", "LIGHT"}:
        bpy.data.objects.remove(obj, do_unlink=True)
    elif obj.type == "MESH":
        obj.hide_render = not (obj.name.startswith("SM_JoyCon_L_") or obj.name.startswith("SM_JoyCon_left_"))
        if isolate and obj.name not in {"SM_JoyCon_left_Button_L", "SM_JoyCon_left_Button_ZL"}:
            obj.hide_render = True
        if obj.name == "SM_JoyCon_left_Button_L":
            obj.color = (0.25, 1.0, 0.45, 1.0)
        elif obj.name == "SM_JoyCon_left_Button_ZL":
            obj.color = (1.0, 0.35, 0.08, 1.0)
        else:
            obj.color = (0.02, 0.55, 0.9, 1.0)

camera_data = bpy.data.cameras.new("CAM_Left_L_QA")
camera = bpy.data.objects.new("CAM_Left_L_QA", camera_data)
scene.collection.objects.link(camera)
camera.location = (2.5, -0.15, 1.7)
camera_data.type = "ORTHO"
camera_data.ortho_scale = 1.55
camera.rotation_euler = (Vector((0.0, -1.285, 0.1)) - camera.location).to_track_quat("-Z", "Y").to_euler()
scene.camera = camera
output.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.render.render(write_still=True)
print(f"VIBECON_LEFT_L_RENDER_READY path={output}")
