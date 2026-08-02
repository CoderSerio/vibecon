"""Audit VibeCon interactive mesh parts before GLB export."""

import bpy


required = {
    "SM_JoyCon_L_Body": "LJUV",
    "SM_JoyCon_left_StickAssembly": "LJUV",
    "SM_JoyCon_left_Button_SL": "LJUV",
    "SM_JoyCon_left_Button_Minus": "LJUV",
    "SM_JoyCon_left_Button_L": "LJUV",
    "SM_JoyCon_left_Button_ZL": "LJUV",
    "SM_JoyCon_R_Body": "RJUV",
    "SM_JoyCon_right_Button_R": "RJUV",
    "SM_JoyCon_right_Button_ZR": "RJUV",
    "SM_JoyCon_right_Button_SL": "RJUV",
    "SM_JoyCon_right_Button_SR": "RJUV",
    "SM_JoyCon_right_StickAssembly": "RJUV",
}

failed = []
for name, uv_name in required.items():
    obj = bpy.data.objects.get(name)
    if obj is None:
        failed.append(f"missing:{name}")
        continue
    obj.data.calc_loop_triangles()
    triangles = len(obj.data.loop_triangles)
    materials = [slot.material.name if slot.material else "<empty>" for slot in obj.material_slots]
    has_uv = uv_name in obj.data.uv_layers
    used_vertices = {
        vertex_index
        for polygon in obj.data.polygons
        for vertex_index in polygon.vertices
    }
    loose_vertices = len(obj.data.vertices) - len(used_vertices)
    boundary_edges = sum(1 for edge in obj.data.edges if edge.is_loose)
    print(
        "VIBECON_ASSET_AUDIT "
        f"name={name} vertices={len(obj.data.vertices)} "
        f"faces={len(obj.data.polygons)} triangles={triangles} "
        f"materials={materials} uv={uv_name}:{has_uv} "
        f"loose_vertices={loose_vertices} loose_edges={boundary_edges}"
    )
    if not has_uv:
        failed.append(f"uv:{name}:{uv_name}")
    if not materials or "<empty>" in materials:
        failed.append(f"material:{name}")
    if loose_vertices:
        failed.append(f"loose_vertices:{name}:{loose_vertices}")

if failed:
    raise RuntimeError("Interactive asset audit failed: " + ", ".join(failed))

print("VIBECON_ASSET_AUDIT_PASS")
