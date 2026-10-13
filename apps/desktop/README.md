# PolyGlid Desktop

The desktop crate is the native GPUI editor-first security workbench:

- workspace file navigation with traversal-safe client controllers;
- document tabs, editable buffers, dirty state, and `Ctrl+S` persistence;
- projects, scanner targets, executions, reports, and plugins backed by the
  application snapshot;
- command palette and keyboard-driven shell controls.

Run it from the repository root:

```bash
cargo run -p polyglid-desktop
```

Use `nix develop` first on Linux so GPUI receives the pinned Wayland, Vulkan,
Mesa, font, and keyboard libraries.
