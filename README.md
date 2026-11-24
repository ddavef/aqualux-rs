# aqualux-rs

A cross-platform Rust reimplementation of Aqualux, a pipe-puzzle game from ~2010.

This repository contains zero copyrighted assets. You must supply your own *Aqualux Deluxe* `resource.n` or `aqualux.swf` (or an extracted asset directory) to load all sprites, layered scene backgrounds, embedded SWF vector fonts (`NEUROPOL`, `Pricedown`, `Helvetica`), sound effects, music, and all 100 campaign levels.

## Extra Features

- Higher FPS and smoother animations.
- Match mode and mirror match mode are both available when unlocked (in the original mirror match mode replaces match mode).
- Custom levels and level editor.
- Endless game mode.

## Building

Requires a Rust toolchain (`rustc` + `cargo` supporting Edition 2024). Pure-Rust GPU rendering (`wgpu` + `winit`) and audio (`rodio` + `symphonia`) require no external SDKs on Windows/macOS and use standard Vulkan/Wayland/X11/ALSA on Linux.

```bash
cargo build --release
```

## Running

```bash
cargo run --release -- --assets /path/to/resource.n
cargo run --release -- --assets /path/to/resource.n --fps 120
cargo run --release -- --assets /path/to/resource.n --extract-assets ./assets
cargo run --release -- --verify-assets --assets /path/to/resource.n
```
