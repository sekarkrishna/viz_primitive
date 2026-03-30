---
name: viz_primitive ecosystem
description: Monorepo structure, package details, publishing setup, and phase status for viz_primitive
type: project
---

Monorepo at sekarkrishna/viz_primitive with three packages:
- `dr2d-rust/` → crates.io as `dr2d` (Apache 2.0) — GPU 2D renderer on wgpu, v0.0.1-alpha.1, Phase 0 complete
- `dr2d-python/` → PyPI as `dr2d` (MIT) — PyO3 binding, Phase 1 not started
- `justviz/` → PyPI as `justviz` (MIT) — Python charts, v0.1.0a1 WIP

**Why:** All packages evolve together; monorepo avoids cross-repo version juggling and enables unified docs.

**How to apply:** Always read CLAUDE.md at root for full context before touching any single package. Changes to dr2d-rust may require updates across the chain.

Publishing tags:
- `dr2d-v*` → cargo publish (needs `CRATES_IO_TOKEN` secret)
- `justviz-v*` → uv publish (needs `PYPI_TOKEN` secret)
- `dr2d-py-v*` → maturin publish (needs `PYPI_TOKEN` secret, Phase 1)

Docs: Material for MkDocs, auto-deploys on push to main → git.sekrad.org/viz_primitive

Legacy repo sekarkrishna/dr2d: only has a LICENSE file, will be archived/deleted once viz_primitive is established.
Cargo.toml repository URL already updated to point to viz_primitive.

GitHub Actions workflows:
- docs.yml — push to main → mkdocs gh-deploy
- ci.yml — cargo test + clippy, pytest
- publish-crates.yml — dr2d-v* tag
- publish-pypi.yml — justviz-v* and dr2d-py-v* tags
