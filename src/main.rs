#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Token {
    name: String,
    address: String,
    creator: String,
    created_at: String,
    curve_percent: f32,
    buys: u64,
    sells: u64,
    volume_sol: f64,
    liquidity_sol: f64,
    notes: String,
    watched: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Data { tokens: Vec<Token>, curve_alert: f32, volume_alert: f64 }
impl Default for Data {
    fn default() -> Self { Self { tokens: vec![], curve_alert: 85.0, volume_alert: 100.0 } }
}
struct App { data: Data, path: PathBuf, import_path: String, query: String, tab: usize, selected: Option<usize>, status: String }
impl App {
    fn new() -> Self {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()).join("PumpfunLaunchTracker");
        let path = base.join("tracker-data.json");
        let (data, status) = match fs::read_to_string(&path) {
            Ok(s) => match serde_json::from_str(&s) { Ok(d) => (d, "Local data loaded".into()), Err(e) => (Data::default(), format!("Could not parse saved data: {e}")) },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Data::default(), "Import a JSON snapshot to begin".into()),
            Err(e) => (Data::default(), format!("Could not read saved data: {e}")),
        };
        Self { data, path, import_path: String::new(), query: String::new(), tab: 0, selected: None, status }
    }
    fn save_data(&mut self) {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            fs::create_dir_all(self.path.parent().unwrap())?;
            fs::write(&self.path, serde_json::to_string_pretty(&self.data)?)?;
            Ok(())
        })();
        self.status = match result { Ok(()) => format!("Saved to {}", self.path.display()), Err(e) => format!("Save failed: {e}") };
    }
    fn import(&mut self) {
        let result = (|| -> Result<Vec<Token>, Box<dyn std::error::Error>> {
            let tokens: Vec<Token> = serde_json::from_str(&fs::read_to_string(self.import_path.trim())?)?;
            for t in &tokens {
                if t.address.trim().is_empty() || !t.curve_percent.is_finite() || !(0.0..=100.0).contains(&t.curve_percent) || !t.volume_sol.is_finite() || t.volume_sol < 0.0 || !t.liquidity_sol.is_finite() || t.liquidity_sol < 0.0 {
                    return Err("Invalid address, curve percentage, volume or liquidity".into());
                }
            }
            Ok(tokens)
        })();
        match result {
            Err(e) => self.status = format!("Import failed: {e}"),
            Ok(tokens) => {
                let count = tokens.len();
                for mut token in tokens {
                    if let Some(old) = self.data.tokens.iter_mut().find(|t| t.address == token.address) {
                        token.notes = old.notes.clone(); token.watched = old.watched; *old = token;
                    } else { self.data.tokens.push(token); }
                }
                self.selected = None; self.save_data(); self.status = format!("Imported {count} records. {}", self.status);
            }
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        for (key, tab) in [(egui::Key::F1,0),(egui::Key::F2,1),(egui::Key::F3,2),(egui::Key::F4,3),(egui::Key::F5,4),(egui::Key::F6,5)] {
            if ctx.input(|i| i.key_pressed(key)) { self.tab = tab; }
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.heading("Pump.fun / Launch Research");
            ui.label("OFFLINE SNAPSHOTS · No live feed · No wallet connection");
            ui.horizontal(|ui| { ui.label("JSON file:"); ui.text_edit_singleline(&mut self.import_path); if ui.button("Import / update").clicked() { self.import(); } if ui.button("Save").clicked() { self.save_data(); } });
            ui.horizontal(|ui| { for (i, name) in ["Timeline", "Bonding curve", "Early trades", "Creator wallets", "Watchlist", "Alerts"].iter().enumerate() { ui.selectable_value(&mut self.tab, i, *name); } });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| { ui.label(&self.status); ui.small("Research only. Snapshot values may be stale. Never enter private keys or seed phrases."); });
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.tab == 5 {
                ui.heading("Snapshot alert rules");
                ui.label("Matching records appear below while this window is open. No background or push notifications.");
                ui.add(egui::Slider::new(&mut self.data.curve_alert, 0.0..=100.0).text("Minimum curve %"));
                ui.add(egui::DragValue::new(&mut self.data.volume_alert).range(0.0..=1e12).prefix("Minimum volume SOL: "));
                for t in &self.data.tokens { if t.curve_percent >= self.data.curve_alert && t.volume_sol >= self.data.volume_alert { ui.label(format!("ALERT: {} · {:.1}% · {:.2} SOL", t.name, t.curve_percent, t.volume_sol)); } }
                return;
            }
            ui.horizontal(|ui| { ui.label("Search name / mint / creator"); ui.text_edit_singleline(&mut self.query); });
            let query = self.query.to_lowercase();
            let mut indices: Vec<usize> = (0..self.data.tokens.len()).filter(|&i| {
                let t = &self.data.tokens[i];
                (self.tab != 4 || t.watched) && format!("{} {} {}", t.name,t.address,t.creator).to_lowercase().contains(&query)
            }).collect();
            indices.sort_by(|&a,&b| self.data.tokens[b].created_at.cmp(&self.data.tokens[a].created_at));
            ui.label(format!("{} records · creation times supplied by import", indices.len()));
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for i in indices {
                    let t = &self.data.tokens[i];
                    let detail = match self.tab { 1 => format!("{:.1}% curve",t.curve_percent), 2 => format!("{} buys / {} sells · {:.2} SOL",t.buys,t.sells,t.volume_sol), 3 => t.creator.clone(), _ => t.created_at.clone() };
                    if ui.selectable_label(self.selected == Some(i), format!("{}   |   {}   |   {}",t.name, detail,t.address)).clicked() { self.selected = Some(i); }
                }
            });
            ui.separator();
            if let Some(i) = self.selected {
                let t = &mut self.data.tokens[i];
                ui.heading(&t.name);
                ui.label(format!("Mint: {}",t.address)); ui.label(format!("Creator: {}",t.creator));
                ui.label(format!("Created: {} | Liquidity: {:.2} SOL",t.created_at,t.liquidity_sol));
                ui.add(egui::ProgressBar::new(t.curve_percent/100.0).text(format!("Bonding curve {:.1}%",t.curve_percent)));
                ui.checkbox(&mut t.watched,"Graduation watchlist");
                ui.label("Research notes (click Save to persist)"); ui.text_edit_multiline(&mut t.notes);
            } else { ui.label("Select a token to view public wallet references and add notes."); }
        });
    }
}
fn main() -> eframe::Result {
    eframe::run_native("Pump.fun Token Launch Tracker", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(App::new()))))
}
