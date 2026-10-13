# Desktop Scope

## Paths

- Application: `apps/desktop/src/main.rs`, `apps/desktop/src/lib.rs`
- UI root and shell: `apps/desktop/src/workbench.rs`
- Shared UI and editor state: `apps/desktop/src/models.rs`
- Commands: `apps/desktop/src/actions.rs`
- Theme tokens: `apps/desktop/src/theme.rs`

## Boundaries

- GPUI entities render and coordinate user interactions.
- Use `DesktopControllers`; do not access SQLite, filesystem services, or
  Wasmtime directly from a view.
- Represent startup, loading, empty, success, operation, and error states.
- Prevent concurrent project mutations through the operation state.
- Keep permanent deletion visually distinct and explicitly confirmed.
- Persist shell changes through `SettingsController`.

## Current UI Scope

The editor and explorer are active product features. Other product sections
may show real snapshot data but must not advertise unavailable mutations.
