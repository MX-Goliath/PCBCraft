#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod theme;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pcbcraft=info,eda_app=info,wgpu_hal=error".into()),
        )
        .init();
    eframe::run_native(
        "PCBCraft",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([1000.0, 650.0]),
            renderer: eframe::Renderer::Wgpu,
            persistence_path: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
                .filter(|dir| dir.join("portable.flag").is_file())
                .map(|dir| dir.join("Data").join("app.ron")),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::PcbCraft::new(cc)))),
    )
}
