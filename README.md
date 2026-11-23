# aqualux-rs

A cross-platform Rust reimplementation of Aqualux, a pipe-puzzle game from ~2010.

This repository contains zero copyrighted assets. You must supply your own *Aqualux Deluxe* `resource.n` or `aqualux.swf` (or an extracted asset directory) to load all sprites, layered scene backgrounds, embedded SWF vector fonts (`NEUROPOL`, `Pricedown`, `Helvetica`), sound effects, music, and all 100 campaign levels.

## Extra Features

- Higher FPS and smoother animations.
- Match mode and mirror match mode are both available when unlocked (in the original mirror match mode replaces match mode).
- Custom levels and level editor.

## Building

Requires a Rust toolchain (`rustc` + `cargo` supporting Edition 2024). Pure-Rust GPU rendering (`wgpu` + `winit`) and audio (`rodio` + `symphonia`) require no external SDKs on Windows/macOS and use standard Vulkan/Wayland/X11/ALSA on Linux.

```bash
cargo build --release
```

## Module Layout

- `src/assets/`: Neko archive extraction (`neko.rs`), extracted directory loading (`dir_loader.rs`), SWF bitmap/sound/XML parser (`swf.rs`), shape/sprite flattener (`swf_sprite.rs`), vector glyph parser (`swf_font.rs`)
- `src/game/`: Tile & port model (`types.rs`, `tile.rs`), score/trophy metrics & Match generator (`scoring.rs`, `awards.rs`), XML level loader (`levels.rs`), flow path tracer (`path.rs`), Campaign & Match simulation (`level_sim.rs`, `level_step.rs`, `match_sim.rs`)
- `src/render/`: `wgpu` device & frame execution (`gpu_core.rs`, `gpu_pipelines.rs`, `gpu_types.rs`, `gpu_bake.rs`, `gpu_text.rs`, `shaders/*.wgsl`), 4x5 `ColorMatrix` (`color_matrix.rs`), GPU command canvas (`canvas.rs`), GPU-baked sprite & glow bank (`tex_bank.rs`), native-DPI SWF vector font renderer (`text.rs`), parallax scenes (`backgrounds.rs`, `bg_sprites.rs`), board & liquid mask renderer (`board.rs`, `board_underlay.rs`, `pipe_mask.rs`), rainbow/fireworks/leaks (`effects.rs`, `emitter.rs`, `particles.rs`)
- `src/ui/`: Screens, modals, HUD, actions, and widgets (`splash.rs`, `title.rs`, `hud.rs`, `modals.rs`, `modal_actions.rs`, `summary.rs`, `campaign_end.rs`, `champion.rs`, `trophy_room.rs`, `actions.rs`, `flows.rs`, `render.rs`, `widgets.rs`)
- `src/window.rs`, `src/audio.rs`, `src/save.rs`, `src/main.rs`

## Running

```bash
cargo run --release -- --assets /path/to/resource.n
cargo run --release -- --assets /path/to/resource.n --fps 120
cargo run --release -- --assets /path/to/resource.n --extract-assets ./assets
cargo run --release -- --verify-assets --assets /path/to/resource.n
```
