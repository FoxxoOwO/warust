//! Main application struct and event loop

use std::sync::Arc;
use anyhow::Result;
use egui::{CentralPanel, Color32, FontId, Panel, RichText, Ui};
use parking_lot::RwLock;
use tracing::info;

use crate::backend::BackendHandle;
use crate::state::{ConnectionStatus, SharedState};
use crate::theme::setup_theme;
use crate::ui::{chat_list, conversation, qr_pairing, settings};


/// Main application struct
pub struct WarustApp {
    state: SharedState,
    backend: Option<BackendHandle>,
    runtime: tokio::runtime::Runtime,
    show_settings: bool,
    tray: Option<crate::tray::SystemTray>,
}

impl WarustApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        // Setup theme
        setup_theme(&cc.egui_ctx);
        
        // Create Tokio runtime for backend
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        
        // Load persisted state or create fresh
        let loaded_state = runtime.block_on(crate::storage::load_state()).unwrap_or_default();
        let run_in_tray = loaded_state.preferences.run_in_tray;
        let state = Arc::new(RwLock::new(loaded_state));
        
        // Spawn backend
        let state_clone = state.clone();
        let backend = runtime.block_on(async {
            crate::backend::spawn_backend(state_clone).await
        })?;

        // Initialize system tray icon if enabled
        let tray = if run_in_tray {
            match crate::tray::SystemTray::new() {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::warn!("Failed to initialize system tray icon: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        Ok(Self {
            state,
            backend: Some(backend),
            runtime,
            show_settings: false,
            tray,
        })
    }
    
    fn backend(&self) -> &BackendHandle {
        self.backend.as_ref().expect("Backend not initialized")
    }
}

impl eframe::App for WarustApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Track focus so incoming messages only toast when the user is elsewhere.
        let focused = ui.input(|i| i.viewport().focused).unwrap_or(true);
        crate::notifications::set_window_focused(focused);

        // Handle system tray messages
        if let Some(tray) = &self.tray {
            while let Ok(msg) = tray.rx.try_recv() {
                match msg {
                    crate::tray::TrayMessage::ShowWindow => {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    crate::tray::TrayMessage::QuitApp => {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        // Intercept close event if close-to-tray is enabled
        if ui.input(|i| i.viewport().close_requested()) {
            let close_to_tray = self.state.read().preferences.close_to_tray && self.tray.is_some();
            if close_to_tray {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // Update connection status from backend
        if let Some(backend) = &self.backend {
            let status = backend.connection_status();
            let mut state = self.state.write();
            if state.connection != status {
                state.connection = status;
            }
        }
        
        // Top bar - always show
        Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("Warust").font(FontId::proportional(18.0)).strong());
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    
                    // Settings button
                    if ui.button("⚙").on_hover_text("Settings").clicked() {
                        self.show_settings = true;
                    }

                    // Scheduled messages manager button
                    let pending_scheduled = self.state.read().scheduled_messages.iter()
                        .filter(|m| m.status == crate::state::ScheduledStatus::Pending)
                        .count();
                    let sched_label = if pending_scheduled > 0 {
                        format!("🕒 Scheduled ({})", pending_scheduled)
                    } else {
                        "🕒 Scheduled".to_string()
                    };
                    if ui.button(sched_label).on_hover_text("Manage scheduled messages").clicked() {
                        self.state.write().show_scheduled_manager = true;
                    }

                    // Global message search button
                    if ui.button("🔍 Search").on_hover_text("Search in messages with filters").clicked() {
                        let mut s = self.state.write();
                        s.show_search_dialog = true;
                        s.search_filter.scope = crate::search::SearchScope::AllChats;
                    }
                    
                    // Connection status indicator
                    let status = self.state.read().connection.clone();
                    let (icon, color) = match status {
                        ConnectionStatus::Connected => ("●", egui::Color32::GREEN),
                        ConnectionStatus::Connecting => ("◐", egui::Color32::YELLOW),
                        ConnectionStatus::Syncing(_) => ("⟳", egui::Color32::YELLOW),
                        ConnectionStatus::QrCode(_) | ConnectionStatus::PairCode(_) => ("○", egui::Color32::BLUE),
                        ConnectionStatus::Error(_) => ("●", egui::Color32::RED),
                        _ => ("○", egui::Color32::GRAY),
                    };
                    ui.colored_label(color, icon);
                    
                    ui.add_space(8.0);
                });
            });
        });
        
        // Show QR pairing if user has no credentials and needs pairing
        let show_pairing = {
            let s = self.state.read();
            s.self_jid.is_none()
        };
        
        if show_pairing {
            CentralPanel::default().show(ui, |ui| {
                qr_pairing::show_qr_pairing(ui, &self.state, self.backend());
            });
        } else {
            // Three-panel layout with conversations is ALWAYS visible
            Panel::left("chat_list")
                .resizable(true)
                .default_size(360.0)
                .min_size(260.0)
                .max_size(480.0)
                .show(ui, |ui| {
                    chat_list::show_chat_list(ui, &self.state, self.backend());
                });
            
            CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::from_rgb(0x11, 0x1B, 0x21)))
                .show(ui, |ui| {
                    conversation::show_conversation(ui, &self.state, self.backend());
                });
        }
        
        // Settings window
        let backend = self.backend.as_ref();
        if self.show_settings {
            let ctx = ui.ctx();
            let state = &self.state;
            if let Some(backend) = backend {
                settings::show_settings_with_ctx(ctx, state, backend, &mut self.show_settings);
            }
        }

        // Active modal dialogs (Search, Export, Scheduled Messages)
        if let Some(backend) = self.backend.as_ref() {
            let ctx = ui.ctx();
            crate::ui::search_dialog::show_search_dialog(ctx, &self.state, backend);
            crate::ui::export_dialog::show_export_dialog(ctx, &self.state, backend);
            crate::ui::schedule_dialog::show_schedule_dialog(ctx, &self.state, backend);
            crate::ui::schedule_dialog::show_scheduled_manager(ctx, &self.state, backend);
            crate::ui::conversation::show_reaction_detail_modal(ctx, &self.state, backend);
            crate::ui::call_dialog::show_call_overlay(ctx, &self.state, backend);
        }

        // Sync runs quietly on the background without any popup toast
        // Request repaint for animations
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
    
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = _gl;
        info!("Shutting down Warust...");
        
        if let Some(backend) = self.backend.take() {
            let _ = self.runtime.block_on(backend.shutdown());
        }
        
        // Persist final state
        let _ = self.runtime.block_on(crate::storage::persist_state(&self.state));
        
        info!("Shutdown complete");
    }
}