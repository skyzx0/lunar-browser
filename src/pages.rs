use crate::search::Results;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const MOON: &str = r#"
            _..._
          .:::::::.             .
         :::::::::::      *
         :::::::::::               .
         `:::::::::'   .
           `':::''

 _     _   _ _   _    _    ____
| |   | | | | \ | |  / \  |  _ \
| |   | | | |  \| | / _ \ | |_) |
| |___| |_| | |\  |/ ___ \|  _ <
|_____|\___/|_| \_/_/   \_\_| \_\
"#;

const SHIELDS: &str = "\
+-------------------------- protection: always on ---------------------------+
|  [+] own engine, no fork    [+] no javascript, ever   [+] https only        |
|  [+] own search engine      [+] encrypted dns         [+] no trackers       |
|  [+] cookies in memory only, wiped when you quit      [+] no third parties  |
+-----------------------------------------------------------------------------+";

pub fn new_tab(pages: usize) -> String {
    format!(
        r#"<title>New tab</title>
<pre>{moon}</pre>
<form action="lunar:search" method="get">
lunar:~$ <input name="q" size="44" placeholder="search {pages} pages with lunar search"> <input type="submit" value="search">
</form>
<pre>{shields}</pre>
<p>ctrl/cmd+t new tab . ctrl/cmd+l address bar . ctrl/cmd+w close tab . ctrl/cmd+, settings</p>"#,
        moon = escape(MOON),
        shields = escape(SHIELDS),
    )
}

/// A Lunar Search results page.
pub fn search(query: &str, results: &Results) -> String {
    let mut out = format!(
        r#"<title>{q} - Lunar Search</title>
<form action="lunar:search" method="get">lunar search:~$ <input name="q" size="50" value="{q}"> <input type="submit" value="search"></form>
"#,
        q = escape(query)
    );
    let status = if !results.loaded {
        "Opening the index (the first time, Lunar downloads its ready-made index, which can take a minute)...".to_string()
    } else if results.crawling {
        format!("{} pages indexed so far; Lunar Search is still crawling, so results keep improving.", results.pages)
    } else {
        format!("{} pages in the index.", results.pages)
    };
    if results.hits.is_empty() {
        out.push_str(&format!("<h2>nothing found for \"{}\"</h2><p>{status}</p>", escape(query)));
        out.push_str("<p>Lunar Search only knows the pages it has crawled. To add a site, open it and choose <b>[ index this site ]</b> in the settings bar.</p>");
        return out;
    }
    out.push_str(&format!("<p>{} of {} matching pages. {status}</p><hr>", results.hits.len(), results.matched));
    for hit in &results.hits {
        let display = hit.url.trim_start_matches("https://").trim_end_matches('/');
        out.push_str(&format!(
            "<div><h3><a href=\"{url}\">{title}</a></h3><p><code>{display}</code><br>{snippet}</p></div>\n",
            url = escape(&hit.url),
            title = escape(&hit.title),
            display = escape(display),
            snippet = hit.snippet,
        ));
    }
    out.push_str("<hr><p>Results come from Lunar Search's own index, kept on this computer. Your searches never leave it.</p>");
    out
}

pub fn error(url: &str, error: &str) -> String {
    let (code, detail) = error.split_once(": ").filter(|(c, _)| c.starts_with("ERR_")).unwrap_or(("", error));
    let (heading, message) = match code {
        "ERR_NAME_NOT_RESOLVED" => ("site not found", "Encrypted DNS couldn't find this address. Check the spelling."),
        "ERR_DNS" => ("can't look up the site", "Lunar couldn't reach its encrypted DNS servers. Are you online?"),
        "ERR_CERT" => ("certificate rejected", "This site's security certificate isn't valid. Someone may be intercepting the connection, so Lunar won't open it."),
        "ERR_SSL" => ("no secure connection", "Lunar only opens encrypted connections, and this site couldn't make one."),
        "ERR_HTTPS_ONLY" => ("https only", "Lunar only opens encrypted (https) connections."),
        "ERR_CONNECTION" => ("connection failed", "The site stopped answering."),
        "ERR_TOO_MANY_REDIRECTS" => ("redirect loop", "This site keeps sending Lunar somewhere else."),
        "ERR_UNSUPPORTED" => ("not a web page", "Lunar shows web pages only. It doesn't download files or run apps."),
        _ => ("can't open this page", ""),
    };
    let message = if message.is_empty() { detail.to_string() } else { format!("{message} ({detail})") };
    format!(
        r#"<title>{heading}</title>
<pre>
     _..._
   .::::. `.
  :::::::.  :      [ X ]
  ::::::::  :
  `::::::' .'
    `'::'-'
</pre>
<h1>&gt; {heading}</h1>
<p>{message}</p>
<p><code>{url}</code></p>
<p>Lunar never lowers its protections for a site, so there is no way past this page.</p>"#,
        heading = escape(heading),
        message = escape(&message),
        url = escape(url),
    )
}
