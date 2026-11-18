mod crypto;
mod sync;
mod text_input;

use std::{sync::Arc, time::Duration};

use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, MouseButton,
    SharedString, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use sync::{SyncClient, Todo, merge, now_ms};
use text_input::{Submit, TextInput};

actions!(todo, [Quit]);

const SYNC_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Active,
    Completed,
}

impl Filter {
    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Active => "Active",
            Filter::Completed => "Completed",
        }
    }

    fn matches(self, todo: &Todo) -> bool {
        if todo.deleted {
            return false;
        }
        match self {
            Filter::All => true,
            Filter::Active => !todo.done,
            Filter::Completed => todo.done,
        }
    }
}

struct TodoApp {
    /// In-memory storage. With sync enabled, the encrypted server copy is the
    /// durable one; deleted todos stay here as tombstones.
    todos: Vec<Todo>,
    filter: Filter,
    input: Entity<TextInput>,
    sync_status: SharedString,
}

impl TodoApp {
    fn new(sync: Option<Arc<SyncClient>>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("What needs to be done? Press Enter…", cx));
        cx.subscribe(&input, |this, input, _: &Submit, cx| {
            let title = input.read(cx).text();
            this.add(title, cx);
            input.update(cx, |input, cx| input.reset(cx));
        })
        .detach();

        let sync_status = match sync {
            Some(client) => {
                let status = format!(
                    "E2EE sync · vault {}… · connecting",
                    &client.vault_id()[..8]
                );
                Self::start_sync(client, cx);
                status
            }
            None => "Local only · set TODO_PASSPHRASE to enable E2EE sync".to_string(),
        };

        Self {
            todos: Vec::new(),
            filter: Filter::All,
            input,
            sync_status: sync_status.into(),
        }
    }

    /// Periodically merges local and remote state. Encryption, decryption and
    /// HTTP all happen on a background thread.
    fn start_sync(client: Arc<SyncClient>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(local) = this.read_with(cx, |this, _| this.todos.clone()) else {
                    break;
                };
                let worker = client.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { worker.sync(&local) })
                    .await;
                let vault = &client.vault_id()[..8];
                let updated = this.update(cx, |this, cx| {
                    match result {
                        Ok(outcome) => {
                            // Keep edits made while the request was in flight.
                            this.todos = merge(&this.todos, &outcome.todos);
                            this.sync_status =
                                format!("E2EE sync · vault {vault}… · v{}", outcome.version).into();
                        }
                        Err(err) => {
                            this.sync_status =
                                format!("E2EE sync · vault {vault}… · error: {err}").into();
                        }
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
                cx.background_executor().timer(SYNC_INTERVAL).await;
            }
        })
        .detach();
    }

    fn touch(todo: &mut Todo) {
        todo.updated_at = now_ms().max(todo.updated_at + 1);
    }

    fn add(&mut self, title: SharedString, cx: &mut Context<Self>) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let now = now_ms();
        self.todos.push(Todo {
            // Random ids so todos created on different clients never collide.
            id: rand::random::<u64>() >> 1,
            title: title.to_string(),
            done: false,
            deleted: false,
            created_at: now,
            updated_at: now,
        });
        cx.notify();
    }

    fn toggle(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            todo.done = !todo.done;
            Self::touch(todo);
            cx.notify();
        }
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            todo.deleted = true;
            Self::touch(todo);
            cx.notify();
        }
    }

    fn clear_completed(&mut self, cx: &mut Context<Self>) {
        for todo in self.todos.iter_mut().filter(|t| t.done && !t.deleted) {
            todo.deleted = true;
            Self::touch(todo);
        }
        cx.notify();
    }

    fn render_item(&self, todo: &Todo, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let id = todo.id;
        let element_id = id as usize;
        let checkbox = div()
            .id(("check", element_id))
            .size(px(20.))
            .flex_none()
            .rounded_full()
            .border_2()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .cursor_pointer()
            .when(todo.done, |d| {
                d.bg(rgb(0x22a06b))
                    .border_color(rgb(0x22a06b))
                    .text_color(rgb(0xffffff))
                    .child("✓")
            })
            .when(!todo.done, |d| {
                d.border_color(rgb(0xb0b0b0))
                    .hover(|s| s.border_color(rgb(0x22a06b)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.toggle(id, cx)));

        let title = div()
            .flex_1()
            .text_size(px(15.))
            .child(todo.title.clone())
            .when(todo.done, |d| d.line_through().text_color(rgb(0xa0a0a0)));

        let delete = div()
            .id(("delete", element_id))
            .px_2()
            .rounded_md()
            .text_color(rgb(0xc0c0c0))
            .cursor_pointer()
            .hover(|s| s.text_color(rgb(0xe5484d)).bg(rgb(0xfdecec)))
            .child("✕")
            .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx)));

        div()
            .id(("todo", element_id))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(rgb(0xeeeeee))
            .hover(|s| s.bg(rgb(0xfafafa)))
            .child(checkbox)
            .child(title)
            .child(delete)
    }
}

impl Focusable for TodoApp {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for TodoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let remaining = self.todos.iter().filter(|t| !t.done && !t.deleted).count();
        let has_completed = self.todos.iter().any(|t| t.done && !t.deleted);
        let has_any = self.todos.iter().any(|t| !t.deleted);
        let visible: Vec<Todo> = self
            .todos
            .iter()
            .filter(|t| self.filter.matches(t))
            .cloned()
            .collect();

        let filters = [Filter::All, Filter::Active, Filter::Completed].map(|f| {
            let selected = self.filter == f;
            div()
                .id(f.label())
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .when(selected, |d| {
                    d.border_color(rgb(0x3b82f6)).text_color(rgb(0x3b82f6))
                })
                .when(!selected, |d| {
                    d.border_color(rgb(0xffffff))
                        .hover(|s| s.border_color(rgb(0xdddddd)))
                })
                .child(f.label())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter = f;
                    cx.notify();
                }))
        });

        let list = if visible.is_empty() {
            div()
                .id("list")
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xb0b0b0))
                .child(if !has_any {
                    "No todos yet"
                } else {
                    "No matching todos"
                })
        } else {
            let items: Vec<_> = visible.iter().map(|t| self.render_item(t, cx)).collect();
            div()
                .id("list")
                .flex_1()
                .overflow_y_scroll()
                .children(items)
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f5f7))
            .text_color(rgb(0x1f2328))
            .p_6()
            .gap_4()
            .child(
                div()
                    .text_size(px(28.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("Todo"),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(self.input.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .bg(rgb(0xffffff))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(0xe3e3e3))
                    .overflow_hidden()
                    .child(list)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_2()
                            .border_t_1()
                            .border_color(rgb(0xeeeeee))
                            .text_size(px(13.))
                            .text_color(rgb(0x6b6f76))
                            .child(format!(
                                "{remaining} item{} left",
                                if remaining == 1 { "" } else { "s" }
                            ))
                            .child(div().flex().gap_1().children(filters))
                            .child(
                                div()
                                    .id("clear")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .when(has_completed, |d| {
                                        d.cursor_pointer().hover(|s| s.bg(rgb(0xf0f0f0))).on_click(
                                            cx.listener(|this, _, _, cx| this.clear_completed(cx)),
                                        )
                                    })
                                    .when(!has_completed, |d| d.text_color(rgb(0xd0d0d0)))
                                    .child("Clear completed"),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x6b6f76))
                    .child(self.sync_status.clone()),
            )
    }
}

fn sync_client_from_env() -> Option<Arc<SyncClient>> {
    let passphrase = std::env::var("TODO_PASSPHRASE")
        .ok()
        .filter(|p| !p.is_empty())?;
    let vault = std::env::var("TODO_VAULT").unwrap_or_else(|_| "default".into());
    let server = std::env::var("TODO_SYNC_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
    let key = crypto::VaultKey::derive(&vault, &passphrase);
    eprintln!(
        "E2EE sync: vault \"{vault}\" -> id {} on {server}",
        key.vault_id()
    );
    Some(Arc::new(SyncClient::new(&server, key)))
}

fn main() {
    let sync = sync_client_from_env();
    Application::new().run(move |cx: &mut App| {
        text_input::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(520.), px(640.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| TodoApp::new(sync, cx)),
            )
            .unwrap();

        window
            .update(cx, |view, window, cx| {
                window.focus(&view.focus_handle(cx));
                cx.activate(true);
            })
            .unwrap();
    });
}
