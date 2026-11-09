//! `jsx!` — a minimal JSX-like proc macro for [GPUI](https://gpui.rs).
//!
//! It expands XML-ish syntax into GPUI's fluent builder calls:
//!
//! ```ignore
//! jsx! {
//!     <div flex flex-row gap-3 on-click={|this, _ev, _window, cx| { /* ... */ }}>
//!         "hello"
//!     </div>
//! }
//! ```
//!
//! expands roughly to:
//!
//! ```ignore
//! ::gpui::div().flex().flex_row().gap_3().on_click(cx.listener(|this, _ev, _window, cx| { /* ... */ }))
//!     .jsx_children([...].concat())
//! ```
//!
//! Rules (see the `gpui-jsx` crate docs for the full table):
//!
//! * lowercase tag `<div>` -> `::gpui::div()`
//! * bare attribute `flex` -> `.flex()`; kebab-case `gap-3` -> `.gap_3()`
//! * `class="flex gap-3"`  -> `.flex().gap_3()`
//! * `id="name"` / `id={expr}` (alias `key=`) -> `.id(...)` (makes the element stateful)
//! * `on_click={|this, ev, window, cx| ...}` -> `.on_click(cx.listener(...))`
//!   (auto-wrapped when the value starts with a closure; passing an already-wrapped
//!   expression such as `cx.listener(...)` also works)
//! * any other `name={expr}` / `name="str"` -> `.name(expr)` — type-checked against
//!   GPUI's builder methods, so unknown attribute names are compile errors
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

enum Attr {
    /// `flex` -> `.flex()`
    Flag(Ident),
    /// `gap-3` -> `.gap_3()` (name already snake_cased by the parser)
    FlagRaw(String),
    /// any other `name=expr` -> `.name(expr)`
    Value(Ident, TokenStream2),
    /// `class="flex gap-3"` -> `.flex().gap_3()`
    Class(Vec<String>),
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

    if parts.len() > 1 {
        Ok(Attr::FlagRaw(snake_from_kebab(&parts)))
    } else {
        Ok(Attr::Flag(first))
    }
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
        Attr::Flag(name) => quote! { .#name() },
        Attr::FlagRaw(name) => {
            let ident = Ident::new(name, proc_macro2::Span::call_site());
            quote! { .#ident() }
        }
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
