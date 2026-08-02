"""Report face-depth distributions around known front controls."""

import bpy
from mathutils import Vector


REGIONS = {
    "SM_JoyCon_L_Body": {
        "StickCap": (0.0, 0.36, 0.15),
        "DPad_Up": (0.0, -0.02, 0.085),
        "DPad_Right": (-0.12, -0.11, 0.085),
        "DPad_Down": (0.0, -0.20, 0.085),
        "DPad_Left": (0.12, -0.11, 0.085),
    },
    "SM_JoyCon_R_Body": {
        "Button_X": (0.0, 0.37, 0.085),
        "Button_A": (-0.12, 0.28, 0.085),
        "Button_B": (0.0, 0.17, 0.085),
        "Button_Y": (0.12, 0.28, 0.085),
        "StickCap": (0.0, -0.03, 0.15),
    },
}


for object_name, regions in REGIONS.items():
    obj = bpy.data.objects.get(object_name)
    if obj is None:
        continue
    center_y = sum((obj.matrix_world @ Vector(corner)).y for corner in obj.bound_box) / 8
    print(f"VIBECON_REGION_OBJECT name={object_name} center_y={center_y:.5f}")
    for name, (horizontal, vertical, radius) in regions.items():
        target_y = center_y + horizontal
        candidates = []
        for polygon in obj.data.polygons:
            center = obj.matrix_world @ polygon.center
            radial = ((center.y - target_y) ** 2 + (center.z - vertical) ** 2) ** 0.5
            if radial <= radius:
                normal = obj.matrix_world.to_3x3() @ polygon.normal
                candidates.append((center.x, normal.x, polygon.index))
        depths = sorted(item[0] for item in candidates)
        front_facing = sum(1 for _, normal_x, _ in candidates if normal_x > 0.25)
        if not depths:
            print(f"REGION name={name} faces=0")
            continue
        quantiles = [depths[int((len(depths) - 1) * fraction)] for fraction in (0, 0.25, 0.5, 0.75, 1)]
        print(
            f"REGION name={name} faces={len(candidates)} front={front_facing} "
            f"xq=({','.join(f'{value:.5f}' for value in quantiles)})"
        )
