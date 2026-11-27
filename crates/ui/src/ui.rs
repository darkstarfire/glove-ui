//! Widgets em modo imediato: o app descreve a tela a cada quadro e o kit
//! desenha e responde ao mouse/teclado. Só redesenha quando há evento.

use glove_config::tr;
use std::collections::HashMap;

use crate::menu::MenuItem;
use crate::text::TextBuffer;
use glove_paint::{
    draw::*,
    icons::{self, Glyph, Icon},
    Color, Fonts, Theme,
};

pub type Id = u64;

/// Tipo de alerta: o ícone grande à esquerda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    /// Aviso comum: só o ícone do app.
    Note,
    /// Cuidado (ex.: perder alterações): triângulo amarelo com o ícone do app no canto.
    Caution,
    /// Problema grave: placa vermelha.
    Stop,
}

/// Quebra `text` em linhas que cabem em `max` (respeitando quebras de linha do texto).
fn wrap_lines(fonts: &Fonts, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let test = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if fonts.measure(&test, px, bold) > max && !line.is_empty() {
                out.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = test;
            }
        }
        out.push(line);
    }
    out
}

/// Marcas de um slider: quantas, rótulos (posição de 0 a 1, texto) e se o valor
/// pula de marca em marca.
pub struct Ticks<'a> {
    pub count: usize,
    pub labels: &'a [(f32, &'a str)],
    pub snap: bool,
}

/// Trilho fino e afundado dos sliders (no moderno, liso e claro, sem o brilho).
fn slider_track(c: &mut Canvas, t: Rect) {
    if glove_paint::theme::modern() {
        let dark = glove_paint::theme::dark();
        c.fill(rrect(t.x, t.y, t.w, t.h, t.h / 2.0), &solid(if dark { rgba(255, 255, 255, 40) } else { rgba(0, 0, 0, 36) }));
        return;
    }
    c.fill(rrect(t.x, t.y, t.w, t.h, t.h / 2.0), &vgrad(t.y, t.y + t.h, &[(0.0, rgba(120, 120, 126, 255)), (1.0, rgba(190, 190, 196, 255))]));
    c.hline(t.x + 2.0, t.y + t.h, t.w - 4.0, rgba(255, 255, 255, 200));
}


/// Conteúdo de uma parte do segmentado.
enum SegLabel<'a> {
    Glyph(Glyph),
    Text(&'a str),
}

/// Altura do cabeçalho das tabelas e recuo de cada nível da árvore.
pub const TABLE_HEADER_H: f32 = 17.0;
pub const TABLE_INDENT: f32 = 16.0;

/// Coluna de uma tabela. `width` é relativa (as larguras dividem a área na proporção).
/// `right` alinha à direita (números) e faz a primeira ordenação ser decrescente.
pub struct Column<'a> {
    pub title: &'a str,
    pub width: f32,
    pub right: bool,
    pub sortable: bool,
}

impl<'a> Column<'a> {
    pub fn new(title: &'a str, width: f32) -> Self {
        Self { title, width, right: false, sortable: true }
    }
    /// Alinhada à direita (números).
    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }
    pub fn fixed(mut self) -> Self {
        self.sortable = false;
        self
    }
}

/// Linha de uma tabela. `children`: `None` = folha, `Some(aberta)` = tem filhos
/// (mostra o triângulo). `depth` é o nível na árvore (recuo).
#[derive(Clone, Default)]
pub struct TableRow {
    pub cells: Vec<String>,
    pub icon: Option<Icon>,
    pub depth: u16,
    pub children: Option<bool>,
    /// Cor da linha (rótulo do Arquivos), por baixo da seleção.
    pub tint: Option<Color>,
}

/// Estado da tabela guardado pelo app: rolagem e ordenação (coluna, crescente).
#[derive(Clone, Debug, Default)]
pub struct TableState {
    pub scroll: f32,
    pub sort: Option<(usize, bool)>,
    view_h: f32,
    row_h: f32,
}

impl TableState {
    pub fn sorted_by(col: usize, ascending: bool) -> Self {
        Self { sort: Some((col, ascending)), ..Self::default() }
    }
    /// Rola o mínimo para a linha `i` ficar visível (ex.: depois das setas).
    pub fn reveal(&mut self, i: usize) {
        if self.row_h <= 0.0 {
            return;
        }
        let y = i as f32 * self.row_h;
        if y < self.scroll {
            self.scroll = y;
        } else if y + self.row_h > self.scroll + self.view_h {
            self.scroll = y + self.row_h - self.view_h;
        }
    }
}

/// Aparência da tabela.
pub struct TableStyle {
    pub row_h: f32,
    pub px: f32,
    /// Linhas alternadas em azul-claro (Monitor); o Arquivos não usa.
    pub zebra: bool,
    /// Outras linhas escolhidas além da principal (Ctrl/Shift + clique no Arquivos).
    pub also: Vec<usize>,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self { row_h: 18.0, px: 11.0, zebra: true, also: Vec::new() }
    }
}

/// O que aconteceu na tabela neste quadro.
#[derive(Clone, Copy, Debug, Default)]
pub struct TableResponse {
    /// Linha clicada (selecionar).
    pub pressed: Option<usize>,
    /// Clique duplo numa linha (abrir).
    pub double: Option<usize>,
    /// Triângulo clicado (abrir/fechar).
    pub toggled: Option<usize>,
    /// A ordenação mudou (o app reordena as linhas).
    pub sorted: bool,
}

/// Triângulo de abrir/fechar (▸ fechado, ▾ aberto), centrado em (cx, cy).
pub fn disclosure_triangle(c: &mut Canvas, cx: f32, cy: f32, open: bool, color: Color) {
    let mut pb = glove_paint::tiny_skia::PathBuilder::new();
    if open {
        pb.move_to(cx - 4.5, cy - 2.5);
        pb.line_to(cx + 4.5, cy - 2.5);
        pb.line_to(cx, cy + 3.5);
    } else {
        pb.move_to(cx - 2.5, cy - 4.5);
        pb.line_to(cx + 3.5, cy);
        pb.line_to(cx - 2.5, cy + 4.5);
    }
    pb.close();
    c.fill(pb.finish(), &solid(color));
}

/// Altura do campo de busca.
pub const SEARCH_H: f32 = 22.0;
/// Tamanho de cada parte do controle segmentado.
pub const SEGMENT_W: f32 = 28.0;
pub const SEGMENT_H: f32 = 22.0;

/// Altura da barra de progresso.
pub const PROGRESS_H: f32 = 12.0;

/// Fase de uma animação de `period_ms` (0 a 1), pelo relógio: todas andam juntas
/// e não precisam guardar estado.
fn anim_phase(period_ms: u64) -> f32 {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    (ms % period_ms) as f32 / period_ms as f32
}

/// Largura da barra de rolagem (15 pt, como na era).
pub const SCROLLBAR_W: f32 = 15.0;
const SCROLL_ARROW_H: f32 = 15.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, (px, py): (f32, f32)) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// Resultado de `Ui::hit`.
/// O que aconteceu na faixa de abas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    Select(usize),
    Close(usize),
    New,
}

/// Altura da faixa de abas.
pub const TAB_STRIP_H: f32 = 24.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Response {
    pub hover: bool,
    /// Botão pressionado sobre a área (segurando).
    pub down: bool,
    /// Acabou de pressionar neste quadro.
    pub pressed: bool,
    /// Soltou sobre a área (clique completo).
    pub clicked: bool,
    /// Segundo clique rápido (clique duplo), no momento de pressionar.
    pub double: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Delete,
    Enter,
    Escape,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// Qualquer outra tecla (para capturar atalhos): nome do keysym.
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

/// Uma tecla com modificadores, já formatada como atalho ("Super+Shift+Q").
#[derive(Clone, Debug)]
pub struct KeyPress {
    pub key: Key,
    pub combo: String,
}

/// Estado de entrada acumulado entre dois quadros.
#[derive(Default)]
pub struct Input {
    /// A janela tem o foco do teclado (é a janela ativa).
    pub active: bool,
    pub pos: (f32, f32),
    pub inside: bool,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    /// O mouse se moveu neste quadro.
    pub moved: bool,
    pub scroll: f32,
    /// 1 = clique simples, 2 = duplo, 3 = triplo (no quadro do clique).
    pub clicks: u8,
    pub keys: Vec<KeyPress>,
    /// Itens de menu (sem atalho) escolhidos na barra de menu.
    pub menu_actions: Vec<String>,
    pub text: String,
    pub mods: Mods,
    /// Botão direito pressionado neste quadro (não conta como clique comum).
    pub right_pressed: bool,
    /// A janela está em zoom (maximizada pelo Glove).
    pub maximized: bool,
    /// Botão do meio pressionado / solto neste quadro.
    pub middle_pressed: bool,
    pub middle_released: bool,
    /// Rolagem para os lados neste quadro (positivo = para a direita).
    pub scroll_x: f32,
    /// O teclado está num widget do sistema dentro da janela (backend GTK: a página de
    /// um navegador), não nos do kit.
    pub native_focus: bool,
}

impl Input {
    /// Houve clique ou tecla neste quadro?
    pub fn has_events(&self) -> bool {
        self.pressed || self.released || self.right_pressed || self.middle_pressed || self.middle_released || self.scroll != 0.0 || self.scroll_x != 0.0 || !self.keys.is_empty() || !self.text.is_empty() || !self.menu_actions.is_empty()
    }

    pub fn end_frame(&mut self) {
        self.pressed = false;
        self.right_pressed = false;
        self.middle_pressed = false;
        self.middle_released = false;
        self.scroll_x = 0.0;
        self.released = false;
        self.scroll = 0.0;
        self.moved = false;
        self.clicks = 0;
        self.keys.clear();
        self.menu_actions.clear();
        self.text.clear();
    }
}

pub(crate) struct PopupState {
    /// Aberto neste quadro: ignora o clique que o abriu.
    fresh: bool,
    pub(crate) id: Id,
    /// Onde a lista aparece dentro da janela (o desenho de reserva).
    rect: Rect,
    pub(crate) items: Vec<String>,
    pub(crate) selected: usize,
    /// O botão da lista, em coordenadas da janela (a superfície própria se alinha a ele).
    pub(crate) anchor: Rect,
    /// Superfície própria (xdg_popup): `None` = ainda não tentada (a plataforma abre
    /// depois do quadro), `Some(true)` = aberta, `Some(false)` = não deu, a lista é
    /// desenhada dentro da janela, como antes.
    pub(crate) native: Option<bool>,
}

/// Altura de cada item da lista suspensa e a folga em cima e embaixo.
pub(crate) const POPUP_ITEM_H: f32 = 20.0;
pub(crate) const POPUP_PAD: f32 = 4.0;
/// Faixa das setas de rolagem da lista mais alta que a tela (como no Tiger).
pub(crate) const POPUP_ARROW_H: f32 = 18.0;

/// Desenha a lista suspensa aberta em `r`: sombra, fundo, itens (o atual com ✓ e o
/// que está sob o mouse aceso). `scroll` desloca os itens para cima; `arrows` diz se
/// há setas de rolagem em cima e embaixo (mais itens naquela direção). É o mesmo
/// desenho dentro da janela (reserva) e na superfície própria.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_popup_list(c: &mut Canvas, fonts: &Fonts, theme: &Theme, r: Rect, items: &[String], selected: usize, hover: Option<usize>, scroll: f32, arrows: (bool, bool)) {
    let item_h = POPUP_ITEM_H;
    if let Some(sh) = Canvas::shadow(-10.0, 0.0, r.w + 20.0, r.h + 12.0, r.w, r.h, &ShadowSpec { blur: 10.0, offset_y: 3.0, radius: 5.0, alpha: 0.35 }, c.scale) {
        c.draw_canvas(&sh, r.x - 10.0, r.y);
    }
    let bg = rgba(255, 255, 255, 245);
    c.fill(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(bg));
    c.stroke(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(rgba(0, 0, 0, 60)), 1.0 / c.scale);
    // Com rolagem, os itens ficam entre as faixas das setas (recortados).
    let (top, bottom) = (r.y + if arrows.0 { POPUP_ARROW_H } else { 0.0 }, r.y + r.h - if arrows.1 { POPUP_ARROW_H } else { 0.0 });
    let clipped = scroll > 0.0 || arrows.0 || arrows.1;
    if clipped {
        let s = c.scale;
        c.set_clip(Some([(r.x * s).floor() as i32, (top * s).floor() as i32, ((r.x + r.w) * s).ceil() as i32, (bottom * s).ceil() as i32]));
    }
    let first = ((scroll - POPUP_PAD) / item_h).floor().max(0.0) as usize;
    for (i, it) in items.iter().enumerate().skip(first) {
        let y = r.y + POPUP_PAD + i as f32 * item_h - scroll;
        if y >= bottom {
            break;
        }
        let hot = hover == Some(i);
        if hot {
            c.fill_rect(r.x, y, r.w, item_h, &vgrad(y, y + item_h, &[(0.0, lighten(theme.accent, 0.2)), (1.0, darken(theme.accent, 0.1))]));
        }
        let color = if hot { Color::WHITE } else { ink() };
        if i == selected {
            fonts.draw(c, "✓", r.x + 6.0, y + 15.0, 13.0, false, color);
        }
        fonts.draw(c, it, r.x + 22.0, y + 15.0, 13.0, false, color);
    }
    if clipped {
        c.set_clip(None);
    }
    // Setas de rolagem: um triângulo no meio da faixa.
    for (on, y0, up) in [(arrows.0, r.y, true), (arrows.1, r.y + r.h - POPUP_ARROW_H, false)] {
        if !on {
            continue;
        }
        let (cx, cy) = (r.x + r.w / 2.0, y0 + POPUP_ARROW_H / 2.0);
        let d = if up { -1.0 } else { 1.0 };
        let mut pb = glove_paint::tiny_skia::PathBuilder::new();
        pb.move_to(cx - 5.0, cy - 2.5 * d);
        pb.line_to(cx + 5.0, cy - 2.5 * d);
        pb.line_to(cx, cy + 2.5 * d);
        pb.close();
        // Um pouco transparente: o modo escuro não troca a cor (fica clara no fundo escuro).
        c.fill(pb.finish(), &solid(with_alpha(ink(), 0.85)));
    }
}

/// Alerta pedido pelo app neste quadro, para a janela própria dele.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AlertReq {
    pub key: u64,
    pub kind: AlertKind,
    pub title: String,
    pub text: String,
    pub buttons: Vec<String>,
}

/// Formato do cursor do mouse sobre a janela (o Glove desenha cada um).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Arrow,
    /// Mãozinha: links e coisas clicáveis numa página.
    Hand,
    /// Barra de texto: campos e textos que se escolhem.
    Text,
}

/// Memória que sobrevive entre quadros.
#[derive(Default)]
pub struct Memory {
    /// O cursor pedido neste quadro (`Ui::set_cursor`; volta à seta a cada quadro).
    pub(crate) mouse_cursor: Cursor,
    /// Alertas em janela própria (regra do Glove: confirmar algo nunca fica dentro da
    /// janela do app): o pedido deste quadro, as respostas que chegaram e se há um aberto.
    pub(crate) alert_native: bool,
    pub(crate) alert_req: Option<AlertReq>,
    pub(crate) alert_results: Vec<(u64, usize)>,
    pub(crate) alert_open: bool,
    active: Option<Id>,
    focus: Option<Id>,
    /// Os campos de texto na ordem em que foram desenhados neste quadro (a ordem do Tab).
    focus_order: Vec<Id>,
    /// Tab (1) ou Shift+Tab (-1) num campo: o cursor vai para o próximo no fim do quadro.
    tab_req: Option<i32>,
    /// O campo que acabou de receber o cursor pelo Tab: escolhe o texto todo (como no Mac).
    focus_enter: Option<Id>,
    pub(crate) popup: Option<PopupState>,
    /// Item escolhido no pop-up no quadro anterior: (id, índice).
    pub(crate) popup_choice: Option<(Id, usize)>,
    /// A plataforma abre a lista suspensa numa superfície própria (pode passar das
    /// bordas da janela). Sem isso, a lista é desenhada dentro da janela.
    pub(crate) native_popups: bool,
    scroll: HashMap<Id, f32>,
    cursor: HashMap<Id, usize>,
    slider_grab: Option<Id>,
    capture: Option<Id>,
    /// Áreas que reagem ao mouse passando por cima, no último quadro. Se o mouse
    /// se mexe sem entrar nem sair de nenhuma, o quadro seria igual: não redesenha.
    pub(crate) hover_rects: Vec<Rect>,
    /// O último quadro leu a posição do mouse direto (arrastar, seguir o ponteiro):
    /// todo movimento redesenha.
    pub(crate) pointer_sensitive: bool,
    /// O app garantiu que este quadro já saiu certo (ver `Ui::settled`).
    pub(crate) settled: bool,
    /// Texto copiado neste quadro (o kit publica na área de transferência).
    pub(crate) clip_out: Option<String>,
    /// Arquivos copiados neste quadro (e se é "recortar").
    pub(crate) clip_files: Option<(Vec<std::path::PathBuf>, bool)>,
    /// Colar pedindo arquivos (text/uri-list) antes de texto.
    pub(crate) paste_files: bool,
    /// Arrastar arquivos para fora (outra janela, outro app, a mesa).
    pub(crate) drag_out: Option<Vec<std::path::PathBuf>>,
    /// Algo sendo arrastado por cima desta janela (de qualquer app).
    pub(crate) dnd_hover: bool,
    /// Arquivos soltos aqui são só lidos (ex.: o Terminal escreve o caminho): pede "copiar", nunca "mover".
    pub(crate) drop_copy: bool,
    /// Arquivos soltos nesta janela e onde.
    pub(crate) dropped: Option<(Vec<std::path::PathBuf>, (f32, f32))>,
    /// Os buffers de edição dos campos de texto de uma linha (`text_field`,
    /// `text_field_in`), pelo id, e os que apareceram neste quadro.
    pub(crate) linhas: HashMap<Id, TextBuffer>,
    pub(crate) linhas_usadas: Vec<Id>,
    /// O campo que abriu o menu do botão direito (as ações dele voltam para esse campo).
    pub(crate) campo_menu: Option<Id>,
    /// Widget que pediu para colar.
    pub(crate) paste_req: Option<Id>,
    /// Texto colado chegando para um widget.
    pub(crate) paste_in: Option<(Id, String)>,
    /// Novo título da janela.
    pub(crate) title: Option<String>,
    /// Pedidos para a janela: redimensionar pelo puxador, minimizar.
    pub(crate) resize_req: bool,
    /// Comandos para o Glove (pelo canal dos menus).
    pub(crate) glove_cmds: Vec<String>,
    /// Último "documento editado" e "documento aberto" contados ao Glove.
    pub(crate) edited_sent: Option<bool>,
    pub(crate) doc_sent: Option<Option<std::path::PathBuf>>,
    pub(crate) minimize_req: bool,
    /// Pedido de zoom (ou de voltar do zoom): moldura própria.
    pub(crate) zoom_req: bool,
    /// Os botões da janela (moldura própria): o apertado.
    pub(crate) title_pressed: Option<glove_paint::chrome::TitleButton>,
    /// Novo tamanho da janela pedido pelo app: (largura, altura, animar).
    pub(crate) size_req: Option<(u32, u32, bool)>,
    /// Novo tamanho mínimo da janela pedido pelo app: (largura, altura, animar).
    pub(crate) min_req: Option<(u32, u32, bool)>,
    /// Pedido para redesenhar daqui a pouco (animações, repetição de setas).
    pub(crate) wake_after: Option<std::time::Duration>,
    /// Dica (nome inteiro sob o mouse): o texto, a área e desde quando o mouse está nela.
    tip: Option<(String, Rect, std::time::Instant)>,
    /// Alguma área de dica estava sob o mouse neste quadro.
    tip_seen: bool,
    /// Onde estão as animações que pediram o próximo quadro (redesenho parcial:
    /// se só elas mudam, só essa área é refeita).
    pub(crate) anim_rects: Vec<Rect>,
    /// Algum pedido de quadro sem área conhecida: o próximo quadro é inteiro.
    pub(crate) anim_full: bool,
    /// Ícone do app (alertas), descoberto pelo app_id.
    pub(crate) app_icon: Option<Icon>,
    /// Sombra do alerta já desfocada: (largura, altura, escala) → canvas.
    alert_shadow: Option<((u32, u32, u32), Canvas)>,
    /// Título do último alerta que já tocou o som (toca uma vez só).
    alert_sounded: Option<String>,
    /// Pedido de arrastar a janela (moldura própria).
    pub(crate) move_req: bool,
    /// Fonte nova para o texto dos documentos (família do fontconfig).
    pub(crate) doc_font_req: Option<String>,
    /// Número da janela desta memória (apps com várias janelas).
    pub(crate) window_id: u32,
    /// Janelas novas pedidas pelo app: (número, opções).
    pub(crate) open_windows: Vec<(u32, crate::platform::WindowOptions)>,
    /// O app pediu para redesenhar as outras janelas também.
    pub(crate) redraw_all: bool,
    /// Descritores novos para observar (ex.: o shell de uma aba nova).
    pub(crate) watch_req: Vec<std::os::fd::RawFd>,
    /// Seta de rolagem apertada: (id, próxima repetição).
    scroll_repeat: Option<(Id, std::time::Instant)>,
    /// Backend GTK: onde o app põe os widgets do sistema (embaixo do desenho do kit).
    #[cfg(feature = "gtk")]
    pub(crate) native_host: Option<crate::gtk_platform::NativeHost>,
    /// Backend GTK fora do Glove: o menu de contexto pedido neste quadro (onde e os itens).
    #[cfg(feature = "gtk")]
    pub(crate) ctx_req: Option<((f32, f32), Vec<crate::menu::MenuItem>)>,
    /// Superfícies de rascunho reaproveitadas (recorte de áreas roláveis).
    scratch: HashMap<Id, Canvas>,
    /// Quais foram usadas neste quadro (as outras são liberadas no fim).
    scratch_used: Vec<Id>,
}

impl Memory {
    /// Esquece os pedidos que uma passada repetida (só para pintar, sem eventos)
    /// deixou: a passada anterior, que valeu, já fez os mesmos.
    pub(crate) fn drop_requests(&mut self) {
        self.clip_out = None;
        self.clip_files = None;
        self.paste_files = false;
        self.drag_out = None;
        self.paste_req = None;
        self.resize_req = false;
        self.glove_cmds.clear();
        self.minimize_req = false;
        self.zoom_req = false;
        self.size_req = None;
        self.min_req = None;
        self.wake_after = None;
        self.move_req = false;
        self.doc_font_req = None;
        self.open_windows.clear();
        self.redraw_all = false;
        self.watch_req.clear();
        #[cfg(feature = "gtk")]
        {
            self.ctx_req = None;
        }
    }

    /// Algum campo do kit está com o cursor?
    #[cfg_attr(not(feature = "gtk"), allow(dead_code))]
    pub(crate) fn focused(&self) -> bool {
        self.focus.is_some()
    }

    /// O teclado foi para fora dos widgets do kit: nenhum campo fica com o cursor.
    #[cfg_attr(not(feature = "gtk"), allow(dead_code))]
    pub(crate) fn clear_focus(&mut self) {
        self.focus = None;
    }

    pub(crate) fn with_app_icon(icon: Icon) -> Self {
        Self { app_icon: Some(icon), ..Self::default() }
    }
}

struct Layout {
    horizontal: bool,
    x0: f32,
    y0: f32,
    cx: f32,
    cy: f32,
    width: f32,
    used_w: f32,
    used_h: f32,
    spacing: f32,
}

pub struct Ui<'a> {
    pub c: &'a mut Canvas,
    pub theme: &'a Theme,
    pub fonts: &'a Fonts,
    /// Fonte dos documentos (Editor etc.).
    pub doc_fonts: &'a Fonts,
    /// Janela redimensionável: desenha o puxador no canto inferior direito.
    pub(crate) resizable: bool,
    input: &'a Input,
    mem: &'a mut Memory,
    stack: Vec<Layout>,
    seed: u64,
    counter: u64,
    /// Clique engolido por um pop-up aberto.
    captured: bool,
    /// Widgets desligados por `set_enabled`/`set_active` (não apaga o bloqueio do pop-up).
    disabled: bool,
    /// Atrás de uma folha (`set_enabled(false)`): os cliques não chegam, e o
    /// `set_active` dos controles não desfaz isso.
    sheet_blocked: bool,
    /// Fora da área visível de uma `scroll_view`: o mesmo, só para o conteúdo dela.
    clip_blocked: bool,
    /// O mouse está sobre uma área reservada por `reserve` (menu desenhado depois).
    reserved: bool,
    /// Controles desligados de propósito (`set_active(false)`): aparecem apagados.
    dimmed: bool,
    /// Pede outro quadro logo em seguida.
    pub redraw: bool,
    /// Pede para fechar a janela.
    pub close: bool,
    /// Áreas que reagiram ao mouse neste quadro e se a posição foi lida direto.
    hover_log: std::cell::RefCell<Vec<Rect>>,
    ptr_used: std::cell::Cell<bool>,
}

const TEXT: f32 = 13.0;
/// As ações do menu do botão direito dos campos de texto (voltam em `menu_actions`, como as
/// dos menus do app, que as ignoram).
const MENU_RECORTAR: &str = "glove-campo:recortar";
const MENU_COPIAR: &str = "glove-campo:copiar";
const MENU_COLAR: &str = "glove-campo:colar";
const MENU_TUDO: &str = "glove-campo:tudo";
const CONTROL_H: f32 = 22.0;

/// Barras de rolagem modernas: o trilho e o puxador (mais forte sob o mouse).
fn modern_scroll_colors(t: &Theme, hot: bool) -> (Color, Color) {
    if t.dark {
        (rgba(36, 36, 39, 255), rgba(255, 255, 255, if hot { 150 } else { 95 }))
    } else {
        (rgba(250, 250, 251, 255), rgba(0, 0, 0, if hot { 125 } else { 80 }))
    }
}

fn ink() -> Color {
    if glove_paint::theme::dark() { rgba(232, 232, 236, 255) } else { rgba(20, 20, 20, 255) }
}

impl<'a> Ui<'a> {
    pub(crate) fn new(c: &'a mut Canvas, theme: &'a Theme, fonts: &'a Fonts, doc_fonts: &'a Fonts, resizable: bool, input: &'a Input, mem: &'a mut Memory) -> Self {
        let (w, h) = (c.w, c.h);
        let captured = match &mem.popup {
            // Lista na superfície própria: a janela fica parada enquanto ela está aberta
            // (um clique na janela só fecha a lista).
            Some(p) if p.native == Some(true) => true,
            Some(p) => p.rect.contains(input.pos) || input.pressed,
            None => false,
        };
        // O alerta da janela abre numa janela própria: enquanto ela está aberta, esta
        // fica parada (nada responde), como embaixo de uma folha.
        let captured = captured || mem.alert_open;
        mem.alert_req = None;
        mem.mouse_cursor = Cursor::Arrow;
        let mut ui = Self { c, theme, fonts, doc_fonts, resizable, input, mem, stack: Vec::new(), seed: 0, counter: 0, captured, disabled: false, sheet_blocked: false, clip_blocked: false, reserved: false, dimmed: false, redraw: false, close: false, hover_log: Default::default(), ptr_used: Default::default() };
        ui.stack.push(Layout { horizontal: false, x0: 0.0, y0: 0.0, cx: 0.0, cy: 0.0, width: w, used_w: 0.0, used_h: 0.0, spacing: 8.0 });
        let _ = h;
        ui
    }

    // -----------------------------------------------------------------------
    // Identificadores e layout
    // -----------------------------------------------------------------------

    fn next_id(&mut self) -> Id {
        self.counter += 1;
        let mut h = self.seed ^ 0x9e37_79b9_7f4a_7c15;
        h = h.wrapping_mul(31).wrapping_add(self.counter);
        h ^ (h >> 29)
    }

    /// Dá a um trecho da interface uma identidade estável (ex.: linhas de uma lista).
    pub fn push_id(&mut self, key: impl std::hash::Hash, f: impl FnOnce(&mut Self)) {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hs);
        self.seed.hash(&mut hs);
        let (old_seed, old_counter) = (self.seed, self.counter);
        self.seed = hs.finish();
        self.counter = 0;
        f(self);
        self.seed = old_seed;
        self.counter = old_counter;
    }

    fn top(&mut self) -> &mut Layout {
        self.stack.last_mut().unwrap()
    }

    pub fn available_width(&self) -> f32 {
        let l = self.stack.last().unwrap();
        if l.horizontal { l.width - (l.cx - l.x0) } else { l.width }
    }

    pub fn cursor(&self) -> (f32, f32) {
        let l = self.stack.last().unwrap();
        (l.cx, l.cy)
    }

    /// Reserva um espaço e devolve onde desenhar.
    pub fn alloc(&mut self, w: f32, h: f32) -> Rect {
        let l = self.top();
        let r = Rect::new(l.cx, l.cy, w, h);
        if l.horizontal {
            l.cx += w + l.spacing;
            l.used_w = l.cx - l.x0 - l.spacing;
            l.used_h = l.used_h.max(h);
        } else {
            l.cy += h + l.spacing;
            l.used_h = l.cy - l.y0 - l.spacing;
            l.used_w = l.used_w.max(w);
        }
        r
    }

    pub fn space(&mut self, s: f32) {
        let l = self.top();
        if l.horizontal { l.cx += s } else { l.cy += s }
    }

    pub fn spacing(&mut self, s: f32) {
        self.top().spacing = s;
    }

    fn child(&mut self, horizontal: bool, x: f32, y: f32, width: f32, f: impl FnOnce(&mut Self)) -> (f32, f32) {
        let spacing = self.top().spacing;
        self.stack.push(Layout { horizontal, x0: x, y0: y, cx: x, cy: y, width, used_w: 0.0, used_h: 0.0, spacing });
        f(self);
        let l = self.stack.pop().unwrap();
        (l.used_w.max(0.0), l.used_h.max(0.0))
    }

    /// Coloca os widgets lado a lado.
    pub fn horizontal(&mut self, f: impl FnOnce(&mut Self)) {
        let (x, y) = self.cursor();
        let width = self.available_width();
        let (w, h) = self.child(true, x, y, width, f);
        self.alloc(w, h);
    }

    /// Bloco com recuo à esquerda.
    pub fn indent(&mut self, by: f32, f: impl FnOnce(&mut Self)) {
        let (x, y) = self.cursor();
        let width = self.available_width() - by;
        let (w, h) = self.child(false, x + by, y, width, f);
        self.alloc(w + by, h);
    }

    /// Área com posição e largura fixas (não ocupa espaço no layout atual).
    pub fn area(&mut self, r: Rect, f: impl FnOnce(&mut Self)) {
        self.child(false, r.x, r.y, r.w, f);
    }

    /// Como `area`, mas devolve o tamanho usado pelo conteúdo (para a janela se
    /// ajustar a ele com `set_window_size`).
    pub fn area_size(&mut self, r: Rect, f: impl FnOnce(&mut Self)) -> (f32, f32) {
        self.child(false, r.x, r.y, r.w, f)
    }

    /// Coluna com largura fixa (para formulários "rótulo: controle").
    pub fn column(&mut self, width: f32, f: impl FnOnce(&mut Self)) {
        let (x, y) = self.cursor();
        let (_, h) = self.child(false, x, y, width, f);
        self.alloc(width, h);
    }

    /// Muda o título da janela.
    pub fn set_title(&mut self, t: &str) {
        self.mem.title = Some(t.to_string());
    }

    /// Desliga (ou religa) a interação com os widgets seguintes (ex.: atrás de uma folha).
    pub fn set_enabled(&mut self, on: bool) {
        self.sheet_blocked = !on;
    }

    /// Reserva `r` para um menu que o app desenha por cima no fim do quadro: os
    /// widgets desenhados antes não recebem o mouse ali (o clique não atravessa).
    /// Desenhe o menu dentro de `unreserved`.
    pub fn reserve(&mut self, r: Rect) {
        self.hover_log.borrow_mut().push(r);
        if r.contains(self.input.pos) {
            self.reserved = true;
        }
    }

    /// Roda `f` com o mouse valendo de novo dentro das áreas reservadas.
    pub fn unreserved<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let old = std::mem::replace(&mut self.reserved, false);
        let r = f(self);
        self.reserved = old;
        r
    }

    /// Esconde o puxador de redimensionar neste quadro (ex.: lembrete recolhido).
    pub fn hide_resize_grip(&mut self) {
        self.resizable = false;
    }

    /// Os widgets agora não recebem o mouse (pop-up aberto, folha por cima, menu reservado).
    fn blocked(&self) -> bool {
        self.captured || self.disabled || self.reserved || self.sheet_blocked || self.clip_blocked
    }

    /// Liga ou desliga os controles seguintes; desligados, aparecem apagados
    /// (diferente de `set_enabled`, que só ignora os cliques, como atrás de uma folha).
    pub fn set_active(&mut self, on: bool) {
        self.disabled = !on;
        self.dimmed = !on;
    }

    pub fn mods(&self) -> &Mods {
        &self.input.mods
    }

    /// Itens de menu (sem atalho) escolhidos neste quadro.
    pub fn menu_actions(&self) -> &[String] {
        &self.input.menu_actions
    }

    /// Muda o tamanho da janela (em pontos). Com `animate`, ela cresce ou encolhe
    /// suave, com o topo parado, como as Preferências da era ao trocar de painel.
    /// Pode ser chamado a cada quadro: só age quando o tamanho pedido muda.
    pub fn set_window_size(&mut self, w: f32, h: f32, animate: bool) {
        self.mem.size_req = Some((w.round().max(1.0) as u32, h.round().max(1.0) as u32, animate));
    }

    /// Tamanho mínimo de uma janela redimensionável, que pode mudar a cada quadro
    /// (nas Preferências, cada painel tem o seu). A janela acompanha o mínimo enquanto
    /// o usuário não a aumentar e nunca fica menor que ele; o que o usuário aumentou
    /// continua valendo quando o mínimo muda. Substitui `set_window_size`.
    pub fn set_window_min_size(&mut self, w: f32, h: f32, animate: bool) {
        self.mem.min_req = Some((w.round().max(1.0) as u32, h.round().max(1.0) as u32, animate));
    }

    /// A moldura de uma folha (o painel que desce da barra, por cima do conteúdo): uma
    /// sombra suave que some aos poucos em volta, o corpo e a borda, com os cantos de
    /// baixo arredondados. `r` é a área da folha.
    pub fn sheet_panel(&mut self, r: Rect) {
        // Camadas cada vez mais largas e fracas (uma moldura cinza de uma cor só parecia
        // um defeito, não uma sombra).
        for i in (1..=7).rev() {
            let g = i as f32 * 2.0;
            self.c.fill(rrect4(r.x - g, r.y, r.w + 2.0 * g, r.h + g, [0.0, 0.0, 6.0 + g, 6.0 + g]), &solid(rgba(0, 0, 0, 9)));
        }
        let shape = || rrect4(r.x, r.y, r.w, r.h, [0.0, 0.0, 6.0, 6.0]);
        self.c.fill(shape(), &vgrad(r.y, r.y + r.h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(246, 246, 248, 255))]));
        self.c.stroke(rrect4(r.x + 0.5, r.y - 0.5, r.w - 1.0, r.h, [0.0, 0.0, 6.0, 6.0]), &solid(rgba(0, 0, 0, 70)), 1.0);
    }

    pub fn minimize(&mut self) {
        self.mem.minimize_req = true;
    }

    /// Zoom (ou volta do zoom), como o botão verde da barra de título.
    pub fn toggle_zoom(&mut self) {
        self.mem.zoom_req = true;
    }

    /// A janela está em zoom?
    pub fn maximized(&self) -> bool {
        self.input.maximized
    }

    /// Os três botões da janela (fechar, minimizar, zoom) desenhados pelo app, no
    /// estilo da barra de título do tema, com o primeiro centrado em (x + 14, cy).
    /// Para janelas com moldura própria (`use_own_frame_main`): minimizar e zoom já
    /// valem; devolve verdadeiro quando o fechar é clicado (o app decide e põe
    /// `ui.close = true`). Ocupa 68 pontos de largura.
    pub fn window_buttons(&mut self, x: f32, cy: f32) -> bool {
        use glove_paint::chrome::{self, TitleBarState, TitleButton};
        let group = Rect::new(x + 4.0, cy - 10.0, 62.0, 20.0);
        let (px, py) = self.input.pos;
        let over = self.input.inside && group.contains((px, py));
        let at = if over { chrome::title_button_at(px - x, py - cy + glove_paint::theme::TITLE_H as f32 / 2.0) } else { None };
        // Os botões não deixam a área ser "da barra" (arrastar) neste quadro.
        if over {
            self.hit(group);
        }
        if self.input.pressed && at.is_some() {
            self.mem.title_pressed = at;
        }
        let pressed = self.mem.title_pressed.filter(|b| Some(*b) == at && self.input.down);
        let mut close = false;
        if self.input.released {
            if let Some(b) = self.mem.title_pressed.take().filter(|b| Some(*b) == at) {
                match b {
                    TitleButton::Close => close = true,
                    TitleButton::Minimize => self.mem.minimize_req = true,
                    TitleButton::Zoom => self.mem.zoom_req = self.resizable,
                }
            }
        }
        let st = TitleBarState { active: self.input.active, hover: over, pressed, zoom_enabled: self.resizable, edited: self.mem.edited_sent == Some(true), pill: false };
        chrome::title_buttons(self.c, self.theme, x, cy, &st);
        if over || self.mem.title_pressed.is_some() {
            self.redraw = true;
        }
        close
    }

    /// Arrastar pela área livre da barra de ferramentas (moldura própria): apertar e
    /// mover arrasta a janela; dois cliques dão o zoom. Chame depois dos controles da
    /// barra (o que um controle já pegou não arrasta).
    pub fn toolbar_drag(&mut self, r: Rect) {
        // Um controle da barra já pegou o clique.
        if self.input.pressed && self.mem.active.is_some() {
            return;
        }
        let h = self.hit(r);
        if h.pressed {
            if h.double {
                self.mem.zoom_req = self.resizable;
            } else {
                self.mem.move_req = true;
            }
        }
    }

    /// Menu de contexto solto: o Glove o desenha no ponto `at` da janela (em pontos), por
    /// cima de tudo e sem ficar preso à janela; os submenus abrem ao lado do item, com a
    /// primeira opção na altura dele, e só sobem se faltar espaço no monitor. O item
    /// escolhido chega em `menu_actions()` (o id dele). Devolve falso fora do Glove (sem
    /// a barra de menu dele): aí o app desenha o menu dentro da janela, como antes.
    pub fn context_menu(&mut self, at: (f32, f32), items: &[crate::menu::MenuItem]) -> bool {
        if items.is_empty() {
            return false;
        }
        // Backend GTK fora do Glove: um menu do GTK no lugar.
        #[cfg(feature = "gtk")]
        if !native_menus() {
            self.mem.ctx_req = Some((at, items.to_vec()));
            return true;
        }
        if !native_menus() {
            return false;
        }
        let mut text = format!("CTX\t{}\t{}\n", at.0, at.1);
        crate::menu::serialize_items(items, &mut text);
        text.push_str("CTXEND");
        self.mem.glove_cmds.push(text);
        true
    }

    /// Pede ao Glove uma captura da tela inteira daqui a `delay_ms`.
    pub fn request_screenshot(&mut self, delay_ms: u64, open: bool) {
        self.mem.glove_cmds.push(format!("SHOT\t{delay_ms}\t{}", open as u8));
    }

    /// Pede ao Glove para o usuário escolher uma área da tela e capturá-la.
    pub fn request_screenshot_area(&mut self, open: bool) {
        self.request_screenshot_area_in(300, open);
    }

    /// O mesmo, começando a escolher daqui a `delay_ms` (a tela congela nessa hora).
    pub fn request_screenshot_area_in(&mut self, delay_ms: u64, open: bool) {
        self.mem.glove_cmds.push(format!("SHOT\t{}\t{}\t1", delay_ms.max(300), open as u8));
    }

    /// Captura de uma janela: o Glove destaca a janela sob o mouse e o clique a fotografa.
    pub fn request_screenshot_window(&mut self, open: bool) {
        self.request_screenshot_window_in(300, open);
    }

    /// O mesmo, começando daqui a `delay_ms`.
    pub fn request_screenshot_window_in(&mut self, delay_ms: u64, open: bool) {
        self.mem.glove_cmds.push(format!("SHOT\t{}\t{}\t2", delay_ms.max(300), open as u8));
    }

    /// Pede ao Glove a gravação de tela: `modo` 0 = tela inteira, 1 = uma área, 2 = uma
    /// janela (como na captura), com o áudio do sistema e/ou do microfone.
    pub fn request_recording(&mut self, modo: usize, audio_sistema: bool, microfone: bool) {
        self.mem.glove_cmds.push(format!("GRAVAR\t{modo}\t{}\t{}", audio_sistema as u8, microfone as u8));
    }

    /// Põe um arquivo na área de transferência de um jeito que continua valendo
    /// depois que o app fecha: no Glove, quem guarda é o compositor (uma imagem
    /// PNG vai também como imagem, para colar num chat); fora dele, o próprio app.
    pub fn copy_file_kept(&mut self, path: &std::path::Path) {
        if std::env::var_os("GLOVE_MENU_SOCKET").is_some_and(|s| !s.is_empty()) {
            self.mem.glove_cmds.push(format!("COPIAR\t{}", path.display()));
        } else {
            self.copy_files(&[path.to_path_buf()], false);
        }
    }

    /// O documento tem alterações não salvas: o Glove põe o ponto no botão de
    /// fechar (como no Mac). Só manda quando muda.
    pub fn set_edited(&mut self, on: bool) {
        if self.mem.edited_sent != Some(on) {
            self.mem.edited_sent = Some(on);
            self.mem.glove_cmds.push(format!("EDITADO\t{}", on as u8));
        }
    }

    /// O arquivo aberto na janela: o Glove mostra o ícone dele no título
    /// (arrastável; Ctrl+clique mostra as pastas do caminho). Só manda quando muda.
    pub fn set_document(&mut self, path: Option<&std::path::Path>) {
        let v = path.map(std::path::Path::to_path_buf);
        if self.mem.doc_sent.as_ref() != Some(&v) {
            self.mem.glove_cmds.push(format!("DOC\t{}", v.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()));
            self.mem.doc_sent = Some(v);
        }
    }

    /// Asks Glove to end the session: "logout", "quit" (leave a nested Glove), "restart" or "shutdown".
    pub fn request_end_session(&mut self, kind: &str) {
        self.mem.glove_cmds.push(format!("SAIR\t{kind}"));
    }

    /// Troca a fonte do texto dos documentos (text_area) desta janela e das
    /// outras do app. Vale a partir do próximo quadro.
    pub fn set_doc_font(&mut self, family: &str) {
        self.mem.doc_font_req = Some(family.to_string());
    }

    /// Começa a arrastar a janela (chame num clique da moldura própria).
    pub fn start_move(&mut self) {
        self.mem.move_req = true;
    }

    /// A janela tem o foco do teclado?
    pub fn window_active(&self) -> bool {
        self.input.active
    }

    /// Número da janela que está sendo desenhada (0 = a primeira).
    pub fn window_id(&self) -> u32 {
        self.mem.window_id
    }

    /// Observa mais um descritor: quando tiver dados, o kit chama
    /// `App::fd_ready_on(fd)` e redesenha. O app garante que ele vive enquanto observado
    /// (devolva `false` em `fd_ready_on` para parar).
    pub fn watch_fd(&mut self, fd: std::os::fd::RawFd) {
        self.mem.watch_req.push(fd);
    }

    /// Faixa de abas (como a dos navegadores da época), na largura de `r`: cada
    /// aba com o título e o × de fechar, mais o "+" de abrir outra no fim.
    pub fn tab_strip(&mut self, r: Rect, titles: &[String], active: usize) -> Option<TabAction> {
        self.tab_strip_with_icons(r, titles, &[], active)
    }

    /// Como `tab_strip`, com um ícone pequeno antes de cada título (ex.: o ícone do site
    /// no navegador). `icons[i]` = None: só o título.
    pub fn tab_strip_with_icons(&mut self, r: Rect, titles: &[String], icons: &[Option<&Canvas>], active: usize) -> Option<TabAction> {
        // Ids num escopo próprio: a faixa não muda os dos controles da janela (o foco fica).
        let mut out = None;
        self.push_id("glove-faixa-de-abas", |ui| out = ui.tab_strip_inner(r, titles, icons, active));
        out
    }

    fn tab_strip_inner(&mut self, r: Rect, titles: &[String], icons: &[Option<&Canvas>], active: usize) -> Option<TabAction> {
        let mut out = None;
        self.c.fill_rect(r.x, r.y, r.w, r.h, &vgrad(r.y, r.y + r.h, &[(0.0, rgba(176, 176, 180, 255)), (1.0, rgba(196, 196, 200, 255))]));
        self.c.hline(r.x, r.y, r.w, rgba(120, 120, 126, 255));
        self.c.hline(r.x, r.y + r.h - 1.0, r.w, rgba(120, 120, 126, 255));
        let plus_w = 26.0;
        let n = titles.len().max(1) as f32;
        let tw = ((r.w - plus_w - 8.0) / n).min(220.0);
        for (i, t) in titles.iter().enumerate() {
            let x = r.x + 4.0 + i as f32 * tw;
            let tab = Rect::new(x, r.y + 3.0, tw - 2.0, r.h - 3.0);
            let id = self.next_id();
            let (hover, _, clicked) = self.interact(id, tab);
            let on = i == active;
            let (a, b) = if on { (rgba(246, 246, 248, 255), rgba(226, 226, 230, 255)) } else if hover { (rgba(214, 214, 218, 255), rgba(200, 200, 204, 255)) } else { (rgba(200, 200, 204, 255), rgba(186, 186, 190, 255)) };
            self.c.fill(rrect4(tab.x, tab.y, tab.w, tab.h, [5.0, 5.0, 0.0, 0.0]), &vgrad(tab.y, tab.y + tab.h, &[(0.0, a), (1.0, b)]));
            self.c.stroke(rrect4(tab.x + 0.5, tab.y + 0.5, tab.w - 1.0, tab.h, [5.0, 5.0, 0.0, 0.0]), &solid(rgba(120, 120, 126, 255)), 1.0);
            if on {
                self.c.fill_rect(tab.x + 1.0, r.y + r.h - 1.0, tab.w - 2.0, 1.0, &solid(b));
            }
            // × de fechar, à esquerda (só aparece com mais de uma aba).
            let close = Rect::new(tab.x + 6.0, tab.y + (tab.h - 12.0) / 2.0, 12.0, 12.0);
            let mut on_close = false;
            if titles.len() > 1 {
                let cid = self.next_id();
                let (ch, _, cc) = self.interact(cid, close);
                on_close = cc;
                if ch {
                    self.c.fill(circle(close.x + 6.0, close.y + 6.0, 6.0), &solid(rgba(0, 0, 0, 40)));
                }
                let ink = solid(rgba(90, 90, 96, 255));
                for (x0, y0, x1, y1) in [(3.5f32, 3.5f32, 8.5f32, 8.5f32), (8.5, 3.5, 3.5, 8.5)] {
                    let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                    pb.move_to(close.x + x0, close.y + y0);
                    pb.line_to(close.x + x1, close.y + y1);
                    self.c.stroke(pb.finish(), &ink, 1.2);
                }
            }
            let icon = icons.get(i).copied().flatten();
            let iw = if icon.is_some() { 20.0 } else { 0.0 };
            let label = self.fonts.ellipsize(t, 11.5, on, tab.w - 44.0 - iw);
            let lw = self.fonts.measure(&label, 11.5, on);
            let lx = tab.x + (tab.w - lw - iw) / 2.0 + iw;
            if let Some(ic) = icon {
                // O ícone (16 pontos) logo antes do título.
                self.c.draw_canvas(ic, lx - iw, tab.y + (tab.h - ic.h) / 2.0);
            }
            self.fonts.draw(self.c, &label, lx, tab.y + tab.h / 2.0 + 4.0, 11.5, on, ink());
            if on_close {
                out = Some(TabAction::Close(i));
            } else if clicked && out.is_none() {
                out = Some(TabAction::Select(i));
            }
        }
        let px = r.x + 4.0 + titles.len() as f32 * tw + 4.0;
        let plus = Rect::new(px, r.y + 4.0, plus_w - 6.0, r.h - 7.0);
        let id = self.next_id();
        let (hover, _, clicked) = self.interact(id, plus);
        if hover {
            self.c.fill(rrect(plus.x, plus.y, plus.w, plus.h, 4.0), &solid(rgba(0, 0, 0, 30)));
        }
        let (cx, cy) = (plus.x + plus.w / 2.0, plus.y + plus.h / 2.0);
        self.c.fill_rect(cx - 5.0, cy - 0.75, 10.0, 1.5, &solid(rgba(70, 70, 76, 255)));
        self.c.fill_rect(cx - 0.75, cy - 5.0, 1.5, 10.0, &solid(rgba(70, 70, 76, 255)));
        if clicked {
            out = Some(TabAction::New);
        }
        if out.is_some() {
            self.redraw = true;
        }
        out
    }

    /// Redesenha também as outras janelas do app depois deste quadro (quando o que
    /// elas mostram depende do que mudou nesta, ex.: uma prévia que segue a seleção).
    pub fn redraw_all_windows(&mut self) {
        self.mem.redraw_all = true;
    }

    /// Abre outra janela do mesmo app, com o número `id` (escolhido pelo app;
    /// se já existir, nada acontece). O `App::ui` desenha cada uma pelo `window_id`.
    pub fn open_window(&mut self, id: u32, opts: crate::platform::WindowOptions) {
        self.mem.open_windows.push((id, opts));
    }

    /// Pede ao Glove para mostrar um protetor de tela agora (botão Testar).
    pub fn request_screensaver(&mut self, kind: &str) {
        self.mem.glove_cmds.push(format!("PROTETOR\t{kind}"));
    }

    /// Põe `text` na área de transferência.
    pub fn copy(&mut self, text: &str) {
        self.mem.clip_out = Some(text.to_string());
    }

    /// Pede o conteúdo da área de transferência; chega depois em `take_paste`.
    pub fn request_paste(&mut self) {
        self.mem.paste_req = Some(0);
    }

    /// Como `request_paste`, mas prefere arquivos: chega a lista `file://`, uma por linha.
    pub fn request_paste_files(&mut self) {
        self.mem.paste_req = Some(0);
        self.mem.paste_files = true;
    }

    /// Copia arquivos de verdade (não só o caminho): os outros apps recebem a lista
    /// de arquivos (text/uri-list), o formato dos gerenciadores de arquivos e, para
    /// uma imagem só, a própria imagem em PNG. `cut` = recortar.
    pub fn copy_files(&mut self, paths: &[std::path::PathBuf], cut: bool) {
        self.mem.clip_files = Some((paths.to_vec(), cut));
    }

    /// Começa a arrastar arquivos para fora da janela (chame com o botão apertado).
    pub fn start_drag_files(&mut self, paths: &[std::path::PathBuf]) {
        self.mem.drag_out = Some(paths.to_vec());
    }

    /// Arquivos soltos nesta janela só são lidos (o app não os move): o arrastar vira "copiar".
    pub fn drop_as_copy(&mut self) {
        self.mem.drop_copy = true;
    }

    /// Algo (arquivos de outro app ou desta janela) está sendo arrastado por cima.
    pub fn drag_hovering(&self) -> bool {
        self.mem.dnd_hover
    }

    /// Arquivos soltos nesta janela e o ponto onde caíram.
    pub fn take_drop(&mut self) -> Option<(Vec<std::path::PathBuf>, (f32, f32))> {
        self.mem.dropped.take()
    }

    /// Texto colado que chegou (pedido com `request_paste`).
    pub fn take_paste(&mut self) -> Option<String> {
        match self.mem.paste_in.take() {
            Some((0, t)) => Some(t),
            other => {
                self.mem.paste_in = other;
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Interação básica
    // -----------------------------------------------------------------------

    fn hovered(&self, r: Rect) -> bool {
        self.hover_log.borrow_mut().push(r);
        !self.blocked() && self.input.inside && r.contains(self.input.pos)
    }

    /// A posição do mouse lida direto (marca o quadro como dependente do ponteiro).
    fn pos(&self) -> (f32, f32) {
        self.ptr_used.set(true);
        self.input.pos
    }

    /// Lógica de clique comum: devolve (hover, pressionado agora, clicado).
    fn interact(&mut self, id: Id, r: Rect) -> (bool, bool, bool) {
        let hover = self.hovered(r);

        if hover && self.input.pressed {
            self.mem.active = Some(id);
            if self.mem.focus != Some(id) {
                self.mem.focus = None;
            }
        }
        let active = self.mem.active == Some(id);
        let mut clicked = false;
        if active && self.input.released {
            clicked = hover;
            self.mem.active = None;
        }
        (hover, active && self.input.down || (active && self.input.pressed), clicked)
    }

    // -----------------------------------------------------------------------
    // Áreas livres (para apps que desenham a própria interface)
    // -----------------------------------------------------------------------

    /// Interação com um retângulo qualquer.
    pub fn hit(&mut self, r: Rect) -> Response {
        let id = self.next_id();
        let (hover, down, clicked) = self.interact(id, r);
        let pressed = hover && self.input.pressed;
        Response { hover, down, pressed, clicked, double: pressed && self.input.clicks >= 2 }
    }

    /// Rolagem do mouse sobre `r` neste quadro (positivo = para baixo).
    pub fn scroll_in(&self, r: Rect) -> f32 {
        if self.hovered(r) { self.input.scroll } else { 0.0 }
    }

    pub fn pointer(&self) -> (f32, f32) {
        self.pos()
    }

    /// O botão do mouse foi pressionado neste quadro (em qualquer lugar da janela).
    pub fn mouse_pressed(&self) -> bool {
        self.input.pressed
    }

    /// Botão do meio pressionado (`true`) ou solto (`false`) dentro de `r` neste quadro.
    pub fn middle_button(&self, r: Rect) -> Option<bool> {
        if self.blocked() || !r.contains(self.pos()) {
            return None;
        }
        if self.input.middle_pressed {
            Some(true)
        } else if self.input.middle_released {
            Some(false)
        } else {
            None
        }
    }

    /// Rolagem para os lados sobre `r` neste quadro (positivo = para a direita).
    pub fn scroll_x_in(&self, r: Rect) -> f32 {
        if self.hovered(r) { self.input.scroll_x } else { 0.0 }
    }

    /// Botão direito pressionado dentro de `r` neste quadro (menus de contexto).
    pub fn right_clicked(&self, r: Rect) -> bool {
        self.input.right_pressed && !self.blocked() && r.contains(self.pos())
    }

    /// Algum botão do mouse foi pressionado ou solto neste quadro.
    pub fn mouse_buttons_changed(&self) -> bool {
        self.input.pressed || self.input.released || self.input.right_pressed
    }

    /// O cursor do mouse neste quadro (mãozinha, barra de texto); sem pedir, a seta.
    pub fn set_cursor(&mut self, c: Cursor) {
        self.mem.mouse_cursor = c;
    }

    /// 1 = clique simples, 2 = duplo, 3 = triplo (no quadro do clique; 0 sem clique).
    pub fn click_count(&self) -> u8 {
        self.input.clicks
    }

    /// O mouse está sobre a janela.
    pub fn pointer_inside(&self) -> bool {
        self.input.inside
    }

    /// O botão do mouse foi solto neste quadro.
    pub fn mouse_released(&self) -> bool {
        self.input.released
    }

    /// O botão do mouse está pressionado.
    pub fn mouse_down(&self) -> bool {
        self.input.down
    }

    /// Diz ao kit que nada do que foi desenhado neste quadro ficou para trás: o
    /// teclado ou o clique só mudaram o que foi desenhado depois. Sem isso, um
    /// quadro com eventos é desenhado duas vezes (a segunda mostra o resultado em
    /// controles desenhados antes da mudança). Vale só para este quadro.
    pub fn settled(&mut self) {
        self.mem.settled = true;
    }

    /// O mouse se moveu neste quadro.
    pub fn moved(&self) -> bool {
        self.ptr_used.set(true);
        self.input.moved
    }

    /// Desenha dentro de `r` com recorte: a função recebe um canvas local
    /// (0,0 = canto de `r`). O canvas é reaproveitado entre quadros.
    pub fn clipped(&mut self, key: u64, r: Rect, f: impl FnOnce(&mut Canvas, &Fonts)) {
        let scale = self.c.scale;
        let id = key ^ 0xc1_1b_ed;
        if !self.visible(r) {
            // Fora da área redesenhada: guarda o rascunho para o próximo quadro.
            if self.mem.scratch.contains_key(&id) {
                self.mem.scratch_used.push(id);
            }
            return;
        }
        let mut sc = match self.mem.scratch.remove(&id) {
            Some(c) if c.w == r.w && c.h == r.h && c.scale == scale => c,
            _ => match Canvas::new(r.w, r.h, scale) {
                Some(c) => c,
                None => return,
            },
        };
        sc.dark_remap = self.c.dark_remap;
        self.c.begin_sub(&mut sc, r.x, r.y);
        sc.clear(Color::TRANSPARENT);
        f(&mut sc, self.fonts);
        self.c.end_sub(&mut sc, r.x, r.y);
        self.mem.scratch.insert(id, sc);
        self.mem.scratch_used.push(id);
    }

    // -----------------------------------------------------------------------
    // Texto
    // -----------------------------------------------------------------------

    pub fn text_width(&self, t: &str, px: f32, bold: bool) -> f32 {
        self.fonts.measure(t, px, bold)
    }

    pub fn label_ex(&mut self, t: &str, px: f32, bold: bool, color: Color) {
        let w = self.fonts.measure(t, px, bold).ceil();
        let h = (px * 1.35).ceil();
        let r = self.alloc(w, h);
        self.fonts.draw(self.c, t, r.x, r.y + px, px, bold, color);
    }

    pub fn label(&mut self, t: &str) {
        self.label_ex(t, TEXT, false, ink());
    }

    pub fn heading(&mut self, t: &str) {
        self.label_ex(t, TEXT, true, ink());
    }

    pub fn small(&mut self, t: &str) {
        self.label_ex(t, 11.0, false, rgba(90, 90, 90, 255));
    }

    /// Rótulo alinhado à direita numa largura fixa (formulários).
    pub fn label_right(&mut self, t: &str, width: f32) {
        // Maior que a coluna: termina em "…" (nunca passa da borda da área).
        let t = self.fonts.ellipsize(t, TEXT, false, width);
        let w = self.fonts.measure(&t, TEXT, false);
        let r = self.alloc(width, CONTROL_H);
        self.fonts.draw(self.c, &t, r.x + width - w, r.y + 15.0, TEXT, false, ink());
    }

    /// Largura de um texto no tamanho dos rótulos (para medir colunas de formulário).
    pub fn label_width(&self, t: &str) -> f32 {
        self.fonts.measure(t, TEXT, false)
    }

    /// Texto em várias linhas, quebrado na largura disponível.
    pub fn paragraph(&mut self, t: &str) {
        let max = self.available_width();
        let mut line = String::new();
        for word in t.split_whitespace() {
            let test = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if self.fonts.measure(&test, 12.0, false) > max && !line.is_empty() {
                self.label_ex(&line, 12.0, false, rgba(50, 50, 50, 255));
                line = word.to_string();
            } else {
                line = test;
            }
        }
        if !line.is_empty() {
            self.label_ex(&line, 12.0, false, rgba(50, 50, 50, 255));
        }
    }

    pub fn separator(&mut self) {
        let w = self.available_width();
        let r = self.alloc(w, 2.0);
        self.c.hline(r.x, r.y, w, rgba(0, 0, 0, 50));
        self.c.hline(r.x, r.y + 1.0, w, rgba(255, 255, 255, 200));
    }

    // -----------------------------------------------------------------------
    // Botões
    // -----------------------------------------------------------------------

    fn push_button(&mut self, t: &str, default: bool, min_w: f32) -> bool {
        let id = self.next_id();
        let w = (self.fonts.measure(t, TEXT, false) + 32.0).max(min_w).ceil();
        let r = self.alloc(w, CONTROL_H);
        let (_, down, clicked) = self.interact(id, r);
        let base = if (down || default) && !self.dimmed { self.theme.accent } else { rgba(238, 238, 240, 255) };
        let base = if down { darken(base, 0.2) } else { base };
        gel_rect(self.c, r.x, r.y, r.w, r.h, r.h / 2.0, base);
        let tw = self.fonts.measure(t, TEXT, false);
        // No moderno, o botão principal (azul liso) tem o texto branco.
        let col = if self.dimmed { rgba(150, 150, 154, 255) } else if self.theme.modern && (down || default) { Color::WHITE } else { ink() };
        self.fonts.draw(self.c, t, r.x + (r.w - tw) / 2.0, r.y + 15.5, TEXT, false, col);
        clicked
    }

    pub fn button(&mut self, t: &str) -> bool {
        self.push_button(t, false, 70.0)
    }

    /// Botão principal (azul).
    pub fn default_button(&mut self, t: &str) -> bool {
        self.push_button(t, true, 70.0)
    }

    pub fn small_button(&mut self, t: &str) -> bool {
        self.push_button(t, false, 0.0)
    }

    /// Botão de gel numa posição livre (grades, teclados). `lit` = aceso (ex.: operação ativa).
    /// Tecla neutra (calculadora, teclados): gel claro, texto normal, escurece ao apertar.
    pub fn key_button(&mut self, r: Rect, t: &str, px: f32, lit: bool) -> bool {
        let id = self.next_id();
        let (_, down, clicked) = self.interact(id, r);
        let base = rgba(242, 242, 244, 255);
        let b = if down { darken(base, 0.22) } else if lit { darken(base, 0.12) } else { base };
        gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, b);
        self.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 5.0), &solid(rgba(120, 120, 126, 255)), 1.0);
        let tw = self.fonts.measure(t, px, false);
        self.fonts.draw(self.c, t, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + px * 0.36, px, false, rgba(25, 25, 25, 255));
        clicked
    }

    pub fn gel_button(&mut self, r: Rect, t: &str, base: Color, px: f32, lit: bool) -> bool {
        let id = self.next_id();
        let (_, down, clicked) = self.interact(id, r);
        let b = if down { darken(base, 0.25) } else if lit { lighten(base, 0.35) } else { base };
        gel_rect(self.c, r.x, r.y, r.w, r.h, (r.h / 2.0).min(10.0), b);
        let tw = self.fonts.measure(t, px, true);
        let ink = if base.red() + base.green() + base.blue() < 1.6 { Color::WHITE } else { rgba(25, 25, 25, 255) };
        self.fonts.draw(self.c, t, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + px * 0.36, px, true, ink);
        clicked
    }

    /// Texto digitado neste quadro.
    pub fn typed(&self) -> &str {
        &self.input.text
    }

    /// Teclas especiais pressionadas neste quadro.
    pub fn keys(&self) -> &[KeyPress] {
        &self.input.keys
    }

    // -----------------------------------------------------------------------
    // Caixas de seleção e rádio
    // -----------------------------------------------------------------------

    pub fn checkbox(&mut self, value: &mut bool, t: &str) -> bool {
        let id = self.next_id();
        let tw = self.fonts.measure(t, TEXT, false);
        let r = self.alloc(20.0 + tw, 18.0);
        let (_, down, clicked) = self.interact(id, r);
        if clicked {
            *value = !*value;
        }
        let (bx, by, s) = (r.x + 1.0, r.y + 2.0, 14.0);
        let base = if *value { self.theme.accent } else { rgba(240, 240, 242, 255) };
        gel_rect(self.c, bx, by, s, s, 3.5, if down { darken(base, 0.2) } else { base });
        if *value {
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            pb.move_to(bx + 3.5, by + 7.5);
            pb.line_to(bx + 6.3, by + 10.5);
            pb.line_to(bx + 11.0, by + 3.5);
            let check = if self.theme.modern { Color::WHITE } else { rgba(20, 30, 60, 240) };
            self.c.stroke(pb.finish(), &solid(check), 2.0);
        }
        let col = if self.dimmed { rgba(150, 150, 154, 255) } else { ink() };
        self.fonts.draw(self.c, t, r.x + 20.0, r.y + 14.0, TEXT, false, col);
        clicked
    }

    pub fn radio(&mut self, selected: bool, t: &str) -> bool {
        let id = self.next_id();
        let tw = self.fonts.measure(t, TEXT, false);
        let r = self.alloc(20.0 + tw, 18.0);
        let (_, down, clicked) = self.interact(id, r);
        let (cx, cy) = (r.x + 8.0, r.y + 9.0);
        let base = if selected { self.theme.accent } else { rgba(240, 240, 242, 255) };
        gel_circle(self.c, cx, cy, 7.0, if down { darken(base, 0.2) } else { base });
        if selected {
            let dot = if self.theme.modern { Color::WHITE } else { rgba(20, 30, 60, 230) };
            self.c.fill(circle(cx, cy + if self.theme.modern { 0.0 } else { 0.5 }, 2.8), &solid(dot));
        }
        self.fonts.draw(self.c, t, r.x + 20.0, r.y + 14.0, TEXT, false, ink());
        clicked && !selected
    }

    /// Grupo de rádios; devolve verdadeiro se mudou.
    pub fn radio_group(&mut self, value: &mut usize, options: &[&str]) -> bool {
        let mut changed = false;
        for (i, o) in options.iter().enumerate() {
            if self.radio(*value == i, o) {
                *value = i;
                changed = true;
            }
        }
        changed
    }

    // -----------------------------------------------------------------------
    // Slider
    // -----------------------------------------------------------------------

    /// Slider horizontal. Devolve verdadeiro enquanto o valor muda.
    pub fn slider(&mut self, value: &mut f32, min: f32, max: f32, width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, 20.0);
        let track = Rect::new(r.x + 8.0, r.y + 8.0, r.w - 16.0, 5.0);
        let hover = self.hovered(r);
        if hover && self.input.pressed {
            self.mem.slider_grab = Some(id);
        }
        let mut changed = false;
        if self.mem.slider_grab == Some(id) {
            if self.input.down || self.input.pressed {
                let t = ((self.pos().0 - track.x) / track.w).clamp(0.0, 1.0);
                let v = min + t * (max - min);
                if (v - *value).abs() > f32::EPSILON {
                    *value = v;
                    changed = true;
                }
            }
            if self.input.released {
                self.mem.slider_grab = None;
            }
        }
        // Trilho fino afundado, sem preenchimento (10.4) e polegar redondo em gel.
        slider_track(self.c, track);
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let kx = track.x + track.w * t;
        let grabbed = self.mem.slider_grab == Some(id);
        let a = self.theme.accent;
        gel_circle(self.c, kx, r.y + 10.5, 7.5, if grabbed { lighten(a, 0.2) } else { lighten(a, 0.5) });
        changed
    }

    /// Slider com marcas de graduação e rótulos embaixo (10.3/10.4): polegar azul em
    /// forma de escudo apontando para as marcas. `labels` são (posição de 0 a 1, texto).
    /// Com `snap`, o valor pula de marca em marca. Apagado (`enabled` falso) não responde.
    pub fn slider_ticks(&mut self, value: &mut f32, min: f32, max: f32, width: f32, ticks: &Ticks, enabled: bool) -> bool {
        let id = self.next_id();
        // O trilho fica na altura do texto do rótulo do formulário.
        let h = if ticks.labels.is_empty() { 29.0 } else { 41.0 };
        let r = self.alloc(width, h);
        let track = Rect::new(r.x + 8.0, r.y + 9.0, r.w - 16.0, 4.0);
        let mut changed = false;
        if enabled {
            if self.hovered(Rect::new(r.x, r.y, r.w, 25.0)) && self.input.pressed {
                self.mem.slider_grab = Some(id);
            }
            if self.mem.slider_grab == Some(id) {
                if self.input.down || self.input.pressed {
                    let mut t = ((self.pos().0 - track.x) / track.w).clamp(0.0, 1.0);
                    if ticks.snap && ticks.count > 1 {
                        let n = (ticks.count - 1) as f32;
                        t = (t * n).round() / n;
                    }
                    let v = min + t * (max - min);
                    if (v - *value).abs() > f32::EPSILON {
                        *value = v;
                        changed = true;
                    }
                }
                if self.input.released {
                    self.mem.slider_grab = None;
                }
            }
        }
        slider_track(self.c, track);
        // Marcas
        let mark = if enabled { rgba(110, 110, 116, 255) } else { rgba(170, 170, 176, 255) };
        if ticks.count > 1 {
            for i in 0..ticks.count {
                let x = (track.x + track.w * i as f32 / (ticks.count - 1) as f32).round() + 0.5;
                self.c.fill_rect(x - 0.5, r.y + 22.0, 1.0, 4.0, &solid(mark));
            }
        }
        // Rótulos (presos dentro da largura do controle)
        let ink_l = if enabled { rgba(40, 40, 44, 255) } else { rgba(150, 150, 156, 255) };
        for (pos, label) in ticks.labels {
            let tw = self.fonts.measure(label, 10.0, false);
            let x = (track.x + track.w * pos - tw / 2.0).clamp(r.x, r.x + r.w - tw);
            self.fonts.draw(self.c, label, x, r.y + 37.0, 10.0, false, ink_l);
        }
        // Polegar redondo, igual ao dos sliders sem marcas (um só padrão).
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let kx = (track.x + track.w * t).round();
        let grabbed = self.mem.slider_grab == Some(id);
        let a = self.theme.accent;
        let base = if !enabled { rgba(238, 238, 240, 255) } else if grabbed { lighten(a, 0.2) } else { lighten(a, 0.5) };
        gel_circle(self.c, kx, track.y + track.h / 2.0, 7.5, base);
        changed
    }

    // -----------------------------------------------------------------------
    // Pop-up (lista suspensa)
    // -----------------------------------------------------------------------

    pub fn popup(&mut self, value: &mut usize, options: &[String], width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, CONTROL_H);
        let mut changed = false;
        if let Some((pid, idx)) = self.mem.popup_choice {
            if pid == id {
                self.mem.popup_choice = None;
                if idx != *value && idx < options.len() {
                    *value = idx;
                    changed = true;
                }
            }
        }
        let open = self.mem.popup.as_ref().is_some_and(|p| p.id == id);
        let (_, _, clicked) = self.interact(id, r);
        if clicked && !open {
            let item_h = 20.0;
            let h = options.len() as f32 * item_h + 8.0;
            let y = (r.y - (*value as f32) * item_h - 4.0).clamp(2.0, (self.c.h - h - 2.0).max(2.0));
            self.mem.popup = Some(PopupState { fresh: true, id, rect: Rect::new(r.x, y, r.w, h), items: options.to_vec(), selected: *value, anchor: r, native: None });
            self.redraw = true;
        }

        let cap = 20.0;
        let ax = r.x + r.w - cap / 2.0;
        if self.theme.modern {
            // Moderno: o corpo liso e, no lugar da ponta de gel, um quadradinho liso na
            // cor de destaque com as duas setas em traço branco.
            gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, rgba(244, 244, 246, 255));
            let (bx, by, bs) = (r.x + r.w - cap + 1.0, r.y + 3.0, r.h - 6.0);
            self.c.fill(rrect(bx, by, bs, bs, 4.0), &solid(if self.dimmed { rgba(170, 170, 176, 255) } else { self.theme.accent }));
            let cx = bx + bs / 2.0;
            let chevron = |c: &mut Canvas, y: f32, d: f32| {
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                pb.move_to(cx - 3.0, y);
                pb.line_to(cx, y + 3.0 * d);
                pb.line_to(cx + 3.0, y);
                c.stroke(pb.finish(), &solid(Color::WHITE), 1.4);
            };
            chevron(self.c, r.y + r.h / 2.0 - 1.5, -1.0);
            chevron(self.c, r.y + r.h / 2.0 + 1.5, 1.0);
        } else {
            // Corpo branco + ponta azul com setas
            gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, rgba(244, 244, 246, 255));
            self.c.fill(rrect4(r.x + r.w - cap, r.y + 1.0, cap - 1.0, r.h - 2.0, [0.0, 4.0, 4.0, 0.0]), &vgrad(r.y, r.y + r.h, &[(0.0, darken(self.theme.accent, 0.05)), (0.5, self.theme.accent), (1.0, lighten(self.theme.accent, 0.5))]));
            self.c.fill(rrect4(r.x + r.w - cap + 1.0, r.y + 2.0, cap - 3.0, r.h * 0.45, [0.0, 3.0, 1.0, 0.0]), &solid(rgba(255, 255, 255, 110)));
            let arrow = |c: &mut Canvas, y: f32, up: bool| {
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                let d = if up { -1.0 } else { 1.0 };
                pb.move_to(ax - 3.5, y);
                pb.line_to(ax + 3.5, y);
                pb.line_to(ax, y + 3.5 * d);
                pb.close();
                c.fill(pb.finish(), &solid(rgba(20, 30, 60, 230)));
            };
            arrow(self.c, r.y + 9.5, true);
            arrow(self.c, r.y + 12.5, false);
        }
        let label = options.get(*value).map(|s| s.as_str()).unwrap_or("");
        let t = self.fonts.ellipsize(label, TEXT, false, r.w - cap - 16.0);
        self.fonts.draw(self.c, &t, r.x + 10.0, r.y + 15.5, TEXT, false, ink());
        changed
    }

    // -----------------------------------------------------------------------
    // Campo de texto
    // -----------------------------------------------------------------------

    /// Edição de uma linha de texto em `r` (foco pelo clique, digitação, setas).
    /// Devolve (mudou, Enter pressionado).
    fn edit_line(&mut self, id: Id, r: Rect, value: &mut String) -> (bool, bool) {
        let hover = self.hovered(r);
        if hover {
            self.mem.mouse_cursor = Cursor::Text;
        }
        if hover && self.input.pressed {
            self.mem.focus = Some(id);
            self.mem.cursor.insert(id, value.chars().count());
        } else if self.input.pressed && !hover && self.mem.focus == Some(id) {
            self.mem.focus = None;
        }
        let focused = self.mem.focus == Some(id);
        let mut changed = false;
        let mut enter = false;
        if focused {
            let mut cur = (*self.mem.cursor.get(&id).unwrap_or(&value.chars().count())).min(value.chars().count());
            let byte = |s: &String, ci: usize| s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len());
            // O texto colado (pedido com Ctrl+V) entra no cursor, numa linha só.
            if let Some((pid, text)) = self.mem.paste_in.take() {
                if pid == id {
                    for ch in text.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).filter(|c| !c.is_control()) {
                        let b = byte(value, cur);
                        value.insert(b, ch);
                        cur += 1;
                        changed = true;
                    }
                } else {
                    self.mem.paste_in = Some((pid, text));
                }
            }
            if self.input.keys.iter().any(|k| k.combo == "Ctrl+V") {
                self.mem.paste_req = Some(id);
            }
            for ch in self.input.text.chars().filter(|c| !c.is_control()) {
                let b = byte(value, cur);
                value.insert(b, ch);
                cur += 1;
                changed = true;
            }
            for k in &self.input.keys {
                match k.key {
                    Key::Backspace if cur > 0 => {
                        let b = byte(value, cur - 1);
                        value.remove(b);
                        cur -= 1;
                        changed = true;
                    }
                    Key::Delete if cur < value.chars().count() => {
                        let b = byte(value, cur);
                        value.remove(b);
                        changed = true;
                    }
                    Key::Left => cur = cur.saturating_sub(1),
                    Key::Right => cur = (cur + 1).min(value.chars().count()),
                    Key::Home => cur = 0,
                    Key::End => cur = value.chars().count(),
                    Key::Enter => {
                        enter = true;
                        self.mem.focus = None;
                    }
                    Key::Escape => self.mem.focus = None,
                    _ => {}
                }
            }
            self.mem.cursor.insert(id, cur);
        }
        (changed, enter)
    }

    /// Campo de uma linha com seleção (como a barra de endereço de um navegador): clicar
    /// põe o cursor no lugar, arrastar escolhe, dois cliques escolhem a palavra e três a
    /// linha; Shift com as setas, Ctrl com as setas anda por palavra; Ctrl+A, Ctrl+C,
    /// Ctrl+X, Ctrl+V e Ctrl+Z; o botão direito abre Recortar, Copiar, Colar e Selecionar
    /// Tudo. O texto longo rola para os lados. `autofocus` põe o cursor nele com tudo
    /// escolhido (digitar troca tudo). Devolve (mudou, Enter).
    pub fn line_edit(&mut self, r: Rect, buf: &mut TextBuffer, autofocus: bool) -> (bool, bool) {
        let id = self.next_id();
        let px = (r.h * 0.56).min(TEXT);
        let (changed, enter, focused) = self.editar_linha(id, r, buf, autofocus.then_some(true), true, px);
        // O campo.
        if self.theme.modern {
            if focused {
                self.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 9.0), &solid(with_alpha(self.theme.accent, 0.4)));
            }
            self.c.fill(rrect(r.x, r.y, r.w, r.h, 7.0), &solid(self.theme.control_border));
            self.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 6.0), &solid(if self.theme.dark { rgba(30, 30, 32, 255) } else { Color::WHITE }));
        } else {
            self.c.fill(rrect(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, 9.0), &solid(with_alpha(self.theme.accent, if focused { 0.5 } else { 0.25 })));
            self.c.fill(rrect(r.x, r.y, r.w, r.h, 7.0), &solid(Color::WHITE));
        }
        self.texto_da_linha(id, r, buf, focused, px);
        (changed, enter)
    }

    /// O motor dos campos de uma linha (`line_edit`, `text_field`, `text_field_in`): o
    /// mouse, o teclado, a área de transferência e o menu do botão direito. `autofocus`:
    /// `Some(true)` põe o cursor com tudo escolhido, `Some(false)` põe o cursor no fim.
    /// `clique_escolhe_tudo`: o primeiro clique escolhe o texto inteiro (a barra de
    /// endereço); sem ele, o clique põe o cursor no lugar. Devolve (mudou, Enter, em foco).
    fn editar_linha(&mut self, id: Id, r: Rect, buf: &mut TextBuffer, autofocus: Option<bool>, clique_escolhe_tudo: bool, px: f32) -> (bool, bool, bool) {
        const PAD: f32 = 6.0;
        let fonts = self.fonts;
        let width = |s: &str| fonts.measure(s, px, false);
        self.mem.focus_order.push(id);
        if self.mem.focus_enter == Some(id) {
            self.mem.focus_enter = None;
            buf.select_all();
            buf.reveal = true;
        }
        match autofocus {
            Some(true) => {
                self.mem.focus = Some(id);
                buf.select_all();
                buf.reveal = true;
            }
            Some(false) => {
                self.mem.focus = Some(id);
                let fim = buf.len();
                buf.set_cursor(fim, false);
                buf.reveal = true;
            }
            None => {}
        }
        // Posição x (no conteúdo) → byte no texto.
        let hit = |buf: &TextBuffer, x: f32| -> usize { fonts.index_at(buf.text(), px, false, x) };
        let hover = self.hovered(r);
        if hover {
            self.mem.mouse_cursor = Cursor::Text;
        }
        let local_x = |ui: &Self, buf: &TextBuffer| ui.pos().0 - r.x - PAD + buf.scroll_x;
        if hover && self.input.pressed {
            let ja = self.mem.focus == Some(id);
            self.mem.focus = Some(id);
            self.mem.active = Some(id);
            let p = hit(buf, local_x(self, buf));
            if self.input.clicks >= 3 {
                buf.select_all();
            } else if self.input.clicks == 2 {
                buf.select_word(p);
            } else if !ja && clique_escolhe_tudo && autofocus.is_none() {
                // O primeiro clique escolhe tudo (digitar troca o endereço inteiro); com o
                // campo já em foco, o clique põe o cursor no lugar.
                buf.select_all();
            } else {
                buf.set_cursor(p, self.input.mods.shift);
            }
            buf.reveal = true;
        } else if self.mem.active == Some(id) && self.input.down && self.input.moved && !self.input.pressed && self.mem.focus == Some(id) {
            let p = hit(buf, local_x(self, buf));
            if p != buf.cursor {
                buf.set_cursor(p, true);
                buf.reveal = true;
            }
        } else if self.input.pressed && !hover && self.mem.focus == Some(id) {
            self.mem.focus = None;
        }
        // O botão direito: o menu do campo (o cursor vai para ele; a seleção fica).
        if hover && self.input.right_pressed && !self.blocked() {
            self.mem.focus = Some(id);
            self.mem.campo_menu = Some(id);
            let sel = buf.has_selection();
            let itens = [
                MenuItem::new(MENU_RECORTAR, tr!("Recortar"), "Ctrl+X").enabled(sel),
                MenuItem::new(MENU_COPIAR, tr!("Copiar"), "Ctrl+C").enabled(sel),
                MenuItem::new(MENU_COLAR, tr!("Colar"), "Ctrl+V"),
                MenuItem::Separator,
                MenuItem::new(MENU_TUDO, tr!("Selecionar Tudo"), "Ctrl+A"),
            ];
            let at = self.pos();
            self.context_menu(at, &itens);
        }
        // O que foi escolhido no menu do campo (chega como as ações dos menus do app).
        let escolha_do_menu = if self.mem.campo_menu == Some(id) { self.input.menu_actions.iter().find_map(|a| [MENU_RECORTAR, MENU_COPIAR, MENU_COLAR, MENU_TUDO].into_iter().find(|m| a == m)) } else { None };
        if escolha_do_menu.is_some() {
            self.mem.campo_menu = None;
            self.mem.focus = Some(id);
        }
        let focused = self.mem.focus == Some(id) && !self.blocked();
        let antes = buf.text().to_string();
        let mut enter = false;
        if focused {
            if let Some((pid, text)) = self.mem.paste_in.take() {
                if pid == id {
                    buf.insert(&text.replace(['\n', '\r'], " "));
                    buf.reveal = true;
                } else {
                    self.mem.paste_in = Some((pid, text));
                }
            }
            match escolha_do_menu {
                Some(MENU_RECORTAR | MENU_COPIAR) if buf.has_selection() => {
                    self.mem.clip_out = Some(buf.selected_text().to_string());
                    if escolha_do_menu == Some(MENU_RECORTAR) {
                        buf.delete();
                        buf.reveal = true;
                    }
                }
                Some(MENU_COLAR) => self.mem.paste_req = Some(id),
                Some(MENU_TUDO) => buf.select_all(),
                _ => {}
            }
            for ch in self.input.text.chars().filter(|c| !c.is_control()) {
                buf.insert(&ch.to_string());
                buf.reveal = true;
            }
            let shift = self.input.mods.shift;
            let word = self.input.mods.ctrl || self.input.mods.alt;
            for k in &self.input.keys {
                match k.combo.as_str() {
                    "Ctrl+A" => {
                        buf.select_all();
                        continue;
                    }
                    "Ctrl+C" | "Ctrl+X" => {
                        if buf.has_selection() {
                            self.mem.clip_out = Some(buf.selected_text().to_string());
                            if k.combo == "Ctrl+X" {
                                buf.delete();
                            }
                        }
                        continue;
                    }
                    "Ctrl+V" => {
                        self.mem.paste_req = Some(id);
                        continue;
                    }
                    "Ctrl+Z" => {
                        buf.undo();
                        continue;
                    }
                    "Ctrl+Shift+Z" | "Ctrl+Y" => {
                        buf.redo();
                        continue;
                    }
                    _ => {}
                }
                match k.key {
                    Key::Backspace => buf.backspace(),
                    Key::Delete => buf.delete(),
                    Key::Left => buf.left(shift, word),
                    Key::Right => buf.right(shift, word),
                    Key::Home => buf.home(shift, true),
                    Key::End => buf.end(shift, true),
                    Key::Enter => {
                        enter = true;
                        self.mem.focus = None;
                    }
                    Key::Escape => self.mem.focus = None,
                    // Tab / Shift+Tab: para o próximo campo (ou o anterior), no fim do quadro.
                    Key::Tab => {
                        self.mem.tab_req = Some(if shift { -1 } else { 1 });
                        continue;
                    }
                    _ => continue,
                }
                buf.reveal = true;
            }
        }
        let changed = buf.text() != antes;
        // O cursor à vista (o texto rola para os lados).
        let vis = (r.w - 2.0 * PAD).max(1.0);
        let total = width(buf.text());
        if buf.reveal {
            buf.reveal = false;
            let cx = width(&buf.text()[..buf.cursor]);
            if cx < buf.scroll_x {
                buf.scroll_x = (cx - 24.0).max(0.0);
            } else if cx > buf.scroll_x + vis {
                buf.scroll_x = cx - vis + 24.0;
            }
        }
        buf.scroll_x = buf.scroll_x.clamp(0.0, (total - vis + 2.0).max(0.0));
        (changed, enter, focused)
    }

    /// O texto de um campo de uma linha, numa superfície de rascunho: corta o que passa do
    /// campo e mostra a seleção e o cursor.
    fn texto_da_linha(&mut self, id: Id, r: Rect, buf: &TextBuffer, focused: bool, px: f32) {
        const PAD: f32 = 6.0;
        let fonts = self.fonts;
        let width = |s: &str| fonts.measure(s, px, false);
        let tr = Rect::new(r.x + 2.0, r.y + 1.0, r.w - 4.0, r.h - 2.0);
        let scale = self.c.scale;
        let mut sc = match self.mem.scratch.remove(&id) {
            Some(c) if c.w == tr.w && c.h == tr.h && c.scale == scale => c,
            _ => match Canvas::new(tr.w, tr.h, scale) {
                Some(c) => c,
                None => return,
            },
        };
        sc.dark_remap = self.c.dark_remap;
        self.c.begin_sub(&mut sc, tr.x, tr.y);
        sc.clear(Color::TRANSPARENT);
        let x0 = PAD - 2.0 - buf.scroll_x;
        let base = tr.h / 2.0 + px * 0.36;
        let (sa, sb) = buf.selection();
        if focused && sa != sb {
            let xa = width(&buf.text()[..sa]);
            let xb = width(&buf.text()[..sb]);
            sc.fill_rect(x0 + xa, (tr.h - px * 1.35) / 2.0, xb - xa, px * 1.35, &solid(with_alpha(self.theme.highlight, 0.45)));
        }
        fonts.draw(&mut sc, buf.text(), x0, base, px, false, ink());
        if focused && sa == sb {
            let cx = (x0 + width(&buf.text()[..buf.cursor])).round();
            sc.fill_rect(cx, (tr.h - px * 1.3) / 2.0, 1.0, px * 1.3, &solid(ink()));
        }
        self.c.end_sub(&mut sc, tr.x, tr.y);
        self.mem.scratch.insert(id, sc);
        self.mem.scratch_used.push(id);
    }

    /// Um campo de uma linha sobre um texto do app (`String`): o buffer de edição (cursor,
    /// seleção, desfazer, rolagem) fica guardado aqui pelo id e acompanha o texto quando o
    /// app o troca. `moldura` desenha o campo (depois da edição, antes do texto). Devolve
    /// (mudou, Enter).
    fn campo_de_texto(&mut self, id: Id, r: Rect, value: &mut String, autofocus: bool, px: f32, moldura: impl FnOnce(&mut Self, bool)) -> (bool, bool) {
        let mut buf = match self.mem.linhas.remove(&id) {
            Some(b) if b.text() == value.as_str() => b,
            _ => {
                // Novo, ou o app trocou o texto: o cursor vai para o fim.
                let mut b = TextBuffer::new(value.replace(['\n', '\r'], " "));
                let fim = b.len();
                b.set_cursor(fim, false);
                b
            }
        };
        let (changed, enter, focused) = self.editar_linha(id, r, &mut buf, autofocus.then_some(false), false, px);
        if changed {
            *value = buf.text().to_string();
        }
        moldura(self, focused);
        self.texto_da_linha(id, r, &buf, focused, px);
        self.mem.linhas.insert(id, buf);
        self.mem.linhas_usadas.push(id);
        (changed, enter)
    }

    /// Campo de texto num retângulo livre (ex.: renomear no lugar). `autofocus` põe
    /// o cursor nele (no fim). Devolve (mudou, Enter).
    pub fn text_field_in(&mut self, r: Rect, value: &mut String, autofocus: bool) -> (bool, bool) {
        let id = self.next_id();
        let px = (r.h * 0.62).min(TEXT);
        self.campo_de_texto(id, r, value, autofocus, px, |ui, focused| {
            if ui.theme.modern {
                if focused {
                    ui.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 7.0), &solid(with_alpha(ui.theme.accent, 0.4)));
                }
                ui.c.fill(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(ui.theme.control_border));
                ui.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 4.0), &solid(if ui.theme.dark { rgba(30, 30, 32, 255) } else { Color::WHITE }));
            } else {
                ui.c.fill(rrect(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, 3.0), &solid(with_alpha(ui.theme.accent, if focused { 0.5 } else { 0.25 })));
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
            }
        })
    }

    /// Algum campo de texto está com o cursor?
    pub fn has_focus(&self) -> bool {
        self.mem.focus.is_some()
    }

    /// O teclado está num widget do sistema dentro da janela (backend GTK), não no kit.
    pub fn native_focus(&self) -> bool {
        self.input.native_focus
    }

    /// Backend GTK: o lugar dos widgets do sistema desta janela (embaixo do desenho do
    /// kit; aparecem onde o app apaga o desenho com `Canvas::clear_rect`).
    #[cfg(feature = "gtk")]
    pub fn native_host(&self) -> Option<crate::gtk_platform::NativeHost> {
        self.mem.native_host.clone()
    }

    /// Campo de texto de uma linha (formulários): cursor no lugar do clique, seleção com o
    /// mouse e com Shift, Ctrl+A/C/X/V/Z, o menu do botão direito, e o texto longo rola
    /// dentro do campo. Devolve verdadeiro quando o texto muda.
    pub fn text_field(&mut self, value: &mut String, width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, CONTROL_H);
        let (changed, _) = self.campo_de_texto(id, r, value, false, TEXT, |ui, focused| {
            // Moldura afundada + anel azul quando em foco (no moderno: liso e arredondado,
            // com o anel em volta).
            if ui.theme.modern {
                if focused {
                    ui.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 7.0), &solid(with_alpha(ui.theme.accent, 0.4)));
                }
                ui.c.fill(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(ui.theme.control_border));
                ui.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 4.0), &solid(if ui.theme.dark { rgba(30, 30, 32, 255) } else { Color::WHITE }));
            } else {
                if focused {
                    ui.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 5.0), &solid(with_alpha(ui.theme.accent, 0.45)));
                }
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
                ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
                ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, 3.0, &vgrad(r.y + 1.0, r.y + 4.0, &[(0.0, rgba(0, 0, 0, 40)), (1.0, rgba(0, 0, 0, 0))]));
            }
        });
        changed
    }

    /// Campo de senha: bolinhas no lugar das letras (nada de copiar o texto).
    /// `autofocus` põe o cursor nele. Devolve verdadeiro quando o Enter é pressionado.
    pub fn password_field_in(&mut self, r: Rect, value: &mut String, autofocus: bool) -> bool {
        let id = self.next_id();
        if autofocus {
            self.mem.focus = Some(id);
        }
        let (_, enter) = self.edit_line(id, r, value);
        let focused = self.mem.focus == Some(id);
        if focused {
            self.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 5.0), &solid(with_alpha(self.theme.accent, 0.45)));
        }
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, 3.0, &vgrad(r.y + 1.0, r.y + 4.0, &[(0.0, rgba(0, 0, 0, 40)), (1.0, rgba(0, 0, 0, 0))]));
        let n = value.chars().count();
        let cur = (*self.mem.cursor.get(&id).unwrap_or(&n)).min(n);
        let step = 8.0;
        let max = ((r.w - 16.0) / step) as usize;
        let first = n.saturating_sub(max);
        let cy = r.y + r.h / 2.0;
        // As bolinhas e o cursor já vêm na cor certa para o tema: sem isso, no modo
        // escuro o claro de `ink()` virava grafite e as bolinhas sumiam no campo.
        let remap = std::mem::replace(&mut self.c.dark_remap, false);
        for i in first..n {
            self.c.fill(circle(r.x + 9.0 + (i - first) as f32 * step, cy, 3.0), &solid(ink()));
        }
        if focused {
            let cx = r.x + 5.5 + (cur.saturating_sub(first)) as f32 * step;
            self.c.fill_rect(cx, r.y + 4.0, 1.0, r.h - 8.0, &solid(ink()));
        }
        self.c.dark_remap = remap;
        enter
    }

    // -----------------------------------------------------------------------
    // Busca, segmentado e itens de barra de ferramentas
    // -----------------------------------------------------------------------

    /// Campo de busca em cápsula com lupa (como no 10.3/10.4). Mostra `placeholder`
    /// em cinza quando vazio; o "x" à direita e o Esc limpam. Devolve verdadeiro
    /// quando o Enter é pressionado.
    pub fn search_field(&mut self, value: &mut String, placeholder: &str, width: f32) -> bool {
        let r = self.alloc(width, SEARCH_H);
        self.search_field_in(r, value, placeholder)
    }

    /// Campo de busca num retângulo livre (altura recomendada: `SEARCH_H`).
    pub fn search_field_in(&mut self, r: Rect, value: &mut String, placeholder: &str) -> bool {
        self.search_field_focus(r, value, placeholder, false)
    }

    /// Campo de busca; `autofocus` põe o cursor nele (ex.: ao abrir o app).
    pub fn search_field_focus(&mut self, r: Rect, value: &mut String, placeholder: &str, autofocus: bool) -> bool {
        let id = self.next_id();
        let clear_id = self.next_id();
        if autofocus {
            self.mem.focus = Some(id);
            self.mem.cursor.insert(id, value.chars().count());
        }
        let clear = Rect::new(r.x + r.w - 19.0, r.y + (r.h - 14.0) / 2.0, 14.0, 14.0);
        // O "x" vem antes do campo, para o clique nele não mover o cursor.
        let mut cleared = false;
        if !value.is_empty() {
            let (_, _, clicked) = self.interact(clear_id, clear);
            if clicked || (self.hovered(clear) && self.input.pressed) {
                cleared = true;
            }
        }
        let field = Rect::new(r.x, r.y, r.w - if value.is_empty() { 0.0 } else { 20.0 }, r.h);
        let (_, enter) = if cleared { (false, false) } else { self.edit_line(id, field, value) };
        let focused = self.mem.focus == Some(id);
        if cleared || focused && self.input.keys.iter().any(|k| k.key == Key::Escape) {
            value.clear();
            self.mem.cursor.insert(id, 0);
            self.mem.focus = Some(id);
        }

        let rad = r.h / 2.0;
        if self.mem.focus == Some(id) {
            self.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, rad + 3.0), &solid(with_alpha(self.theme.accent, 0.45)));
        }
        self.c.fill(rrect(r.x, r.y, r.w, r.h, rad), &solid(rgba(118, 118, 124, 255)));
        let inner = || rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, rad - 1.0);
        self.c.fill(inner(), &solid(Color::WHITE));
        self.c.fill_clipped(rrect(r.x, r.y, r.w, 4.0, 0.0), &vgrad(r.y + 1.0, r.y + 4.0, &[(0.0, rgba(0, 0, 0, 55)), (1.0, rgba(0, 0, 0, 0))]), inner());
        icons::magnifier(self.c, r.x + 12.0, r.y + r.h / 2.0, 12.0, rgba(70, 70, 74, 255));

        let tx = r.x + 22.0;
        let avail = r.w - 22.0 - if value.is_empty() { 8.0 } else { 24.0 };
        let base = r.y + r.h / 2.0 + 4.5;
        let px = 12.0;
        if value.is_empty() {
            if self.mem.focus != Some(id) {
                let t = self.fonts.ellipsize(placeholder, px, false, avail);
                self.fonts.draw(self.c, &t, tx, base, px, false, rgba(150, 150, 154, 255));
            }
        } else {
            // Texto longo: mostra o final (onde se digita).
            let mut shown: String = value.clone();
            while self.fonts.measure(&shown, px, false) > avail && !shown.is_empty() {
                shown.remove(0);
            }
            self.fonts.draw(self.c, &shown, tx, base, px, false, ink());
            // Botão de limpar: bolinha cinza com "x" branco.
            let (cx, cy) = (clear.x + 7.0, clear.y + 7.0);
            self.c.fill(circle(cx, cy, 6.5), &solid(rgba(150, 150, 156, 255)));
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            pb.move_to(cx - 2.5, cy - 2.5);
            pb.line_to(cx + 2.5, cy + 2.5);
            pb.move_to(cx + 2.5, cy - 2.5);
            pb.line_to(cx - 2.5, cy + 2.5);
            self.c.stroke(pb.finish(), &solid(Color::WHITE), 1.4);
        }
        if self.mem.focus == Some(id) {
            let cur = (*self.mem.cursor.get(&id).unwrap_or(&0)).min(value.chars().count());
            let full_w = self.fonts.measure(value, px, false);
            let before: String = value.chars().take(cur).collect();
            let off = (full_w - avail).max(0.0);
            let cx = tx + self.fonts.measure(&before, px, false) - off;
            self.c.fill_rect(cx.max(tx), r.y + 4.0, 1.0, r.h - 8.0, &solid(ink()));
        }
        enter
    }

    /// Controle segmentado com glifos (10.3/10.4): cápsula clara com divisórias e o
    /// segmento escolhido em azul claro. `selected` = None para botões momentâneos
    /// (ex.: Voltar/Avançar); `enabled` vazio = todos ativos. Devolve o segmento clicado.
    pub fn segmented(&mut self, glyphs: &[Glyph], selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
        let r = self.alloc(glyphs.len() as f32 * SEGMENT_W, SEGMENT_H);
        self.segmented_in(r, glyphs, selected, enabled)
    }

    /// Segmentado num retângulo livre (as partes dividem a largura igualmente).
    pub fn segmented_in(&mut self, r: Rect, glyphs: &[Glyph], selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
        let n = glyphs.len().max(1);
        let widths = vec![r.w / n as f32; n];
        let labels: Vec<SegLabel> = glyphs.iter().map(|g| SegLabel::Glyph(*g)).collect();
        self.segments(r, &labels, &widths, selected, enabled)
    }

    /// Segmentado com textos (abas do 10.3/10.4), centrado na largura disponível.
    pub fn segmented_text(&mut self, labels: &[&str], selected: Option<usize>) -> Option<usize> {
        let avail = self.available_width();
        let (x0, y0) = self.cursor();
        self.alloc(avail, SEGMENT_H);
        let widths: Vec<f32> = labels.iter().map(|l| (self.fonts.measure(l, 12.0, false) + 24.0).round()).collect();
        let total: f32 = widths.iter().sum();
        let r = Rect::new((x0 + (avail - total) / 2.0).round(), y0, total, SEGMENT_H);
        let segs: Vec<SegLabel> = labels.iter().map(|l| SegLabel::Text(l)).collect();
        self.segments(r, &segs, &widths, selected, &[])
    }

    fn segments(&mut self, r: Rect, labels: &[SegLabel], widths: &[f32], selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
        let rad = if self.theme.modern { 6.0 } else { 4.0 };
        let modern = self.theme.modern;
        let outline = if modern { self.theme.control_border } else { rgba(132, 132, 138, 255) };
        self.c.fill(rrect(r.x, r.y, r.w, r.h, rad), &solid(outline));
        let inner = || rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, rad - 1.0);
        let mut out = None;
        let a = self.theme.accent;
        let mut x = r.x;
        for (i, label) in labels.iter().enumerate() {
            let id = self.next_id();
            let seg = Rect::new(x.round(), r.y, (x + widths[i]).round() - x.round(), r.h);
            x += widths[i];
            let on = enabled.get(i).copied().unwrap_or(true);
            let (_, down, clicked) = if on { self.interact(id, seg) } else { (false, false, false) };
            if clicked {
                out = Some(i);
            }
            let sel = selected == Some(i);
            let stops: [(f32, Color); 4] = if sel {
                [(0.0, lighten(a, 0.72)), (0.48, lighten(a, 0.52)), (0.52, lighten(a, 0.4)), (1.0, lighten(a, 0.64))]
            } else {
                [(0.0, rgba(253, 253, 253, 255)), (0.48, rgba(242, 242, 242, 255)), (0.52, rgba(232, 232, 232, 255)), (1.0, rgba(246, 246, 246, 255))]
            };
            let stops = stops.map(|(t, c)| (t, if down { darken(c, 0.15) } else { c }));
            if modern {
                // Moderno: liso; o escolhido em cinza mais forte.
                let bg = match (self.theme.dark, sel || down) {
                    (true, true) => rgba(118, 118, 124, 255),
                    (true, false) => rgba(78, 78, 82, 255),
                    (false, true) => rgba(212, 212, 216, 255),
                    (false, false) => rgba(255, 255, 255, 255),
                };
                self.c.fill_clipped(rrect(seg.x, r.y, seg.w, r.h, 0.0), &solid(bg), inner());
            } else {
                self.c.fill_clipped(rrect(seg.x, r.y, seg.w, r.h, 0.0), &vgrad(r.y, r.y + r.h, &stops), inner());
            }
            if i > 0 {
                self.c.vline(seg.x, r.y + 1.0, r.h - 2.0, if modern { self.theme.control_border } else { rgba(150, 150, 156, 255) });
            }
            let color = if !on {
                if self.theme.dark { rgba(110, 110, 116, 255) } else { rgba(165, 165, 170, 255) }
            } else if modern {
                self.theme.text
            } else {
                rgba(30, 30, 34, 255)
            };
            match label {
                SegLabel::Glyph(g) => icons::glyph(self.c, *g, (seg.x + seg.w / 2.0).round(), (r.y + r.h / 2.0).round(), 12.0, color),
                SegLabel::Text(t) => {
                    let tw = self.fonts.measure(t, 12.0, false);
                    self.fonts.draw(self.c, t, seg.x + (seg.w - tw) / 2.0, r.y + 15.5, 12.0, false, color);
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Formulário e caixa com abas
    // -----------------------------------------------------------------------

    /// Linha de formulário da era: rótulo alinhado à direita numa coluna de `label_w`
    /// (termina em ":" se tiver texto) e os controles à esquerda numa coluna comum.
    pub fn form_row(&mut self, label: &str, label_w: f32, f: impl FnOnce(&mut Self)) {
        self.horizontal(|ui| {
            let t = if label.is_empty() || label.ends_with(':') { label.to_string() } else { format!("{label}:") };
            ui.label_right(&t, label_w);
            ui.column(ui.available_width(), f);
        });
    }

    /// Nota pequena embaixo de um controle de formulário, alinhada com a coluna dos controles.
    pub fn form_note(&mut self, label_w: f32, text: &str) {
        let sp = self.stack.last().map(|l| l.spacing).unwrap_or(8.0);
        self.indent(label_w + sp, |ui| ui.small(text));
    }

    /// Caixa em relevo com abas (segmentado de texto) centradas na borda de cima.
    /// Devolve verdadeiro quando a aba muda.
    pub fn tab_box(&mut self, value: &mut usize, labels: &[&str], height: f32, f: impl FnOnce(&mut Self)) -> bool {
        let w = self.available_width();
        let (x0, y0) = self.cursor();
        let r = self.alloc(w, height);
        let b = Rect::new(r.x, r.y + SEGMENT_H / 2.0, r.w, r.h - SEGMENT_H / 2.0);
        self.c.fill(rrect(b.x, b.y, b.w, b.h, 6.0), &solid(rgba(0, 0, 0, 12)));
        self.c.stroke(rrect(b.x + 0.5, b.y + 0.5, b.w - 1.0, b.h - 1.0, 6.0), &solid(rgba(0, 0, 0, 55)), 1.0);
        self.c.fill_clipped(rrect(b.x, b.y, b.w, 5.0, 0.0), &vgrad(b.y + 1.0, b.y + 5.0, &[(0.0, rgba(0, 0, 0, 22)), (1.0, rgba(0, 0, 0, 0))]), rrect(b.x + 1.0, b.y + 1.0, b.w - 2.0, b.h - 2.0, 5.0));
        let mut changed = false;
        self.area(Rect::new(x0, y0, w, SEGMENT_H), |ui| {
            if let Some(i) = ui.segmented_text(labels, Some(*value)) {
                if i != *value {
                    *value = i;
                    changed = true;
                }
            }
        });
        self.area(Rect::new(b.x + 18.0, b.y + SEGMENT_H / 2.0 + 14.0, b.w - 36.0, b.h - SEGMENT_H / 2.0 - 24.0), f);
        changed
    }

    /// Item de barra de ferramentas: ícone com o nome embaixo (Arquivos, Preferências).
    /// Realce ao passar o mouse e ao apertar; `selected` deixa marcado.
    pub fn toolbar_item_in(&mut self, r: Rect, icon: Icon, label: &str, selected: bool) -> bool {
        let id = self.next_id();
        let (hover, down, clicked) = self.interact(id, r);
        if selected || down {
            self.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(0, 0, 0, if down { 45 } else { 30 })));
        } else if hover {
            self.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(255, 255, 255, 60)));
        }
        let size = (r.h - 22.0).clamp(16.0, 34.0);
        icons::draw(self.c, self.fonts, icon, r.x + (r.w - size) / 2.0, r.y + 3.0, size, label.chars().next());
        let t = self.fonts.ellipsize(label, 11.0, false, r.w - 4.0);
        let tw = self.fonts.measure(&t, 11.0, false);
        self.fonts.draw(self.c, &t, r.x + (r.w - tw) / 2.0, r.y + r.h - 6.0, 11.0, false, ink());
        clicked
    }

    // -----------------------------------------------------------------------
    // Captura de atalho
    // -----------------------------------------------------------------------

    /// Campo que grava a próxima combinação de teclas.
    pub fn shortcut_field(&mut self, value: &mut String, width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, CONTROL_H);
        let (_, _, clicked) = self.interact(id, r);
        if clicked {
            self.mem.capture = Some(id);
        }
        let capturing = self.mem.capture == Some(id);
        let mut changed = false;
        if capturing {
            if let Some(k) = self.input.keys.iter().find(|k| !k.combo.is_empty()) {
                if k.key == Key::Escape {
                    self.mem.capture = None;
                } else {
                    *value = k.combo.clone();
                    self.mem.capture = None;
                    changed = true;
                }
            }
        }
        let base = if capturing { lighten(self.theme.accent, 0.6) } else { rgba(244, 244, 246, 255) };
        gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, base);
        let shown = if capturing { tr!("Pressione as teclas…").to_string() } else { pretty_combo(value) };
        let tw = self.fonts.measure(&shown, 12.0, false);
        self.fonts.draw(self.c, &shown, r.x + (r.w - tw) / 2.0, r.y + 15.0, 12.0, false, ink());
        changed
    }

    // -----------------------------------------------------------------------
    // Abas (segmentadas) e caixa de grupo
    // -----------------------------------------------------------------------

    /// Abas soltas (segmentado de texto centrado). Para abas sobre uma caixa, use `tab_box`.
    pub fn tabs(&mut self, value: &mut usize, labels: &[&str]) -> bool {
        match self.segmented_text(labels, Some(*value)) {
            Some(i) if i != *value => {
                *value = i;
                true
            }
            _ => false,
        }
    }

    /// Seção que abre e fecha (10.2–10.4): triângulo e título em negrito; clicar em
    /// qualquer um dos dois alterna. Devolve verdadeiro quando muda. O app desenha o
    /// conteúdo logo depois, se `open`.
    pub fn disclosure(&mut self, open: &mut bool, title: &str) -> bool {
        let id = self.next_id();
        let tw = self.fonts.measure(title, 12.0, true);
        let r = self.alloc(18.0 + tw, 18.0);
        let (hover, down, clicked) = self.interact(id, r);
        let _ = hover;
        if clicked {
            *open = !*open;
            self.redraw = true;
        }
        let color = if down { rgba(30, 30, 34, 255) } else { rgba(90, 90, 96, 255) };
        disclosure_triangle(self.c, r.x + 6.0, r.y + 9.0, *open, color);
        self.fonts.draw(self.c, title, r.x + 16.0, r.y + 13.5, 12.0, true, ink());
        clicked
    }

    /// Caixa clara com borda suave, de altura fixa, para agrupar controles.
    pub fn group(&mut self, height: f32, f: impl FnOnce(&mut Self)) {
        let w = self.available_width();
        let r = self.alloc(w, height);
        self.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(0, 0, 0, 22)));
        self.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 5.0), &solid(rgba(255, 255, 255, 70)));
        self.area(Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0), f);
    }

    /// Pede um novo quadro daqui a `d` (sem eventos), para animações.
    /// O quadro pedido assim é refeito por inteiro; se só uma área muda, use
    /// [`Ui::wake_after_in`].
    /// Dica: parado sobre `r` por meio segundo, aparece um balão com `text` (o nome
    /// inteiro de um arquivo cortado na lista, por exemplo). Some ao sair ou clicar.
    pub fn tooltip(&mut self, r: Rect, text: &str) {
        if text.is_empty() || !self.hovered(r) {
            return;
        }
        self.mem.tip_seen = true;
        let same = self.mem.tip.as_ref().is_some_and(|(t, rr, _)| t == text && *rr == r);
        if !same || self.input.pressed {
            self.mem.tip = Some((text.to_string(), r, std::time::Instant::now()));
        }
    }

    /// Dica só se o texto não coube em `max_w` (com a fonte `px`): o caso comum.
    pub fn tooltip_if_cut(&mut self, r: Rect, text: &str, px: f32, bold: bool, max_w: f32) {
        if self.fonts.measure(text, px, bold) > max_w {
            self.tooltip(r, text);
        }
    }

    pub fn wake_after(&mut self, d: std::time::Duration) {
        self.mem.wake_after = Some(self.mem.wake_after.map_or(d, |w| w.min(d)));
        self.mem.anim_full = true;
    }

    /// Pede um novo quadro daqui a `d` em que só `r` muda: o kit redesenha e
    /// manda para a tela só essa área (pulso do botão, listras do progresso…).
    pub fn wake_after_in(&mut self, d: std::time::Duration, r: Rect) {
        self.mem.wake_after = Some(self.mem.wake_after.map_or(d, |w| w.min(d)));
        self.mem.anim_rects.push(r);
    }

    /// O retângulo cai na área que está sendo redesenhada? (Fora dela, o
    /// desenho é descartado; pular o trabalho caro é opcional.)
    pub fn visible(&self, r: Rect) -> bool {
        self.c.visible(r.x, r.y, r.w, r.h)
    }

    // -----------------------------------------------------------------------
    // Alerta
    // -----------------------------------------------------------------------

    /// Alerta da era como folha que desce do topo da janela (o resto escurece e fica
    /// bloqueado: o app deve chamar `set_enabled(false)` antes de desenhar o resto
    /// enquanto o alerta estiver aberto). `buttons` vão da esquerda para a direita: o
    /// último é o padrão (Enter, azul pulsante), "Cancelar" responde ao Esc e, com três
    /// botões, o primeiro fica afastado à esquerda (ex.: "Não salvar").
    /// Devolve o índice do botão escolhido.
    pub fn alert(&mut self, kind: AlertKind, title: &str, text: &str, buttons: &[&str]) -> Option<usize> {
        // No Glove, o alerta é uma janela própria, presa a esta (a resposta volta aqui,
        // num quadro seguinte). Fora dele, a folha dentro da janela, como antes.
        if self.mem.alert_native {
            use std::hash::{Hash, Hasher};
            let mut hs = std::collections::hash_map::DefaultHasher::new();
            (kind as u8, title, text, buttons).hash(&mut hs);
            let key = hs.finish();
            if let Some(p) = self.mem.alert_results.iter().position(|r| r.0 == key) {
                let (_, i) = self.mem.alert_results.remove(p);
                self.mem.alert_open = false;
                return Some(i);
            }
            self.mem.alert_req = Some(AlertReq { key, kind, title: title.into(), text: text.into(), buttons: buttons.iter().map(|b| b.to_string()).collect() });
            self.set_enabled(false);
            return None;
        }
        self.alert_impl(kind, title, text, buttons, true)
    }

    /// Alerta que ocupa a janela inteira (janela própria de alerta, como a de
    /// Desligar): sem folha nem fundo escurecido, e a janela toma a altura do texto.
    pub fn alert_window(&mut self, kind: AlertKind, title: &str, text: &str, buttons: &[&str]) -> Option<usize> {
        self.alert_impl(kind, title, text, buttons, false)
    }

    fn alert_impl(&mut self, kind: AlertKind, title: &str, text: &str, buttons: &[&str], sheet: bool) -> Option<usize> {
        // Avisos e erros tocam o alerta do painel Som uma vez ao aparecer.
        if matches!(kind, AlertKind::Caution | AlertKind::Stop) && self.mem.alert_sounded.as_deref() != Some(title) {
            self.mem.alert_sounded = Some(title.to_string());
            glove_config::sons::alert();
        }
        use std::time::Duration;
        self.set_enabled(true);
        let (w, h) = (self.c.w, self.c.h);
        let (sx, sw) = if sheet {
            let sw = (w - 24.0).min(440.0).max(200.0);
            (((w - sw) / 2.0).round(), sw)
        } else {
            (0.0, w)
        };
        const ICON: f32 = 64.0;
        let tx = sx + 20.0 + ICON + 16.0;
        let tw = sx + sw - 20.0 - tx;
        let title_lines = wrap_lines(self.fonts, title, 13.0, true, tw);
        let text_lines = if text.is_empty() { Vec::new() } else { wrap_lines(self.fonts, text, 11.0, false, tw) };
        let text_h = title_lines.len() as f32 * 17.0 + if text_lines.is_empty() { 0.0 } else { 6.0 + text_lines.len() as f32 * 15.0 };
        let natural = (16.0 + text_h.max(ICON - 4.0) + 20.0 + CONTROL_H + 16.0).round();
        if !sheet {
            self.set_window_size(w, natural, false);
            self.c.fill_rect(0.0, 0.0, w, h, &solid(self.theme.body_b));
            self.c.pinstripes(0.0, 0.0, w, h, self.theme.body_a, self.theme.body_b);
        }
        let sh = natural.min(h);
        if sheet {
            // Fundo escurecido, sombra e a folha listrada encostada no topo.
            self.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
            // O desfoque da sombra é caro: só é refeito quando o tamanho muda (o pulso redesenha).
            let key = (sw as u32, sh as u32, self.c.scale.to_bits());
            if self.mem.alert_shadow.as_ref().map(|(k, _)| *k) != Some(key) {
                self.mem.alert_shadow = Canvas::shadow(-14.0, 0.0, sw + 28.0, sh + 18.0, sw, sh, &ShadowSpec { blur: 14.0, offset_y: 4.0, radius: 6.0, alpha: 0.45 }, self.c.scale).map(|c| (key, c));
            }
            if let Some((_, shadow)) = &self.mem.alert_shadow {
                self.c.draw_canvas(shadow, sx - 14.0, 0.0);
            }
            let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
            self.c.fill(shape(), &solid(self.theme.body_b));
            self.c.pinstripes(sx, 0.0, sw, sh - 6.0, self.theme.body_a, self.theme.body_b);
            self.c.stroke(shape(), &solid(rgba(0, 0, 0, 80)), 1.0 / self.c.scale);
        }

        // Ícone
        let (ix, iy) = (sx + 20.0, 14.0);
        let app = self.mem.app_icon.unwrap_or(Icon::Window);
        match kind {
            AlertKind::Note => icons::draw(self.c, self.fonts, app, ix, iy, ICON, None),
            AlertKind::Caution => {
                icons::alert_caution(self.c, ix, iy, ICON);
                icons::draw(self.c, self.fonts, app, ix + ICON - 30.0, iy + ICON - 30.0, 32.0, None);
            }
            AlertKind::Stop => icons::alert_stop(self.c, ix, iy, ICON),
        }

        // Textos
        let mut y = 16.0 + 13.0;
        for l in &title_lines {
            self.fonts.draw(self.c, l, tx, y, 13.0, true, ink());
            y += 17.0;
        }
        y += 6.0 - 17.0 + 11.0 + 4.0;
        for l in &text_lines {
            self.fonts.draw(self.c, l, tx, y, 11.0, false, rgba(30, 30, 30, 255));
            y += 15.0;
        }

        // Botões (o padrão pulsa: ~1,6 s por ciclo a 5 quadros/s, só com o alerta aberto)
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        // Só a janela ativa pulsa (como no original); nas outras o botão fica parado
        // e o app não gasta nada.
        let pulse = if self.input.active && !glove_config::reduce_effects() {
            0.5 + 0.5 * ((ms % 1600) as f32 / 1600.0 * std::f32::consts::TAU).sin()
        } else {
            0.5
        };
        let by = sh - 16.0 - CONTROL_H;
        let widths: Vec<f32> = buttons.iter().map(|b| (self.fonts.measure(b, TEXT, false) + 32.0).max(80.0).ceil()).collect();
        let mut x = sx + sw - 20.0;
        let mut rects = vec![Rect::new(0.0, 0.0, 0.0, 0.0); buttons.len()];
        for i in (0..buttons.len()).rev() {
            if buttons.len() == 3 && i == 0 {
                rects[0] = Rect::new(tx, by, widths[0], CONTROL_H);
                break;
            }
            x -= widths[i];
            rects[i] = Rect::new(x, by, widths[i], CONTROL_H);
            x -= 12.0;
        }
        let mut chosen = None;
        let last = buttons.len().saturating_sub(1);
        if self.input.active && !buttons.is_empty() && !glove_config::reduce_effects() {
            // Só o botão padrão muda a cada quadro do pulso.
            self.wake_after_in(Duration::from_millis(200), rects[last]);
        }
        for (i, b) in buttons.iter().enumerate() {
            let id = self.next_id();
            let r = rects[i];
            let (_, down, clicked) = self.interact(id, r);
            let base = if i == last {
                let a = self.theme.accent;
                if down { darken(a, 0.2) } else { mix(darken(a, 0.08), lighten(a, 0.3), pulse) }
            } else if down {
                darken(rgba(238, 238, 240, 255), 0.2)
            } else {
                rgba(238, 238, 240, 255)
            };
            gel_rect(self.c, r.x, r.y, r.w, r.h, r.h / 2.0, base);
            let bw = self.fonts.measure(b, TEXT, false);
            // Moderno: o botão principal (azul) tem o texto branco, no claro e no escuro.
            let col = if i == last && self.theme.modern { Color::WHITE } else { ink() };
            self.fonts.draw(self.c, b, r.x + (r.w - bw) / 2.0, r.y + 15.5, TEXT, false, col);
            if clicked {
                chosen = Some(i);
            }
        }
        let keys = &self.input.keys;
        if keys.iter().any(|k| k.key == Key::Enter) && !buttons.is_empty() {
            chosen = Some(last);
        }
        if keys.iter().any(|k| k.key == Key::Escape) {
            if let Some(i) = buttons.iter().position(|b| b.eq_ignore_ascii_case("Cancelar") || b.eq_ignore_ascii_case(tr!("Cancelar"))) {
                chosen = Some(i);
            } else if buttons.len() == 1 {
                chosen = Some(0);
            }
        }
        // Nada embaixo responde enquanto o alerta está aberto.
        self.set_enabled(false);
        chosen
    }

    // -----------------------------------------------------------------------
    // Progresso e espera
    // -----------------------------------------------------------------------

    /// Barra de progresso (12 pt de altura). `value` de 0 a 1; `None` = indeterminada,
    /// com listras que andam (só pede novos quadros enquanto aparece).
    pub fn progress(&mut self, value: Option<f32>, width: f32) {
        let r = self.alloc(width, PROGRESS_H);
        self.progress_in(r, value);
    }

    /// Barra de progresso num retângulo livre.
    pub fn progress_in(&mut self, r: Rect, value: Option<f32>) {
        let phase = if value.is_none() {
            // Reduzir efeitos: as listras andam devagar (5 quadros/s em vez de 20).
            self.wake_after_in(std::time::Duration::from_millis(if glove_config::reduce_effects() { 200 } else { 50 }), r);
            anim_phase(900)
        } else {
            0.0
        };
        progress_bar(self.c, r.x, r.y, r.w, r.h, value, phase, self.theme.accent);
    }

    /// Giratório de espera de `size` pt (12 raios, uma volta por segundo).
    pub fn spinner(&mut self, size: f32) {
        let r = self.alloc(size, size);
        self.spinner_at(r.x + size / 2.0, r.y + size / 2.0, size);
    }

    /// Giratório centrado em (cx, cy).
    pub fn spinner_at(&mut self, cx: f32, cy: f32, size: f32) {
        let h = size / 2.0 + 1.0;
        let ms = if glove_config::reduce_effects() { 250 } else { 1000 / 12 };
        self.wake_after_in(std::time::Duration::from_millis(ms), Rect::new(cx - h, cy - h, h * 2.0, h * 2.0));
        spinner(self.c, cx, cy, size / 2.0, anim_phase(1000), rgba(60, 60, 64, 255));
    }

    // -----------------------------------------------------------------------
    // Rolagem
    // -----------------------------------------------------------------------

    /// Área rolável na vertical com a barra de rolagem em gel à direita, sempre visível
    /// como na era (sem polegar quando tudo cabe). Aplica a roda do mouse, as setas, o
    /// clique no trilho e o arrasto do polegar a `offset` e devolve a área do conteúdo.
    /// Área com rolagem vertical para um formulário: o conteúdo é desenhado deslocado,
    /// recortado na área, e só recebe o mouse dentro dela. `content` é a altura do
    /// conteúdo (a do quadro anterior); devolve a deste quadro. A barra só aparece
    /// quando o conteúdo não cabe.
    pub fn scroll_view(&mut self, area: Rect, offset: &mut f32, content: f32, f: impl FnOnce(&mut Self)) -> f32 {
        let inner = if content > area.h + 0.5 { self.scroll_area(area, offset, content, 30.0) } else { area };
        *offset = offset.clamp(0.0, (content - inner.h).max(0.0));
        let s = self.c.scale;
        let old_clip = self.c.clip();
        self.c.set_clip(Some([(inner.x * s).floor() as i32, (inner.y * s).floor() as i32, ((inner.x + inner.w) * s).ceil() as i32, ((inner.y + inner.h) * s).ceil() as i32]));
        let was = self.clip_blocked;
        self.hover_log.borrow_mut().push(inner);
        if !inner.contains(self.input.pos) {
            self.clip_blocked = true;
        }
        let (_, h) = self.child(false, inner.x, inner.y - *offset, inner.w, f);
        self.clip_blocked = was;
        self.c.set_clip(old_clip);
        h
    }

    pub fn scroll_area(&mut self, area: Rect, offset: &mut f32, content: f32, line: f32) -> Rect {
        let id = self.next_id();
        if self.hovered(area) && self.input.scroll != 0.0 {
            *offset += self.input.scroll;
        }
        // A barra para acima do puxador de redimensionar.
        let at_corner = self.resizable && area.x + area.w >= self.c.w - 0.5 && area.y + area.h >= self.c.h - 0.5;
        let sb = Rect::new(area.x + area.w - SCROLLBAR_W, area.y, SCROLLBAR_W, if at_corner { area.h - SCROLLBAR_W } else { area.h });
        self.vscrollbar(id, sb, offset, content, area.h, line);
        Rect::new(area.x, area.y, area.w - SCROLLBAR_W, area.h)
    }

    /// Barra de rolagem vertical avulsa em `sb` (para quem cuida da própria rolagem, como
    /// uma imagem com zoom). `offset` vai de 0 a `content - view`.
    pub fn vscrollbar_in(&mut self, sb: Rect, offset: &mut f32, content: f32, view: f32, line: f32) {
        let id = self.next_id();
        self.vscrollbar(id, sb, offset, content, view, line);
    }

    /// Barra de rolagem horizontal em `sb` (as duas setas juntas à direita, como a vertical).
    /// `offset` vai de 0 a `content - view`.
    pub fn hscrollbar(&mut self, sb: Rect, offset: &mut f32, content: f32, view: f32, line: f32) {
        let id = self.next_id();
        let max = (content - view).max(0.0);
        let aw = SCROLL_ARROW_H;
        let arrows = sb.w >= 2.0 * aw + 30.0 && !self.theme.modern;
        let track = Rect::new(sb.x, sb.y, sb.w - if arrows { 2.0 * aw } else { 0.0 }, sb.h);
        let left = Rect::new(track.x + track.w, sb.y, aw, sb.h);
        let right = Rect::new(left.x + aw, sb.y, aw, sb.h);
        let thumb_w = if content > 0.0 { (track.w * view / content).clamp(24.0_f32.min(track.w), track.w) } else { track.w };
        let thumb_x = |o: f32| track.x + if max > 0.0 { o / max * (track.w - thumb_w) } else { 0.0 };
        let grab = id ^ 0x5d5d;
        if max > 0.0 && self.input.pressed {
            let mx = self.pos().0;
            if self.hovered(track) {
                let tx = thumb_x(*offset);
                if mx >= tx && mx < tx + thumb_w {
                    self.mem.slider_grab = Some(grab);
                    self.mem.scroll.insert(grab, mx - tx);
                } else {
                    let page = (view - line).max(line);
                    *offset += if mx < tx { -page } else { page };
                }
            } else if arrows && self.hovered(left) {
                *offset -= line;
            } else if arrows && self.hovered(right) {
                *offset += line;
            }
        }
        if self.mem.slider_grab == Some(grab) {
            if self.input.down {
                let off = *self.mem.scroll.get(&grab).unwrap_or(&0.0);
                let t = ((self.pos().0 - off - track.x) / (track.w - thumb_w).max(1.0)).clamp(0.0, 1.0);
                *offset = t * max;
            }
            if self.input.released {
                self.mem.slider_grab = None;
            }
        }
        *offset = offset.clamp(0.0, max);
        let (accent, grabbed) = (self.theme.accent, self.mem.slider_grab == Some(grab));
        if self.theme.modern {
            // Moderno: trilho liso e o puxador fino e arredondado, sem setas.
            let (track_bg, thumb) = modern_scroll_colors(self.theme, grabbed || self.hovered(sb));
            let c = &mut *self.c;
            c.fill_rect(sb.x, sb.y, sb.w, sb.h, &solid(track_bg));
            c.hline(sb.x, sb.y, sb.w, self.theme.separator);
            if max > 0.0 {
                let tx = thumb_x(*offset);
                let th = 7.0;
                c.fill(rrect(tx + 2.0, sb.y + (sb.h - th) / 2.0, (thumb_w - 4.0).max(th), th, th / 2.0), &solid(thumb));
            }
            return;
        }
        let c = &mut *self.c;
        c.fill_rect(sb.x, sb.y, sb.w, sb.h, &vgrad(sb.y, sb.y + sb.h, &[(0.0, rgba(200, 200, 204, 255)), (0.35, rgba(238, 238, 240, 255)), (1.0, rgba(226, 226, 230, 255))]));
        c.hline(sb.x, sb.y, sb.w, rgba(150, 150, 156, 255));
        if max > 0.0 {
            let tx = thumb_x(*offset);
            let base = if grabbed { darken(accent, 0.15) } else { accent };
            gel_rect(c, tx + 1.0, sb.y + 2.0, thumb_w - 2.0, sb.h - 3.0, (sb.h - 3.0) / 2.0, base);
        }
        if arrows {
            for (r, is_left) in [(left, true), (right, false)] {
                c.fill_rect(r.x, r.y + 1.0, r.w, r.h - 1.0, &vgrad(r.y, r.y + r.h, &[(0.0, rgba(222, 222, 226, 255)), (0.5, rgba(252, 252, 252, 255)), (1.0, rgba(236, 236, 240, 255))]));
                c.vline(r.x, r.y, r.h, rgba(150, 150, 156, 255));
                let ink = if max > 0.0 { rgba(70, 70, 74, 255) } else { rgba(170, 170, 174, 255) };
                let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0 + 0.5);
                let d = if is_left { -1.0 } else { 1.0 };
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                pb.move_to(cx - 2.0 * d, cy - 3.5);
                pb.line_to(cx - 2.0 * d, cy + 3.5);
                pb.line_to(cx + 2.5 * d, cy);
                pb.close();
                c.fill(pb.finish(), &solid(ink));
            }
        }
    }

    /// Barra de rolagem vertical: trilho em relevo, polegar em cápsula de gel e as duas
    /// setas juntas embaixo (padrão da era).
    fn vscrollbar(&mut self, id: Id, sb: Rect, offset: &mut f32, content: f32, view: f32, line: f32) {
        use std::time::{Duration, Instant};
        let max = (content - view).max(0.0);
        let arrows = sb.h >= 2.0 * SCROLL_ARROW_H + 30.0 && !self.theme.modern;
        let track = Rect::new(sb.x, sb.y, sb.w, sb.h - if arrows { 2.0 * SCROLL_ARROW_H } else { 0.0 });
        let up = Rect::new(sb.x, track.y + track.h, sb.w, SCROLL_ARROW_H);
        let down = Rect::new(sb.x, up.y + SCROLL_ARROW_H, sb.w, SCROLL_ARROW_H);
        let thumb_h = if content > 0.0 { (track.h * view / content).clamp(24.0_f32.min(track.h), track.h) } else { track.h };
        let thumb_y = |o: f32| track.y + if max > 0.0 { o / max * (track.h - thumb_h) } else { 0.0 };
        let (grab, up_id, down_id) = (id ^ 0x5c5c, id ^ 0x5c01, id ^ 0x5c02);

        if max > 0.0 && self.input.pressed {
            let my = self.pos().1;
            if self.hovered(track) {
                let ty = thumb_y(*offset);
                if my >= ty && my < ty + thumb_h {
                    self.mem.slider_grab = Some(grab);
                    self.mem.scroll.insert(grab, my - ty);
                } else if crate::platform::SCROLL_JUMP.load(std::sync::atomic::Ordering::Relaxed) {
                    // "Rolar até aqui": o meio do puxador vai para o clique e já pode arrastar.
                    let room = (track.h - thumb_h).max(1.0);
                    *offset = ((my - track.y - thumb_h / 2.0) / room * max).clamp(0.0, max);
                    self.mem.slider_grab = Some(grab);
                    self.mem.scroll.insert(grab, thumb_h / 2.0);
                } else {
                    let page = (view - line).max(line);
                    *offset += if my < ty { -page } else { page };
                }
            } else if arrows && (self.hovered(up) || self.hovered(down)) {
                let (aid, d) = if self.hovered(up) { (up_id, -line) } else { (down_id, line) };
                *offset += d;
                self.mem.active = Some(aid);
                // Segurar a seta repete, depois de uma pausa.
                self.mem.scroll_repeat = Some((aid, Instant::now() + Duration::from_millis(350)));
                self.wake_after(Duration::from_millis(350));
            }
        }
        if self.mem.slider_grab == Some(grab) {
            if self.input.down {
                let off = *self.mem.scroll.get(&grab).unwrap_or(&0.0);
                let t = ((self.pos().1 - off - track.y) / (track.h - thumb_h).max(1.0)).clamp(0.0, 1.0);
                *offset = t * max;
            }
            if self.input.released {
                self.mem.slider_grab = None;
            }
        }
        let held = match self.mem.scroll_repeat {
            Some((aid, _)) if aid == up_id || aid == down_id => Some(aid),
            _ => None,
        };
        if let Some(aid) = held {
            if !self.input.down || self.input.released {
                self.mem.scroll_repeat = None;
                if self.mem.active == Some(aid) {
                    self.mem.active = None;
                }
            } else {
                let r = if aid == up_id { up } else { down };
                let now = Instant::now();
                if let Some((_, at)) = self.mem.scroll_repeat.filter(|(_, at)| now >= *at) {
                    let _ = at;
                    if self.hovered(r) {
                        *offset += if aid == up_id { -line } else { line };
                    }
                    self.mem.scroll_repeat = Some((aid, now + Duration::from_millis(50)));
                }
                self.wake_after(Duration::from_millis(50));
            }
        }
        *offset = offset.clamp(0.0, max);

        // Seta apertada (fica azul enquanto o mouse está sobre ela).
        let lit_up = self.input.down && held == Some(up_id) && self.hovered(up);
        let lit_down = self.input.down && held == Some(down_id) && self.hovered(down);
        let (accent, grabbed) = (self.theme.accent, self.mem.slider_grab == Some(grab));

        if self.theme.modern {
            let (track_bg, thumb) = modern_scroll_colors(self.theme, grabbed || self.hovered(sb));
            let c = &mut *self.c;
            c.fill_rect(sb.x, sb.y, sb.w, sb.h, &solid(track_bg));
            c.vline(sb.x, sb.y, sb.h, self.theme.separator);
            if max > 0.0 {
                let ty = thumb_y(*offset);
                let tw = 7.0;
                c.fill(rrect(sb.x + (sb.w - tw) / 2.0, ty + 2.0, tw, (thumb_h - 4.0).max(tw), tw / 2.0), &solid(thumb));
            }
            let _ = (lit_up, lit_down, accent);
            return;
        }
        // Trilho em relevo com listras leves
        let c = &mut *self.c;
        c.fill_rect(sb.x, sb.y, sb.w, sb.h, &linear(sb.x, 0.0, sb.x + sb.w, 0.0, &[(0.0, rgba(200, 200, 204, 255)), (0.35, rgba(238, 238, 240, 255)), (1.0, rgba(226, 226, 230, 255))]));
        c.pinstripes(sb.x + 1.0, track.y, sb.w - 1.0, track.h, rgba(255, 255, 255, 0), rgba(255, 255, 255, 60));
        c.vline(sb.x, sb.y, sb.h, rgba(150, 150, 156, 255));
        if max > 0.0 {
            let ty = thumb_y(*offset);
            let base = if grabbed { darken(accent, 0.15) } else { accent };
            gel_rect(c, sb.x + 2.0, ty + 1.0, sb.w - 3.0, thumb_h - 2.0, (sb.w - 3.0) / 2.0, base);
        }
        if arrows {
            for (r, is_up, lit) in [(up, true, lit_up), (down, false, lit_down)] {
                let fill = if lit {
                    linear(r.x, 0.0, r.x + r.w, 0.0, &[(0.0, darken(accent, 0.1)), (0.5, lighten(accent, 0.25)), (1.0, accent)])
                } else {
                    linear(r.x, 0.0, r.x + r.w, 0.0, &[(0.0, rgba(222, 222, 226, 255)), (0.5, rgba(252, 252, 252, 255)), (1.0, rgba(236, 236, 240, 255))])
                };
                c.fill_rect(r.x + 1.0, r.y, r.w - 1.0, r.h, &fill);
                c.hline(r.x, r.y, r.w, rgba(150, 150, 156, 255));
                let ink = if lit { Color::WHITE } else if max > 0.0 { rgba(70, 70, 74, 255) } else { rgba(170, 170, 174, 255) };
                let (cx, cy) = (r.x + r.w / 2.0 + 0.5, r.y + r.h / 2.0);
                let d = if is_up { -1.0 } else { 1.0 };
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                pb.move_to(cx - 3.5, cy - 2.0 * d);
                pb.line_to(cx + 3.5, cy - 2.0 * d);
                pb.line_to(cx, cy + 2.5 * d);
                pb.close();
                c.fill(pb.finish(), &solid(ink));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Lista
    // -----------------------------------------------------------------------

    pub fn list(&mut self, selected: &mut Option<usize>, rows: &[String], width: f32, height: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, height);
        let row_h = 19.0;
        let content = rows.len() as f32 * row_h;
        let mut off = *self.mem.scroll.get(&id).unwrap_or(&0.0);
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        let inner = self.scroll_area(Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0), &mut off, content, row_h);
        self.mem.scroll.insert(id, off);
        let r = Rect::new(r.x, r.y, inner.w + 2.0, r.h);

        let mut changed = false;
        let hover = self.hovered(r);
        if hover && self.input.pressed {
            let idx = ((self.pos().1 - r.y - 1.0 + off) / row_h).floor() as isize;
            if idx >= 0 && (idx as usize) < rows.len() && *selected != Some(idx as usize) {
                *selected = Some(idx as usize);
                changed = true;
            }
        }

        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        let first = (off / row_h).floor() as usize;
        let visible = (height / row_h).ceil() as usize + 1;
        for i in first..(first + visible).min(rows.len()) {
            let y = r.y + 1.0 + i as f32 * row_h - off;
            let (top, bot) = (y.max(r.y + 1.0), (y + row_h).min(r.y + r.h - 1.0));
            if bot <= top {
                continue;
            }
            let sel = *selected == Some(i);
            if sel {
                self.c.fill_rect(r.x + 1.0, top, r.w - 2.0, bot - top, &vgrad(y, y + row_h, &[(0.0, lighten(self.theme.accent, 0.15)), (1.0, darken(self.theme.accent, 0.1))]));
            } else if i % 2 == 1 {
                self.c.fill_rect(r.x + 1.0, top, r.w - 2.0, bot - top, &solid(rgba(237, 243, 254, 255)));
            }
            if y >= r.y && y + row_h <= r.y + r.h + 4.0 {
                let t = self.fonts.ellipsize(&rows[i], 12.0, false, r.w - 16.0);
                self.fonts.draw(self.c, &t, r.x + 8.0, y + 14.0, 12.0, false, if sel { Color::WHITE } else { ink() });
            }
        }
        changed
    }

    // -----------------------------------------------------------------------
    // Tabela
    // -----------------------------------------------------------------------

    /// Tabela da era (10.4): cabeçalho em gel claro com divisórias, a coluna ordenada
    /// em azul com ▲/▼, linhas alternadas opcionais, seleção chapada (cinza quando a
    /// janela não está ativa), recuo e triângulos de expansão para árvores e barra de
    /// rolagem em gel. O app guarda as linhas já ordenadas: o kit só troca
    /// `state.sort` quando um cabeçalho é clicado e avisa em `sorted`.
    pub fn table(&mut self, area: Rect, cols: &[Column], rows: &[TableRow], selected: Option<usize>, state: &mut TableState, style: &TableStyle) -> TableResponse {
        let id = self.next_id();
        let mut resp = TableResponse::default();
        let hh = TABLE_HEADER_H;
        let rh = style.row_h;

        // Corpo com a barra de rolagem
        let content = rows.len() as f32 * rh;
        let body = self.scroll_area(Rect::new(area.x, area.y + hh, area.w, area.h - hh), &mut state.scroll, content, rh);
        state.view_h = body.h;
        state.row_h = rh;
        let total_w: f32 = cols.iter().map(|c| c.width).sum::<f32>().max(1.0);
        let xs: Vec<f32> = cols.iter().scan(0.0, |acc, c| {
            let x = *acc;
            *acc += c.width / total_w * body.w;
            Some(x.round())
        }).collect();
        let col_end = |j: usize| xs.get(j + 1).copied().unwrap_or(body.w);
        let tree = rows.iter().any(|r| r.children.is_some() || r.depth > 0);

        // Cabeçalho: clique ordena (de novo na mesma coluna inverte).
        for (j, col) in cols.iter().enumerate() {
            let cid = self.next_id();
            let r = Rect::new(area.x + xs[j], area.y, col_end(j) - xs[j], hh);
            let (_, _, clicked) = if col.sortable { self.interact(cid, r) } else { (false, false, false) };
            if clicked {
                state.sort = Some(match state.sort {
                    Some((c, asc)) if c == j => (j, !asc),
                    _ => (j, !col.right),
                });
                resp.sorted = true;
                state.scroll = 0.0;
            }
        }

        // Linhas: clique seleciona, duplo ativa, triângulo abre/fecha.
        let first = (state.scroll / rh).floor().max(0.0) as usize;
        let count = (body.h / rh).ceil() as usize + 1;
        if self.hovered(body) && self.input.pressed {
            let i = ((self.pos().1 - body.y + state.scroll) / rh).floor();
            if i >= 0.0 && (i as usize) < rows.len() {
                let i = i as usize;
                let row = &rows[i];
                let tx = body.x + xs.first().copied().unwrap_or(0.0) + 4.0 + row.depth as f32 * TABLE_INDENT;
                if tree && row.children.is_some() && self.pos().0 >= tx - 2.0 && self.pos().0 < tx + TABLE_INDENT {
                    resp.toggled = Some(i);
                } else {
                    resp.pressed = Some(i);
                    if self.input.clicks >= 2 {
                        resp.double = Some(i);
                    }
                }
                self.redraw = true;
            }
        }

        // Dica com o texto inteiro da primeira coluna (o nome) quando ele foi cortado.
        for (i, row) in rows.iter().enumerate().skip(first).take(count) {
            let Some(cell) = row.cells.first() else { continue };
            let mut x0 = xs.first().copied().unwrap_or(0.0) + 6.0;
            if tree {
                x0 += row.depth as f32 * TABLE_INDENT + TABLE_INDENT;
            }
            if row.icon.is_some() {
                x0 += (rh - 2.0).min(16.0) + 4.0;
            }
            let avail = col_end(0) - 6.0 - x0;
            let y = body.y + i as f32 * rh - state.scroll;
            let (top, bot) = (y.max(body.y), (y + rh).min(body.y + body.h));
            if bot > top && self.fonts.measure(cell, style.px, false) > avail {
                self.tooltip(Rect::new(body.x, top, col_end(0), bot - top), cell);
            }
        }

        // Desenho do cabeçalho (inclui o canto sobre a barra de rolagem).
        let a = self.theme.accent;
        let normal = [(0.0, rgba(255, 255, 255, 255)), (0.4, rgba(245, 245, 245, 255)), (0.5, rgba(236, 236, 236, 255)), (0.56, rgba(243, 243, 243, 255)), (1.0, rgba(255, 255, 255, 255))];
        let lit = [(0.0, lighten(a, 0.82)), (0.4, lighten(a, 0.58)), (0.5, lighten(a, 0.42)), (0.56, lighten(a, 0.5)), (1.0, lighten(a, 0.8))];
        let (hy, hb) = (area.y, area.y + hh);
        // Moderno: cabeçalho liso, sem a coluna azul; a ordenada fica em negrito.
        let modern = self.theme.modern;
        if modern {
            self.c.fill_rect(area.x, hy, area.w, hh, &solid(Color::WHITE));
        } else {
            self.c.fill_rect(area.x, hy, area.w, hh, &vgrad(hy, hb, &normal));
        }
        let sorted_col = state.sort.map(|(c, _)| c);
        let sep = if modern { rgba(222, 222, 226, 255) } else { rgba(196, 196, 200, 255) };
        for (j, col) in cols.iter().enumerate() {
            let (x0, x1) = (area.x + xs[j], area.x + col_end(j));
            let on = sorted_col == Some(j);
            if on && !modern {
                self.c.fill_rect(x0, hy, x1 - x0, hh, &vgrad(hy, hb, &lit));
            }
            if j > 0 {
                self.c.vline(x0, hy + 3.0, hh - 6.0, sep);
            }
            let ink = if modern && !on { rgba(90, 90, 96, 255) } else { rgba(25, 25, 28, 255) };
            let bold = modern && on;
            let tw = self.fonts.measure(col.title, 11.0, bold);
            let arrow_w = if on { 13.0 } else { 0.0 };
            let avail = x1 - x0 - 12.0 - arrow_w;
            let t = self.fonts.ellipsize(col.title, 11.0, bold, avail);
            let tw = tw.min(avail);
            let base = hy + 12.5;
            // Seta de ordenação do lado oposto ao texto.
            let (tx, ax) = if col.right { (x1 - 6.0 - tw, x0 + 9.0) } else { (x0 + 6.0, x1 - 9.0) };
            self.fonts.draw(self.c, &t, tx, base, 11.0, bold, ink);
            if on {
                let asc = state.sort.is_some_and(|(_, asc)| asc);
                let cy = hy + hh / 2.0;
                let d = if asc { -1.0 } else { 1.0 };
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                if modern {
                    // Divisa fina (chevron) em cinza.
                    pb.move_to(ax - 3.5, cy - 1.5 * d);
                    pb.line_to(ax, cy + 1.5 * d);
                    pb.line_to(ax + 3.5, cy - 1.5 * d);
                    self.c.stroke_round(pb.finish(), &solid(rgba(110, 110, 116, 255)), 1.3);
                } else {
                    pb.move_to(ax - 3.5, cy - 2.0 * d);
                    pb.line_to(ax + 3.5, cy - 2.0 * d);
                    pb.line_to(ax, cy + 2.5 * d);
                    pb.close();
                    self.c.fill(pb.finish(), &solid(darken(a, 0.35)));
                }
            }
        }
        if modern {
            self.c.hline(area.x, hb - 1.0, area.w, sep);
        } else {
            if let Some(j) = sorted_col.filter(|&j| j < cols.len()) {
                self.c.hline(area.x + xs[j], hy, col_end(j) - xs[j], darken(a, 0.2));
            }
            self.c.hline(area.x, hb - 1.0, area.w, rgba(160, 160, 166, 255));
            if let Some(j) = sorted_col.filter(|&j| j < cols.len()) {
                self.c.hline(area.x + xs[j], hb - 1.0, col_end(j) - xs[j], darken(a, 0.05));
            }
        }
        self.c.vline(body.x + body.w, hy + 1.0, hh - 2.0, sep);

        // Corpo, desenhado numa superfície recortada.
        let active = self.input.active;
        let off = state.scroll;
        let fonts = self.fonts;
        let px = style.px;
        let zebra = style.zebra;
        let also = &style.also;
        self.clipped(id, body, |c, _| {
            c.clear(Color::WHITE);
            if zebra {
                // Listras também abaixo da última linha, como na era.
                let n = (off / rh).floor() as usize;
                let mut i = n;
                loop {
                    let y = i as f32 * rh - off;
                    if y >= body.h {
                        break;
                    }
                    if i % 2 == 1 {
                        // Moderno: o cinza bem leve das listas de hoje, não o azul.
                        let z = if modern { rgba(244, 245, 247, 255) } else { rgba(241, 246, 254, 255) };
                        c.fill_rect(0.0, y, body.w, rh, &solid(z));
                    }
                    i += 1;
                }
            }
            for (i, row) in rows.iter().enumerate().skip(first).take(count) {
                let y = i as f32 * rh - off;
                let sel = selected == Some(i) || also.contains(&i);
                if let (Some(t), false) = (row.tint, sel) {
                    c.fill_rect(0.0, y, body.w, rh, &vgrad(y, y + rh, &[(0.0, lighten(t, 0.45)), (1.0, lighten(t, 0.2))]));
                }
                if sel {
                    c.fill_rect(0.0, y, body.w, rh, &solid(if active { a } else { rgba(212, 212, 216, 255) }));
                }
                let ink = if sel && active { Color::WHITE } else { rgba(20, 20, 20, 255) };
                let base = y + (rh + px) / 2.0 - 1.5;
                for (j, col) in cols.iter().enumerate() {
                    let Some(cell) = row.cells.get(j) else { continue };
                    let (mut x0, x1) = (xs[j] + 6.0, col_end(j) - 6.0);
                    if j == 0 {
                        if tree {
                            x0 += row.depth as f32 * TABLE_INDENT;
                            if let Some(open) = row.children {
                                let tc = if sel && active { Color::WHITE } else { rgba(100, 100, 106, 255) };
                                disclosure_triangle(c, x0 + 4.0, y + rh / 2.0, open, tc);
                            }
                            x0 += TABLE_INDENT;
                        }
                        if let Some(icon) = row.icon {
                            let s = (rh - 2.0).min(16.0);
                            icons::draw(c, fonts, icon, x0, y + (rh - s) / 2.0, s, None);
                            x0 += s + 4.0;
                        }
                    }
                    if x1 <= x0 {
                        continue;
                    }
                    let t = fonts.ellipsize(cell, px, false, x1 - x0);
                    let x = if col.right { x1 - fonts.measure(&t, px, false) } else { x0 };
                    fonts.draw(c, &t, x, base, px, false, ink);
                }
            }
        });
        resp
    }

    // -----------------------------------------------------------------------
    // Ícones
    // -----------------------------------------------------------------------

    pub fn icon(&mut self, icon: Icon, size: f32) {
        let r = self.alloc(size, size);
        icons::draw(self.c, self.fonts, icon, r.x, r.y, size, None);
    }

    /// Ícone grande com nome embaixo (barra de painéis das Preferências).
    pub fn icon_tile(&mut self, icon: Icon, t: &str, selected: bool) -> bool {
        let w = (self.fonts.measure(t, 11.0, false) + 12.0).max(64.0);
        let r = self.alloc(w, 58.0);
        self.toolbar_item_in(r, icon, t, selected)
    }

    // -----------------------------------------------------------------------
    // Área de texto (várias linhas)
    // -----------------------------------------------------------------------

    /// Editor de texto com seleção, rolagem e área de transferência.
    /// Devolve verdadeiro se o texto mudou neste quadro.
    pub fn text_area(&mut self, buf: &mut TextBuffer, r: Rect) -> bool {
        let px_doc = if buf.font_px > 0.0 { buf.font_px } else { 13.0 };
        let px_size: f32 = px_doc;
        let lh: f32 = (px_doc * 1.3).round();
        const PAD: f32 = 4.0;
        const SB: f32 = 15.0;
        let id = self.next_id();
        let tr = Rect::new(r.x, r.y, r.w - SB, r.h);
        // A barra de rolagem para acima do puxador de redimensionar.
        let at_corner = self.resizable && r.x + r.w >= self.c.w - 0.5 && r.y + r.h >= self.c.h - 0.5;
        let sb = Rect::new(r.x + r.w - SB, r.y, SB, if at_corner { r.h - SB } else { r.h });
        let fonts = self.doc_fonts;
        let expand = |s: &str| s.replace('\t', "    ");
        let width = |s: &str| fonts.measure(&expand(s), px_size, false);

        if self.mem.focus.is_none() && !self.blocked() {
            self.mem.focus = Some(id);
        }
        let before_len = buf.len();
        let before_text_changed = buf.modified;
        let mut changed = false;

        // Recuo da linha pelo alinhamento (linhas mais largas que a área ficam à esquerda).
        let avail = tr.w - 2.0 * PAD - 2.0;
        let align = buf.align;
        let off = move |buf: &TextBuffer, start: usize| -> f32 {
            if align == crate::text::Align::Left {
                return 0.0;
            }
            let lw = width(&buf.text()[start..buf.line_end(start)]);
            match align {
                crate::text::Align::Center => ((avail - lw) / 2.0).max(0.0),
                _ => (avail - lw).max(0.0),
            }
        };
        // Posição (relativa ao conteúdo) → byte no texto.
        let hit = |buf: &TextBuffer, starts: &[usize], x: f32, y: f32| -> usize {
            let line = ((y / lh).floor().max(0.0) as usize).min(starts.len() - 1);
            let start = starts[line];
            let end = buf.line_end(start);
            let x = x - off(buf, start);
            let s = &buf.text()[start..end];
            let mut best = start;
            let mut prev_w = 0.0;
            for (i, ch) in s.char_indices() {
                let w = width(&s[..i + ch.len_utf8()]);
                if x < (prev_w + w) / 2.0 {
                    return start + i;
                }
                prev_w = w;
                best = start + i + ch.len_utf8();
            }
            best
        };

        let mut starts = buf.line_starts();
        let content_h = starts.len() as f32 * lh + 2.0 * PAD;
        let max_scroll = (content_h - r.h).max(0.0);

        // Mouse no texto
        let hover = self.hovered(tr);
        let local = |ui: &Self, buf: &TextBuffer| (ui.pos().0 - tr.x - PAD + buf.scroll_x, ui.pos().1 - tr.y - PAD + buf.scroll_y);
        if hover && self.input.pressed {
            self.mem.focus = Some(id);
            self.mem.active = Some(id);
            let (x, y) = local(self, buf);
            let p = hit(buf, &starts, x, y);
            if self.input.clicks >= 3 {
                let (_, ls) = buf.line_of(p);
                let le = buf.line_end(ls);
                buf.anchor = ls;
                buf.cursor = le;
            } else if self.input.clicks == 2 {
                buf.select_word(p);
            } else {
                buf.set_cursor(p, self.input.mods.shift);
            }
            buf.want_x = None;
        } else if self.mem.active == Some(id) && self.input.down && self.input.moved && !self.input.pressed {
            let (x, y) = local(self, buf);
            let p = hit(buf, &starts, x, y);
            if p != buf.cursor {
                buf.set_cursor(p, true);
            }
        }
        if self.hovered(r) && self.input.scroll != 0.0 {
            buf.scroll_y = (buf.scroll_y + self.input.scroll).clamp(0.0, max_scroll);
        }

        // Barra de rolagem (desenhada aqui; o texto fica à esquerda dela)
        self.vscrollbar(id, sb, &mut buf.scroll_y, content_h, r.h, lh);

        // Teclado
        let focused = self.mem.focus == Some(id) && !self.blocked();
        if focused {
            if let Some((pid, text)) = self.mem.paste_in.take() {
                if pid == id {
                    buf.insert(&text);
                } else {
                    self.mem.paste_in = Some((pid, text));
                }
            }
            for ch in self.input.text.chars().filter(|c| !c.is_control()) {
                buf.insert(&ch.to_string());
            }
            let shift = self.input.mods.shift;
            let word = self.input.mods.ctrl || self.input.mods.alt;
            let page_lines = ((r.h / lh) as isize - 1).max(1);
            for k in &self.input.keys {
                match k.combo.as_str() {
                    "Ctrl+A" => {
                        buf.select_all();
                        continue;
                    }
                    "Ctrl+C" | "Ctrl+X" => {
                        if buf.has_selection() {
                            self.mem.clip_out = Some(buf.selected_text().to_string());
                            if k.combo == "Ctrl+X" {
                                buf.delete();
                            }
                        }
                        continue;
                    }
                    "Ctrl+V" => {
                        self.mem.paste_req = Some(id);
                        continue;
                    }
                    "Ctrl+Z" => {
                        buf.undo();
                        continue;
                    }
                    "Ctrl+Shift+Z" | "Ctrl+Y" => {
                        buf.redo();
                        continue;
                    }
                    _ => {}
                }
                let vertical = |buf: &mut TextBuffer, starts: &[usize], delta: isize| {
                    let (line, start) = buf.line_of(buf.cursor);
                    let x = buf.want_x.unwrap_or_else(|| off(buf, start) + width(&buf.text()[start..buf.cursor]));
                    let target = (line as isize + delta).clamp(0, starts.len() as isize - 1) as usize;
                    let p = if delta < 0 && line == 0 {
                        0
                    } else if delta > 0 && line + 1 == starts.len() {
                        buf.len()
                    } else {
                        hit(buf, starts, x, target as f32 * lh + lh / 2.0)
                    };
                    buf.set_cursor(p, shift);
                    buf.want_x = Some(x);
                };
                match k.key {
                    Key::Backspace => buf.backspace(),
                    Key::Delete => buf.delete(),
                    Key::Enter => buf.insert("\n"),
                    Key::Tab => buf.insert("\t"),
                    Key::Left => buf.left(shift, word),
                    Key::Right => buf.right(shift, word),
                    Key::Up => vertical(buf, &starts, -1),
                    Key::Down => vertical(buf, &starts, 1),
                    Key::PageUp => vertical(buf, &starts, -page_lines),
                    Key::PageDown => vertical(buf, &starts, page_lines),
                    Key::Home => buf.home(shift, self.input.mods.ctrl),
                    Key::End => buf.end(shift, self.input.mods.ctrl),
                    _ => {}
                }
                starts = buf.line_starts();
            }
        }
        if buf.len() != before_len || (buf.modified && !before_text_changed) {
            changed = true;
            starts = buf.line_starts();
        }

        // Mantém o cursor visível
        let content_h = starts.len() as f32 * lh + 2.0 * PAD;
        let max_scroll = (content_h - r.h).max(0.0);
        if buf.reveal {
            buf.reveal = false;
            let (line, start) = buf.line_of(buf.cursor);
            let cy = line as f32 * lh;
            if cy < buf.scroll_y {
                buf.scroll_y = cy;
            } else if cy + lh + 2.0 * PAD > buf.scroll_y + r.h {
                buf.scroll_y = cy + lh + 2.0 * PAD - r.h;
            }
            let cx = off(buf, start) + width(&buf.text()[start..buf.cursor]);
            let vis = tr.w - 2.0 * PAD - 8.0;
            if cx < buf.scroll_x {
                buf.scroll_x = (cx - 40.0).max(0.0);
            } else if cx > buf.scroll_x + vis {
                buf.scroll_x = cx - vis + 40.0;
            }
        }
        buf.scroll_y = buf.scroll_y.clamp(0.0, max_scroll);

        // Desenho numa superfície de rascunho (recorta o que sai da área)
        let scale = self.c.scale;
        let mut sc = match self.mem.scratch.remove(&id) {
            Some(c) if c.w == tr.w && c.h == tr.h && c.scale == scale => c,
            _ => match Canvas::new(tr.w, tr.h, scale) {
                Some(c) => c,
                None => return changed,
            },
        };
        sc.dark_remap = self.c.dark_remap;
        self.c.begin_sub(&mut sc, tr.x, tr.y);
        sc.clear(buf.paper.map(|(r, g, b)| rgba(r, g, b, 255)).unwrap_or(Color::WHITE));
        let (sel_a, sel_b) = buf.selection();
        let sel_color = if focused { self.theme.highlight } else { rgba(212, 212, 216, 255) };
        let first = (buf.scroll_y / lh).floor() as usize;
        let count = (r.h / lh).ceil() as usize + 2;
        for line in first..(first + count).min(starts.len()) {
            let start = starts[line];
            let end = buf.line_end(start);
            let y = PAD + line as f32 * lh - buf.scroll_y;
            let x0 = PAD - buf.scroll_x + off(buf, start);
            if sel_a != sel_b && sel_a <= end && sel_b >= start {
                let a = sel_a.max(start);
                let b = sel_b.min(end);
                let xa = width(&buf.text()[start..a]);
                let mut xb = width(&buf.text()[start..b]);
                if sel_b > end {
                    xb += 6.0;
                }
                sc.fill_rect(x0 + xa, y, (xb - xa).max(0.0), lh, &solid(sel_color));
            }
            let s = expand(&buf.text()[start..end]);
            fonts.draw(&mut sc, &s, x0, y + px_size, px_size, false, ink());
            if focused && sel_a == sel_b && buf.cursor >= start && buf.cursor <= end {
                let cx = x0 + width(&buf.text()[start..buf.cursor]);
                sc.fill_rect(cx.round(), y + 1.0, 1.0, lh - 2.0, &solid(ink()));
            }
        }
        self.c.end_sub(&mut sc, tr.x, tr.y);
        self.mem.scratch.insert(id, sc);
        self.mem.scratch_used.push(id);

        // Moldura
        self.c.hline(r.x, r.y, r.w, rgba(120, 120, 126, 255));
        self.c.fill_rect(r.x, r.y, r.w, 3.0, &vgrad(r.y, r.y + 3.0, &[(0.0, rgba(0, 0, 0, 35)), (1.0, rgba(0, 0, 0, 0))]));

        changed
    }

    // -----------------------------------------------------------------------
    // Fim do quadro: desenha o pop-up aberto por cima de tudo
    // -----------------------------------------------------------------------

    /// As ranhuras do puxador, com o canto inferior direito em `(x, y)`.
    pub(crate) fn grip_marks(&mut self, x: f32, y: f32) {
        let (cx, cy) = (x.floor(), y.floor());
        // No moderno, duas linhas finas e claras (como nas janelas de hoje), sem as cavas.
        if glove_paint::theme::modern() {
            let cor = if glove_paint::theme::dark() { rgba(255, 255, 255, 90) } else { rgba(0, 0, 0, 80) };
            for len in [8.0f32, 4.0] {
                for t in 0..(len as i32) {
                    self.c.fill_cell(cx - len + t as f32, cy - t as f32, 1.0, 1.0, cor);
                }
            }
            return;
        }
        for len in [13.0f32, 10.0, 7.0, 4.0] {
            let n = len as i32;
            for t in 0..n {
                let (x, y) = (cx - len + t as f32, cy - t as f32);
                self.c.fill_cell(x, y, 1.0, 1.0, rgba(70, 70, 74, 230));
                self.c.fill_cell(x + 1.0, y, 1.0, 1.0, rgba(255, 255, 255, 220));
            }
        }
    }

    /// Puxador no canto inferior direito (redimensionar arrastando).
    fn resize_grip(&mut self) {
        let (w, h) = (self.c.w, self.c.h);
        let r = Rect::new(w - 15.0, h - 15.0, 15.0, 15.0);
        // Como no Tiger: quatro ranhuras diagonais direto sobre a janela (sem caixa),
        // pixel a pixel (nítidas): um traço escuro com um claro colado, como cavadas.
        self.grip_marks(w - 2.0, h - 2.0);
        // Vale também com uma folha aberta (controles desligados): a janela Abrir/Salvar
        // é uma folha que ocupa a janela toda, e a folha de um app cresce com a janela dele.
        if !self.captured && !self.reserved && self.input.pressed && r.contains(self.input.pos) {
            self.mem.resize_req = true;
        }
    }

    /// O Tab entre os campos de texto: com um campo em foco, vai para o próximo (Shift+Tab,
    /// o anterior), dando a volta; sem nenhum em foco, o Tab vai para o primeiro. Um campo
    /// de várias linhas (que usa o Tab para inserir tabulação) fica de fora.
    fn tab_focus(&mut self) {
        let order = std::mem::take(&mut self.mem.focus_order);
        let dir = match self.mem.tab_req.take() {
            Some(d) => d,
            None if self.mem.focus.is_none() && self.input.active && !self.blocked() => match self.input.keys.iter().find(|k| k.key == Key::Tab && (k.combo == "Tab" || k.combo == "Shift+Tab")) {
                Some(k) => if k.combo == "Shift+Tab" { -1 } else { 1 },
                None => return,
            },
            None => return,
        };
        if order.is_empty() {
            return;
        }
        let n = order.len() as i32;
        let next = match self.mem.focus.and_then(|f| order.iter().position(|x| *x == f)) {
            Some(i) => order[(i as i32 + dir).rem_euclid(n) as usize],
            None if dir > 0 => order[0],
            None => order[order.len() - 1],
        };
        self.mem.focus = Some(next);
        self.mem.focus_enter = Some(next);
        self.mem.wake_after = Some(std::time::Duration::ZERO);
    }

    pub(crate) fn finish(mut self) -> (bool, bool) {
        self.tab_focus();
        // Pop-up aberto: os itens acendem pela posição do mouse.
        let sensitive = self.ptr_used.get() || self.mem.popup.as_ref().is_some_and(|p| p.native != Some(true)) || self.mem.capture.is_some();
        self.mem.pointer_sensitive = sensitive;
        self.mem.hover_rects = std::mem::take(&mut *self.hover_log.borrow_mut());
        if self.resizable {
            self.resize_grip();
        }
        // A dica: depois de meio segundo parado na área, um balão embaixo dela.
        if !std::mem::take(&mut self.mem.tip_seen) || self.mem.popup.is_some() {
            self.mem.tip = None;
        } else if let Some((text, r, since)) = self.mem.tip.clone() {
            let wait = std::time::Duration::from_millis(550);
            let el = since.elapsed();
            if el < wait {
                self.mem.wake_after = Some(self.mem.wake_after.map_or(wait - el, |w| w.min(wait - el)));
                self.mem.anim_full = true;
            } else {
                let px = 11.5;
                let tw = self.fonts.measure(&text, px, false).min(self.c.w - 16.0);
                let (bw, bh) = (tw + 14.0, 20.0);
                let x = r.x.clamp(4.0, (self.c.w - bw - 4.0).max(4.0));
                let y = if r.y + r.h + 4.0 + bh <= self.c.h - 2.0 { r.y + r.h + 4.0 } else { (r.y - bh - 4.0).max(2.0) };
                self.c.fill(rrect(x + 1.0, y + 2.0, bw, bh, 4.0), &solid(rgba(0, 0, 0, 40)));
                self.c.fill(rrect(x, y, bw, bh, 4.0), &solid(rgba(255, 255, 220, 250)));
                self.c.stroke(rrect(x + 0.5, y + 0.5, bw - 1.0, bh - 1.0, 4.0), &solid(rgba(0, 0, 0, 90)), 1.0);
                let t = self.fonts.ellipsize(&text, px, false, self.c.w - 16.0);
                self.fonts.draw(self.c, &t, x + 7.0, y + 14.5, px, false, rgba(20, 20, 20, 255));
            }
        }
        if let Some(mut p) = self.mem.popup.take() {
            match p.native {
                // A plataforma abre a superfície própria logo depois deste quadro.
                None if self.mem.native_popups => {
                    p.fresh = false;
                    self.mem.popup = Some(p);
                }
                // Aberta na superfície própria: um clique na janela (fora dela) fecha
                // sem mudar nada; a plataforma some com a superfície.
                Some(true) => {
                    if self.input.pressed || self.input.right_pressed {
                        self.redraw = true;
                    } else {
                        self.mem.popup = Some(p);
                    }
                }
                _ => self.popup_in_window(p),
            }
        }
        if self.input.released {
            self.mem.active = None;
        }
        // Libera superfícies de recorte que não apareceram neste quadro.
        let used = std::mem::take(&mut self.mem.scratch_used);
        self.mem.scratch.retain(|k, _| used.contains(k));
        let usadas = std::mem::take(&mut self.mem.linhas_usadas);
        self.mem.linhas.retain(|k, _| usadas.contains(k));
        (self.redraw, self.close)
    }

    /// A lista suspensa desenhada dentro da janela (reserva: sem superfície própria).
    fn popup_in_window(&mut self, mut p: PopupState) {
        let r = p.rect;
        let fresh = std::mem::replace(&mut p.fresh, false);
        let item_h = 20.0;
        let hover = if r.contains(self.input.pos) { Some(((self.input.pos.1 - r.y - 4.0) / item_h).floor() as isize) } else { None };
        let mut keep = true;
        if !fresh && (self.input.released || self.input.pressed) {
            if let Some(h) = hover {
                if self.input.released {
                    // Na margem (fora dos itens) só fecha, sem mudar o valor.
                    if h >= 0 && (h as usize) < p.items.len() {
                        self.mem.popup_choice = Some((p.id, h as usize));
                    }
                    keep = false;
                    self.redraw = true;
                }
            } else if self.input.pressed {
                keep = false;
                self.redraw = true;
            }
        }
        if keep {
            let hot = hover.filter(|h| *h >= 0).map(|h| h as usize);
            draw_popup_list(self.c, self.fonts, self.theme, r, &p.items, p.selected, hot, 0.0, (false, false));
            self.mem.popup = Some(p);
        }
    }
}

/// "Super+Return" → "Super + Enter" (mais legível).
pub fn pretty_combo(s: &str) -> String {
    s.split('+')
        .map(|p| match p {
            "Return" => "Enter",
            "BackSpace" => "Backspace",
            "Comma" | "comma" => ",",
            "Up" => "↑",
            "Down" => "↓",
            "Left" => "←",
            "Right" => "→",
            "Print" => "Print Screen",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// O app fala com o Glove (barra de menu global): os menus de contexto podem ser soltos.
pub fn native_menus() -> bool {
    std::env::var_os("GLOVE_MENU_SOCKET").is_some_and(|p| !p.is_empty())
}
