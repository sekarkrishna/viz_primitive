// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Headless export example — renders a scene to RGBA and writes to stdout.
//!
//! Run with: `cargo run --example headless_export -p dr2d --features headless`
//!
//! The output is raw RGBA pixel data (width × height × 4 bytes).
//! Pipe to a file or image tool:
//!   cargo run --example headless_export -p dr2d --features headless > output.raw

#[cfg(feature = "headless")]
fn main() {
    use std::io::Write;

    use dr2d::HeadlessRenderer;

    let width = 256u32;
    let height = 256u32;

    let mut renderer = pollster::block_on(HeadlessRenderer::new()).expect("Failed to create headless renderer");

    let pixels = pollster::block_on(renderer.render_to_image(width, height))
        .expect("Failed to render image");

    assert_eq!(pixels.len(), (width * height * 4) as usize);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(&pixels).expect("Failed to write pixel data");

    eprintln!(
        "Wrote {}×{} RGBA image ({} bytes) to stdout",
        width,
        height,
        pixels.len()
    );
}

#[cfg(not(feature = "headless"))]
fn main() {
    eprintln!("This example requires the `headless` feature.");
    eprintln!("Run with: cargo run --example headless_export -p dr2d --features headless");
    std::process::exit(1);
}
