#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod theme;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "eda_app=info,wgpu_hal=error".into()),
        )
        .init();
    eframe::run_native(
        "PCBCraft",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([1000.0, 650.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::PcbCraft::new(cc)))),
    )
}
