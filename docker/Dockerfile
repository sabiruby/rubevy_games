# Build and run the games on WSL2 (WSLg) without touching the host: the image carries the Rust
# toolchain, Bevy's build dependencies and a software Vulkan driver (Mesa's lavapipe), and the
# window goes to the Windows desktop through WSLg.
FROM rust:1-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    # build
    pkg-config g++ \
    libx11-dev libxkbcommon-dev libwayland-dev \
    # run: X11/Wayland, GL and Vulkan (lavapipe is the software driver)
    libx11-6 libxcursor1 libxrandr2 libxi6 libxinerama1 libxkbcommon0 libxkbcommon-x11-0 \
    libwayland-client0 libwayland-cursor0 libwayland-egl1 \
    libgl1-mesa-dri libegl1 libgbm1 libgles2 \
    libvulkan1 mesa-vulkan-drivers vulkan-tools \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# lavapipe: no GPU needed, and the same picture everywhere
ENV WGPU_BACKEND=vulkan \
    VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
    CARGO_TARGET_DIR=/target

WORKDIR /app
