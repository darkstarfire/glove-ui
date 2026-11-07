//! Meu App: modelo de app da glove-ui. Troque o nome, o app_id e comece a desenhar.

use glove_ui::{run, tr, App, Menu, MenuItem, Ui, WindowOptions};

/// O estado do app. A interface é redesenhada a partir dele a cada quadro.
struct MeuApp {
    nota: String,
    salvas: Vec<String>,
}

impl App for MeuApp {
    /// Os menus que aparecem na barra de menu do Glove quando o app está em foco.
    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(tr!("Arquivo"), vec![MenuItem::new("limpar", tr!("Limpar Notas"), "Ctrl+L").enabled(!self.salvas.is_empty())])]
    }

    fn ui(&mut self, ui: &mut Ui) {
        for acao in ui.menu_actions().to_vec() {
            if acao == "limpar" {
                self.salvas.clear();
            }
        }
        ui.heading(tr!("Minhas notas"));
        ui.horizontal(|ui| {
            ui.text_field(&mut self.nota, 200.0);
            if ui.default_button(tr!("Guardar")) && !self.nota.trim().is_empty() {
                self.salvas.push(std::mem::take(&mut self.nota));
            }
        });
        ui.space(8.0);
        if self.salvas.is_empty() {
            ui.small(tr!("Nenhuma nota ainda."));
        }
        for n in &self.salvas {
            ui.label(&format!("• {n}"));
        }
    }
}

fn main() {
    let opts = WindowOptions {
        title: tr!("Meu App").into(),
        // Identificador do app (o Glove usa para o Dock e os menus).
        app_id: "org.exemplo.MeuApp".into(),
        size: (340, 260),
        fixed: false,
        min_size: Some((280, 200)),
    };
    if let Err(e) = run(opts, MeuApp { nota: String::new(), salvas: Vec::new() }) {
        eprintln!("{e}");
    }
}
