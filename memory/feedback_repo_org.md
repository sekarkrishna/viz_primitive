---
name: Repo organization — monorepo decision
description: User confirmed monorepo in viz_primitive as the right structure for the viz stack
type: feedback
---

Use monorepo in viz_primitive for all viz_primitive packages. Do not suggest splitting into separate repos.

**Why:** The user explicitly chose monorepo after evaluating 3 options (dr2d-only repo, viz_primitive monorepo, fully separate repos). Tight coupling between Rust core → Python binding → chart library makes separate repos add friction with no benefit.

**How to apply:** When suggesting project structure or CI changes, always think in terms of the monorepo. Cross-package changes (e.g. dr2d API change + justviz update) should be a single PR.

The legacy sekarkrishna/dr2d repo will eventually be archived. Don't recommend using it for development.
