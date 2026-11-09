//! `gpui-jsx` — JSX-like syntax for [GPUI](https://gpui.rs).
//!
//! ```ignore
//! use gpui_jsx::jsx;
//!
//! fn render(cx: &mut Context<Self>) -> impl IntoElement {
//!     jsx! {
//!         <div flex flex-col gap-3>
//!             <div text-xl>"todos"</div>
//!             {self.todos.iter().map(|todo| jsx! { /* ... */ }).collect::<Vec<_>>()}
//!         </div>
//!     }
//! }
//! ```
//!
//! The macro expands to GPUI's fluent builder calls, so everything is
//! type-checked by rustc: unknown attribute names are compile errors.
//!
//! # Syntax
//!
//! | JSX | Expansion |
//! |---|---|
//! | `<div flex>` | `::gpui::div().flex()` |
//! | `<div gap-3 />` | `.gap_3()` (kebab-case normalizes to snake_case) |
//! | `<div class="flex gap-3">` | `.flex().gap_3()` |
//! | `<div bg={rgb(0x505050)}>` | `.bg(rgb(0x505050))` |
//! | `<div id="x" />` (alias `key=`) | `.id("x")` — makes the element stateful |
//! | `<div track_focus={&self.focus_handle}>` | `.track_focus(&self.focus_handle)` |
//! | `<div on_click={\|this, ev, window, cx\| …}>` | `.on_click(cx.listener(..))` — auto-wrapped when the value is a closure |
//! | `<div>"text"</div>` | text child |
//! | `<div>{expr}</div>` | `expr` converted via [`IntoChild`] |
//! | `<div>{items.collect::<Vec<_>>()}</div>` | flattened into children |
//! | `<div>{cond.then(\|\| jsx! {…})}</div>` | renders when `Some` |
//!
//! Attribute notes:
//! * `overflow_y_scroll` and other stateful-only methods (those defined on
//!   `StatefulInteractiveElement`, e.g. `.overflow_y_scroll()`,
//!   `.on_click(...)`) only compile on `<div id="…">` — GPUI requires a stateful
//!   element there.
//! * `stroke-line-separator` style dashed-name attributes map to snake_case
//!   method names; `text-3xl`-style number-suffixed attributes work too
//!   (`text_3xl`).
//!
//! Raw text must be in quotes (`"label"`) or braces (`{expr}`).
//!
//! # `IntoChild` coverage
//!
//! `{expr}` children must implement [`IntoChild`]. Because GPUI's
//! `IntoElement` is foreign (and blanket-implemented over its `Element`
//! trait), a blanket `IntoChild` impl would block the `Vec`/`Option`
//! conversions every JSX needs, so the crate enumerates the common cases:
//!
//! * `Div`, `Stateful<Div>`, any `T: IntoElement` inside `Vec<_>`/`Option<_>`,
//!   and `()` (renders nothing)
//! * `Entity<V: Render>`, `AnyView` — views as children
//! * `String`, `SharedString`, `Cow<'static, str>`, `&'static str`
//! * primitive numbers and `bool` — rendered as text
//!
//! Embedding a custom element type that isn't listed? Convert it first:
//! `{my_element.into_any_element()}` — [`AnyElement`] implements `IntoChild`.

pub use gpui_jsx_macros::jsx;

use std::borrow::Cow;

use gpui::{
    AnyElement, AnyView, Div, Entity, IntoElement, ParentElement, Render, SharedString, Stateful,
};

/// Consumes a batch of children produced by the macro.
/// Implemented as a free function (rather than an extension method)
/// so expansions need no trait imports at the call site.
pub fn extend_children<P: ParentElement>(mut parent: P, children: Vec<AnyElement>) -> P {
    parent.extend(children);
    parent
}

/// Converts a `{expression}` child into a batch of [`AnyElement`]s.
pub trait IntoChild {
    fn into_any_child(self) -> Vec<AnyElement>;
}

impl IntoChild for AnyElement {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self]
    }
}

impl IntoChild for Div {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

impl IntoChild for Stateful<Div> {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

impl<T: IntoElement> IntoChild for Vec<T> {
    fn into_any_child(self) -> Vec<AnyElement> {
        self.into_iter().map(|el| el.into_any_element()).collect()
    }
}

impl<T: IntoElement> IntoChild for Option<T> {
    fn into_any_child(self) -> Vec<AnyElement> {
        self.into_iter().map(|el| el.into_any_element()).collect()
    }
}

impl IntoChild for () {
    fn into_any_child(self) -> Vec<AnyElement> {
        Vec::new()
    }
}

impl IntoChild for String {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

impl IntoChild for SharedString {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

impl IntoChild for Cow<'static, str> {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

impl IntoChild for &'static str {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.to_string().into_any_element()]
    }
}

impl<V: Render> IntoChild for Entity<V> {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

impl IntoChild for AnyView {
    fn into_any_child(self) -> Vec<AnyElement> {
        vec![self.into_any_element()]
    }
}

macro_rules! impl_display_child {
    ($($ty:ty),*) => {
        $(
            impl IntoChild for $ty {
                fn into_any_child(self) -> Vec<AnyElement> {
                    vec![self.to_string().into_any_element()]
                }
            }
        )*
    };
}

impl_display_child!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, isize, usize, f32, f64, bool);
