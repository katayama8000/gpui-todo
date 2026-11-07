mod text_input;

use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, MouseButton,
    SharedString, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use text_input::{Submit, TextInput};

actions!(todo, [Quit]);

#[derive(Clone)]
struct Todo {
    id: usize,
    title: SharedString,
    done: bool,
}

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
        match self {
            Filter::All => true,
            Filter::Active => !todo.done,
            Filter::Completed => todo.done,
        }
    }
}

struct TodoApp {
    /// In-memory storage (lost when the app quits)
    todos: Vec<Todo>,
    next_id: usize,
    filter: Filter,
    input: Entity<TextInput>,
}

impl TodoApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("What needs to be done? Press Enter…", cx));
        cx.subscribe(&input, |this, input, _: &Submit, cx| {
            let title = input.read(cx).text();
            this.add(title, cx);
            input.update(cx, |input, cx| input.reset(cx));
        })
        .detach();

        Self {
            todos: Vec::new(),
            next_id: 0,
            filter: Filter::All,
            input,
        }
    }

    fn add(&mut self, title: SharedString, cx: &mut Context<Self>) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        self.todos.push(Todo {
            id: self.next_id,
            title: title.to_string().into(),
            done: false,
        });
        self.next_id += 1;
        cx.notify();
    }

    fn toggle(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            todo.done = !todo.done;
            cx.notify();
        }
    }

    fn remove(&mut self, id: usize, cx: &mut Context<Self>) {
        self.todos.retain(|t| t.id != id);
        cx.notify();
    }

    fn clear_completed(&mut self, cx: &mut Context<Self>) {
        self.todos.retain(|t| !t.done);
        cx.notify();
    }

    fn render_item(&self, todo: &Todo, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let id = todo.id;
        let checkbox = div()
            .id(("check", id))
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
            .id(("delete", id))
            .px_2()
            .rounded_md()
            .text_color(rgb(0xc0c0c0))
            .cursor_pointer()
            .hover(|s| s.text_color(rgb(0xe5484d)).bg(rgb(0xfdecec)))
            .child("✕")
            .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx)));

        div()
            .id(("todo", id))
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
        let remaining = self.todos.iter().filter(|t| !t.done).count();
        let has_completed = self.todos.iter().any(|t| t.done);
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
                .when(selected, |d| d.border_color(rgb(0x3b82f6)).text_color(rgb(0x3b82f6)))
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
                .child(if self.todos.is_empty() {
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
                            .child(format!("{remaining} item{} left", if remaining == 1 { "" } else { "s" }))
                            .child(div().flex().gap_1().children(filters))
                            .child(
                                div()
                                    .id("clear")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .when(has_completed, |d| {
                                        d.cursor_pointer()
                                            .hover(|s| s.bg(rgb(0xf0f0f0)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.clear_completed(cx)
                                            }))
                                    })
                                    .when(!has_completed, |d| d.text_color(rgb(0xd0d0d0)))
                                    .child("Clear completed"),
                            ),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
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
                |_, cx| cx.new(TodoApp::new),
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
