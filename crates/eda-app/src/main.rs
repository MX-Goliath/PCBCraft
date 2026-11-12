#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod theme;
mod updates;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pcbcraft=info,eda_app=info,wgpu_hal=error".into()),
        )
        .init();
    match updates::install::before_start() {
        Ok(true) => return Ok(()),
        Err(error) => {
            tracing::error!("Update startup: {error}");
            if std::env::args_os()
                .nth(1)
                .is_some_and(|a| a == "--pcbcraft-apply-update")
            {
                return Ok(());
            }
        }
        Ok(false) => {}
    }
    let result = eframe::run_native(
        "PCBCraft",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([1000.0, 650.0]),
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                // Prefer immediate/mailbox presentation for tool previews and
                // software cursor styles, with wgpu's supported-mode fallback.
                present_mode: wgpu::PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: Some(1),
                ..Default::default()
            },
            persistence_path: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
                .filter(|dir| dir.join("portable.flag").is_file())
                .map(|dir| dir.join("Data").join("app.ron")),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::PcbCraft::new(cc)))),
    );
    if result.is_ok()
        && let Err(error) = updates::install::after_exit()
    {
        rfd::MessageDialog::new()
            .set_title("PCBCraft update")
            .set_description(format!(
                "The update is retained for the next launch. {error}"
            ))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
    result
}
