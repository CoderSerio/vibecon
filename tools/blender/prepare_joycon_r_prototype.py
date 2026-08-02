"""Normalize the downloaded right Joy-Con into a VibeCon-ready GLB prototype.

This script is intentionally non-destructive to the downloaded FBX: it reads
the prepared .blend workfile and writes a separately named Blender file and
GLB. It uses separate Mesh objects instead of a rig so Three.js can animate
individual buttons by name.
"""

import os
from pathlib import Path

import bpy
from mathutils import Vector


blend_output = Path(os.environ["VIBECON_PREPARED_BLEND"]).expanduser().resolve()
glb_output = Path(os.environ["VIBECON_PREPARED_GLB"]).expanduser().resolve()


def make_material(name, color, roughness, metallic=0.0, emission=None):
    material = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    material.use_nodes = True
    material.diffuse_color = (*color, 1.0)
    nodes = material.node_tree.nodes
    bsdf = next((node for node in nodes if node.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        bsdf = nodes.new("ShaderNodeBsdfPrincipled")
        output = next((node for node in nodes if node.type == "OUTPUT_MATERIAL"), None)
        if output is None:
            output = nodes.new("ShaderNodeOutputMaterial")
        material.node_tree.links.new(bsdf.outputs["BSDF"], output.inputs["Surface"])
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if emission:
        bsdf.inputs["Emission Color"].default_value = (*emission, 1.0)
        bsdf.inputs["Emission Strength"].default_value = 1.6
    return material


red = make_material("MAT_Plastic_VibeCon_Red", (1.0, 0.045, 0.021), 0.38)
black = make_material("MAT_Plastic_Button_Black", (0.003, 0.021, 0.032), 0.62)
light = make_material("MAT_Emissive_Status", (0.02, 0.16, 0.08), 0.35, emission=(0.05, 1.0, 0.35))

object_names = {
    "Base_Combined": "SM_JoyCon_R_Body",
    "Joystick_top": "SM_JoyCon_R_StickCap",
    "Joystick_Body": "SM_JoyCon_R_StickBase",
    "Button_A": "SM_JoyCon_R_Button_A",
    "Button_B": "SM_JoyCon_R_Button_B",
    "Button_X": "SM_JoyCon_R_Button_X",
    "Button_Y": "SM_JoyCon_R_Button_Y",
    "Button_Plus": "SM_JoyCon_R_Button_Plus",
    "Home_Button": "SM_JoyCon_R_Button_Home",
    "Button_ZR": "SM_JoyCon_R_Button_ZR",
    "Button_R": "SM_JoyCon_R_Button_R",
    "Button_Rmiddle": "SM_JoyCon_R_Shoulder_Inner",
    "Button_SR": "SM_JoyCon_R_Button_SR",
    "Button_SL": "SM_JoyCon_R_Button_SL",
    "Release_button": "SM_JoyCon_R_Button_Release",
    "Side_Part_Main": "SM_JoyCon_R_Rail",
    "Screw": "SM_JoyCon_R_Screw_01",
    "Screw1": "SM_JoyCon_R_Screw_02",
    "Screw2": "SM_JoyCon_R_Screw_03",
    "Screw3": "SM_JoyCon_R_Screw_04",
    "Light1": "SM_JoyCon_R_StatusLight_01",
    "Light2": "SM_JoyCon_R_StatusLight_02",
    "Light3": "SM_JoyCon_R_StatusLight_03",
    "Light4": "SM_JoyCon_R_StatusLight_04",
    "Button": "SM_JoyCon_R_Button_Unmapped",
}

scene_collection = bpy.context.scene.collection
collection = bpy.data.collections.get("COL_JoyCon_R") or bpy.data.collections.new("COL_JoyCon_R")
if collection.name not in scene_collection.children:
    scene_collection.children.link(collection)

for obj in list(bpy.context.scene.objects):
    if obj.type != "MESH":
        continue
    if obj.name == "Cube":
        bpy.data.objects.remove(obj, do_unlink=True)
        continue
    obj.name = object_names.get(obj.name, f"SM_JoyCon_R_{obj.name.replace(' ', '_')}")
    obj.data.name = obj.name
    for linked_collection in list(obj.users_collection):
        linked_collection.objects.unlink(obj)
    collection.objects.link(obj)
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
    obj.select_set(False)

    obj.data.materials.clear()
    if "Body" in obj.name or obj.name.endswith("_SR") or obj.name.endswith("_SL"):
        obj.data.materials.append(red)
    elif "StatusLight" in obj.name:
        obj.data.materials.append(light)
    else:
        obj.data.materials.append(black)

meshes = [obj for obj in collection.objects if obj.type == "MESH"]
min_corner = Vector((float("inf"),) * 3)
max_corner = Vector((float("-inf"),) * 3)
for obj in meshes:
    for corner in obj.bound_box:
        point = obj.matrix_world @ Vector(corner)
        min_corner = Vector(map(min, min_corner, point))
        max_corner = Vector(map(max, max_corner, point))

control = bpy.data.objects.get("CTRL_JoyCon_R")
if not control:
    control = bpy.data.objects.new("CTRL_JoyCon_R", None)
    collection.objects.link(control)
control.empty_display_type = "PLAIN_AXES"
control.location = (min_corner + max_corner) / 2
for obj in meshes:
    world_matrix = obj.matrix_world.copy()
    obj.parent = control
    # Parenting after moving each origin must preserve the already-correct
    # world-space geometry; otherwise the buttons visibly pull away from the
    # body when the controller empty is offset from world origin.
    obj.matrix_parent_inverse.identity()
    obj.matrix_world = world_matrix

for obj in bpy.context.selected_objects:
    obj.select_set(False)
for obj in [control, *meshes]:
    obj.select_set(True)
bpy.context.view_layer.objects.active = control

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
print(f"VIBECON_JOYCON_R_READY meshes={len(meshes)} triangles={triangles} glb={glb_output}")
