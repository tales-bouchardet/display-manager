#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod libs;
use libs::functions::{
    list_displays,
    find_properties,
    set_resolution,
    auto_adjust,
    move_display,
    display_brightness,
    reset_monitor,
    set_primary_display,
    verify_vcp
};
use eframe::egui::{self, RichText};
use eframe::egui::IconData;
use std::sync::Arc;

struct DisplayManager {
    opcao_1: String,
    opcao_2: String,
    check: bool,
    valor_slider: f32,
    monitor_index: u32,
    resolutions: Vec<(u32, u32)>,
    is_primary: bool,
    vcp_supported: bool,
}

impl Default for DisplayManager {
    fn default() -> Self {
        let mut app = Self {
            opcao_1: String::new(),
            opcao_2: String::new(),
            check: false,
            valor_slider: 50.0,
            monitor_index: 0,
            resolutions: Vec::new(),
            is_primary: false,
            vcp_supported: false,
        };
        app.refresh_monitor(0);
        app
    }
}

impl DisplayManager {
    fn refresh_monitor(&mut self, index: u32) {
        self.monitor_index = index;
        if let Ok(props) = find_properties(index) {
            self.opcao_1 = props.name.clone();
            self.opcao_2 = format!("{}x{}", props.resolution.w, props.resolution.h);
            self.resolutions = props.supported_resolutions.iter().map(|r| (r.sw, r.sh)).collect();
            self.is_primary = props.is_primary;
            self.check = props.is_primary;
        }
        if let Ok((supported, brightness)) = verify_vcp(index) {
            self.vcp_supported = supported;
            if supported {
                self.valor_slider = brightness as f32;
            }
        } else {
            self.vcp_supported = false;
        }
    }
}

impl eframe::App for DisplayManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    ctx.set_style({
        let mut style = (*ctx.style()).clone();
        style.text_styles.iter_mut().for_each(|(_, font_id)| {
            font_id.size = 15.0;
        });
        style
    });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().interact_size.y = 30.0;
            egui::ComboBox::from_id_salt("monitor")
                .width(150.0)
                .height(30.0)
                .selected_text(&self.opcao_1)
                .show_ui(ui, |ui| {
                    for monitor in list_displays().unwrap() {
                        if ui.selectable_value(&mut self.opcao_1, monitor.name.clone(), monitor.name).clicked() {
                            self.refresh_monitor(monitor.index);
                        }
                    }
                });

                ui.add_space(8.0);

            egui::ComboBox::from_id_salt("resolucao")
                .width(300.0)
                .selected_text(&self.opcao_2)
                .show_ui(ui, |ui| {
                    for (w, h) in self.resolutions.clone() {
                        let label = format!("{}x{}", w, h);
                        if ui.selectable_value(&mut self.opcao_2, label.clone(), label).clicked() {
                            let _ = set_resolution(self.monitor_index, w, h);
                            self.refresh_monitor(self.monitor_index);
                        }
                    }
                });

            ui.add_space(8.0);

            ui.add_enabled_ui(!self.is_primary, |ui| {
                if ui.checkbox(&mut self.check, "Principal").clicked() && !self.is_primary {
                    let _ = set_primary_display(self.monitor_index);
                    self.refresh_monitor(self.monitor_index);
                }
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("< Mover").clicked() {
                    if let Ok(current) = find_properties(self.monitor_index) {
                        let displays = list_displays().unwrap();
                        let leftmost = displays.iter()
                            .map(|d| find_properties(d.index).unwrap().position.left)
                            .min().unwrap_or(0);
                        let target_x = leftmost - current.resolution.w;
                        let _ = move_display(self.monitor_index, target_x, current.position.top);
                        self.refresh_monitor(self.monitor_index);
                    }
                }
                ui.add_space(220.0);
                if ui.button("Mover >").clicked() {
                    if let Ok(current) = find_properties(self.monitor_index) {
                        let displays = list_displays().unwrap();
                        let rightmost = displays.iter()
                            .map(|d| find_properties(d.index).unwrap().position.right)
                            .max().unwrap_or(0);
                        let _ = move_display(self.monitor_index, rightmost, current.position.top);
                        self.refresh_monitor(self.monitor_index);
                    }
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.label(RichText::new("Virtual Channel Processing (VCP)").size(12.0));
            });

                ui.add_enabled_ui(self.vcp_supported, |ui| {
                    ui.label(RichText::new("Brilho").size(12.0));
                    ui.scope(|ui| {
                        ui.spacing_mut().slider_width = 305.0;
                        if ui.add(egui::Slider::new(&mut self.valor_slider, 0.0..=100.0).min_decimals(0).max_decimals(0)).drag_stopped() {
                            let _ = display_brightness(self.monitor_index, self.valor_slider as u32);
                            self.refresh_monitor(self.monitor_index);
                        }
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() / 5.0);
                        if ui.button("Ajustar Bordas").clicked() {
                            let _ = auto_adjust(self.monitor_index);
                            self.refresh_monitor(self.monitor_index);
                        }

                        if ui.button("Redefinir VCP").clicked() {
                            let _ = reset_monitor(self.monitor_index);
                            self.refresh_monitor(self.monitor_index);
                        }
                    });
                });
                ui.add_space(20.0);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    ui.label(egui::RichText::new("made by: bouchardet").size(11.0));
                });
            });

    }
}


fn main() -> eframe::Result<()> {
    let icon_bytes = include_bytes!("../icon.png");
    let image = image::load_from_memory(icon_bytes).expect("Imagem inválida").to_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();

    let icon = Arc::new(IconData { rgba, width, height });

    let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
        .with_inner_size([367.0, 315.0])
        .with_title("Display Manager")
        .with_resizable(false)
        .with_maximize_button(false)
        .with_minimize_button(false)
        .with_icon(icon),
    ..Default::default()
    };

    eframe::run_native(
        "Display Manager",
        options,
        Box::new(|_| Ok(Box::new(DisplayManager::default()))),
    )
}
