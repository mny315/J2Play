# Gamepad body artwork

`gamepad-body.png` is project artwork distributed under the repository's MIT
License, as described in the root `NOTICE`.

The project owner supplied `ChatGPT Image Sep 23, 2026, 11_45_00 AM.png` on
2026-09-23 for the physical-controls editor. The supplied image is retained
separately in the working directory; it is not a build input.

- Source SHA-256: `01948a9b6a0d2b4b76837efe29e07cbbfb678224a87e8f07144ca178dbb0a40e`
- Embedded PNG SHA-256: `ff88d29e7d3486bf6611b4af1fbc7a3bba023efe11b03c4f92febd6a3ab827fd`
- Processing: OpenAI's built-in `imagegen` tool, background extraction, 2026-09-23.
- Output: 1774 × 887, RGBA PNG. The application preserves alpha, trims transparent
  padding through texture coordinates, and scales artwork and controls together.

Final edit prompt:

> Use case: background-extraction. Precisely cut out the blank gray game controller silhouette from this image onto a real transparent PNG alpha background. This is a UI asset, preserve the original body and do not add controls, text or marks. Preserve shape, grayscale colors, original smooth white edge highlight, bumps and lighting. Remove the entire blurred background and space between handles. Clean antialiased outline without isolated pixels, colored blue/purple/cyan edge fringes, noise or halo. Keep all the controller, centered with only a small transparent margin on each side, landscape PNG. Every pixel outside the controller must be fully transparent, and the interior must be opaque.
