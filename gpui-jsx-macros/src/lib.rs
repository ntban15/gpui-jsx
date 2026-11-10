//! `jsx!` — a minimal JSX-like proc macro for [GPUI](https://gpui.rs).
//!
//! It expands XML-ish syntax into GPUI's fluent builder calls:
//!
//! ```ignore
//! jsx! {
//!     <div class="flex flex-row gap-3" id="root" on_click={|this, ev, window, cx| { /* ... */ }}>
//!         "hello"
//!     </div>
//! }
//! ```
//!
//! expands roughly to:
//!
//! ```ignore
//! ::gpui::div().flex().flex_row().gap_3().id("root").on_click(cx.listener(|this, ev, window, cx| { /* ... */ }))
//!     .extend_children([...])
//! ```
//!
//! Rules (see the `gpui-jsx` crate docs for the full table):
//!
//! * lowercase tag `<div>` -> `::gpui::div()`
//! * `class="flex gap-3"` -> `.flex().gap_3()` — the ONLY way to apply styling,
//!   like Tailwind's `className` in React (kebab-case maps to snake_case methods)
//! * `id="name"` / `id={expr}` (alias `key=`) -> `.id(...)` (makes the element stateful)
//! * `on_click={|this, ev, window, cx| ...}` -> `.on_click(cx.listener(...))`
//!   (auto-wrapped when the value starts with a closure; passing an already-wrapped
//!   expression such as `cx.listener(...)` also works)
//! * any other `name={expr}` / `name="str"` -> `.name(expr)` — type-checked against
//!   GPUI's builder methods, so unknown attribute names are compile errors
//! * `hover={|style| style.bg(...)}` -> `.hover(|style| ...)` — GPUI's `hover` closure
//!   takes precedence over `class=` for state-dependent styling
//! * bare attributes (`flex`, `gap-3`) are a compile error with a pointer to `class=`
//! * children: nested elements, `"string literals"`, and `{expressions}`; expressions
//!   are converted with `IntoChild` and may be elements, `Vec<element>`s, `Option`s
//!   or `()` (numbers render as text)

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Ident, LitStr, Token,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    token,
};

struct Element {
    tag: Ident,
    attrs: Vec<Attr>,
    children: Vec<Child>,
}

#[derive(Debug)]
enum Attr {
    /// `class="flex gap-3"` -> `.flex().gap_3()`
    Class(Vec<String>),
    /// any other `name=expr` -> `.name(expr)` (name already snake_cased by the parser)
    Value(Ident, TokenStream2),
}

enum Child {
    Element(Element),
    Expr(TokenStream2),
    Text(String),
}

fn snake_from_kebab(parts: &[String]) -> String {
    parts.join("_")
}

impl Parse for Element {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _lt: Token![<] = input.parse()?;
        let tag = Ident::parse_any(input)?;
        if tag.to_string().chars().next().unwrap().is_uppercase() {
            return Err(syn::Error::new(
                tag.span(),
                "component tags are not supported yet; use lowercase gpui element tags like `div`",
            ));
        }

        // Attributes until `>` or `/>`
        let mut attrs = Vec::new();
        loop {
            if input.peek(Token![/]) {
                input.parse::<Token![/]>()?;
                input.parse::<Token![>]>()?;
                return Ok(Element { tag, attrs, children: vec![] });
            }
            if input.peek(Token![>]) {
                input.parse::<Token![>]>()?;
                break;
            }
            attrs.push(parse_attr(input)?);
        }

        // Children until `</tag>`
        let mut children = Vec::new();
        loop {
            if input.peek(Token![<]) && input.peek2(Token![/]) {
                input.parse::<Token![<]>()?;
                input.parse::<Token![/]>()?;
                let closing: Ident = Ident::parse_any(input)?;
                if closing != tag {
                    return Err(syn::Error::new(
                        closing.span(),
                        format!("mismatched closing tag: expected `</{tag}>`"),
                    ));
                }
                input.parse::<Token![>]>()?;
                break;
            }
            if input.peek(Token![<]) {
                children.push(Child::Element(input.parse()?));
                continue;
            }
            if input.peek(token::Brace) {
                let content;
                syn::braced!(content in input);
                let expr: TokenStream2 = content.parse()?;
                children.push(Child::Expr(expr));
                continue;
            }
            if input.peek(LitStr) {
                let text: LitStr = input.parse()?;
                children.push(Child::Text(text.value()));
                continue;
            }
            return Err(input.error(
                "expected a child element, `{expr}`, a \"string literal\", or the closing tag; \
                 wrap raw text in quotes or braces",
            ));
        }

        Ok(Element { tag, attrs, children })
    }
}

fn parse_attr(input: ParseStream) -> syn::Result<Attr> {
    // Attribute name: ident (`-` (ident | number))*
    let first = Ident::parse_any(input)?;
    let mut parts = vec![first.to_string()];
    while input.peek(Token![-]) {
        input.parse::<Token![-]>()?;
        if input.peek(syn::Lit) {
            let lit: syn::Lit = input.parse()?;
            let piece = match &lit {
                // `3xl` is ONE literal with suffix `xl` (not Int(3) + Ident(xl))
                syn::Lit::Int(i) => format!("{}{}", i.base10_digits(), i.suffix()),
                syn::Lit::Float(f) => f.base10_digits().replace('.', "_"),
                other => {
                    return Err(syn::Error::new(
                        other.to_token_stream_span(),
                        "unsupported literal in attribute name",
                    ))
                }
            };
            parts.push(piece);
        } else {
            parts.push(Ident::parse_any(input)?.to_string());
        }
    }

    let has_value = input.peek(Token![=]);

    // `class="flex gap-3"` special case (must be a plain string).
    if parts == ["class"] {
        if !has_value {
            return Err(syn::Error::new(
                first.span(),
                "`class` requires a string, e.g. class=\"flex gap-3\"",
            ));
        }
        input.parse::<Token![=]>()?;
        let lit: LitStr = input.parse()?;
        let names = lit
            .value()
            .split_whitespace()
            .map(|s| s.replace('-', "_"))
            .collect::<Vec<_>>();
        for name in &names {
            if !name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') {
                return Err(syn::Error::new(
                    lit.span(),
                    format!("unsupported class name `{name}` (must start with a letter or `_`)"),
                ));
            }
            for c in name.chars() {
                if !c.is_alphanumeric() && c != '_' {
                    return Err(syn::Error::new(
                        lit.span(),
                        format!("unsupported class name `{name}` (must be an ident)"),
                    ));
                }
            }
        }
        return Ok(Attr::Class(names));
    }

    if has_value {
        input.parse::<Token![=]>()?;
        let name = if parts.len() > 1 {
            Ident::new(&snake_from_kebab(&parts), first.span())
        } else {
            first
        };
        let value = parse_attr_value(input)?;
        return Ok(Attr::Value(name, value));
    }

    // Bare attributes (`<div flex>` / `<div gap-3>`) are deliberately rejected —
    // styling goes through `class="..."` only, like React's `className`.
    Err(syn::Error::new(
        first.span(),
        format!(
            "bare attribute `{}` is not supported; use class=\"{}\" for styling \
             (e.g. class=\"flex gap-3\"), or name={{expr}} for a valued attribute",
            snake_from_kebab(&parts),
            snake_from_kebab(&parts),
        ),
    ))
}

/// `attr={expr}` or `attr="literal"` (or a bare number).
fn parse_attr_value(input: ParseStream) -> syn::Result<TokenStream2> {
    if input.peek(token::Brace) {
        let content;
        syn::braced!(content in input);
        return content.parse::<TokenStream2>();
    }
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        return Ok(quote! { #lit });
    }
    // Allow `attr=42`.
    if input.peek(syn::Lit) {
        let lit: syn::Lit = input.parse()?;
        return Ok(quote! { #lit });
    }
    Err(input.error("expected `{expr}` or a string literal after `=`"))
}

/// Does this token stream start with a closure (`|...` or `move |...`)?
fn looks_like_closure(tokens: &TokenStream2) -> bool {
    let mut iter = tokens.clone().into_iter();
    match iter.next() {
        Some(proc_macro2::TokenTree::Punct(p)) if p.as_char() == '|' => true,
        Some(proc_macro2::TokenTree::Ident(id)) if id == "move" => true,
        _ => false,
    }
}

trait LitSpan {
    fn to_token_stream_span(&self) -> proc_macro2::Span;
}
impl LitSpan for syn::Lit {
    fn to_token_stream_span(&self) -> proc_macro2::Span {
        use quote::ToTokens;
        self.to_token_stream().into_iter().next().unwrap().span()
    }
}

fn expand_attr(attr: &Attr) -> TokenStream2 {
    match attr {
        Attr::Class(names) => {
            let mut out = TokenStream2::new();
            for name in names {
                let ident = Ident::new(name, proc_macro2::Span::call_site());
                out.extend(quote! { .#ident() });
            }
            out
        }
        Attr::Value(name, value) => {
            let method = name.to_string();
            // `id` / `key` -> ElementId (stateful interactivity).
            if method == "id" || method == "key" {
                return quote! { .id(#value) };
            }
            // Event handlers: `on_click={|this, ev, window, cx| ...}` binds
            // `cx.listener(...)` automatically when the value is a closure.
            if method.starts_with("on_") && looks_like_closure(value) {
                return quote! { .#name(cx.listener(#value)) };
            }
            quote! { .#name(#value) }
        }
    }
}

fn expand_child(child: &Child) -> TokenStream2 {
    match child {
        Child::Element(el) => {
            let inner = expand_element(el);
            quote! { ::gpui_jsx::IntoChild::into_any_child(#inner) }
        }
        Child::Expr(expr) => quote! { ::gpui_jsx::IntoChild::into_any_child(#expr) },
        Child::Text(text) => quote! { ::gpui_jsx::IntoChild::into_any_child(#text) },
    }
}

fn expand_element(el: &Element) -> TokenStream2 {
    let tag = &el.tag;
    // Lowercase tag -> gpui element constructor (e.g. `gpui::div()`).
    let mut tokens = quote! { ::gpui::#tag() };

    for attr in &el.attrs {
        tokens.extend(expand_attr(attr));
    }

    if !el.children.is_empty() {
        let child_calls = el.children.iter().map(expand_child);
        tokens = quote! {
            ::gpui_jsx::extend_children(
                #tokens,
                {
                    let mut __children: ::std::vec::Vec<::gpui::AnyElement> = ::std::vec::Vec::new();
                    #(__children.extend(#child_calls);)*
                    __children
                },
            )
        };
    }

    tokens
}

#[proc_macro]
pub fn jsx(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Element);
    expand_element(&parsed).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(input: &str) -> Result<Vec<Attr>, syn::Error> {
        // wrap the attribute list in a fake element
        let src = format!("<div {input} />");
        syn::parse_str::<Element>(&src).map(|el| el.attrs)
    }

    #[test]
    fn bare_attribute_is_rejected_with_pointer_to_class() {
        let err = attrs("flex").unwrap_err().to_string();
        assert!(err.contains("class=\"flex\""), "unexpected message: {err}");
    }

    #[test]
    fn bare_kebab_attribute_is_rejected() {
        let err = attrs("gap-3").unwrap_err().to_string();
        assert!(err.contains("class=\"gap_3\""), "unexpected message: {err}");
    }

    #[test]
    fn class_kebab_names_become_snake_case() {
        let attrs = attrs("class=\"flex flex-col gap-3 text-3xl overflow-y-scroll border-1\"").unwrap();
        match &attrs[..] {
            [Attr::Class(names)] => {
                assert_eq!(names, &["flex", "flex_col", "gap_3", "text_3xl", "overflow_y_scroll", "border_1"]);
            }
            other => panic!("expected single Class attr, got {other:?}"),
        }
    }

    #[test]
    fn class_rejects_bad_idents() {
        assert!(attrs("class=\"gap 3\"").is_err());
        assert!(attrs("class=\"gap_3!\"").is_err());
    }

    #[test]
    fn valued_attrs_still_work() {
        let attrs = attrs(r#"id="root" on_click={|this, ev, window, cx| {}} bg={rgb(1)}"#).unwrap();
        assert!(matches!(attrs.as_slice(), [Attr::Value(a, _), Attr::Value(b, _), Attr::Value(c, _)]
            if a == "id" && b == "on_click" && c == "bg"));
    }
}
