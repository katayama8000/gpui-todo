# gpui-todo

A simple todo app built with [GPUI](https://www.gpui.rs/), the GPU-accelerated UI framework from [Zed](https://zed.dev/).

Todos are kept in memory only — they are lost when the app quits, unless you enable the optional [end-to-end encrypted sync](#end-to-end-encrypted-sync).

## Features

- Add a todo by typing and pressing <kbd>Enter</kbd> (blank input is ignored)
- Toggle completion with the round checkbox (completed items are struck through)
- Delete a todo with ✕
- Filter by **All / Active / Completed**
- Remaining item counter and **Clear completed**
- Text input with IME support (e.g. Japanese), selection, copy / cut / paste

## Requirements

- macOS
- Rust (edition 2024, tested with rustc 1.90)

Xcode is **not** required: the `runtime_shaders` feature of `gpui` is enabled, so Metal shaders are compiled at runtime instead of at build time.

## Run

```sh
cargo run
```

### Auto-reload during development

GPUI has no built-in hot reload. Instead, use [watchexec](https://github.com/watchexec/watchexec) to rebuild and restart the app whenever a `.rs` file is saved:

```sh
brew install watchexec
watchexec -r -e rs -- cargo run
```

- `-r` — kill the running app and restart it on change
- `-e rs` — only watch `.rs` files

Since todos live in memory, they are reset on every restart.

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| <kbd>Enter</kbd> | Add todo |
| <kbd>⌘A</kbd> | Select all |
| <kbd>⌘C</kbd> / <kbd>⌘X</kbd> / <kbd>⌘V</kbd> | Copy / Cut / Paste |
| <kbd>←</kbd> / <kbd>→</kbd> (+ <kbd>Shift</kbd>) | Move cursor (extend selection) |
| <kbd>Home</kbd> / <kbd>End</kbd> | Move to start / end |
| <kbd>⌃⌘Space</kbd> | Character palette |
| <kbd>⌘Q</kbd> | Quit |

## Project structure

```
src/
├── main.rs        # TodoApp view, todo state, filters, window setup
└── text_input.rs  # Reusable single-line text input (based on gpui's examples/input.rs)
```

`TextInput` emits a `Submit` event on <kbd>Enter</kbd>; `TodoApp` subscribes to it, adds the todo, and resets the input.

## End-to-end encrypted sync

Optionally, several app instances can share one todo list through a small sync server (`src/bin/server.rs`). The server is treated as untrusted: it only ever stores and returns ciphertext.

The quickest way is the Makefile, which starts the server and two clients that share a vault:

```sh
make dev                                  # Ctrl-C, or closing both windows, stops everything
make dev PASSPHRASE='my secret' VAULT=work
```

Or start each part yourself:

```sh
make server        # terminal 1: the server (in-memory, logs what it receives)
make client        # terminals 2 and 3: clients using the same vault name and passphrase
```

`make help` lists all targets (`test`, `test-sync`, `lint`, …). Without make, run `cargo run --bin server` and `TODO_PASSPHRASE='…' cargo run`.

| Variable | Default | Meaning |
|---|---|---|
| `TODO_PASSPHRASE` | (unset → local only) | Secret shared by your clients. Never sent to the server. |
| `TODO_VAULT` | `default` | Vault name; part of the key derivation salt. |
| `TODO_SYNC_URL` | `http://127.0.0.1:8787` | Server URL used by the clients. |
| `TODO_SERVER_ADDR` | `127.0.0.1:8787` | Address the server binds to. |

How it works (`src/crypto.rs`, `src/sync.rs`):

- **Key derivation**: Argon2id(passphrase, salt = SHA-256(vault name)), then HKDF-SHA256 splits it into a 256-bit encryption key and a vault id. The server only learns the vault id.
- **Encryption**: the whole list is serialized to JSON and sealed with XChaCha20-Poly1305 using a random 192-bit nonce. The vault id is bound as associated data, so a blob cannot be replayed into another vault, and any tampering fails decryption.
- **Sync**: clients poll every second. Each todo carries an `updated_at` timestamp, and deletions are kept as tombstones, so concurrent edits are merged per item on the client (last writer wins). The server only does optimistic concurrency with `If-Match` / `X-Version`.

Run `make test` for the unit tests, and `make test-sync` for a two-client round trip against a temporary server.

Limitations of this experiment:

- The server can still see metadata: vault id, blob size, and when clients write.
- No protection against rollback: a malicious server could serve an older (still authentic) blob.
- No key rotation or per-device keys; anyone with the passphrase has full access, and changing it starts a new, empty vault.
- Last-writer-wins relies on client clocks.
- The server keeps data in memory only and uses plain HTTP; put it behind TLS for anything beyond localhost.
