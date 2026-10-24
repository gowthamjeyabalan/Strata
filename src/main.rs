//! Strata: a whiteboard where every image, sketch, shape and flowchart box is a node in a graph.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autosave;
mod backdrop;
mod board;
mod calendar;
mod canvas;
mod dates;
mod editor;
mod export;
mod history;
mod insert;
mod keys;
mod markdown;
mod overview;
mod palettes;
mod radial;
mod search;
mod settings;
mod panels;
mod plot;
mod shapes;
mod snap;
mod theme;
mod tools;

fn main() -> eframe::Result<()> {
    carry_over_settings();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Strata")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([720.0, 480.0])
            .with_drag_and_drop(true)
            // See-through, so the Windows backdrop (Mica or Acrylic) can show behind the panels.
            .with_transparent(true)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../brand/png/strata-icon-blue-256.png")).expect("icon is a valid PNG")),
        ..Default::default()
    };
    eframe::run_native("Strata", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}

/// Strata was called Overlay. The first time it starts, it takes over Overlay's settings (layout,
/// workspaces, recent boards, shortcuts) and any recovery copies.
fn carry_over_settings() {
    let Some(appdata) = std::env::var_os("APPDATA").map(std::path::PathBuf::from) else { return };
    let (old, new) = (appdata.join("Overlay"), appdata.join("Strata"));
    if new.exists() || !old.exists() {
        return;
    }
    for sub in ["data", "recovery"] {
        let Ok(dir) = std::fs::read_dir(old.join(sub)) else { continue };
        let _ = std::fs::create_dir_all(new.join(sub));
        for e in dir.flatten() {
            let to = new.join(sub).join(e.file_name());
            // Recovery copies take the new file type.
            let to = if to.extension().is_some_and(|x| x == "overlay") { to.with_extension("strata") } else { to };
            let _ = std::fs::copy(e.path(), to);
        }
    }
}
