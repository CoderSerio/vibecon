"""List disconnected mesh islands to identify interactive Joy-Con parts."""

from collections import deque

import bpy
from mathutils import Vector


for obj in bpy.context.scene.objects:
    if obj.type != "MESH":
        continue

    mesh = obj.data
    adjacency = [set() for _ in mesh.vertices]
    for edge in mesh.edges:
        a, b = edge.vertices
        adjacency[a].add(b)
        adjacency[b].add(a)

    unseen = set(range(len(mesh.vertices)))
    islands = []
    while unseen:
        root = unseen.pop()
        queue = deque([root])
        indices = [root]
        while queue:
            current = queue.popleft()
            for neighbor in adjacency[current]:
                if neighbor not in unseen:
                    continue
                unseen.remove(neighbor)
                queue.append(neighbor)
                indices.append(neighbor)

        world = [obj.matrix_world @ mesh.vertices[index].co for index in indices]
        minimum = Vector((min(v.x for v in world), min(v.y for v in world), min(v.z for v in world)))
        maximum = Vector((max(v.x for v in world), max(v.y for v in world), max(v.z for v in world)))
        islands.append((len(indices), (minimum + maximum) / 2, maximum - minimum))

    print(f"VIBECON_ISLANDS object={obj.name} count={len(islands)}")
    for rank, (count, center, size) in enumerate(sorted(islands, reverse=True), start=1):
        print(
            f"ISLAND rank={rank:02d} vertices={count:04d} "
            f"center=({center.x:.4f},{center.y:.4f},{center.z:.4f}) "
            f"size=({size.x:.4f},{size.y:.4f},{size.z:.4f})"
        )
