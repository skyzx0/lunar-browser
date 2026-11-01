//! Milky Ray's own pages: the new tab page and Lunar Search results, both
//! drawn in the current theme.

use lunar_browser::search::Results;
use serde_json::Value;

use crate::store::Shared;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A theme value safe to put in CSS: no quotes, braces or semicolons.
fn css_value(v: Option<&Value>, default: &str) -> String {
    let owned = match v {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => s.clone(),
        _ => default.to_string(),
    };
    let s = owned.as_str();
    let clean: String = s.chars().filter(|c| !matches!(c, '"' | '\'' | '{' | '}' | ';' | '<' | '>' | '\\')).collect();
    if clean.trim().is_empty() { default.to_string() } else { clean }
}

/// The theme as CSS variables, plus the background and font.
fn theme_css(settings: &Value) -> String {
    let t = settings.get("theme").cloned().unwrap_or(Value::Null);
    let get = |k: &str, d: &str| css_value(t.get(k), d);
    let custom_font = get("customFont", "");
    let font = match get("font", "system").as_str() {
        _ if !custom_font.is_empty() => format!("{custom_font}, Inter, system-ui, sans-serif"),
        "rounded" => "ui-rounded, 'SF Pro Rounded', Nunito, system-ui, sans-serif".into(),
        "serif" => "ui-serif, 'New York', Charter, Georgia, serif".into(),
        "mono" => "ui-monospace, 'SF Mono', Menlo, Consolas, monospace".into(),
        "vt323" => "'VT323', ui-monospace, monospace".into(),
        _ => "Inter, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif".into(),
    };
    let background = match get("wallpaper", "solid").as_str() {
        "image" => match t.get("image").and_then(Value::as_str).filter(|i| i.starts_with("data:image/")) {
            Some(image) => format!("url(\"{}\") center / cover no-repeat fixed, var(--bg1)", image.replace('"', "")),
            None => "var(--bg1)".into(),
        },
        "gradient" | "stars" => "linear-gradient(var(--angle), var(--bg1), var(--bg2)) fixed".into(),
        _ => "var(--bg1)".into(),
    };
    let scheme = if t.get("light").and_then(Value::as_bool).unwrap_or(false) { "light" } else { "dark" };
    format!(
        "@font-face{{font-family:'VT323';src:url(vt323.ttf)}}\
         :root{{--bg1:{bg1};--bg2:{bg2};--angle:{angle}deg;--surface:{surface};--text:{text};--muted:{muted};--accent:{accent};\
         --radius:{radius}px;--font:{font};--size:{size}px;--border:color-mix(in srgb,var(--text) 11%,transparent);color-scheme:{scheme}}}\
         html,body{{margin:0;min-height:100%;color:var(--text);font:var(--size)/1.55 var(--font);background:{background};-webkit-font-smoothing:antialiased}}\
         a{{color:var(--accent)}}",
        bg1 = get("bg1", "#141518"),
        bg2 = get("bg2", "#141518"),
        angle = get("angle", "180").parse::<f64>().unwrap_or(180.0),
        surface = get("surface", "#1d1f23"),
        text = get("text", "#e6e4df"),
        muted = get("muted", "#8b8984"),
        accent = get("accent", "#e0a458"),
        radius = get("radius", "8").parse::<f64>().unwrap_or(8.0).clamp(0.0, 40.0),
        size = get("pageSize", "15").parse::<f64>().unwrap_or(15.0).clamp(11.0, 24.0),
    )
}

fn custom_css(settings: &Value, key: &str) -> String {
    settings.get(key).and_then(Value::as_str).unwrap_or("").replace("</", "<\\/")
}

pub fn new_tab(settings: &Shared, pages: usize) -> String {
    let s = settings.read().unwrap().clone();
    let nt = s.get("newTab").cloned().unwrap_or(Value::Null);
    let flag = |k: &str, d: bool| nt.get(k).and_then(Value::as_bool).unwrap_or(d);
    let greeting = nt.get("greeting").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let mut shortcuts = String::new();
    if let Some(list) = s.get("bookmarks").and_then(Value::as_array) {
        for b in list.iter().take(12) {
            let url = b.get("url").and_then(Value::as_str).unwrap_or("");
            let title = b.get("title").and_then(Value::as_str).unwrap_or(url);
            if url.starts_with("https://") || url.starts_with("http://") {
                let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())).unwrap_or_default();
                shortcuts.push_str(&format!(
                    "<a class=link href=\"{u}\"><span class=t>{t}</span><span class=h>{h}</span></a>",
                    u = escape(url),
                    t = escape(title),
                    h = escape(&host)
                ));
            }
        }
    }
    let clock = if flag("clock", true) {
        "<div class=time><span id=clock></span><span id=date></span></div><script>function t(){const d=new Date();\
         clock.textContent=d.toLocaleTimeString([], {hour:'2-digit',minute:'2-digit'});\
         date.textContent=d.toLocaleDateString([], {weekday:'long',month:'long',day:'numeric'})}t();setInterval(t,10000)</script>"
    } else {
        ""
    };
    let heading = if greeting.is_empty() { String::new() } else { format!("<h1>{}</h1>", escape(&greeting)) };
    let search = if flag("search", true) {
        format!("<form class=search action=\"search\" method=get><input name=q autofocus autocomplete=off placeholder=\"Search\"><span class=count>{pages} pages</span></form>")
    } else {
        String::new()
    };
    let links = if shortcuts.is_empty() { String::new() } else { format!("<nav class=links>{shortcuts}</nav>") };
    format!(
        r#"<!doctype html><html><head><meta charset=utf-8><title>New tab</title><style>{theme}
body{{display:flex;flex-direction:column;align-items:center;min-height:100vh;padding:22vh 24px 40px;box-sizing:border-box}}
main{{width:min(600px,100%)}}
.time{{display:flex;align-items:baseline;justify-content:space-between;margin-bottom:20px;color:var(--muted)}}
#clock{{font-size:15px;font-variant-numeric:tabular-nums;color:var(--text)}}
#date{{font-size:13px}}
h1{{font-size:22px;font-weight:600;letter-spacing:-.01em;margin:0 0 14px}}
.search{{display:flex;align-items:center;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius)}}
.search:focus-within{{border-color:var(--accent)}}
.search input{{flex:1;background:none;border:0;color:var(--text);font:inherit;font-size:16px;padding:13px 16px;outline:none}}
.count{{color:var(--muted);font-size:12px;padding-right:14px;font-variant-numeric:tabular-nums}}
.links{{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:1px;margin-top:22px;border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;background:var(--border)}}
.link{{display:flex;flex-direction:column;padding:10px 14px;background:var(--bg1);text-decoration:none;color:var(--text)}}
.link:hover{{background:var(--surface)}}
.link .t{{font-size:13px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.link .h{{font-size:12px;color:var(--muted)}}
{custom}</style></head><body><main>{clock}{heading}{search}{links}</main></body></html>"#,
        theme = theme_css(&s),
        custom = custom_css(&s, "newTabCss"),
    )
}

pub fn search(settings: &Shared, query: &str, results: &Results, action: &str) -> String {
    let s = settings.read().unwrap().clone();
    let mut list = String::new();
    for hit in &results.hits {
        let display = hit.url.trim_start_matches("https://").trim_start_matches("www.").trim_end_matches('/');
        list.push_str(&format!(
            "<article><div class=url>{d}</div><a class=title href=\"{u}\">{t}</a><p>{snip}</p></article>",
            u = escape(&hit.url),
            t = escape(&hit.title),
            d = escape(display),
            snip = hit.snippet
        ));
    }
    let status = if !results.loaded {
        "Opening the index. The first time, Lunar Search downloads its ready-made index.".to_string()
    } else if results.hits.is_empty() {
        format!("No results in {} indexed pages. Lunar Search grows as it crawls, and you can add any site with Index this site in Settings.", results.pages)
    } else {
        format!("{} results from {} indexed pages{}", results.matched, results.pages, if results.crawling { ", still crawling" } else { "" })
    };
    format!(
        r#"<!doctype html><html><head><meta charset=utf-8><title>{q} - Lunar Search</title><style>{theme}
body{{padding:28px max(24px,calc(50vw - 340px)) 60px}}
form{{display:flex;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius)}}
form:focus-within{{border-color:var(--accent)}}
input{{flex:1;background:none;border:0;color:var(--text);font:inherit;font-size:15px;padding:10px 14px;outline:none}}
.status{{color:var(--muted);font-size:12px;margin:12px 2px 18px}}
article{{margin:0 0 22px}}
.url{{color:var(--muted);font-size:12px;word-break:break-all}}
.title{{font-size:17px;text-decoration:none;font-weight:500;line-height:1.35}}.title:hover{{text-decoration:underline}}
p{{margin:4px 0 0;color:color-mix(in srgb,var(--text) 82%,transparent)}}b{{color:var(--text);font-weight:600}}
{custom}</style></head><body><form action="{action}" method=get><input name=q value="{q}" autocomplete=off></form>
<div class=status>{status}</div>{list}</body></html>"#,
        theme = theme_css(&s),
        custom = custom_css(&s, "newTabCss"),
        q = escape(query),
        action = escape(action),
        status = escape(&status),
    )
}
