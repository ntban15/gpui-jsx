# gpui-jsx

**macOS prerequisites:**

1. GPUI needs the Metal Toolchain to build.
   If `gpui_apple` fails with `cannot execute tool 'metal'`, install it:

   ```
   xcodebuild -downloadComponent MetalToolchain
   ```

2. GPUI needs `font-kit` to render text — it's an opt-in feature on
   `gpui_platform` (`font-kit = ["gpui_macos/font-kit"]`). Without it the app
   opens, layout works, and **no text appears anywhere** (only a
   `WARN gpui_macos::platform: gpui_macos was compiled without the `font-kit`
   feature, so no text will be rendered.` is emitted, which you'll never see
   without a logger). The `examples` package enables it. If something looks
   text-free in your fork, check this first and init `env_logger` in `main`.

then `cargo run -p examples --example todo`.

JSX-like syntax for [GPUI](https://gpui.rs) — a thin proc macro that expands
to GPUI's fluent builder calls, so everything stays type-checked by rustc.

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    jsx! {
        <div class="flex flex-col gap-3 size-full">
            <div class="text-xl">"hi"</div>
        </div>
    }
}
```

expands to

```rust
::gpui_jsx::extend_children(::gpui::div().flex().flex_col().gap_3().size_full(), vec![
    ::gpui_jsx::IntoChild::into_any_child("hi"),
])
```

* GPUI on crates.io is an abandoned unrelated package; `zed`'s GPUI is pulled
  in from git, pinned to a known-good rev.
* `gpui-jsx` — the runtime half: re-exports `jsx!`, plus the `IntoChild`
  trait (what `{expr}` children may be) and `extend_children`, used by the
  expansion.
* `gpui-jsx-macros` — the proc macro itself.
* `examples/` — demo apps written in the JSX syntax; one file per example under `examples/examples/` (cargo auto-discovery). `cargo run -p examples --example todo`

## Crate boundaries

The macro emits `::gpui::…` and `::gpui_jsx::…` paths directly. Styling class
names become method calls (`gap-3` → `.gap_3()`), so rustc validates them.

## Syntax

**Styling goes through `class=` only** (like React's `className` with Tailwind);
bare attributes such as `<div flex>` are a compile error with a pointer to
`class=`. Valued styles (`bg={...}`) remain explicit attributes, like React's
`style={{...}}`.

| JSX | Expansion |
|---|---|
| `<div class="flex flex-col gap-3">` | `.flex().flex_col().gap_3()` (kebab → snake_case) |
| `<div bg={rgb(0x505050)}>` | `.bg(rgb(0x505050))` |
| `<div id="x" />` / `key="x"` | `.id("x")` — stateful element |
| `<div track_focus={&self.focus_handle}>` | `.track_focus(&self.focus_handle)` |
| `<div hover={\|s\| s.bg(rgb(0x313244))}>` | `.hover(\|s\| ...)` — state-dependent styling |
| `<div on_click={\|this, ev, window, cx\| …}>` | `.on_click(cx.listener(\|this, ev, window, cx\| …))` |
| `<div>"text"</div>` | text child |
| `<div>{expr}</div>` | `expr` via `IntoChild` (elements, `Vec`/`Option` of elements, numbers, strings) |
| `<div>{items.iter().map(\|x\| jsx! {…}).collect::<Vec<_>>()}</div>` | flattened children |
| `<div>{cond.then(\|\| jsx! {…})}</div>` | conditional children |

Raw text must be in quotes or braces. See `gpui-jsx/src/lib.rs` for the full table.
