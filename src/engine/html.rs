use super::entities::decode;

#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, Default)]
pub struct Element {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    fn new(tag: &str, attrs: Vec<(String, String)>) -> Self {
        Element { tag: tag.to_string(), attrs, children: Vec::new() }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        collect_text(self, &mut out);
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub fn find(&self, tag: &str) -> Option<&Element> {
        for child in &self.children {
            if let Node::Element(el) = child {
                if el.tag == tag {
                    return Some(el);
                }
                if let Some(found) = el.find(tag) {
                    return Some(found);
                }
            }
        }
        None
    }
}

fn collect_text(el: &Element, out: &mut String) {
    for child in &el.children {
        match child {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => {
                out.push(' ');
                collect_text(e, out);
            }
        }
    }
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr", "keygen",
];
const SKIPPED: &[&str] = &["script", "style", "noscript", "template", "svg", "math", "iframe", "object"];
const RAW_TEXT: &[&str] = &["title", "textarea", "xmp"];
/// Opening one of these closes an open <p>.
const CLOSES_P: &[&str] = &[
    "address", "article", "aside", "blockquote", "details", "div", "dl", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "main", "nav",
    "ol", "p", "pre", "section", "table", "ul", "menu",
];
const MAX_DEPTH: usize = 400;

enum Token {
    Start { name: String, attrs: Vec<(String, String)>, self_closing: bool },
    End(String),
    Text(String),
}

fn find_ci(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let first = needle.first()?.to_ascii_lowercase();
    let last_start = haystack.len().checked_sub(needle.len())?;
    (0..=last_start).find(|&i| {
        haystack[i].to_ascii_lowercase() == first && haystack[i..i + needle.len()].eq_ignore_ascii_case(needle)
    })
}

struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn starts_with_ci(&self, s: &str) -> bool {
        let rest = self.rest().as_bytes();
        rest.len() >= s.len() && rest[..s.len()].eq_ignore_ascii_case(s.as_bytes())
    }

    fn take_until_ci(&mut self, end: &str) -> &'a str {
        let rest = self.rest();
        match find_ci(rest.as_bytes(), end.as_bytes()) {
            Some(i) => {
                self.pos += i + end.len();
                &rest[..i]
            }
            None => {
                self.pos = self.src.len();
                rest
            }
        }
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos >= self.src.len() {
            return None;
        }
        let rest = self.rest();
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            self.pos += end;
            return Some(Token::Text(decode(&rest[..end])));
        }
        if self.starts_with_ci("<!--") {
            self.pos += 4;
            self.take_until_ci("-->");
            return self.next();
        }
        if self.starts_with_ci("<!") || self.starts_with_ci("<?") {
            self.take_until_ci(">");
            return self.next();
        }
        if self.starts_with_ci("</") {
            self.pos += 2;
            let name = self.read_name();
            self.take_until_ci(">");
            if name.is_empty() {
                return self.next();
            }
            return Some(Token::End(name));
        }
        let after = rest[1..].chars().next();
        if !after.map_or(false, |c| c.is_ascii_alphabetic()) {
            self.pos += 1;
            return Some(Token::Text("<".into()));
        }
        self.pos += 1;
        let name = self.read_name();
        let (attrs, self_closing) = self.read_attrs();
        Some(Token::Start { name, attrs, self_closing })
    }

    fn read_name(&mut self) -> String {
        let rest = self.rest();
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        self.pos += end;
        rest[..end].to_ascii_lowercase()
    }

    fn skip_ws(&mut self) {
        let rest = self.rest();
        let n = rest.len() - rest.trim_start().len();
        self.pos += n;
    }

    fn read_attrs(&mut self) -> (Vec<(String, String)>, bool) {
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            let rest = self.rest();
            if rest.is_empty() {
                return (attrs, false);
            }
            if rest.starts_with("/>") {
                self.pos += 2;
                return (attrs, true);
            }
            if rest.starts_with('>') {
                self.pos += 1;
                return (attrs, false);
            }
            if rest.starts_with('/') {
                self.pos += 1;
                continue;
            }
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '=' || c == '>' || c == '/')
                .unwrap_or(rest.len())
                .max(rest.chars().next().map_or(1, |c| c.len_utf8()));
            let name = rest[..end].to_ascii_lowercase();
            self.pos += end;
            self.skip_ws();
            let mut value = String::new();
            if self.rest().starts_with('=') {
                self.pos += 1;
                self.skip_ws();
                let rest = self.rest();
                if let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    let close = rest[1..].find(q).map_or(rest.len(), |i| i + 1);
                    value = decode(&rest[1..close]);
                    self.pos += (close + 1).min(rest.len());
                } else {
                    let end = rest.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(rest.len());
                    value = decode(&rest[..end]);
                    self.pos += end;
                }
            }
            if !attrs.iter().any(|(k, _): &(String, String)| *k == name) {
                attrs.push((name, value));
            }
        }
    }
}

struct TreeBuilder {
    stack: Vec<Element>,
}

impl TreeBuilder {
    fn current(&mut self) -> &mut Element {
        self.stack.last_mut().expect("root is never popped")
    }

    fn pop(&mut self) {
        if self.stack.len() > 1 {
            let el = self.stack.pop().unwrap();
            self.current().children.push(Node::Element(el));
        }
    }

    fn open_index(&self, tag: &str, boundary: &[&str]) -> Option<usize> {
        for (i, el) in self.stack.iter().enumerate().rev() {
            if el.tag == tag {
                return Some(i);
            }
            if boundary.contains(&el.tag.as_str()) {
                return None;
            }
        }
        None
    }

    fn close_to(&mut self, index: usize) {
        while self.stack.len() > index.max(1) {
            self.pop();
        }
    }

    fn close_if_open(&mut self, tag: &str, boundary: &[&str]) {
        if let Some(i) = self.open_index(tag, boundary) {
            self.close_to(i);
        }
    }

    fn text(&mut self, text: String) {
        let current = self.current();
        if let Some(Node::Text(last)) = current.children.last_mut() {
            last.push_str(&text);
        } else {
            current.children.push(Node::Text(text));
        }
    }

    fn start(&mut self, name: &str, attrs: Vec<(String, String)>, self_closing: bool) {
        const TABLE: &[&str] = &["table"];
        const LIST: &[&str] = &["ul", "ol", "menu"];
        if CLOSES_P.contains(&name) {
            self.close_if_open("p", &["button", "table", "td", "th", "li"]);
        }
        match name {
            "li" => self.close_if_open("li", LIST),
            "dt" | "dd" => {
                self.close_if_open("dt", &["dl"]);
                self.close_if_open("dd", &["dl"]);
            }
            "tr" => {
                self.close_if_open("td", TABLE);
                self.close_if_open("th", TABLE);
                self.close_if_open("tr", TABLE);
            }
            "td" | "th" => {
                self.close_if_open("td", &["tr", "table"]);
                self.close_if_open("th", &["tr", "table"]);
            }
            "option" => self.close_if_open("option", &["select"]),
            "a" => self.close_if_open("a", &[]),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                for h in ["h1", "h2", "h3", "h4", "h5", "h6"] {
                    if self.current().tag == h {
                        self.pop();
                    }
                }
            }
            _ => {}
        }
        let el = Element::new(name, attrs);
        if VOID.contains(&name) || self_closing {
            self.current().children.push(Node::Element(el));
        } else if self.stack.len() < MAX_DEPTH {
            self.stack.push(el);
        }
    }

    fn end(&mut self, name: &str) {
        if name == "p" && self.open_index("p", &[]).is_none() {
            return;
        }
        if let Some(i) = self.open_index(name, &[]) {
            self.close_to(i);
        }
    }
}

pub fn parse(html: &str) -> Element {
    let mut tokens = Tokenizer { src: html, pos: 0 };
    let mut tree = TreeBuilder { stack: vec![Element::new("#document", Vec::new())] };
    while let Some(token) = tokens.next() {
        match token {
            Token::Text(t) => tree.text(t),
            Token::End(name) => tree.end(&name),
            Token::Start { name, attrs, self_closing } => {
                if SKIPPED.contains(&name.as_str()) && !self_closing {
                    tokens.take_until_ci(&format!("</{name}"));
                    tokens.take_until_ci(">");
                    continue;
                }
                if RAW_TEXT.contains(&name.as_str()) && !self_closing {
                    let raw = tokens.take_until_ci(&format!("</{name}"));
                    tokens.take_until_ci(">");
                    let mut el = Element::new(&name, attrs);
                    el.children.push(Node::Text(decode(raw)));
                    tree.current().children.push(Node::Element(el));
                    continue;
                }
                tree.start(&name, attrs, self_closing);
            }
        }
    }
    while tree.stack.len() > 1 {
        tree.pop();
    }
    tree.stack.pop().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implied_endings() {
        let doc = parse("<ul><li>one<li>two</ul><p>a<p>b");
        let ul = doc.find("ul").unwrap();
        assert_eq!(ul.children.len(), 2);
        assert_eq!(doc.text(), "one two a b");
    }

    #[test]
    fn scripts_are_dropped() {
        let doc = parse("<p>hi<script>alert('</p>')</script> there</p><style>p{}</style>");
        assert_eq!(doc.text(), "hi there");
    }

    #[test]
    fn attributes_and_entities() {
        let doc = parse(r#"<a href="/x?a=1&amp;b=2" title=t>Tom &amp; Jerry &copy;</a>"#);
        let a = doc.find("a").unwrap();
        assert_eq!(a.attr("href"), Some("/x?a=1&b=2"));
        assert_eq!(a.attr("title"), Some("t"));
        assert_eq!(a.text(), "Tom & Jerry ©");
    }

    #[test]
    fn stray_lt_is_text() {
        assert_eq!(parse("1 < 2 <b>ok</b>").text(), "1 < 2 ok");
    }
}
