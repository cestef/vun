//! Documents rendered at run time and baked at compile time agree, and escape what they must.

use crate::{Doctype, Each, Raw, Render, Text, attr, html, tag, template};

template! {
    static PAGE = {
        doctype
        html [lang = "en"] {
            head { meta [charset = "utf-8"] title { _ } }
            body {
                a [href = _, class = "link"] { "go & see" }
                input [r#type = "text", required]
                p { "count: " _ }
            }
        }
    }
}

template! {
    static STILL = { p { "no holes" } }
}

#[test]
fn a_template_bakes_its_text_and_fills_its_holes_in_order() {
    let page = PAGE.html(&("A <title>", "/x?a=1&b=\"2\"", 42));
    let expected = concat!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>A &lt;title&gt;</title></head>",
        "<body><a href=\"/x?a=1&amp;b=&quot;2&quot;\" class=\"link\">go &amp; see</a>",
        "<input type=\"text\" required><p>count: 42</p></body></html>",
    );
    assert_eq!(page, expected);
}

#[test]
fn a_template_without_holes_renders_as_its_text() {
    assert_eq!(Render::html(&STILL), "<p>no holes</p>");
    assert_eq!(STILL.text(), "<p>no holes</p>");
}

#[test]
fn run_time_elements_render_as_the_baked_document_does() {
    let title = "A <title>";
    let page = (
        Doctype,
        tag::html.attrs([attr::lang.is("en")]).with((
            tag::head.with((tag::meta.attrs([attr::charset.is("utf-8")]), tag::title.with(title))),
            tag::body.with((
                tag::a.attrs([attr::href.is("/x?a=1&b=\"2\""), attr::class.is("link")]).with("go & see"),
                tag::input.attrs([attr::r#type.is("text"), attr::required.on()]),
                tag::p.with(("count: ", 42)),
            )),
        )),
    );
    assert_eq!(page.html(), PAGE.html(&(title, "/x?a=1&b=\"2\"", 42)));
}

#[test]
fn text_escapes_and_raw_does_not() {
    let both = (Text("<b>"), Raw::new("<b>"), Each(["x", "&"].iter()));
    assert_eq!(both.html(), "&lt;b&gt;<b>x&amp;");
}

#[test]
fn a_document_measures_what_it_bakes() {
    const DOC: crate::Doc = html! { p [class = "a\"b"] { "1 < 2" _ } br };
    assert_eq!(DOC.len(), "<p class=\"a&quot;b\">1 &lt; 2</p><br>".len());
    assert_eq!(DOC.holes(), 1);
}
