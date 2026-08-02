"""Extract the two Joy-Con shells from the combined FullSwitch FBX.

The Sketchfab export stores the left Joy-Con, screen and right Joy-Con as
three disconnected components in one mesh. This script separates those
components, discards the screen, assigns the supplied PBR textures, and writes
two independent GLB files for the local VibeCon prototype.

Required environment variables:
  VIBECON_FULL_FBX       path to FullSwitch.fbx
  VIBECON_FULL_TEXTURES  directory containing L_*, R_* and S_* textures
  VIBECON_FULL_OUTPUT    directory for the working blend and GLBs
"""

import os
from pathlib import Path

import bpy


fbx_path = Path(os.environ["VIBECON_FULL_FBX"]).expanduser().resolve()
texture_dir = Path(os.environ["VIBECON_FULL_TEXTURES"]).expanduser().resolve()
color_texture_dir = Path(
    os.environ.get("VIBECON_FULL_COLOR_TEXTURES", str(texture_dir))
).expanduser().resolve()
output_dir = Path(os.environ["VIBECON_FULL_OUTPUT"]).expanduser().resolve()

if not fbx_path.is_file():
    raise RuntimeError(f"FullSwitch FBX does not exist: {fbx_path}")
if not texture_dir.is_dir():
    raise RuntimeError(f"Texture directory does not exist: {texture_dir}")


def load_image(path: Path, colorspace: str):
    if not path.is_file():
        return None
    image = bpy.data.images.load(str(path), check_existing=True)
    image.colorspace_settings.name = colorspace
    return image


def make_material(prefix: str, key: str):
    material = bpy.data.materials.new(f"MAT_FullJoyCon_{prefix}")
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    nodes.clear()

    output = nodes.new("ShaderNodeOutputMaterial")
    output.location = (420, 0)
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.location = (80, 0)
    bsdf.inputs["Roughness"].default_value = 0.42
    links.new(bsdf.outputs["BSDF"], output.inputs["Surface"])

    color = load_image(color_texture_dir / f"{key}_Colour.png", "sRGB")
    if color:
        color_node = nodes.new("ShaderNodeTexImage")
        color_node.name = f"{prefix} Color"
        color_node.image = color
        color_node.location = (-420, 80)
        links.new(color_node.outputs["Color"], bsdf.inputs["Base Color"])

    roughness = load_image(texture_dir / f"{key}_Roughness.png", "Non-Color")
    if roughness:
        rough_node = nodes.new("ShaderNodeTexImage")
        rough_node.name = f"{prefix} Roughness"
        rough_node.image = roughness
        rough_node.location = (-420, -100)
        links.new(rough_node.outputs["Color"], bsdf.inputs["Roughness"])

    metallic = load_image(texture_dir / f"{key}_Metallic.png", "Non-Color")
    if metallic:
        metal_node = nodes.new("ShaderNodeTexImage")
        metal_node.name = f"{prefix} Metallic"
        metal_node.image = metallic
        metal_node.location = (-420, -280)
        links.new(metal_node.outputs["Color"], bsdf.inputs["Metallic"])

    normal_path = texture_dir / f"{key}_Normal_OpenGL.png"
    if not normal_path.is_file():
        normal_path = texture_dir / f"{key}_Normal_OpenGl.png"
    normal = load_image(normal_path, "Non-Color")
    if normal:
        normal_node = nodes.new("ShaderNodeTexImage")
        normal_node.name = f"{prefix} Normal"
        normal_node.image = normal
        normal_node.location = (-420, -460)
        normal_map = nodes.new("ShaderNodeNormalMap")
        normal_map.location = (-80, -180)
        links.new(normal_node.outputs["Color"], normal_map.inputs["Color"])
        links.new(normal_map.outputs["Normal"], bsdf.inputs["Normal"])

    return material


bpy.ops.import_scene.fbx(filepath=str(fbx_path))
source = next((obj for obj in bpy.context.scene.objects if obj.type == "MESH" and obj.name != "Cube"), None)
if source is None:
    raise RuntimeError("FullSwitch FBX did not contain a mesh")

# The source mesh has three disconnected islands. Separate them in-place; the
# active material index identifies each island independently of Blender's
# generated object suffixes.
bpy.context.view_layer.objects.active = source
source.select_set(True)
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.mesh.separate(type="LOOSE")
bpy.ops.object.mode_set(mode="OBJECT")

parts = []
for obj in bpy.context.scene.objects:
    if obj.type != "MESH" or obj.name == "Cube":
        continue
    used = {poly.material_index for poly in obj.data.polygons}
    if len(used) != 1:
        raise RuntimeError(f"Expected one source material per component: {obj.name} uses {used}")
    parts.append((next(iter(used)), obj))

by_material = {index: obj for index, obj in parts}
for required in (0, 1, 2):
    if required not in by_material:
        raise RuntimeError(f"Missing FullSwitch component with material index {required}")

left_obj = by_material[0]
right_obj = by_material[1]
screen = by_material[2]

left_obj.name = "SM_JoyCon_L_Body"
right_obj.name = "SM_JoyCon_R_Body"


def select_uv_map(obj, uv_name: str):
    """Select the UV set that belongs to this separated controller shell."""
    if uv_name not in obj.data.uv_layers:
        available = [layer.name for layer in obj.data.uv_layers]
        raise RuntimeError(f"Missing UV map {uv_name} on {obj.name}; found {available}")
    obj.data.uv_layers.active = obj.data.uv_layers[uv_name]
    for layer in obj.data.uv_layers:
        layer.active_render = layer.name == uv_name


# The FBX duplicates the left, right and screen UV sets onto every disconnected
# component. Blender otherwise keeps RJUV active for both separated Joy-Cons,
# which makes the left shell sample the wrong region of every PBR texture.
select_uv_map(left_obj, "LJUV")
select_uv_map(right_obj, "RJUV")

left_material = make_material("L", "L")
right_material = make_material("R", "R")
left_obj.data.materials.clear()
left_obj.data.materials.append(left_material)
right_obj.data.materials.clear()
right_obj.data.materials.append(right_material)

# Remove the screen and the imported helper objects. The original FBX remains
# untouched and can always be re-imported if the extraction rules change.
bpy.data.objects.remove(screen, do_unlink=True)
for obj in list(bpy.context.scene.objects):
    if obj.type in {"CAMERA", "LIGHT"} or obj.name == "Cube":
        bpy.data.objects.remove(obj, do_unlink=True)

collection = bpy.data.collections.new("COL_VibeCon_FullJoyCons")
bpy.context.scene.collection.children.link(collection)
for obj in (left_obj, right_obj):
    for linked in list(obj.users_collection):
        linked.objects.unlink(obj)
    collection.objects.link(obj)

output_dir.mkdir(parents=True, exist_ok=True)
blend_path = output_dir / "VibeCon_FullJoyCons_working.blend"
left_path = output_dir / "joycon-left.full.glb"
right_path = output_dir / "joycon-right.full.glb"

bpy.ops.wm.save_as_mainfile(filepath=str(blend_path))

def export_one(obj, path):
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
    )


export_one(left_obj, left_path)
export_one(right_obj, right_path)
print(
    "VIBECON_FULL_JOYCONS_READY "
    f"left_vertices={len(left_obj.data.vertices)} "
    f"right_vertices={len(right_obj.data.vertices)} "
    f"left={left_path} right={right_path}"
)
