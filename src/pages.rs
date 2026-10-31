//! Lunar's own pages, written as HTML and drawn by Lunar's engine like any
//! other page.

use crate::settings::Settings;

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
|  [+] encrypted dns          [+] no trackers or ads    [+] no third parties  |
|  [+] cookies in memory only, wiped when you quit                            |
+-----------------------------------------------------------------------------+";

pub fn new_tab(settings: &Settings) -> String {
    format!(
        r#"<title>New tab</title>
<pre>{moon}</pre>
<form action="{action}" method="get">
lunar:~$ <input name="{param}" size="44" placeholder="search {engine}"> <input type="submit" value="search">
</form>
<pre>{shields}</pre>
<p>ctrl/cmd+t new tab . ctrl/cmd+l address bar . ctrl/cmd+w close tab . ctrl/cmd+, settings</p>"#,
        moon = escape(MOON),
        action = settings.search.url,
        param = settings.search.param,
        engine = settings.search.name,
        shields = escape(SHIELDS),
    )
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
