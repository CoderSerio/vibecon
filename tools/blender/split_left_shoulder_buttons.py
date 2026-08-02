"""Split L and ZL from the two largest dark connected shoulder regions."""
import os
from pathlib import Path
from collections import defaultdict, deque
import bpy

path = Path(os.environ["VIBECON_INTERACTIVE_BLEND"]).resolve()
body = bpy.data.objects["SM_JoyCon_L_Body"]
uv = body.data.uv_layers["LJUV"]
image = bpy.data.images["L_Colour.png"]
w, h = image.size
pixels = list(image.pixels)
def sample(u, v):
    x=max(0,min(w-1,int((u%1)*w))); y=max(0,min(h-1,int((v%1)*h)))
    i=(y*w+x)*4
    return tuple(pixels[i+j] for j in range(3))

candidate=set()
for p in body.data.polygons:
    c=body.matrix_world @ p.center
    if not (-0.19<=c.x<=0.04 and -1.36<=c.y<=-1.13 and 0.52<=c.z<=0.647):
        continue
    coords=[uv.data[i].uv for i in p.loop_indices]
    u=sum(v.x for v in coords)/len(coords); v=sum(v.y for v in coords)/len(coords)
    if max(sample(u,v))<0.45: candidate.add(p.index)

edge_faces=defaultdict(list)
for index in candidate:
    for edge in body.data.polygons[index].edge_keys: edge_faces[edge].append(index)
neighbors=defaultdict(set)
for group in edge_faces.values():
    for index in group: neighbors[index].update(i for i in group if i!=index)
remaining=set(candidate); components=[]
while remaining:
    seed=remaining.pop(); component={seed}; queue=deque([seed])
    while queue:
        current=queue.popleft()
        for neighbor in neighbors[current]:
            if neighbor in remaining:
                remaining.remove(neighbor); component.add(neighbor); queue.append(neighbor)
    components.append(component)
major=sorted(components,key=len,reverse=True)[:2]
if [len(c) for c in major] != [60,38]:
    raise RuntimeError(f"Unexpected shoulder components: {[len(c) for c in major]}")

selected=major[0] | major[1]
for p in body.data.polygons: p.select=p.index in selected
bpy.ops.object.select_all(action="DESELECT"); body.select_set(True); bpy.context.view_layer.objects.active=body
bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.separate(type="SELECTED"); bpy.ops.object.mode_set(mode="OBJECT")
combined=[o for o in bpy.context.selected_objects if o!=body and o.type=="MESH"]
if len(combined)!=1: raise RuntimeError(f"Expected combined shoulder object, got {len(combined)}")
shoulders=combined[0]; bpy.context.view_layer.objects.active=shoulders
bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.separate(type="LOOSE"); bpy.ops.object.mode_set(mode="OBJECT")
parts=[o for o in bpy.context.selected_objects if o!=body and o.type=="MESH"]
if len(parts)!=2: raise RuntimeError(f"Expected two loose shoulder parts, got {len(parts)}")

def avg_world_x(obj):
    return sum((obj.matrix_world @ v.co).x for v in obj.data.vertices)/len(obj.data.vertices)
parts.sort(key=avg_world_x,reverse=True)
for obj, name in zip(parts,("L","ZL")):
    obj.name=f"SM_JoyCon_left_Button_{name}"
    obj.data.name=f"GEO_JoyCon_left_Button_{name}"
    if "LJUV" not in obj.data.uv_layers: raise RuntimeError(f"{name} lost LJUV")
    obj.data.uv_layers.active=obj.data.uv_layers["LJUV"]
bpy.ops.wm.save_as_mainfile(filepath=str(path))
print(f"VIBECON_LEFT_SHOULDERS_READY L_faces={len(parts[0].data.polygons)} ZL_faces={len(parts[1].data.polygons)}")
