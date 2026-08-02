"""Split and pivot the right analogue stick assembly."""
import os
from pathlib import Path
import bpy
from mathutils import Vector

path=Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).resolve(); body=bpy.data.objects["SM_JoyCon_R_Body"]
name="SM_JoyCon_right_StickAssembly"
if bpy.data.objects.get(name): raise RuntimeError(f"{name} already exists")
center_y=sum((body.matrix_world@Vector(c)).y for c in body.bound_box)/8
target_z=-0.044; radius=0.15; minimum_front_x=0.09
selected=[]
for p in body.data.polygons:
    c=body.matrix_world@p.center; radial=((c.y-center_y)**2+(c.z-target_z)**2)**0.5
    p.select=radial<=radius and c.x>=minimum_front_x
    if p.select: selected.append(p.index)
if len(selected)<100: raise RuntimeError(f"Right stick selector found only {len(selected)} faces")
bpy.ops.object.select_all(action="DESELECT"); body.select_set(True); bpy.context.view_layer.objects.active=body
bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.separate(type="SELECTED"); bpy.ops.object.mode_set(mode="OBJECT")
parts=[o for o in bpy.context.selected_objects if o!=body and o.type=="MESH"]
if len(parts)!=1: raise RuntimeError(f"Expected one stick object, got {len(parts)}")
stick=parts[0]; stick.name=name; stick.data.name="GEO_JoyCon_right_StickAssembly"
if "RJUV" not in stick.data.uv_layers: raise RuntimeError("Right stick lost RJUV")
stick.data.uv_layers.active=stick.data.uv_layers["RJUV"]
for layer in stick.data.uv_layers: layer.active_render=layer.name=="RJUV"
bpy.context.view_layer.objects.active=stick; stick.select_set(True)
points=[stick.matrix_world@v.co for v in stick.data.vertices]; minimum_x=min(p.x for p in points); maximum_x=max(p.x for p in points)
top=[p for p in points if p.x>=maximum_x-0.015]
bpy.context.scene.cursor.location=(minimum_x,sum(p.y for p in top)/len(top),sum(p.z for p in top)/len(top))
bpy.ops.object.origin_set(type="ORIGIN_CURSOR")
bpy.ops.wm.save_as_mainfile(filepath=str(path))
print(f"VIBECON_RIGHT_STICK_READY faces={len(selected)} vertices={len(stick.data.vertices)} pivot={tuple(round(v,5) for v in stick.location)}")
