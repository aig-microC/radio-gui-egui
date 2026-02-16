//! # radio-gui
//!
//! programa para oir emisoras de internet con Interfaz gráfica.
//!
//! **Autor:** AIG / asistente de IA de Google
//! **Fecha:** 2025-02-15
//! **Revisión:** 
//! **Fecha:** 
//! **Licencia:** MIT, https://opensource.org/license/mit
//! **Repositorio:** https://github.com/aig-microC 
//! El programa necesita tener instalado en el sistema vlc (https://images.videolan.org/vlc/index.es.html)
//! En Debian: sudo apt install vlc -y
//!

use eframe::egui;
use chrono::{Local, Timelike};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let mut estado = Radio::load().unwrap_or_else(|_| Radio::default());
    estado.reproducir_actual();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 340.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Radio GUI - VLC",
        options,
        Box::new(|_cc| Ok(Box::new(estado))),
    )
}

struct Emisora { nombre: String, url: String }

#[derive(PartialEq)]
enum ModoReproduccion { Normal, EsperandoNoticias, EnNoticias }

struct Radio {
    lista_emisoras: Vec<Emisora>,
    noticias_url: String,
    inicio_noticias: u32,
    fin_noticias: u32,
    ultima_estacion: usize,
    proceso_vlc: Option<Child>,
    modo: ModoReproduccion,
    minutos_temporizador: u32,
    instante_final: Option<Instant>,
}

impl Default for Radio {
    fn default() -> Self {
        Self {
            lista_emisoras: Vec::new(),
            noticias_url: String::new(),
            inicio_noticias: 0,
            fin_noticias: 0,
            ultima_estacion: 0,
            proceso_vlc: None,
            modo: ModoReproduccion::Normal,
            minutos_temporizador: 0,
            instante_final: None,
        }
    }
}

impl Radio {
    fn get_base_path() -> PathBuf {
        std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let base = Self::get_base_path();
        let contenido = fs::read_to_string(base.join("emisoras.m3u"))?;
        let mut lista_emisoras = Vec::new();
        let mut nombre_temp = String::from("Desconocida");

        for linea in contenido.lines() {
            let l = linea.trim();
            if l.starts_with("#EXTINF:") {
                if let Some((_, n)) = l.rsplit_once(',') { nombre_temp = n.to_string(); }
            } else if !l.is_empty() && !l.starts_with('#') {
                lista_emisoras.push(Emisora { nombre: nombre_temp.clone(), url: l.to_string() });
                nombre_temp = String::from("Desconocida");
            }
        }

        let noticias_url = fs::read_to_string(base.join("noticias.m3u"))?
            .lines().find(|l| !l.is_empty() && !l.starts_with('#'))
            .unwrap_or_default().to_string();

        let tiempos = fs::read_to_string(base.join("minutos_noticias.txt"))?;
        let mut lineas = tiempos.lines();
        let inicio = lineas.next().unwrap_or("0").trim().parse()?;
        let fin = lineas.next().unwrap_or("0").trim().parse()?;
        let ultima = fs::read_to_string(base.join("última_estación.txt"))?.trim().parse().unwrap_or(0);

        Ok(Self { lista_emisoras, noticias_url, inicio_noticias: inicio, fin_noticias: fin, ultima_estacion: ultima, proceso_vlc: None, modo: ModoReproduccion::Normal, minutos_temporizador: 0, instante_final: None })
    }

    fn reproducir_url(&mut self, url: &str) {
        self.detener_vlc();
        let _ = Command::new("cvlc").arg(url).spawn().map(|p| self.proceso_vlc = Some(p));
    }

    fn reproducir_actual(&mut self) {
        if let Some(e) = self.lista_emisoras.get(self.ultima_estacion) {
            let url = e.url.clone();
            self.reproducir_url(&url);
        }
    }

    fn detener_vlc(&mut self) {
        if let Some(mut hijo) = self.proceso_vlc.take() { let _ = hijo.kill(); }
    }

    fn guardar_indice(&self) {
        let base = Self::get_base_path();
        let _ = fs::write(base.join("última_estación.txt"), self.ultima_estacion.to_string());
    }

    fn toggle_noticias(&mut self) {
        if self.modo == ModoReproduccion::Normal {
            self.modo = ModoReproduccion::EsperandoNoticias;
        } else {
            if self.modo == ModoReproduccion::EnNoticias { self.reproducir_actual(); }
            self.modo = ModoReproduccion::Normal;
        }
    }

    fn gestionar_temporizador(&mut self) {
        if self.minutos_temporizador == 0 { self.minutos_temporizador = 90; }
        else if self.minutos_temporizador > 10 { self.minutos_temporizador -= 10; }
        else { self.minutos_temporizador = 0; }

        self.instante_final = if self.minutos_temporizador > 0 {
            Some(Instant::now() + Duration::from_secs((self.minutos_temporizador * 60) as u64))
        } else { None };
    }

    fn siguiente(&mut self) {
        if !self.lista_emisoras.is_empty() {
            self.ultima_estacion = (self.ultima_estacion + 1) % self.lista_emisoras.len();
            self.guardar_indice();
            self.reproducir_actual();
        }
    }

    fn anterior(&mut self) {
        if !self.lista_emisoras.is_empty() {
            self.ultima_estacion = if self.ultima_estacion == 0 { self.lista_emisoras.len() - 1 } else { self.ultima_estacion - 1 };
            self.guardar_indice();
            self.reproducir_actual();
        }
    }
}

impl Drop for Radio { fn drop(&mut self) { self.detener_vlc(); } }

impl eframe::App for Radio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let minuto_actual = Local::now().minute();

        // Lógica Automática de Noticias
        if self.modo == ModoReproduccion::EsperandoNoticias && minuto_actual == self.inicio_noticias {
            let url = self.noticias_url.clone();
            self.reproducir_url(&url);
            self.modo = ModoReproduccion::EnNoticias;
        } else if self.modo == ModoReproduccion::EnNoticias && minuto_actual == self.fin_noticias {
            self.reproducir_actual();
            self.modo = ModoReproduccion::EsperandoNoticias;
        }

        // Lógica Temporizador
        if let Some(fin) = self.instante_final {
            if Instant::now() >= fin {
                self.guardar_indice();
                self.detener_vlc();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        ctx.request_repaint_after(Duration::from_secs(1));

        // --- LÓGICA DE TECLADO ---
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { 
            self.guardar_indice();
            self.detener_vlc(); 
            ctx.send_viewport_cmd(egui::ViewportCommand::Close); 
        }
        if ctx.input(|i| i.key_pressed(egui::Key::A)) { self.anterior(); }
        if ctx.input(|i| i.key_pressed(egui::Key::S)) { self.siguiente(); }
        if ctx.input(|i| i.key_pressed(egui::Key::N)) { self.toggle_noticias(); }
        if ctx.input(|i| i.key_pressed(egui::Key::T)) { self.gestionar_temporizador(); }

        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let row_1_3 = rect.top() + (rect.height() * 0.33);
            let tam_base = 24.0;

            // 1. Dibujar Emisora
            let nombre = self.lista_emisoras.get(self.ultima_estacion).map(|e| e.nombre.as_str()).unwrap_or("Radio");
            let galley_nom = ui.painter().layout_no_wrap(nombre.to_string(), egui::FontId::proportional(tam_base), ui.visuals().strong_text_color());
            ui.painter().galley(egui::pos2(rect.center().x - (galley_nom.size().x / 2.0), row_1_3), galley_nom, ui.visuals().strong_text_color());

            // 2. Subtítulos
            let mut offset_y = 35.0;
            if self.modo != ModoReproduccion::Normal {
                let (txt, col) = if self.modo == ModoReproduccion::EnNoticias { 
                    ("🔴 En Noticias".to_string(), egui::Color32::RED) 
                } else { 
                    (format!("⏳ Listo para noticias ({:02})", self.inicio_noticias), egui::Color32::KHAKI) 
                };
                let g = ui.painter().layout_no_wrap(txt, egui::FontId::proportional(tam_base * 0.4), col);
                ui.painter().galley(egui::pos2(rect.center().x - (g.size().x / 2.0), row_1_3 + offset_y), g, col);
                offset_y += 20.0;
            }

            if let Some(fin) = self.instante_final {
                let resta = (fin.duration_since(Instant::now()).as_secs() / 60) + 1;
                let txt = format!("⏱ Apagado en {} min", resta);
                let g = ui.painter().layout_no_wrap(txt, egui::FontId::proportional(tam_base * 0.4), egui::Color32::LIGHT_BLUE);
                ui.painter().galley(egui::pos2(rect.center().x - (g.size().x / 2.0), row_1_3 + offset_y), g, egui::Color32::LIGHT_BLUE);
            }

            // --- FILA BOTONES 3/4 ---
            let area_btns = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + rect.height()*0.75), egui::pos2(rect.right(), rect.top() + rect.height()*0.85));
            ui.scope_builder(egui::UiBuilder::new().max_rect(area_btns), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    if ui.button("(a)nterior").clicked() { self.anterior(); }
                    if ui.button("(s)iguiente").clicked() { self.siguiente(); }

                    if self.modo != ModoReproduccion::Normal {
                        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::YELLOW;
                        ui.visuals_mut().widgets.inactive.fg_stroke.color = egui::Color32::WHITE;
                    }
                    if ui.button("(n)oticias").clicked() { self.toggle_noticias(); }
                    ui.reset_style();

                    if self.minutos_temporizador > 0 {
                        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::from_rgb(0, 200, 200);
                        ui.visuals_mut().widgets.inactive.fg_stroke.color = egui::Color32::WHITE;
                    }
                    if ui.button("(t)emporización").clicked() { self.gestionar_temporizador(); }
                    ui.reset_style();
                });
            });

            // --- SALIR 4/4 ---
            let area_salir = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + rect.height()*0.90), egui::pos2(rect.right(), rect.bottom()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(area_salir), |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.add_space(28.0);
                    let btn_salir = egui::Button::new(egui::RichText::new("Salir del Programa").color(egui::Color32::LIGHT_RED));
                    if ui.add(btn_salir).clicked() { 
                        self.guardar_indice();
                        self.detener_vlc(); 
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close); 
                    }
                });
            });
        });
    }
}
