//! Lays a document out on a character grid `cols` wide: word wrapping,
//! indentation for lists and quotes, ASCII underlines for headings, tables
//! as rows. The result is a list of lines made of styled runs, which the UI
//! draws as-is.

use super::doc::{Block, Doc, Field, FieldKind, Inline, Style};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Link(String),
    Field(usize),
}

#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub style: Style,
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Default)]
pub struct Line {
    pub runs: Vec<Run>,
}

impl Line {
    pub fn width(&self) -> usize {
        self.runs.iter().map(|r| r.text.chars().count()).sum()
    }

    /// The run under column `col`, if any.
    pub fn run_at(&self, col: usize) -> Option<&Run> {
        let mut x = 0;
        for run in &self.runs {
            let w = run.text.chars().count();
            if col >= x && col < x + w {
                return Some(run);
            }
            x += w;
        }
        None
    }
}

/// How a form field looks on the page.
pub fn field_text(field: &Field, focused: bool) -> String {
    match &field.kind {
        FieldKind::Text | FieldKind::Password => {
            let shown: String = if field.kind == FieldKind::Password {
                "*".repeat(field.value.chars().count())
            } else {
                field.value.clone()
            };
            let w = field.width;
            let mut chars: Vec<char> = shown.chars().collect();
            if focused {
                chars.push('_');
            }
            // Show the end of the text when it is longer than the box.
            let start = chars.len().saturating_sub(w);
            let mut visible: String = chars[start..].iter().collect();
            if visible.is_empty() && !focused && !field.label.is_empty() {
                visible = field.label.chars().take(w).collect();
            }
            let pad = w.saturating_sub(visible.chars().count());
            format!("[{visible}{}]", " ".repeat(pad))
        }
        FieldKind::Checkbox => if field.checked { "[x]" } else { "[ ]" }.into(),
        FieldKind::Radio => if field.checked { "(*)" } else { "( )" }.into(),
        FieldKind::Submit => format!("[ {} ]", field.label),
        FieldKind::Select(options) => {
            let text = options.iter().find(|(v, _)| *v == field.value).map(|(_, t)| t.as_str()).unwrap_or("");
            let text: String = text.chars().take(field.width).collect();
            let pad = field.width.saturating_sub(text.chars().count());
            format!("[{text}{} v]", " ".repeat(pad))
        }
        FieldKind::Hidden => String::new(),
    }
}

struct Writer<'a> {
    doc: &'a Doc,
    focus: Option<usize>,
    cols: usize,
    lines: Vec<Line>,
}

struct Atom {
    text: String,
    style: Style,
    action: Option<Action>,
    space_before: bool,
}

impl<'a> Writer<'a> {
    fn blank(&mut self) {
        if self.lines.last().map_or(false, |l| !l.runs.is_empty()) {
            self.lines.push(Line::default());
        }
    }

    fn push_line(&mut self, line: Line) {
        self.lines.push(merge(line));
    }

    /// Breaks inline content into words, fields and breaks.
    fn atoms(&self, inlines: &[Inline]) -> Vec<Option<Atom>> {
        let mut atoms = Vec::new();
        let mut space = false;
        for inline in inlines {
            match inline {
                Inline::Text { text, style, link } => {
                    let action = link.clone().map(Action::Link);
                    for (i, word) in text.split(|c: char| c.is_whitespace() && c != '\u{a0}').enumerate() {
                        if i > 0 {
                            space = true;
                        }
                        if word.is_empty() {
                            continue;
                        }
                        atoms.push(Some(Atom { text: word.replace('\u{a0}', " "), style: *style, action: action.clone(), space_before: space }));
                        space = false;
                    }
                }
                Inline::Field(i) => {
                    let field = &self.doc.fields[*i];
                    let style = if field.kind == FieldKind::Submit { Style::Button } else { Style::Field };
                    atoms.push(Some(Atom {
                        text: field_text(field, self.focus == Some(*i)),
                        style,
                        action: Some(Action::Field(*i)),
                        space_before: true,
                    }));
                    space = true;
                }
                Inline::Break => {
                    atoms.push(None);
                    space = false;
                }
            }
        }
        atoms
    }

    /// Word-wraps inline content. `first` prefixes the first line, `rest`
    /// the following ones (for bullets and quote bars).
    fn wrap(&mut self, inlines: &[Inline], first: &[Run], rest: &[Run], style_override: Option<Style>) {
        let indent: usize = rest.iter().map(|r| r.text.chars().count()).sum();
        let avail = self.cols.saturating_sub(indent).max(10);
        let mut line = Line { runs: first.to_vec() };
        let mut col = 0;
        let mut started = false;
        for atom in self.atoms(inlines) {
            let Some(mut atom) = atom else {
                self.push_line(std::mem::replace(&mut line, Line { runs: rest.to_vec() }));
                col = 0;
                started = true;
                continue;
            };
            if let Some(s) = style_override {
                if atom.action.is_none() {
                    atom.style = s;
                }
            }
            let mut text = atom.text;
            let len = text.chars().count();
            let gap = usize::from(atom.space_before && col > 0);
            if col > 0 && col + gap + len > avail {
                self.push_line(std::mem::replace(&mut line, Line { runs: rest.to_vec() }));
                col = 0;
            } else if gap == 1 {
                // The space takes the link's style only when both sides are the same link.
                let prev = line.runs.last().map(|r| r.action.clone()).flatten();
                let same = prev.is_some() && prev == atom.action;
                line.runs.push(Run { text: " ".into(), style: if same { atom.style } else { Style::Text }, action: if same { atom.action.clone() } else { None } });
                col += 1;
            }
            // Words longer than a line are cut.
            while text.chars().count() > avail - col {
                let head: String = text.chars().take(avail - col).collect();
                text = text.chars().skip(avail - col).collect();
                line.runs.push(Run { text: head, style: atom.style, action: atom.action.clone() });
                self.push_line(std::mem::replace(&mut line, Line { runs: rest.to_vec() }));
                col = 0;
            }
            col += text.chars().count();
            line.runs.push(Run { text, style: atom.style, action: atom.action });
            started = true;
        }
        if col > 0 || !started {
            if line.runs.iter().any(|r| !r.text.trim().is_empty()) {
                self.push_line(line);
            }
        }
    }

    fn block(&mut self, block: &Block, first: &[Run], rest: &[Run]) {
        match block {
            Block::Para(inlines) => {
                self.blank();
                self.wrap(inlines, first, rest, None);
                self.blank();
            }
            Block::Lines(inlines) => self.wrap(inlines, first, rest, None),
            Block::Gap => self.blank(),
            Block::Heading(level, inlines) => {
                self.blank();
                let start = self.lines.len();
                self.wrap(inlines, first, rest, Some(Style::Heading));
                if *level <= 2 {
                    let w = self.lines[start..].iter().map(Line::width).max().unwrap_or(0).min(self.cols);
                    let indent: usize = rest.iter().map(|r| r.text.chars().count()).sum();
                    let mark = if *level == 1 { "=" } else { "-" };
                    let mut runs = rest.to_vec();
                    runs.push(Run { text: mark.repeat(w.saturating_sub(indent)), style: Style::Heading, action: None });
                    self.push_line(Line { runs });
                }
                self.blank();
            }
            Block::Pre(inlines) => {
                let indent: usize = rest.iter().map(|r| r.text.chars().count()).sum();
                let avail = self.cols.saturating_sub(indent).max(10);
                let mut line = Line { runs: first.to_vec() };
                let mut col = 0;
                for inline in inlines {
                    let (text, style, action) = match inline {
                        Inline::Text { text, style, link } => (text.clone(), *style, link.clone().map(Action::Link)),
                        Inline::Field(i) => (field_text(&self.doc.fields[*i], self.focus == Some(*i)), Style::Field, Some(Action::Field(*i))),
                        Inline::Break => ("\n".into(), Style::Code, None),
                    };
                    for (i, part) in text.replace('\t', "    ").split('\n').enumerate() {
                        if i > 0 {
                            self.push_line(std::mem::replace(&mut line, Line { runs: rest.to_vec() }));
                            col = 0;
                        }
                        let mut part: String = part.to_string();
                        while col + part.chars().count() > avail {
                            let head: String = part.chars().take(avail - col).collect();
                            part = part.chars().skip(avail - col).collect();
                            line.runs.push(Run { text: head, style, action: action.clone() });
                            self.push_line(std::mem::replace(&mut line, Line { runs: rest.to_vec() }));
                            col = 0;
                        }
                        col += part.chars().count();
                        line.runs.push(Run { text: part, style, action: action.clone() });
                    }
                }
                self.push_line(line);
                self.blank();
            }
            Block::Rule => {
                let indent: usize = rest.iter().map(|r| r.text.chars().count()).sum();
                let mut runs = first.to_vec();
                runs.push(Run { text: "-".repeat(self.cols.saturating_sub(indent).min(80)), style: Style::Dim, action: None });
                self.push_line(Line { runs });
                self.blank();
            }
            Block::List { ordered, items } => {
                for (n, item) in items.iter().enumerate() {
                    let bullet = if *ordered { format!("{}. ", n + 1) } else { "* ".to_string() };
                    let mut item_first = first.to_vec();
                    item_first.push(Run { text: bullet.clone(), style: Style::Dim, action: None });
                    let mut item_rest = rest.to_vec();
                    item_rest.push(Run { text: " ".repeat(bullet.chars().count()), style: Style::Text, action: None });
                    for (i, b) in item.iter().enumerate() {
                        let f = if i == 0 { item_first.clone() } else { item_rest.clone() };
                        self.block(b, &f, &item_rest);
                        // Items stay together: drop the blank line paragraphs leave.
                        if self.lines.last().map_or(false, |l| l.runs.is_empty()) {
                            self.lines.pop();
                        }
                    }
                }
                self.blank();
            }
            Block::Quote(blocks) => {
                let mut bar = rest.to_vec();
                bar.push(Run { text: "| ".into(), style: Style::Dim, action: None });
                for (i, b) in blocks.iter().enumerate() {
                    let mut f = if i == 0 { first.to_vec() } else { rest.to_vec() };
                    f.push(Run { text: "| ".into(), style: Style::Dim, action: None });
                    self.block(b, &f, &bar);
                }
                self.blank();
            }
            Block::Table(rows) => {
                for row in rows {
                    let mut inlines = Vec::new();
                    for (i, cell) in row.iter().enumerate() {
                        if i > 0 {
                            inlines.push(Inline::Text { text: " | ".into(), style: Style::Dim, link: None });
                        }
                        inlines.extend(cell.iter().cloned());
                    }
                    self.wrap(&inlines, first, rest, None);
                }
                self.blank();
            }
        }
    }
}

/// Joins neighbouring runs that look and act the same.
fn merge(line: Line) -> Line {
    let mut runs: Vec<Run> = Vec::with_capacity(line.runs.len());
    for run in line.runs {
        if let Some(last) = runs.last_mut() {
            if last.style == run.style && last.action == run.action {
                last.text.push_str(&run.text);
                continue;
            }
        }
        runs.push(run);
    }
    Line { runs }
}

/// Lays out `doc` for a window `cols` characters wide. `focus` is the form
/// field being typed into, drawn with a cursor.
pub fn layout(doc: &Doc, cols: usize, focus: Option<usize>) -> Vec<Line> {
    let mut w = Writer { doc, focus, cols: cols.max(20), lines: Vec::new() };
    for block in &doc.blocks {
        w.block(block, &[], &[]);
    }
    while w.lines.last().map_or(false, |l| l.runs.is_empty()) {
        w.lines.pop();
    }
    w.lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{doc::build, html::parse};

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.runs.iter().map(|r| r.text.as_str()).collect()).collect()
    }

    #[test]
    fn wraps_and_underlines() {
        let doc = build(&parse("<h1>Title</h1><p>one two three four five six seven</p>"));
        let lines = text(&layout(&doc, 20, None));
        assert_eq!(lines[0], "Title");
        assert_eq!(lines[1], "=====");
        assert_eq!(lines[2], "");
        assert!(lines.iter().all(|l| l.chars().count() <= 20));
        assert!(lines.contains(&"one two three four".to_string()));
    }

    #[test]
    fn lists_and_links() {
        let doc = build(&parse("<ul><li>a <a href='/x'>b</a></li><li>c</li></ul>"));
        let lines = layout(&doc, 40, None);
        assert_eq!(text(&lines), vec!["* a b", "* c"]);
        assert_eq!(lines[0].run_at(4).unwrap().action, Some(Action::Link("/x".into())));
    }
}
