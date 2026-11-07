//! Uma lista com busca e os menus do app na barra de menu global do Glove
//! (Arquivo ▸ Novo Item, Apagar), com atalhos de teclado.
//!
//!     cargo run --release -p glove-ui --example lista

use glove_ui::{run, tr, App, Menu, MenuItem, Ui, WindowOptions};

struct Lista {
    itens: Vec<String>,
    selecionado: Option<usize>,
    busca: String,
    novos: u32,
}

impl Lista {
    fn visiveis(&self) -> Vec<String> {
        let b = self.busca.to_lowercase();
        self.itens.iter().filter(|i| i.to_lowercase().contains(&b)).cloned().collect()
    }
}

impl App for Lista {
    // Os menus vão para a barra de menu do Glove. Cada item tem um id; o atalho,
    // quando houver, faz o mesmo que escolher o item.
    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            tr!("Arquivo"),
            vec![MenuItem::new("novo", tr!("Novo Item"), "Ctrl+N"), MenuItem::Separator, MenuItem::new("apagar", tr!("Apagar"), "Delete").enabled(self.selecionado.is_some())],
        )]
    }

    fn ui(&mut self, ui: &mut Ui) {
        // Os itens de menu escolhidos (ou os atalhos usados) desde o último quadro.
        for acao in ui.menu_actions().to_vec() {
            match acao.as_str() {
                "novo" => {
                    self.novos += 1;
                    self.itens.push(tr!("Item {}", self.novos));
                }
                "apagar" => {
                    if let Some(nome) = self.selecionado.and_then(|i| self.visiveis().get(i).cloned()) {
                        self.itens.retain(|x| *x != nome);
                        self.selecionado = None;
                    }
                }
                _ => {}
            }
        }
        ui.search_field(&mut self.busca, tr!("Buscar"), 220.0);
        ui.space(8.0);
        let linhas = self.visiveis();
        let w = ui.available_width();
        ui.list(&mut self.selecionado, &linhas, w, 200.0);
    }
}

fn main() {
    let app = Lista { itens: vec![tr!("Maçã").into(), tr!("Banana").into(), tr!("Laranja").into()], selecionado: None, busca: String::new(), novos: 0 };
    let opts = WindowOptions { title: tr!("Lista").into(), app_id: "exemplo.Lista".into(), size: (300, 300), fixed: false, min_size: Some((240, 200)) };
    if let Err(e) = run(opts, app) {
        eprintln!("{e}");
    }
}
