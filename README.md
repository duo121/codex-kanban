# codex-kanban

`codex-kanban` is a public fork of [openai/codex](https://github.com/openai/codex) that adds a board-based multi-session workflow on top of the official Codex CLI.

The project keeps the official chat experience as intact as possible and layers kanban-style session management on top of it instead of rebuilding the whole TUI.

## Install and run

Install from npm:

```bash
npm install -g @duo121/codex-kanban
```

Start with either command:

```bash
codexkb
# or
codex-kanban
```

Homebrew support:

- Recommended:

```bash
brew tap duo121/homebrew-codex-kanban
brew install --cask codex-kanban
```

- `brew install --cask codex-kanban` without a tap only works after the cask is accepted into `homebrew/cask`.

## Why this fork exists

The official Codex CLI is optimized around one terminal window per active session.

That model breaks down when you want to drive many concurrent tasks at once:

- too many terminal windows
- fragmented session state
- hard to see which session is still running
- high context-switch cost when following up

`codex-kanban` keeps the official interaction model on the right side and adds lightweight board/session management so one Codex window can manage a small working set of sessions.

## Design principles

- Reuse the official Codex UI wherever possible
- Keep `/resume` semantics unchanged
- Avoid introducing a permanent custom sidebar
- Minimize fork drift so upstream sync stays manageable
- Treat boards as an organizational layer, not a new transcript store

## Current MVP scope

The current implementation focuses on a low-drift kanban workflow:

- `/kb` opens the board picker
- `~` opens the current board's session picker
- create, rename, delete, and switch boards
- create, rename, remove, search, and reorder sessions inside a board
- board-local session state tracking:
  - `running`
  - `needs attention`
  - `seen`
  - `waiting approval`
  - `errored`
- persistence for boards, board sessions, ordering, and status snapshots
- mirrored behavior in both `codex-tui` and `codex-tui-app-server`

Removing a session from a board does not delete the underlying Codex thread. It remains available through the normal `/resume` flow.

## Interaction model

### `/kb`

Global board entry point:

- search boards
- `n` create board
- `r` rename board
- `d` delete board with confirmation
- `Enter` bind the current window to the selected board

### `~`

Current board session entry point:

- search sessions in the current board
- `n` create a new session inside the bound board
- `r` rename the selected board session
- `d` remove it from the board with confirmation
- `Shift+Up` / `Shift+Down` reorder sessions
- `Enter` switch the active chat area to that session

## Architecture notes

This fork intentionally keeps most changes in isolated modules:

- `codex-rs/tui/src/app/boards.rs`
- `codex-rs/tui/src/app/board_state_sync.rs`
- `codex-rs/tui_app_server/src/app/boards.rs`
- `codex-rs/tui_app_server/src/app/board_state_sync.rs`
- `codex-rs/state/src/runtime/boards.rs`
- `codex-rs/core/src/boards.rs`

That structure is deliberate so future upstream sync work stays focused and predictable.

## Building from source

Follow the upstream install guide first:

- [Installing and building](./docs/install.md)

Then run the Rust workspace locally:

```bash
cd codex-rs
cargo run -p codex-cli --bin codex
```

## Project docs

- [Slash commands](./docs/slash_commands.md)
- [Product spec](./docs/codex-kanban-product-spec.md)
- [Vercel docs-site deployment](./docs/vercel-deploy.md)
- [Release deploy checklist](./docs/release-deploy.md)
- [Contributing](./docs/contributing.md)
- [Installing & building](./docs/install.md)

## Upstream sync strategy

The repository keeps both remotes:

- `origin` -> `duo121/codex-kanban`
- `upstream` -> `openai/codex`

Recommended maintenance flow:

1. Fetch upstream regularly.
2. Rebase or merge upstream into `main`.
3. Keep kanban changes isolated to board-related modules and minimal app wiring.
4. Mirror TUI behavior in both local and app-server variants.

## Vercel note

Vercel is appropriate for a project website or documentation site for `codex-kanban`.

It is not a runtime target for the Rust TUI itself. The CLI and TUI still run locally on the user's machine.

## License

This repository remains under the [Apache-2.0 License](LICENSE).
