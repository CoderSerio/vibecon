"""Create a left Joy-Con prototype from the normalized right-side workfile.

The downloaded model contains a right Joy-Con with independent, untextured
button meshes. Mirroring it provides the housing and rail; the four face
buttons are then repurposed as the physical four-piece left D-pad.
"""

import os
from pathlib import Path

import bpy


blend_output = Path(os.environ["VIBECON_LEFT_BLEND"]).expanduser().resolve()
glb_output = Path(os.environ["VIBECON_LEFT_GLB"]).expanduser().resolve()
root_right = bpy.data.objects.get("CTRL_JoyCon_R")
if root_right is None:
    raise RuntimeError("Expected CTRL_JoyCon_R in the right prototype workfile")

collection = bpy.data.collections.get("COL_JoyCon_R")
if collection is None:
    raise RuntimeError("Expected COL_JoyCon_R in the right prototype workfile")

blue = bpy.data.materials.get("MAT_Plastic_VibeCon_Blue")
if blue is None:
    blue = bpy.data.materials.new("MAT_Plastic_VibeCon_Blue")
    blue.use_nodes = True
    bsdf = next((node for node in blue.node_tree.nodes if node.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        bsdf = blue.node_tree.nodes.new("ShaderNodeBsdfPrincipled")
        output = next((node for node in blue.node_tree.nodes if node.type == "OUTPUT_MATERIAL"), None)
        if output is None:
            output = blue.node_tree.nodes.new("ShaderNodeOutputMaterial")
        blue.node_tree.links.new(bsdf.outputs["BSDF"], output.inputs["Surface"])
    # #0AB9E6 in linear RGB for Blender's Principled material.
    bsdf.inputs["Base Color"].default_value = (0.003, 0.485, 0.791, 1.0)
    bsdf.inputs["Metallic"].default_value = 0.0
    bsdf.inputs["Roughness"].default_value = 0.38

root_left = bpy.data.objects.new("CTRL_JoyCon_L", None)
collection.objects.link(root_left)
root_left.matrix_world = root_right.matrix_world.copy()
root_left.scale.x *= -1

name_map = {
    "SM_JoyCon_R_Body": "SM_JoyCon_L_Body",
    "SM_JoyCon_R_StickCap": "SM_JoyCon_L_StickCap",
    "SM_JoyCon_R_StickBase": "SM_JoyCon_L_StickBase",
    "SM_JoyCon_R_Button_A": "SM_JoyCon_L_DPad_Left",
    "SM_JoyCon_R_Button_B": "SM_JoyCon_L_DPad_Down",
    "SM_JoyCon_R_Button_X": "SM_JoyCon_L_DPad_Up",
    "SM_JoyCon_R_Button_Y": "SM_JoyCon_L_DPad_Right",
    "SM_JoyCon_R_Button_Plus": "SM_JoyCon_L_Button_Minus",
    "SM_JoyCon_R_Button_Home": "SM_JoyCon_L_Button_Capture",
    "SM_JoyCon_R_Button_R": "SM_JoyCon_L_Button_L",
    "SM_JoyCon_R_Button_ZR": "SM_JoyCon_L_Button_ZL",
    "SM_JoyCon_R_Button_SR": "SM_JoyCon_L_Button_SR",
    "SM_JoyCon_R_Button_SL": "SM_JoyCon_L_Button_SL",
    "SM_JoyCon_R_Button_Release": "SM_JoyCon_L_Button_Release",
    "SM_JoyCon_R_Shoulder_Inner": "SM_JoyCon_L_Shoulder_Inner",
    "SM_JoyCon_R_Rail": "SM_JoyCon_L_Rail",
    "SM_JoyCon_R_Button_Unmapped": "SM_JoyCon_L_Button_Unmapped",
}

for child in list(root_right.children):
    copied = child.copy()
    copied.data = child.data.copy()
    collection.objects.link(copied)
    copied.parent = root_left
    copied.matrix_parent_inverse.identity()
    copied.matrix_basis = child.matrix_basis.copy()
    copied.name = name_map.get(child.name, child.name.replace("JoyCon_R", "JoyCon_L"))
    copied.data.name = copied.name
    if copied.name.endswith("_Body") or copied.name.endswith("_SR") or copied.name.endswith("_SL"):
        copied.data.materials.clear()
        copied.data.materials.append(blue)

# This deliverable represents one Joy-Con only.
for child in list(root_right.children):
    bpy.data.objects.remove(child, do_unlink=True)
bpy.data.objects.remove(root_right, do_unlink=True)
collection.name = "COL_JoyCon_L"

meshes = [obj for obj in collection.objects if obj.type == "MESH"]
bpy.ops.object.select_all(action="DESELECT")
root_left.select_set(True)
for obj in meshes:
    obj.select_set(True)
bpy.context.view_layer.objects.active = root_left

blend_output.parent.mkdir(parents=True, exist_ok=True)
glb_output.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.save_as_mainfile(filepath=str(blend_output))
bpy.ops.export_scene.gltf(
    filepath=str(glb_output),
    export_format="GLB",
    use_selection=True,
    export_yup=True,
    export_apply=True,
)
triangles = sum(sum(max(0, len(poly.vertices) - 2) for poly in obj.data.polygons) for obj in meshes)
print(f"VIBECON_JOYCON_L_READY meshes={len(meshes)} triangles={triangles} glb={glb_output}")
