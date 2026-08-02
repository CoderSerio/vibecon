"""Create brighter, cleaner Joy-Con base-color textures without touching originals.

The downloaded model intentionally contains worn and desaturated albedo maps.
This script recolors only chromatic shell regions while preserving dark buttons,
rails and small baked details. Normal/metallic/roughness maps remain unchanged.
"""

import argparse
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter


TARGETS = {
    "L": np.array([0x0A, 0xB9, 0xE6], dtype=np.float32) / 255.0,
    "R": np.array([0xFF, 0x3C, 0x28], dtype=np.float32) / 255.0,
}


def clean_texture(source: Path, destination: Path, side: str) -> None:
    image = Image.open(source).convert("RGBA")
    pixels = np.asarray(image, dtype=np.float32) / 255.0
    rgb = pixels[..., :3]

    maximum = rgb.max(axis=-1)
    minimum = rgb.min(axis=-1)
    chroma = maximum - minimum

    if side == "L":
        hue_mask = (rgb[..., 1] > rgb[..., 0] * 1.08) & (rgb[..., 2] > rgb[..., 0] * 1.12)
    else:
        hue_mask = (rgb[..., 0] > rgb[..., 1] * 1.15) & (rgb[..., 0] > rgb[..., 2] * 1.12)

    raw_mask = hue_mask & (chroma > 0.075) & (maximum > 0.16)
    mask_image = Image.fromarray((raw_mask * 255).astype(np.uint8), mode="L")

    # Close narrow scratch gaps but retain the much larger black button holes.
    mask_image = mask_image.filter(ImageFilter.MaxFilter(11))
    mask_image = mask_image.filter(ImageFilter.MinFilter(11))
    mask_image = mask_image.filter(ImageFilter.GaussianBlur(2.0))
    mask = np.asarray(mask_image, dtype=np.float32) / 255.0

    luminance = rgb @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)
    shell_luminance = luminance[raw_mask]
    median = float(np.median(shell_luminance)) if shell_luminance.size else 0.5
    light_variation = np.clip(1.0 + (luminance - median) * 0.28, 0.9, 1.1)
    clean_rgb = np.clip(TARGETS[side] * light_variation[..., None], 0.0, 1.0)

    # A strong but not total blend keeps subtle baked curvature and edge detail.
    strength = (mask * 0.9)[..., None]
    pixels[..., :3] = rgb * (1.0 - strength) + clean_rgb * strength

    destination.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.round(pixels * 255).astype(np.uint8), mode="RGBA").save(destination)
    print(f"VIBECON_CLEAN_TEXTURE_READY side={side} path={destination}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source_dir", type=Path)
    parser.add_argument("output_dir", type=Path)
    args = parser.parse_args()

    for side in ("L", "R"):
        clean_texture(
            args.source_dir / f"{side}_Colour.png",
            args.output_dir / f"{side}_Colour.png",
            side,
        )


if __name__ == "__main__":
    main()
