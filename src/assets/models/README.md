# Local Joy-Con prototypes

`joycon-left.interactive.glb` and `joycon-right.interactive.glb` are the current
local development assets. They are extracted from the disconnected left/right
mesh islands inside the downloaded `FullSwitch.fbx`; the middle console island
is intentionally excluded. Named button and stick meshes are split in the
development Blender file so Three.js can animate their real geometry. The
older `*.full.glb` and `*.prototype.glb` files are retained only as reversible
references while the 3D preview is being validated.

All binary assets are deliberately ignored by Git until their redistribution
terms are verified. Do not ship or publish them in a release without a license
review.

The Blender preparation scripts live in [`tools/blender`](../../../tools/blender),
especially `prepare_full_switch_joycons.py` for the current extraction route.
