import os
from pathlib import Path
import bpy
from mathutils import Vector

output=Path(os.environ["VIBECON_RIGHT_RENDER"]).resolve()
scene=bpy.context.scene; scene.render.engine="BLENDER_WORKBENCH"
scene.display.shading.light="STUDIO"; scene.display.shading.color_type="OBJECT"
scene.display.shading.show_shadows=True; scene.display.shading.show_cavity=True
scene.render.resolution_x=520; scene.render.resolution_y=720; scene.render.resolution_percentage=100
scene.render.image_settings.file_format="PNG"; scene.render.filepath=str(output)
for obj in list(scene.objects):
    if obj.type in {"CAMERA","LIGHT"}: bpy.data.objects.remove(obj,do_unlink=True)
    elif obj.type=="MESH":
        obj.hide_render=not (obj.name=="SM_JoyCon_R_Body" or obj.name.startswith("SM_JoyCon_right_"))
        if obj.name=="SM_JoyCon_right_Button_R": obj.color=(0.25,1.0,0.45,1.0)
        elif obj.name=="SM_JoyCon_right_Button_ZR": obj.color=(1.0,0.35,0.08,1.0)
        elif obj.name=="SM_JoyCon_right_StickAssembly": obj.color=(0.25,1.0,0.45,1.0)
        else: obj.color=(0.95,0.08,0.04,1.0)
camera_data=bpy.data.cameras.new("CAM_Right_QA"); camera=bpy.data.objects.new("CAM_Right_QA",camera_data); scene.collection.objects.link(camera)
camera.location=(2.4,2.4,1.6) if os.environ.get("VIBECON_RIGHT_SHOULDER_VIEW")=="1" else (3.0,1.285,0.12)
camera_data.type="ORTHO"; camera_data.ortho_scale=1.45
camera.rotation_euler=(Vector((0.0,1.285,0.05))-camera.location).to_track_quat("-Z","Y").to_euler(); scene.camera=camera
output.parent.mkdir(parents=True,exist_ok=True); bpy.ops.render.render(write_still=True)
print(f"VIBECON_RIGHT_RENDER_READY path={output}")
