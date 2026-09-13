# Running the games on WSL2

The windowed mode needs a GPU Bevy can use. On WSL2 that is usually not the host itself: WSLg
gives the window, but Vulkan may have no driver installed. The fix that works here — and the one
proven on the author's other Bevy game — is to build and run in a container that carries a
software Vulkan driver (Mesa's lavapipe) and to send the window to the Windows desktop through
WSLg.

```bash
docker/build.sh              # image + cargo build --release -p sabibots
docker/run.sh                # a window on the Windows desktop
docker/run.sh sabibots release --headless 15    # no window, the result on stdout
```

The image (`docker/Dockerfile`) has the Rust toolchain, Bevy's build dependencies, the X11 and
Wayland client libraries winit opens (`libXcursor`, `libXrandr`, `libXi`, `libxkbcommon-x11`, the
Wayland trio) and `mesa-vulkan-drivers`. `docker/run.sh` mounts `/mnt/wslg` and
`/tmp/.X11-unix`, passes `DISPLAY`, `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir`,
and the container's `WGPU_BACKEND=vulkan` plus `VK_ICD_FILENAMES=…lvp_icd…` picks lavapipe.

The cargo registry and the build directory are named volumes (`rubevy-games-cargo`,
`rubevy-games-target`), so the repository stays clean and a second build is fast.

## Without Docker

* **The host has a working Vulkan or GL driver** — `cargo run -p sabibots` is enough, nothing here
  applies.
* **No GPU at all** — `cargo run -p sabibots -- --headless 15` runs the same systems and prints the
  result. That is how the games are checked in this repository.
* **The real GPU** (an NVIDIA card behind WSL) — cross-compiling to
  `x86_64-pc-windows-gnu` and running the `.exe` from WSL uses the native driver. Not set up here;
  worth doing when a game needs the frame rate.

## Why lavapipe is enough for these games

They are 2D, a few dozen sprites, and the interesting work is in the VM. Software rendering keeps
the picture identical on every machine, which is what the `--headless` checks depend on too.
