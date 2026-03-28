---
name: docs-engineering
description: Audit and update all project documentation to stay in sync with the current development status.
---

When performing documentation engineering, always follow these steps:

1. **Survey recent changes** by running `git log --oneline -20` and skimming the diff of recent commits. This surfaces new features, removed dependencies, and behavioral changes that documentation may not yet reflect.

2. **Audit** all documentation against the current codebase and development status. The review scope must include — without exception:
   - `README.md` — features list, prerequisites, acknowledgements
   - `CHANGELOG.md` — release notes and version history
   - `CLAUDE.md` — stack, architecture, key gotchas, project conventions
   - `doc/development.md`, `doc/user-guide.md`, `doc/api.md`, `doc/troubleshooting.md`, `doc/TODOs.md` (skip files that don't exist yet)
   - Code comments for human developers

3. **Revise and update** any documentation that is stale, incomplete, or inconsistent with the current code. Ensure new features, removed dependencies, behavioral changes, and architectural decisions are reflected accurately.

4. **Remove completed items** from `doc/TODOs.md`. If a summary of completed work is warranted, add a brief note before removing the items.

5. **Commit** documentation changes using the `commit-and-push` skill, grouped by topic. Do not mix unrelated documentation changes in a single commit.
