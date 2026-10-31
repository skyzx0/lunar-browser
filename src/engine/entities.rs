//! HTML character references: &amp; &#169; &#xA9; and the common named ones.

const NAMED: &[(&str, &str)] = &[
    ("amp", "&"), ("lt", "<"), ("gt", ">"), ("quot", "\""), ("apos", "'"), ("nbsp", "\u{a0}"),
    ("copy", "©"), ("reg", "®"), ("trade", "™"), ("mdash", "—"), ("ndash", "–"), ("hellip", "…"),
    ("laquo", "«"), ("raquo", "»"), ("lsquo", "‘"), ("rsquo", "’"), ("ldquo", "“"), ("rdquo", "”"),
    ("sbquo", "‚"), ("bdquo", "„"), ("bull", "•"), ("middot", "·"), ("times", "×"), ("divide", "÷"),
    ("deg", "°"), ("plusmn", "±"), ("euro", "€"), ("pound", "£"), ("yen", "¥"), ("cent", "¢"),
    ("sect", "§"), ("para", "¶"), ("larr", "←"), ("rarr", "→"), ("uarr", "↑"), ("darr", "↓"),
    ("harr", "↔"), ("hearts", "♥"), ("frac12", "½"), ("frac14", "¼"), ("frac34", "¾"),
    ("sup2", "²"), ("sup3", "³"), ("micro", "µ"), ("iexcl", "¡"), ("iquest", "¿"),
    ("shy", ""), ("zwj", ""), ("zwnj", ""), ("thinsp", " "), ("ensp", " "), ("emsp", " "),
    ("dagger", "†"), ("Dagger", "‡"), ("permil", "‰"), ("prime", "′"), ("Prime", "″"),
    ("minus", "−"), ("le", "≤"), ("ge", "≥"), ("ne", "≠"), ("asymp", "≈"), ("infin", "∞"),
    ("check", "✓"), ("star", "☆"), ("agrave", "à"), ("aacute", "á"), ("acirc", "â"),
    ("atilde", "ã"), ("auml", "ä"), ("aring", "å"), ("aelig", "æ"), ("ccedil", "ç"),
    ("egrave", "è"), ("eacute", "é"), ("ecirc", "ê"), ("euml", "ë"), ("igrave", "ì"),
    ("iacute", "í"), ("icirc", "î"), ("iuml", "ï"), ("ntilde", "ñ"), ("ograve", "ò"),
    ("oacute", "ó"), ("ocirc", "ô"), ("otilde", "õ"), ("ouml", "ö"), ("oslash", "ø"),
    ("ugrave", "ù"), ("uacute", "ú"), ("ucirc", "û"), ("uuml", "ü"), ("yacute", "ý"),
    ("yuml", "ÿ"), ("szlig", "ß"), ("Agrave", "À"), ("Aacute", "Á"), ("Auml", "Ä"),
    ("Eacute", "É"), ("Ouml", "Ö"), ("Uuml", "Ü"), ("Ntilde", "Ñ"), ("Ccedil", "Ç"),
];

/// Replaces character references in `s`. Unknown ones are left as written.
pub fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
            .map_or(rest.len(), |e| e + 1);
        let name = &rest[1..end];
        let decoded = if let Some(num) = name.strip_prefix('#') {
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => num.parse::<u32>().ok(),
            };
            code.map(|c| match c {
                // Windows-1252 quirks that pages still rely on.
                0x80 => '€', 0x91 => '‘', 0x92 => '’', 0x93 => '“', 0x94 => '”', 0x96 => '–', 0x97 => '—',
                0 => '\u{fffd}',
                c => char::from_u32(c).unwrap_or('\u{fffd}'),
            }.to_string())
        } else {
            NAMED.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string())
        };
        match decoded {
            Some(text) if !name.is_empty() => {
                out.push_str(&text);
                rest = &rest[end..];
                if rest.starts_with(';') {
                    rest = &rest[1..];
                }
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes() {
        assert_eq!(decode("a &amp; b &lt;c&gt; &#65;&#x42; &copy &unknown; &"), "a & b <c> AB © &unknown; &");
    }
}
