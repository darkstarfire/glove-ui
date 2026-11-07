//! Um formulário: campos com rótulo, menu de opções, controle deslizante,
//! botões de rádio, caixa de seleção, botão padrão (Enter) e um alerta.
//!
//!     cargo run --release -p glove-ui --example formulario

use glove_ui::{run, tr, AlertKind, App, Ui, WindowOptions};

struct Formulario {
    nome: String,
    tema: usize,
    volume: f32,
    tamanho: usize,
    avisos: bool,
    salvo: bool,
}

impl App for Formulario {
    fn ui(&mut self, ui: &mut Ui) {
        // Coluna dos rótulos, alinhados à direita, como nas Preferências.
        let lw = 100.0;
        ui.form_row(tr!("Nome:"), lw, |ui| {
            ui.text_field(&mut self.nome, 200.0);
        });
        ui.form_row(tr!("Tema:"), lw, |ui| {
            let temas = [tr!("Claro").to_string(), tr!("Grafite").to_string()];
            ui.popup(&mut self.tema, &temas, 140.0);
        });
        ui.form_row(tr!("Volume:"), lw, |ui| {
            ui.slider(&mut self.volume, 0.0, 1.0, 160.0);
        });
        ui.form_row(tr!("Tamanho:"), lw, |ui| {
            ui.radio_group(&mut self.tamanho, &[tr!("Pequeno"), tr!("Grande")]);
        });
        ui.form_row("", lw, |ui| {
            ui.checkbox(&mut self.avisos, tr!("Mostrar avisos"));
        });
        ui.space(12.0);
        // O botão padrão (azul pulsante) também responde ao Enter.
        if ui.default_button(tr!("Salvar")) {
            self.salvo = true;
        }
        // Alerta em folha: enquanto aberto, o resto da janela não responde.
        if self.salvo {
            let texto = tr!("Olá, {}!", if self.nome.is_empty() { tr!("pessoa") } else { self.nome.as_str() });
            if ui.alert(AlertKind::Note, tr!("Salvo"), &texto, &[tr!("OK")]).is_some() {
                self.salvo = false;
            }
        }
    }
}

fn main() {
    let app = Formulario { nome: String::new(), tema: 0, volume: 0.5, tamanho: 0, avisos: true, salvo: false };
    let opts = WindowOptions { title: tr!("Formulário").into(), app_id: "exemplo.Formulario".into(), size: (380, 260), fixed: true, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("{e}");
    }
}
