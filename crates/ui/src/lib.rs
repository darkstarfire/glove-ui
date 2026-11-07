//! glove-ui: a biblioteca dos apps do Glove.
//!
//! Interface em modo imediato, desenhada por software com o visual do Glove,
//! nítida em qualquer escala e sem gastar CPU quando nada acontece.
//!
//! Um app implementa [`App`] (no mínimo `ui`, que desenha a janela a cada
//! quadro) e chama [`run`]. Veja `docs/GUIA.md` e os exemplos em `examples/`.

pub mod clipfiles;
pub mod filebrowser;
pub mod image;
pub mod menu;
pub mod print;
mod panels;
mod platform;
mod text;
pub mod ui;
pub use panels::{ColorPanel, FontPanel};
pub use text::{Align, TextBuffer};

pub use glove_paint as paint;
/// Configurações do Glove (tema, escala, teclado…) que todo app segue.
pub use glove_config as config;
/// Traduções (a macro `tr!`), o idioma em uso e o separador decimal.
pub use glove_config::{idioma, tr, tr_dyn, virgula};
pub use platform::{app_icon, local_time, run, theme_from_config, use_own_frame, App, WindowOptions};
pub use menu::{Menu, MenuItem};
pub use filebrowser::{Choice, FileBrowser};
pub use ui::{disclosure_triangle, AlertKind, Column, Rect, TableResponse, TableRow, TableState, TableStyle, Ticks, Ui, SCROLLBAR_W, SEARCH_H, SEGMENT_H, SEGMENT_W, TABLE_HEADER_H};
pub use glove_paint::icons::Glyph;
