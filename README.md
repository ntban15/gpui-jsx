# gpui-jsx

**macOS prerequisite:** GPUI needs the Metal Toolchain to build.
If `gpui_apple` fails with `cannot execute tool 'metal'`, install it:

```
xcodebuild -downloadComponent MetalToolchain
```

then `cargo run -p todo`.

JSX-like syntax for [GPUI](https://gpui.rs) — a thin proc macro that expands
to GPUI's fluent builder calls, so everything stays type-checked by rustc.

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    jsx! {
        <div flex flex-col gap-3 size-full>
            <div text-xl>"hi"</div>
        </div>
    }
}
```

expands to

```rust
::gpui::div().flex().flex_col().gap_3().size_full()
    .jsx_children(["hi".into_any_child()].concat())
```

* GPUI on crates.io is an abandoned unrelated package; `zed`'s GPUI is pulled
  in from git, pinned to a known-good rev.
* `gpui-jsx` — the runtime half: re-exports `jsx!`, plus the `IntoChild`
  trait (what `{expr}` children may be) and the `JsxChildrenExt::jsx_children`
  builder extension used by the expansion.
* `gpui-jsx-macros` — the proc macro itself.
* `todo/` — a demo app written in the JSX syntax. `cargo run -p todo`

## Crate boundaries

The macro emits `::gpui::…` and `::gpui_jsx::…` paths directly. Attribute
names become method calls (`gap-3` → `.gap_3()`), so rustc validates them.

## Syntax

| JSX | Expansion |
|---|---|
| `<div flex>` | `::gpui::div().flex()` |
| `<div gap-3 />` | `.gap_3()` |
| `<div class="flex gap-3">` | `.flex().gap_3()` |
| `<div bg={rgb(0x505050)}>` | `.bg(rgb(0x505050))` |
| `<div id="x" />` / `key="x"` | `.id("x")` — stateful element |
| `<div track_focus={&self.focus_handle}>` | `.track_focus(&self.focus_handle)` |
| `<div on_click={\|this, ev, window, cx\| …}>` | `.on_click(cx.listener(\|this, ev, window, cx\| …))` |
| `<div>"text"</div>` | text child |
| `<div>{expr}</div>` | `expr` via `IntoChild` (elements, `Vec`/`Option` of elements, numbers, strings) |
| `<div>{items.iter().map(\|x\| jsx! {…}).collect::<Vec<_>>()}</div>` | flattened children |
| `<div>{cond.then(\|\| jsx! {…})}</div>` | conditional children |

Raw text must be in quotes or braces. See `gpui-jsx/src/lib.rs` for the full table.
