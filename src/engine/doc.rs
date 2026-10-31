//! From element tree to Lunar's document model: a list of blocks (headings,
//! paragraphs, lists, tables...) made of words, links and form fields. This
//! is built once per page; laying it out for a window width is layout.rs.

use super::html::{Element, Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Text,
    Bold,
    Italic,
    Code,
    Heading,
    Link,
    Dim,
    Field,
    Button,
}

#[derive(Debug, Clone)]
pub enum Inline {
    /// A run of text; whitespace in it is collapsed when laid out.
    Text { text: String, style: Style, link: Option<String> },
    /// A form control, by index into `Doc::fields`.
    Field(usize),
    Break,
}

#[derive(Debug, Clone)]
pub enum Block {
    /// A paragraph (<p>): blank lines around it.
    Para(Vec<Inline>),
    /// Text in any other box (<div>...): no blank lines around it.
    Lines(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    /// Preformatted: whitespace and line breaks kept.
    Pre(Vec<Inline>),
    Rule,
    /// A blank line, around boxes that group several blocks (one search
    /// result, one card...). Repeated gaps collapse into one.
    Gap,
    List { ordered: bool, items: Vec<Vec<Block>> },
    Quote(Vec<Block>),
    Table(Vec<Vec<Vec<Inline>>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Text,
    Password,
    Hidden,
    Checkbox,
    Radio,
    Submit,
    Select(Vec<(String, String)>),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub kind: FieldKind,
    pub name: String,
    pub value: String,
    pub checked: bool,
    pub form: Option<usize>,
    pub width: usize,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub action: String,
    pub method: String,
}

#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub title: String,
    pub blocks: Vec<Block>,
    pub fields: Vec<Field>,
    pub forms: Vec<Form>,
    pub base: Option<String>,
}

#[derive(Clone, Default)]
struct Ctx {
    bold: bool,
    italic: bool,
    code: bool,
    link: Option<String>,
    form: Option<usize>,
}

struct Builder {
    doc: Doc,
}

const BLOCKS: &[&str] = &[
    "address", "article", "aside", "body", "center", "dd", "details", "dialog", "div", "dl", "dt",
    "fieldset", "figcaption", "figure", "footer", "form", "header", "hgroup", "html", "legend",
    "main", "nav", "p", "section", "summary", "caption", "search",
];
/// Things that are never shown.
const HIDDEN: &[&str] = &["head", "title", "meta", "link", "base", "datalist", "map", "area", "audio", "video", "canvas"];

/// Class names that, by near-universal convention, mean "not shown until
/// clicked" or "for screen readers only". Lunar doesn't apply CSS, so it
/// recognises these instead.
const HIDDEN_CLASSES: &[&str] = &[
    "sr-only", "visually-hidden", "visuallyhidden", "screen-reader-text", "screenreader", "hidden",
    "dropdown-content", "dropdown-menu", "vector-dropdown-content", "mw-jump-link", "navbox",
    "cookie-banner", "cookie-consent", "modal", "popup", "skip-link", "noprint",
];

fn is_hidden(el: &Element) -> bool {
    if HIDDEN.contains(&el.tag.as_str()) || el.attr("hidden").is_some() || el.attr("aria-hidden") == Some("true") {
        return true;
    }
    if let Some(class) = el.attr("class") {
        if class.split_whitespace().any(|c| HIDDEN_CLASSES.contains(&c.to_ascii_lowercase().as_str())) {
            return true;
        }
    }
    // Language switchers: dozens of links to the same page in other languages.
    if el.tag == "a" && el.attr("hreflang").is_some() && el.attr("lang").is_some() {
        return true;
    }
    if let Some(style) = el.attr("style") {
        let s: String = style.to_ascii_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
        if s.contains("display:none") || s.contains("visibility:hidden") {
            return true;
        }
    }
    false
}

impl Builder {
    /// Walks children, returning blocks; inline content collects in `inline`
    /// and is flushed into a paragraph whenever a block starts.
    fn blocks(&mut self, el: &Element, ctx: &Ctx) -> Vec<Block> {
        let mut out = Vec::new();
        let mut inline = Vec::new();
        self.walk(el, ctx, &mut out, &mut inline);
        flush(&mut out, &mut inline);
        out
    }

    fn walk(&mut self, el: &Element, ctx: &Ctx, out: &mut Vec<Block>, inline: &mut Vec<Inline>) {
        for child in &el.children {
            match child {
                Node::Text(t) => inline.push(Inline::Text { text: t.clone(), style: style_of(ctx), link: ctx.link.clone() }),
                Node::Element(e) => self.element(e, ctx, out, inline),
            }
        }
    }

    fn element(&mut self, el: &Element, ctx: &Ctx, out: &mut Vec<Block>, inline: &mut Vec<Inline>) {
        if is_hidden(el) {
            return;
        }
        let tag = el.tag.as_str();
        match tag {
            "br" => inline.push(Inline::Break),
            "b" | "strong" | "mark" => {
                let c = Ctx { bold: true, ..ctx.clone() };
                self.walk(el, &c, out, inline);
            }
            "i" | "em" | "cite" | "dfn" | "var" => {
                let c = Ctx { italic: true, ..ctx.clone() };
                self.walk(el, &c, out, inline);
            }
            "code" | "kbd" | "samp" | "tt" => {
                let c = Ctx { code: true, ..ctx.clone() };
                self.walk(el, &c, out, inline);
            }
            "a" => {
                let href = el.attr("href").map(str::trim).filter(|h| !h.is_empty() && !h.to_ascii_lowercase().starts_with("javascript:"));
                let c = Ctx { link: href.map(String::from).or(ctx.link.clone()), ..ctx.clone() };
                let (before, blocks_before) = (inline.len(), out.len());
                self.walk(el, &c, out, inline);
                // A link with no visible text (an image link, say) still needs something to click.
                // Blocks inside the link may have flushed its text into paragraphs already.
                let has_text = out.len() > blocks_before
                    || inline.get(before..).map_or(false, |new| {
                        new.iter().any(|i| matches!(i, Inline::Text { text, .. } if !text.trim().is_empty()))
                    });
                if !has_text && c.link.is_some() {
                    let label = el.attr("title").or(el.attr("aria-label")).unwrap_or("link");
                    inline.push(Inline::Text { text: format!("[{label}]"), style: Style::Link, link: c.link.clone() });
                }
            }
            "img" => {
                let alt = el.attr("alt").unwrap_or("").trim();
                if !alt.is_empty() {
                    let style = if ctx.link.is_some() { Style::Link } else { Style::Dim };
                    inline.push(Inline::Text { text: format!(" [{alt}] "), style, link: ctx.link.clone() });
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                flush(out, inline);
                let level = tag.as_bytes()[1] - b'0';
                let mut content = Vec::new();
                let c = Ctx { bold: true, ..ctx.clone() };
                let mut nested = Vec::new();
                self.walk(el, &c, &mut nested, &mut content);
                out.push(Block::Heading(level, content));
            }
            "pre" | "listing" | "plaintext" => {
                flush(out, inline);
                let mut content = Vec::new();
                let c = Ctx { code: true, ..ctx.clone() };
                let mut nested = Vec::new();
                self.walk(el, &c, &mut nested, &mut content);
                out.push(Block::Pre(content));
            }
            "hr" => {
                flush(out, inline);
                out.push(Block::Rule);
            }
            "ul" | "ol" | "menu" | "dir" => {
                flush(out, inline);
                let mut items = Vec::new();
                for child in &el.children {
                    match child {
                        Node::Element(li) if li.tag == "li" => {
                            if !is_hidden(li) {
                                let blocks = self.blocks(li, ctx);
                                if !blocks.is_empty() {
                                    items.push(blocks);
                                }
                            }
                        }
                        Node::Element(other) => {
                            let blocks = self.blocks(&wrap(other), ctx);
                            if !blocks.is_empty() {
                                items.push(blocks);
                            }
                        }
                        Node::Text(t) if !t.trim().is_empty() => {
                            items.push(vec![Block::Lines(vec![Inline::Text { text: t.clone(), style: style_of(ctx), link: None }])]);
                        }
                        _ => {}
                    }
                }
                if !items.is_empty() {
                    out.push(Block::List { ordered: tag == "ol", items });
                }
            }
            "blockquote" => {
                flush(out, inline);
                let inner = self.blocks(el, ctx);
                out.push(Block::Quote(inner));
            }
            "table" => {
                flush(out, inline);
                let mut rows = Vec::new();
                self.table_rows(el, ctx, &mut rows);
                if !rows.is_empty() {
                    out.push(Block::Table(rows));
                }
            }
            "form" => {
                let index = self.doc.forms.len();
                self.doc.forms.push(Form {
                    action: el.attr("action").unwrap_or("").to_string(),
                    method: el.attr("method").unwrap_or("get").to_ascii_lowercase(),
                });
                flush(out, inline);
                let c = Ctx { form: Some(index), ..ctx.clone() };
                self.walk(el, &c, out, inline);
                flush(out, inline);
            }
            "input" => self.input(el, ctx, inline),
            "button" => {
                let label = el.text();
                let kind = el.attr("type").unwrap_or("submit").to_ascii_lowercase();
                if kind == "submit" && ctx.form.is_some() {
                    let label = if label.is_empty() { "Submit".to_string() } else { label };
                    let i = self.field(Field {
                        kind: FieldKind::Submit,
                        name: el.attr("name").unwrap_or("").to_string(),
                        value: el.attr("value").unwrap_or("").to_string(),
                        checked: false,
                        form: ctx.form,
                        width: 0,
                        label,
                    });
                    inline.push(Inline::Field(i));
                } else if !label.is_empty() {
                    inline.push(Inline::Text { text: format!("[{label}]"), style: Style::Dim, link: None });
                }
            }
            "textarea" => {
                let i = self.field(Field {
                    kind: FieldKind::Text,
                    name: el.attr("name").unwrap_or("").to_string(),
                    value: el.text(),
                    checked: false,
                    form: ctx.form,
                    width: 40,
                    label: String::new(),
                });
                inline.push(Inline::Field(i));
            }
            "select" => {
                let mut options = Vec::new();
                let mut selected = None;
                collect_options(el, &mut options, &mut selected);
                if !options.is_empty() {
                    let value = options[selected.unwrap_or(0)].0.clone();
                    let width = options.iter().map(|(_, t)| t.chars().count()).max().unwrap_or(4).min(30);
                    let i = self.field(Field {
                        kind: FieldKind::Select(options),
                        name: el.attr("name").unwrap_or("").to_string(),
                        value,
                        checked: false,
                        form: ctx.form,
                        width,
                        label: String::new(),
                    });
                    inline.push(Inline::Field(i));
                }
            }
            "li" | "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" => {
                // Outside their proper parents: treat as plain blocks.
                flush(out, inline);
                self.walk(el, ctx, out, inline);
                flush(out, inline);
            }
            "p" => {
                flush(out, inline);
                self.walk(el, ctx, out, inline);
                if has_content(inline) {
                    out.push(Block::Para(std::mem::take(inline)));
                }
                inline.clear();
            }
            t if BLOCKS.contains(&t) => {
                flush(out, inline);
                let start = out.len();
                self.walk(el, ctx, out, inline);
                flush(out, inline);
                if out.len() - start >= 3 {
                    out.insert(start, Block::Gap);
                    out.push(Block::Gap);
                }
            }
            _ => self.walk(el, ctx, out, inline),
        }
    }

    fn input(&mut self, el: &Element, ctx: &Ctx, inline: &mut Vec<Inline>) {
        let kind = el.attr("type").unwrap_or("text").to_ascii_lowercase();
        let name = el.attr("name").unwrap_or("").to_string();
        let value = el.attr("value").unwrap_or("").to_string();
        let placeholder = el.attr("placeholder").unwrap_or("").to_string();
        let (kind, label) = match kind.as_str() {
            "hidden" => (FieldKind::Hidden, String::new()),
            "checkbox" => (FieldKind::Checkbox, String::new()),
            "radio" => (FieldKind::Radio, String::new()),
            "submit" | "image" => {
                let label = if !value.is_empty() { value.clone() } else { el.attr("alt").unwrap_or("Submit").to_string() };
                (FieldKind::Submit, label)
            }
            "button" | "reset" | "file" | "color" | "range" => return,
            "password" => (FieldKind::Password, placeholder),
            _ => (FieldKind::Text, placeholder),
        };
        let width = el.attr("size").and_then(|s| s.parse().ok()).unwrap_or(30).clamp(4, 60);
        let checked = el.attr("checked").is_some();
        let value = if matches!(kind, FieldKind::Checkbox | FieldKind::Radio) && value.is_empty() { "on".into() } else { value };
        // Unnamed checkboxes and radios are menu toggles drawn by CSS, not form fields.
        let hidden = kind == FieldKind::Hidden
            || (matches!(kind, FieldKind::Checkbox | FieldKind::Radio) && name.is_empty());
        let i = self.field(Field { kind, name, value, checked, form: ctx.form, width, label });
        if !hidden {
            inline.push(Inline::Field(i));
        }
    }

    fn field(&mut self, field: Field) -> usize {
        self.doc.fields.push(field);
        self.doc.fields.len() - 1
    }

    fn table_rows(&mut self, el: &Element, ctx: &Ctx, rows: &mut Vec<Vec<Vec<Inline>>>) {
        for child in &el.children {
            let Node::Element(e) = child else { continue };
            if is_hidden(e) {
                continue;
            }
            match e.tag.as_str() {
                "tr" => {
                    let mut cells = Vec::new();
                    for cell in &e.children {
                        let Node::Element(td) = cell else { continue };
                        if (td.tag == "td" || td.tag == "th") && !is_hidden(td) {
                            let c = Ctx { bold: ctx.bold || td.tag == "th", ..ctx.clone() };
                            let mut content = Vec::new();
                            let mut nested = Vec::new();
                            self.walk(td, &c, &mut nested, &mut content);
                            // Blocks inside a cell are flattened to text in the row.
                            for block in nested {
                                flatten(block, &mut content);
                            }
                            cells.push(content);
                        }
                    }
                    if cells.iter().any(|c| has_content(c)) {
                        rows.push(cells);
                    }
                }
                "thead" | "tbody" | "tfoot" => self.table_rows(e, ctx, rows),
                "caption" => {
                    let mut content = Vec::new();
                    let mut nested = Vec::new();
                    self.walk(e, &Ctx { bold: true, ..ctx.clone() }, &mut nested, &mut content);
                    rows.push(vec![content]);
                }
                _ => {}
            }
        }
    }
}

fn wrap(el: &Element) -> Element {
    Element { tag: "div".into(), attrs: Vec::new(), children: vec![Node::Element(el.clone())] }
}

fn collect_options(el: &Element, options: &mut Vec<(String, String)>, selected: &mut Option<usize>) {
    for child in &el.children {
        if let Node::Element(e) = child {
            if e.tag == "option" {
                let text = e.text();
                let value = e.attr("value").map(String::from).unwrap_or_else(|| text.clone());
                if e.attr("selected").is_some() {
                    *selected = Some(options.len());
                }
                options.push((value, text));
            } else {
                collect_options(e, options, selected);
            }
        }
    }
}

fn flatten(block: Block, out: &mut Vec<Inline>) {
    out.push(Inline::Text { text: " ".into(), style: Style::Text, link: None });
    match block {
        Block::Para(i) | Block::Lines(i) | Block::Heading(_, i) | Block::Pre(i) => out.extend(i),
        Block::List { items, .. } => items.into_iter().flatten().for_each(|b| flatten(b, out)),
        Block::Quote(bs) => bs.into_iter().for_each(|b| flatten(b, out)),
        Block::Table(rows) => rows.into_iter().flatten().for_each(|c| out.extend(c)),
        Block::Rule | Block::Gap => {}
    }
}

fn has_content(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Text { text, .. } => !text.trim().is_empty(),
        Inline::Field(_) => true,
        Inline::Break => false,
    })
}

fn flush(out: &mut Vec<Block>, inline: &mut Vec<Inline>) {
    if has_content(inline) {
        out.push(Block::Lines(std::mem::take(inline)));
    } else {
        inline.clear();
    }
}

fn style_of(ctx: &Ctx) -> Style {
    if ctx.link.is_some() {
        Style::Link
    } else if ctx.code {
        Style::Code
    } else if ctx.bold {
        Style::Bold
    } else if ctx.italic {
        Style::Italic
    } else {
        Style::Text
    }
}

/// Builds the document model for a parsed page.
pub fn build(root: &Element) -> Doc {
    let mut b = Builder { doc: Doc::default() };
    b.doc.title = root.find("title").map(|t| t.text()).unwrap_or_default();
    b.doc.base = root.find("base").and_then(|e| e.attr("href")).map(String::from);
    // When a page marks its main content, show that and skip the menus around it.
    let body = root
        .find("main")
        .filter(|m| m.text().len() > 200)
        .or_else(|| root.find("body"))
        .unwrap_or(root);
    b.doc.blocks = b.blocks(body, &Ctx::default());
    b.doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::html::parse;

    #[test]
    fn builds_blocks_and_forms() {
        let doc = build(&parse(
            "<title>T</title><h1>Hi</h1><p>a <a href=/x>link</a></p><ul><li>1</li></ul>\
             <form action=/s><input name=q><input type=submit value=Go></form>",
        ));
        assert_eq!(doc.title, "T");
        assert!(matches!(doc.blocks[0], Block::Heading(1, _)));
        assert!(matches!(doc.blocks[2], Block::List { .. }));
        assert_eq!(doc.forms.len(), 1);
        assert_eq!(doc.fields.len(), 2);
        assert_eq!(doc.fields[1].kind, FieldKind::Submit);
    }

    #[test]
    fn blocks_inside_links() {
        let doc = build(&parse("<p>x <a href=/a><div><h3>Title</h3><p>text</p></div></a> y</p><a href=/b><img src=i></a>"));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Heading(3, _))));
    }
}
