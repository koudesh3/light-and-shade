# light and shade

A native studio for cutting paintings into layers, painting normals, and lighting them.
Rust, `wgpu` (Metal, Vulkan, DX12) and `egui`.

`projects/<name>/` holds one painting. `reference/pothos/` holds the browser tools' source and
Python pipeline this was ported from.

![The studio showing the pothos](docs/studio.png)

## Run

```sh
cargo run -p light-and-shade        # from the repo
./scripts/bundle-mac.sh             # builds target/Light and Shade.app
```

The Rust version is pinned in `rust-toolchain.toml`. The app reads paintings from `LAS_PROJECTS`, or
from the nearest `projects/` folder.

Drag the painting or the ball to move the light. Scroll or pinch to zoom, hold Space and drag to pan,
and double-click or press F to fit. Keys 1, 2 and 3 switch views, and O orbits the light.

## Layout

| Crate | Holds |
|---|---|
| `crates/core` | Project format and view maths. No GPU or UI. |
| `crates/render` | The `wgpu` renderer and its WGSL shader. |
| `crates/app` | The `egui` studio. OS-specific code stays in `platform.rs`. |

## Capturing the app

These run the real app and need no OS permissions. `LAS_SHOT` saves the window and exits.

```sh
LAS_SHOT=shots/a.png LAS_MODE=normals LAS_ZOOM=2 LAS_LIGHT_X=1.4 cargo run -p light-and-shade
LAS_SHOT=shots/b.png LAS_DRAG=300,500,1000,200 LAS_SCROLL=4 cargo run -p light-and-shade
```
