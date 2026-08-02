"""Render the extracted Joy-Cons from the same front-facing axis used by VibeCon."""

import os
from pathlib import Path

import bpy
from mathutils import Vector


output_path = Path(
    os.environ.get(
        "VIBECON_COMPARE_RENDER",
        "/Users/carbon/Desktop/nintendo-switch-with-detachable-joycons/work/VibeCon_FullJoyCons_front_compare.png",
    )
).expanduser()

scene = bpy.context.scene

debug_parts = os.environ.get("VIBECON_COMPARE_DEBUG_PARTS") == "1"
if debug_parts:
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "OBJECT"
    scene.display.shading.show_shadows = True
    scene.display.shading.show_cavity = True
    for obj in scene.objects:
        if obj.type != "MESH":
            continue
        if "Stick" in obj.name:
            obj.color = (0.08, 1.0, 0.22, 1.0)
        elif "_L_" in obj.name:
            obj.color = (0.02, 0.55, 0.9, 1.0)
        elif "_R_" in obj.name:
            obj.color = (1.0, 0.08, 0.04, 1.0)

# The source FBX retains UV maps for the left Joy-Con, right Joy-Con and
# screen on every separated mesh. Select the matching map explicitly so this
# comparison also validates the export configuration.
for obj in scene.objects:
    if obj.type != "MESH":
        continue
    uv_name = "LJUV" if "_L_" in obj.name else "RJUV" if "_R_" in obj.name else None
    if uv_name and uv_name in obj.data.uv_layers:
        obj.data.uv_layers.active = obj.data.uv_layers[uv_name]
        for layer in obj.data.uv_layers:
            layer.active_render = layer.name == uv_name

scene.render.engine = "BLENDER_WORKBENCH" if debug_parts else "BLENDER_EEVEE"
scene.render.resolution_x = 1100
scene.render.resolution_y = 620
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.filepath = str(output_path)
scene.render.film_transparent = False

scene.world.color = (0.008, 0.018, 0.016)
scene.view_settings.look = "AgX - Medium High Contrast"

for obj in list(scene.objects):
    if obj.type in {"CAMERA", "LIGHT"}:
        bpy.data.objects.remove(obj, do_unlink=True)


def point_at(obj, target):
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()


camera_data = bpy.data.cameras.new("CAM_VibeCon_Compare")
camera = bpy.data.objects.new("CAM_VibeCon_Compare", camera_data)
scene.collection.objects.link(camera)
camera.location = (6.0, 0.0, 0.0)
camera_data.type = "ORTHO"
camera_data.ortho_scale = 3.65
point_at(camera, (0.0, 0.0, 0.0))
scene.camera = camera

key_data = bpy.data.lights.new("KEY_Front", "AREA")
key_data.energy = 360
key_data.shape = "RECTANGLE"
key_data.size = 4.0
key = bpy.data.objects.new("KEY_Front", key_data)
scene.collection.objects.link(key)
key.location = (4.0, -1.5, 3.0)
point_at(key, (0.0, 0.0, 0.0))

fill_data = bpy.data.lights.new("FILL_Front", "AREA")
fill_data.energy = 140
fill_data.size = 5.0
fill = bpy.data.objects.new("FILL_Front", fill_data)
scene.collection.objects.link(fill)
fill.location = (3.0, 2.5, 0.5)
point_at(fill, (0.0, 0.0, 0.0))

rim_data = bpy.data.lights.new("RIM_Back", "AREA")
rim_data.energy = 220
rim_data.size = 3.0
rim = bpy.data.objects.new("RIM_Back", rim_data)
scene.collection.objects.link(rim)
rim.location = (-2.5, 0.0, 2.0)
point_at(rim, (0.0, 0.0, 0.0))

output_path.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.render.render(write_still=True)
print(f"VIBECON_COMPARE_RENDER_READY path={output_path}")
