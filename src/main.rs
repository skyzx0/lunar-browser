//! Lunar Browser: an independent web browser. Its engine (src/engine) and
//! networking (src/net) are its own; it is not a fork of any browser.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod devshot;
mod engine;
mod net;
mod pages;
mod search;
mod settings;

use std::sync::Arc;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Stroke};

use app::color;

fn setup_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("vt323".into(), Arc::new(FontData::from_static(include_bytes!("../assets/VT323-Regular.ttf"))));
    // VT323 first; egui's built-in fonts after it for characters VT323 lacks.
    for family in [FontFamily::Monospace, FontFamily::Proportional] {
        fonts.families.entry(family).or_default().insert(0, "vt323".into());
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    for (_, font) in style.text_styles.iter_mut() {
        font.family = FontFamily::Monospace;
        font.size = 19.0;
    }
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = color::BG;
    v.window_fill = color::PANEL;
    v.extreme_bg_color = color::PANEL;
    v.faint_bg_color = color::PANEL;
    v.override_text_color = Some(color::TEXT);
    v.hyperlink_color = color::ACCENT;
    v.selection.bg_fill = Color32::from_rgb(0x1f, 0x3a, 0x52);
    v.selection.stroke = Stroke::new(1.0, color::ACCENT);
    v.window_stroke = Stroke::new(1.0, color::LINE);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = 0.into();
        w.bg_stroke = Stroke::new(1.0, color::LINE);
    }
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = color::PANEL;
    v.widgets.hovered.weak_bg_fill = color::FIELD;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, color::ACCENT);
    v.widgets.active.weak_bg_fill = color::LINE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, color::LINE);
    v.window_corner_radius = 0.into();
    v.menu_corner_radius = 0.into();
    ctx.set_global_style(style);
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--dump" {
        let ok = app::dump(&args[2..].join(" "), 100);
        std::process::exit(if ok { 0 } else { 1 });
    }
    if args.len() >= 3 && args[1] == "--crawl" {
        let minutes = args.get(3).and_then(|m| m.parse().ok()).unwrap_or(24 * 60);
        app::crawl(args[2].parse().unwrap_or(1000), minutes);
        return Ok(());
    }
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Lunar Browser")
        .with_app_id("lunar-browser")
        .with_inner_size([1200.0, 820.0])
        .with_min_inner_size([560.0, 360.0]);
    if cfg!(target_os = "macos") {
        viewport = viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false);
    }
    if let Some(icon) = icon {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Lunar Browser",
        options,
        Box::new(|cc| {
            setup_style(&cc.egui_ctx);
            Ok(Box::new(app::LunarApp::new()))
        }),
    )
}
