//! O app mínimo: uma janela com um texto e um botão.
//!
//!     cargo run --release -p glove-ui --example ola
//!
//! Roda dentro do Glove (ou de outro desktop Wayland). Tudo que aparece é
//! redesenhado só quando algo acontece (um clique, uma tecla).

use glove_ui::{run, tr, App, Ui, WindowOptions};

struct Ola {
    cliques: u32,
}

impl App for Ola {
    // Chamado a cada quadro: descreve a janela inteira, de cima para baixo.
    fn ui(&mut self, ui: &mut Ui) {
        ui.heading(tr!("Olá, Glove!"));
        ui.label(&tr!("Você clicou {} vezes.", self.cliques));
        ui.space(8.0);
        // O botão devolve verdadeiro no quadro em que foi clicado.
        if ui.button(tr!("Clique aqui")) {
            self.cliques += 1;
        }
    }
}

fn main() {
    let opts = WindowOptions { title: tr!("Olá").into(), app_id: "exemplo.Ola".into(), size: (320, 150), fixed: true, min_size: None };
    if let Err(e) = run(opts, Ola { cliques: 0 }) {
        eprintln!("{e}");
    }
}
