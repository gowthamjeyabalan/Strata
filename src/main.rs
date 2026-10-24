//! Overlay: a whiteboard where every image, sketch, shape and flowchart box is a node in a graph.
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Overlay")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([720.0, 480.0])
            .with_drag_and_drop(true)
            // See-through, so the Windows backdrop (Mica or Acrylic) can show behind the panels.
            .with_transparent(true)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../brand/png/overlay-icon-blue-256.png")).expect("icon is a valid PNG")),
        ..Default::default()
    };
    eframe::run_native("Overlay", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
