"""Split R and ZR from the two largest dark connected shoulder regions."""
import os
from pathlib import Path
from collections import defaultdict, deque
import bpy

path=Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).resolve(); body=bpy.data.objects["SM_JoyCon_R_Body"]
uv=body.data.uv_layers["RJUV"]; image=bpy.data.images["R_Colour.png"]; w,h=image.size; pixels=list(image.pixels)
def sample(u,v):
    x=max(0,min(w-1,int((u%1)*w))); y=max(0,min(h-1,int((v%1)*h))); i=(y*w+x)*4
    return tuple(pixels[i+j] for j in range(3))
candidate=set()
for p in body.data.polygons:
    c=body.matrix_world @ p.center
    if not (-0.19<=c.x<=0.04 and 1.13<=c.y<=1.36 and 0.52<=c.z<=0.647): continue
    coords=[uv.data[i].uv for i in p.loop_indices]; u=sum(v.x for v in coords)/len(coords); v=sum(v.y for v in coords)/len(coords)
    if max(sample(u,v))<0.45: candidate.add(p.index)
edge_faces=defaultdict(list)
for i in candidate:
    for e in body.data.polygons[i].edge_keys: edge_faces[e].append(i)
neighbors=defaultdict(set)
for group in edge_faces.values():
    for i in group: neighbors[i].update(j for j in group if j!=i)
remaining=set(candidate); components=[]
while remaining:
    seed=remaining.pop(); comp={seed}; queue=deque([seed])
    while queue:
        cur=queue.popleft()
        for nxt in neighbors[cur]:
            if nxt in remaining: remaining.remove(nxt); comp.add(nxt); queue.append(nxt)
    components.append(comp)
major=sorted(components,key=len,reverse=True)[:2]
if [len(c) for c in major] != [59,38]: raise RuntimeError(f"Unexpected components {[len(c) for c in major]}")
selected=major[0]|major[1]
for p in body.data.polygons: p.select=p.index in selected
bpy.ops.object.select_all(action="DESELECT"); body.select_set(True); bpy.context.view_layer.objects.active=body
bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.separate(type="SELECTED"); bpy.ops.object.mode_set(mode="OBJECT")
combined=[o for o in bpy.context.selected_objects if o!=body and o.type=="MESH"]
if len(combined)!=1: raise RuntimeError(f"Expected one combined object, got {len(combined)}")
shoulders=combined[0]; bpy.context.view_layer.objects.active=shoulders
bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.separate(type="LOOSE"); bpy.ops.object.mode_set(mode="OBJECT")
parts=[o for o in bpy.context.selected_objects if o!=body and o.type=="MESH"]
if len(parts)!=2: raise RuntimeError(f"Expected two parts, got {len(parts)}")
def avg_x(o): return sum((o.matrix_world@v.co).x for v in o.data.vertices)/len(o.data.vertices)
parts.sort(key=avg_x,reverse=True)
for obj,name in zip(parts,("R","ZR")):
    obj.name=f"SM_JoyCon_right_Button_{name}"; obj.data.name=f"GEO_JoyCon_right_Button_{name}"
    if "RJUV" not in obj.data.uv_layers: raise RuntimeError(f"{name} lost RJUV")
    obj.data.uv_layers.active=obj.data.uv_layers["RJUV"]
bpy.ops.wm.save_as_mainfile(filepath=str(path))
print(f"VIBECON_RIGHT_SHOULDERS_READY R_faces={len(parts[0].data.polygons)} ZR_faces={len(parts[1].data.polygons)}")
