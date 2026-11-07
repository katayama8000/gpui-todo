# gpui-todo

A simple todo app built with [GPUI](https://www.gpui.rs/), the GPU-accelerated UI framework from [Zed](https://zed.dev/).

Todos are kept in memory only — they are lost when the app quits.

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
