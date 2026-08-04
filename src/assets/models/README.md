# Local Joy-Con prototypes

`joycon-left.interactive.glb` and `joycon-right.interactive.glb` are the current
local development assets. They are extracted from the disconnected left/right
mesh islands inside the downloaded `FullSwitch.fbx`; the middle console island
is intentionally excluded. Named button and stick meshes are split in the
development Blender file so Three.js can animate their real geometry. The
older `*.full.glb` and `*.prototype.glb` files are retained only as reversible
references while the 3D preview is being validated.

The detailed local assets are derived from Glaid's
[Nintendo Switch with Detachable Joycons](https://sketchfab.com/3d-models/nintendo-switch-with-detachable-joycons-d2cd14f8e6484978a02290d111578b34),
published under Sketchfab's Free Standard license. That license allows use in
derivative works but forbids giving users access to the model as a stand-alone
file. The source GLBs therefore remain ignored and are never committed or
placed in a public release.

Clean checkouts and packaged builds automatically use the license-safe
procedural Joy-Con assembled in `src/motion/procedural-joycon.ts`. Local
development continues to prefer the detailed GLBs when they are present.

The Blender preparation scripts live in [`tools/blender`](../../../tools/blender),
especially `prepare_full_switch_joycons.py` for the current extraction route.
