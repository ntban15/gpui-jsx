//! A TODO app written in `jsx!` — the same "look" as the README example,
//! but the markup is JSX-like instead of chained builder calls:
//!
//! ```ignore
//! jsx! {
//!     <div flex flex-col gap-3>
//!         <div text-xl>"todos"</div>
//!     </div>
//! }
//! ```
//!
//! Build and run:  cargo run -p todo

use gpui::{
    App, Bounds, ClickEvent, Context, FocusHandle, Focusable, KeyDownEvent, Render, Window,
    WindowBounds, WindowOptions, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use gpui_jsx::jsx;

// ---------- state ----------

#[derive(Clone)]
struct Todo {
    id: u64,
    text: String,
    done: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Active,
    Done,
}

struct TodoApp {
    focus_handle: FocusHandle,
    draft: String,
    todos: Vec<Todo>,
    next_id: u64,
    filter: Filter,
}

impl TodoApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let todos = vec![
            Todo { id: 1, text: "Buy groceries".into(), done: true },
            Todo { id: 2, text: "Ship gpui-jsx".into(), done: false },
            Todo { id: 3, text: "Explore GPUI internals".into(), done: false },
        ];
        Self {
            focus_handle: cx.focus_handle(),
            draft: String::new(),
            todos,
            next_id: 4,
            filter: Filter::All,
        }
    }

    fn visible_todos(&self) -> Vec<(u64, String, bool)> {
        self.todos
            .iter()
            .filter(|todo| match self.filter {
                Filter::All => true,
                Filter::Active => !todo.done,
                Filter::Done => todo.done,
            })
            .map(|todo| (todo.id, todo.text.clone(), todo.done))
            .collect()
    }

    fn remaining(&self) -> usize {
        self.todos.iter().filter(|todo| !todo.done).count()
    }

    fn add_draft(&mut self, cx: &mut Context<Self>) {
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.todos.push(Todo { id: self.next_id, text, done: false });
        self.next_id += 1;
        self.draft.clear();
        cx.notify();
    }

    fn toggle(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(todo) = self.todos.iter_mut().find(|todo| todo.id == id) {
            todo.done = !todo.done;
            cx.notify();
        }
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        self.todos.retain(|todo| todo.id != id);
        cx.notify();
    }

    fn clear_completed(&mut self, cx: &mut Context<Self>) {
        self.todos.retain(|todo| !todo.done);
        cx.notify();
    }
}

impl Focusable for TodoApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ---------- view ----------

impl Render for TodoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.visible_todos();
        let remaining = self.remaining();
        let done_count = self.todos.len() - remaining;
        let draft = self.draft.clone();
        let focus_handle = self.focus_handle.clone();

        jsx! {
            <div
                size-full
                flex
                flex-col
                px-6
                py-5
                gap-4
                bg={rgb(0x1e1e2e)}
                text_color={rgb(0xcdd6f4)}
            >
                // header
                <div flex flex-col>
                    <div text-3xl>"todos"</div>
                    <div text-color={rgb(0x6c7086)} text-sm>
                        {format!("{remaining} active, {done_count} done")}
                    </div>
                </div>

                // input row
                <div
                    id="new-todo"
                    track_focus={&focus_handle}
                    on_click={move |_this, _event: &ClickEvent, window, cx| window.focus(&focus_handle, cx)}
                    on_key_down={|this, event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.modifiers.platform || event.keystroke.modifiers.control {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "enter" => this.add_draft(cx),
                            "escape" => {
                                this.draft.clear();
                                cx.notify();
                            }
                            "backspace" => {
                                this.draft.pop();
                                cx.notify();
                            }
                            _ => {
                                if let Some(typed) = &event.keystroke.key_char {
                                    this.draft.push_str(typed);
                                    cx.notify();
                                }
                            }
                        }
                    }}
                    flex
                    items-center
                    gap-3
                    w-full
                    h-11
                    px-4
                    rounded-md
                    bg={rgb(0x313244)}
                    border_1
                    border_color={rgb(0x45475a)}
                >
                    <div text-color={rgb(0xa6e3a1)}>"▸"</div>
                    {if draft.is_empty() {
                        "Type a todo, press Enter…".to_string()
                    } else {
                        draft.clone()
                    }}
                </div>

                // list
                <div id="todo-list" flex flex-col flex-1 overflow-y-scroll gap-2>
                    {rows.iter().map(|(id, text, done)| {
                        let id = *id;
                        let text = text.clone();
                        jsx! {
                            <div
                                id={format!("todo-{id}")}
                                on_click={move |this, _event: &ClickEvent, _window, cx| this.toggle(id, cx)}
                                flex
                                items-center
                                gap-3
                                w-full
                                px-4
                                py-2
                                rounded-md
                                bg={rgb(0x313244)}
                                hover={|style| style.bg(rgb(0x45475a))}
                            >
                                <div
                                    flex
                                    items-center
                                    justify-center
                                    size-5
                                    rounded-md
                                    border_1
                                    border_color={if *done { rgb(0xa6e3a1) } else { rgb(0x6c7086) }}
                                    bg={if *done { rgb(0xa6e3a1) } else { rgb(0x313244) }}
                                    text_color={rgb(0x1e1e2e)}
                                >
                                    {(*done).then(|| "✓".to_string())}
                                </div>
                            {if *done {
                                jsx! { <div class="flex-1 line-through" text_color={rgb(0x6c7086)}>{text.clone()}</div> }
                            } else {
                                jsx! { <div class="flex-1" text_color={rgb(0xcdd6f4)}>{text.clone()}</div> }
                            }}
                                <div
                                    id={format!("delete-{id}")}
                                    on_click={move |this, _event: &ClickEvent, _window, cx| {
                                        cx.stop_propagation();
                                        this.remove(id, cx);
                                    }}
                                    flex
                                    items-center
                                    justify-center
                                    size-6
                                    rounded-md
                                    text_color={rgb(0x6c7086)}
                                    hover={|style| style.bg(rgb(0x45475a))}
                                >
                                    "✕"
                                </div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                // footer: filters + clear completed
                <div flex items-center justify-between gap-3>
                    <div flex gap-2>
                        {[
                            (Filter::All, "all"),
                            (Filter::Active, "active"),
                            (Filter::Done, "done"),
                        ]
                        .into_iter()
                        .map(|(filter, label)| {
                            let selected = self.filter == filter;
                            jsx! {
                                <div
                                    id={format!("filter-{label}")}
                                    on_click={move |this, _event: &ClickEvent, _window, cx| {
                                        this.filter = filter;
                                        cx.notify();
                                    }}
                                    px-3
                                    py-1
                                    rounded-md
                                    text-sm
                                    bg={if selected { rgb(0x89b4fa) } else { rgb(0x313244) }}
                                    text_color={if selected { rgb(0x1e1e2e) } else { rgb(0xcdd6f4) }}
                                >
                                    {label.to_string()}
                                </div>
                            }
                        })
                        .collect::<Vec<_>>()}
                    </div>
                    {(done_count > 0).then(|| {
                        jsx! {
                            <div
                                id="clear-completed"
                                on_click={|this, _event: &ClickEvent, _window, cx| this.clear_completed(cx)}
                                px-3
                                py-1
                                rounded-md
                                text-sm
                                text_color={rgb(0x6c7086)}
                                hover={|style| style.text_color(rgb(0xf38ba8))}
                            >
                                "clear completed"
                            </div>
                        }
                    })}
                </div>
            </div>
        }
    }
}

// ---------- entry point ----------

fn main() {
    // Keep gpui's own warnings visible — e.g. "compiled without the font-kit
    // feature, so no text will be rendered" explains a totally blank window.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.), px(640.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| TodoApp::new(cx));
                window.focus(&view.focus_handle(cx), cx);
                view
            },
        )
        .unwrap();
    });
}
