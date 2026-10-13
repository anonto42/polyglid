# Agent Startup

Use this loader before implementation, diagnosis, or review.

## Load

1. Read `memory/MEMORY.md`.
2. Check the current branch and dirty worktree.
3. Read `scopes/README.md`, then only the scope for the code being touched.
4. Read `rules/README.md`, then only matching rule files.
5. Read `context/architecture.md` when work crosses UI, client, core, storage,
   runtime, or contract boundaries.
6. Read today's ignored `history/YYYY-MM-DD/caveman.md` only when it exists and
   continuation context is needed. Use Asia/Dhaka for the date.

## Hard Boundaries

- The active product UI is the GPUI editor workbench. Do not restore the removed
  Dioxus desktop or add a second shipped desktop UI stack.
- Product areas must be connected to real client controllers; do not present
  scanner, execution, report, plugin, marketplace, or settings controls as
  working when only a visual shell exists.
- `apps/` contains runnable products; reusable logic belongs in `crates/`.
  Crates must not depend on an app.
- Preserve explicit confirmation and direct-child validation for permanent
  project-folder deletion.
- Preserve least-privilege capability checks, plugin signature verification,
  Wasmtime limits, and safe output-path validation.
- Propagate failures to the desktop UI with useful messages.
- Do not introduce unrelated files, features, or refactors.

## Work Loop

- Inspect targeted code with `rg` and focused reads.
- Preserve unrelated user changes.
- Prefer existing files unless the task introduces a real logical component.
- Verify narrowly first, then run workspace checks when shared contracts change.
- Update stable memory only when a decision or fact is validated and likely to
  help future work.
- Put temporary continuation notes under the ignored daily history folder; do
  not commit routine session transcripts.
