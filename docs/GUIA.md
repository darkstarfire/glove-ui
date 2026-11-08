# Guia da glove-ui

*In English: [GUIDE.md](GUIDE.md)*

A glove-ui é a biblioteca com que os apps do Glove são feitos: janelas Wayland, controles no
visual do Mac OS X Tiger (gel, listras, metal escovado) desenhados por código, menus na barra
global do Glove, folhas de Abrir/Salvar, impressão, arrastar e soltar e traduções.

## Começar

O jeito mais rápido é copiar a pasta [`modelo/`](../modelo) e trocar o nome do app:

```sh
cp -r modelo ~/meu-app && cd ~/meu-app
cargo run --release
```

Ou, num projeto seu, acrescente a dependência:

```toml
[dependencies]
glove-ui = { git = "https://github.com/fabioaacarneiro/glove-ui", tag = "v0.2.7" }
```

Precisa do Rust 1.85 ou mais novo e das bibliotecas de desenvolvimento do `xkbcommon`
(`libxkbcommon-dev` no Debian/Ubuntu). O app roda no Glove e em outros desktops Wayland.

## Como funciona

A interface é em **modo imediato**: o app guarda o estado e, a cada quadro, a função `ui`
descreve a janela inteira a partir dele. Não há objetos de botão para criar ou apagar: um botão
é uma chamada que desenha e devolve se foi clicado.

```rust
use glove_ui::{run, tr, App, Ui, WindowOptions};

struct Ola { cliques: u32 }

impl App for Ola {
    fn ui(&mut self, ui: &mut Ui) {
        ui.heading(tr!("Olá, Glove!"));
        if ui.button(tr!("Clique aqui")) {
            self.cliques += 1;
        }
        ui.label(&tr!("Você clicou {} vezes.", self.cliques));
    }
}

fn main() {
    let opts = WindowOptions { title: "Olá".into(), app_id: "org.exemplo.Ola".into(), size: (320, 150), fixed: true, min_size: None };
    run(opts, Ola { cliques: 0 }).unwrap();
}
```

O quadro só é desenhado quando algo acontece (clique, tecla, mudança de configuração): parado,
o app não gasta processador. Para animar ou atualizar sozinho (um relógio), devolva um intervalo
em `App::tick`.

## O trait `App`

| Método | Para quê |
|---|---|
| `ui(&mut self, ui)` | **Obrigatório.** Desenha a janela. |
| `menus(&self) -> Vec<Menu>` | Os menus do app na barra de menu do Glove. |
| `tick(&self) -> Option<Duration>` | Redesenhar nesse ritmo mesmo sem eventos. |
| `on_tick(&mut self)` | Chamado a cada batida do `tick`. |
| `close_requested(&mut self) -> bool` | Devolva `false` para perguntar antes de fechar (ex.: salvar). |
| `config_changed(&mut self)` | O usuário mudou o tema ou outra preferência do Glove. |

Para fechar a janela pelo código: `ui.close = true`.

## Organizar a janela

- Os controles vão **de cima para baixo**; `ui.space(px)` dá um respiro.
- `ui.horizontal(|ui| { … })` põe lado a lado.
- `ui.form_row("Nome:", largura_do_rotulo, |ui| { … })`: rótulo à direita e o controle ao lado,
  como nos painéis das Preferências.
- `ui.area(Rect::new(x, y, w, h), |ui| { … })`: posição exata.
- `ui.available_width()`: a largura que sobra.
- `ui.tab_box(&mut aba, &["Um", "Dois"], altura, |ui| { … })` e `ui.tabs(…)`: abas.

## Controles

| Chamada | O que faz |
|---|---|
| `label`, `heading`, `small`, `paragraph` | Textos (normal, título, pequeno, com quebra de linha) |
| `button(t) -> bool`, `default_button(t)` | Botões; o padrão é azul pulsante e responde ao Enter |
| `checkbox(&mut bool, t)`, `radio_group(&mut usize, &[t])` | Caixa de seleção e botões de rádio |
| `popup(&mut usize, &[String], largura)` | Menu de opções |
| `slider(&mut f32, min, max, largura)` | Controle deslizante |
| `text_field(&mut String, largura)`, `search_field(…)`, `text_area(&mut TextBuffer, r)` | Campos de texto |
| `list(&mut Option<usize>, &[String], w, h)`, `table(…)` | Lista e tabela com colunas |
| `segmented_text(&[t], Some(i))` | Botões segmentados |
| `progress(Some(0.5), largura)`, `spinner(tamanho)` | Andamento |
| `icon(…)`, `icon_tile(…)` | Ícones no estilo do Glove (`glove_ui::paint::icons::Icon`) |
| `alert(tipo, título, texto, &[botões]) -> Option<usize>` | Alerta em folha; o último botão é o padrão e "Cancelar" responde ao Esc |
| `tab_strip(r, &títulos, ativa) -> Option<TabAction>` | Faixa de abas com × e +; devolve `Select`, `Close` ou `New` (altura `TAB_STRIP_H`) |

## Várias janelas e descritores

- `ui.open_window(id, opções)` abre outra janela do mesmo app; `ui.window_id()` diz qual está
  sendo desenhada e `App::window_closed(id)` avisa quando uma fecha.
- `ui.redraw_all_windows()` redesenha as outras janelas depois deste quadro (ex.: uma prévia que
  acompanha a seleção da janela principal).
- `App::watch_fd` observa um descritor desde o começo; `ui.watch_fd(fd)` acrescenta outros depois
  (ex.: o shell de uma aba nova). Quando um tem dados, o kit chama `App::fd_ready_on(fd)`.

## Menus na barra do Glove

```rust
fn menus(&self) -> Vec<Menu> {
    vec![Menu::new(tr!("Arquivo"), vec![
        MenuItem::new("novo", tr!("Novo"), "Ctrl+N"),
        MenuItem::Separator,
        MenuItem::new("apagar", tr!("Apagar"), "Delete").enabled(self.algo_escolhido),
    ])]
}

fn ui(&mut self, ui: &mut Ui) {
    for acao in ui.menu_actions().to_vec() {
        match acao.as_str() { "novo" => { /* … */ } _ => {} }
    }
    // …
}
```

O atalho de um item faz o mesmo que escolhê-lo. Os menus aparecem quando o app é aberto pelo
Glove (o Dock, a Busca Rápida, o Terminal…), que liga o app à barra; fora do Glove, os atalhos
continuam valendo.

## Abrir e salvar, imprimir

`FileBrowser` mostra a folha de Abrir/Salvar do Glove (`ui.file_sheet(&mut fb)` devolve a
escolha). A impressão (CUPS, Salvar PDF, Pré-visualizar) fica no módulo `print`.

## Traduções

Escreva os textos no idioma de origem dentro de `tr!` (`tr!("Abrir")`, `tr!("{} itens", n)`) e
ponha as traduções em `locales/<idioma>.po`, no formato do gettext:

```po
msgid "Abrir"
msgstr "Open"
```

Aponte a variável `GLOVE_LOCALES` para essa pasta na compilação (o modelo já faz isso em
`.cargo/config.toml`). O idioma segue o `LANG` do sistema; `GLOVE_LANG=en` força um. Os textos
da própria glove-ui (Abrir, Salvar, Cancelar…) já vêm traduzidos.

Os apps do Glove escrevem em português e trazem `en.po`; um app escrito em inglês pode trazer um
`pt.po` do mesmo jeito.

## Desenho próprio

`ui.c` é o canvas da janela (`glove_ui::paint`): retângulos arredondados, gradientes, gel,
metal, sombras e texto (`ui.fonts`). As cores do tema estão em `ui.theme`.

## Exemplos

```sh
cargo run --release -p glove-ui --example ola         # o mínimo
cargo run --release -p glove-ui --example formulario  # campos, opções, alerta
cargo run --release -p glove-ui --example lista       # lista com busca e menus
```

Os 32 apps do [Glove](https://github.com/fabioaacarneiro/glove) (pasta `crates/`) são exemplos
maiores: Arquivos, Preferências, Editor, Terminal, Correio…
