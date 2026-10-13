# Desktop UI Rules

- Use the existing GPUI 0.2 entity, action, and render patterns.
- Keep `WorkbenchState` as the shared shell and editor state until a distinct
  feature requires a separate logical store.
- Call controllers from event handlers; do not bypass the client boundary.
- Keep synchronous controller work short and surface operation errors in the
  workbench status until background task infrastructure is introduced.
- Clear stale notices/errors when a new mutation starts.
- Disable conflicting actions while a mutation is active.
- Preserve keyboard and accessibility behavior, including labels, status roles,
  alert roles, busy states, focus, and confirmation text.
- Add reusable colors and visual constants to `theme.rs`.
