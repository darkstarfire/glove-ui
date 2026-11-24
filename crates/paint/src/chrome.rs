//! Peças da interface desenhadas pelo compositor: barra de título,
//! barra de menu, menus suspensos e o Dock. Tudo em coordenadas lógicas,
//! gerado na escala da tela.

use glove_i18n::tr;
use crate::draw::*;
use crate::font::Fonts;
use crate::icons::{self, Icon};
use crate::theme::{MENUBAR_H, TITLE_H, Theme, Variant};
use tiny_skia::Color;

// ---------------------------------------------------------------------------
// Barra de título
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TitleButton {
    Close,
    Minimize,
    Zoom,
}

pub const BUTTON_R: f32 = 6.5;
const BUTTONS: [(TitleButton, f32); 3] = [(TitleButton::Close, 14.0), (TitleButton::Minimize, 34.0), (TitleButton::Zoom, 54.0)];

/// Qual botão está sob (x, y), em coordenadas da barra de título.
pub fn title_button_at(x: f32, y: f32) -> Option<TitleButton> {
    let cy = TITLE_H as f32 / 2.0;
    BUTTONS
        .iter()
        .find(|(_, cx)| (x - cx).powi(2) + (y - cy).powi(2) <= (BUTTON_R + 2.0).powi(2))
        .map(|(b, _)| *b)
}

/// A área dos três botões (para mostrar os símbolos ao passar o mouse).
pub fn over_button_group(x: f32, y: f32) -> bool {
    (4.0..66.0).contains(&x) && (2.0..(TITLE_H as f32 - 2.0)).contains(&y)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TitleBarState {
    pub active: bool,
    pub hover: bool,
    pub pressed: Option<TitleButton>,
    /// Zoom disponível (janelas de tamanho fixo mostram o botão desbotado).
    pub zoom_enabled: bool,
    /// Documento com alterações não salvas: ponto no botão de fechar.
    pub edited: bool,
    /// O app tem barra de ferramentas: a pílula no canto direito a esconde.
    pub pill: bool,
}

/// Pílula do canto direito do título (esconde a barra de ferramentas): (x, y, w, h).
pub fn title_pill_rect(w: f32) -> (f32, f32, f32, f32) {
    let h = TITLE_H as f32;
    (w - 30.0, h / 2.0 - 5.5, 20.0, 11.0)
}

/// Onde ficam o ícone do documento e o título: (x do ícone, x do texto, texto já cortado).
/// Sem documento, o ícone fica em None.
pub fn title_layout(fonts: &Fonts, w: f32, title: &str, doc: bool) -> (Option<f32>, f32, String) {
    let px = 13.0;
    let icon = if doc { 20.0 } else { 0.0 };
    let avail = (w - 2.0 * 72.0 - icon).max(0.0);
    let t = fonts.ellipsize(title, px, false, avail);
    let tw = fonts.measure(&t, px, false);
    let x = ((w - tw - icon) / 2.0).max(72.0);
    if doc { (Some(x), x + icon, t) } else { (None, x, t) }
}

/// Os três botões da janela (fechar, minimizar, zoom) com o primeiro centrado em
/// (x0 + 14, cy): na barra de título do Glove e nas janelas que desenham a própria
/// moldura (a barra de ferramentas da Jangada, por exemplo).
pub fn title_buttons(c: &mut Canvas, theme: &Theme, x0: f32, cy: f32, st: &TitleBarState) {
    for (b, bx) in BUTTONS {
        let cx = x0 + bx;
        let mut base = if st.active {
            match b {
                TitleButton::Close => theme.close,
                TitleButton::Minimize => theme.minimize,
                TitleButton::Zoom => theme.zoom,
            }
        } else {
            theme.inactive_button
        };
        if st.pressed == Some(b) {
            base = darken(base, 0.3);
        }
        let enabled = b != TitleButton::Zoom || st.zoom_enabled;
        if !st.active && theme.variant == Variant::Listras {
            // Inativa no 10.2: só o contorno do botão, com um brilho por dentro.
            c.fill(circle(cx, cy, BUTTON_R), &vgrad(cy - BUTTON_R, cy + BUTTON_R, &[(0.0, rgba(255, 255, 255, 120)), (1.0, rgba(225, 225, 228, 120))]));
            c.stroke(circle(cx, cy, BUTTON_R - 0.5), &solid(rgba(168, 168, 168, 255)), 1.0);
            c.fill(oval(cx - BUTTON_R * 0.55, cy - BUTTON_R * 0.85, BUTTON_R * 1.1, BUTTON_R * 0.7), &solid(rgba(255, 255, 255, 170)));
            continue;
        }
        if theme.modern {
            // Moderno: bolinhas planas com um contorno fino, sem brilho.
            let r = BUTTON_R - 0.5;
            let fill = if enabled { base } else { with_alpha(base, 0.45) };
            c.fill(circle(cx, cy, r), &solid(fill));
            c.stroke(circle(cx, cy, r - 0.25), &solid(with_alpha(darken(base, 0.35), if theme.dark { 0.5 } else { 0.35 })), 0.75);
            if !enabled {
                continue;
            }
        } else if !enabled {
            // Indisponível: o mesmo gel, desbotado.
            gel_circle(c, cx, cy, BUTTON_R, with_alpha(lighten(base, 0.35), 0.4));
            continue;
        } else {
            gel_circle(c, cx, cy, BUTTON_R, base);
        }
        // Alterações não salvas: o ponto escuro no meio do botão vermelho.
        if b == TitleButton::Close && st.edited && !st.hover {
            c.fill(circle(cx, cy, 2.3), &solid(with_alpha(darken(base, 0.65), 0.9)));
        }
        if st.hover {
            let ink = solid(with_alpha(darken(base, 0.7), 0.85));
            let (l, lw) = (2.6, 1.4);
            match b {
                TitleButton::Close => {
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(cx - l, cy - l);
                    pb.line_to(cx + l, cy + l);
                    pb.move_to(cx + l, cy - l);
                    pb.line_to(cx - l, cy + l);
                    c.stroke(pb.finish(), &ink, lw);
                }
                TitleButton::Minimize => c.fill_rect(cx - l - 0.5, cy - lw / 2.0, 2.0 * l + 1.0, lw, &ink),
                TitleButton::Zoom => {
                    c.fill_rect(cx - l - 0.5, cy - lw / 2.0, 2.0 * l + 1.0, lw, &ink);
                    c.fill_rect(cx - lw / 2.0, cy - l - 0.5, lw, 2.0 * l + 1.0, &ink);
                }
            }
        }
    }
}

pub fn titlebar(fonts: &Fonts, theme: &Theme, w: f32, title: &str, st: &TitleBarState, doc_icon: Option<Icon>, scale: f32) -> Option<Canvas> {
    let h = TITLE_H as f32;
    let mut c = Canvas::new(w, h, scale)?;

    match theme.variant {
        _ if theme.modern => {
            // Moderno: liso, da cor da janela (a barra e o corpo parecem uma peça só).
            let fill = match (theme.dark, st.active) {
                (true, true) => rgba(54, 54, 58, 255),
                (true, false) => rgba(44, 44, 48, 255),
                (false, true) => rgba(232, 232, 235, 255),
                (false, false) => rgba(244, 244, 246, 255),
            };
            c.fill_rect(0.0, 0.0, w, h, &solid(fill));
        }
        Variant::Metal => {
            let (top, bot) = if st.active {
                (theme.metal_top, theme.metal_bottom)
            } else {
                (lighten(theme.metal_top, 0.3), lighten(theme.metal_bottom, 0.4))
            };
            c.brushed_metal(0.0, 0.0, w, h, top, bot);
            c.metal_sheen(0.0, 0.0, w, h, w);
        }
        Variant::Claro => {
            // Degradê cinza claro e liso, sem listras (10.4). A inativa fica mais clara.
            let (top, bot) = if st.active {
                (rgba(244, 244, 244, 255), rgba(232, 232, 232, 255))
            } else {
                (rgba(248, 248, 248, 255), rgba(240, 240, 240, 255))
            };
            c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
        }
        Variant::Listras => {
            // Listras claras, quase sem degradê (10.2). A inativa fica translúcida.
            let (top, bot) = if st.active {
                (rgba(246, 246, 246, 255), rgba(226, 226, 226, 255))
            } else {
                (rgba(250, 250, 250, 200), rgba(238, 238, 238, 200))
            };
            c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
            let a = if st.active { 0.6 } else { 0.35 };
            c.pinstripes(0.0, 0.0, w, h, rgba(255, 255, 255, 0), with_alpha(Color::WHITE, a));
        }
        _ => {
            let (top, bot) = if st.active {
                (rgba(248, 248, 248, 255), rgba(206, 206, 208, 255))
            } else {
                (rgba(250, 250, 250, 255), rgba(232, 232, 234, 255))
            };
            c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
            let a = if st.active { 0.35 } else { 0.18 };
            c.pinstripes(0.0, 0.0, w, h, rgba(255, 255, 255, 0), with_alpha(Color::WHITE, a));
        }
    }
    c.hline(0.0, 0.0, w, if theme.dark { rgba(255, 255, 255, 34) } else { rgba(255, 255, 255, 200) });
    let edge = if theme.modern {
        if theme.dark { rgba(22, 22, 24, 255) } else { rgba(206, 206, 210, 255) }
    } else if theme.variant == Variant::Claro {
        rgba(150, 150, 152, 255)
    } else {
        rgba(120, 120, 124, 255)
    };
    // Metal: título e barra de ferramentas são uma peça só (sem a linha entre eles).
    if theme.variant != Variant::Metal {
        c.hline(0.0, h - 1.0 / scale, w, edge);
    }
    c.clip_top_corners(if theme.modern { 10.0 } else { 6.0 });

    let cy = h / 2.0;
    title_buttons(&mut c, theme, 0.0, cy, st);

    let px = 13.0;
    if w - 2.0 * 72.0 > 20.0 {
        let (ix, x, t) = title_layout(fonts, w, title, doc_icon.is_some());
        if let (Some(ix), Some(icon)) = (ix, doc_icon) {
            icons::draw(&mut c, fonts, icon, ix, cy - 8.0, 16.0, None);
        }
        let color = if st.active { theme.title_text } else { theme.title_text_inactive };
        if theme.modern {
            fonts.draw(&mut c, &t, x, 15.5, px, true, color);
        } else {
            fonts.draw_embossed(&mut c, &t, x, 15.5, px, false, color, rgba(255, 255, 255, 180));
        }
    }
    // Pílula de vidro que esconde a barra de ferramentas.
    if st.pill {
        let (x, y, pw, ph) = title_pill_rect(w);
        c.fill(rrect(x, y, pw, ph, ph / 2.0), &vgrad(y, y + ph, &[(0.0, rgba(252, 252, 252, 255)), (1.0, rgba(214, 214, 218, 255))]));
        c.stroke(rrect(x + 0.5, y + 0.5, pw - 1.0, ph - 1.0, ph / 2.0 - 0.5), &solid(rgba(120, 120, 126, if st.active { 220 } else { 140 })), 1.0);
        c.fill(rrect(x + 3.0, y + 1.5, pw - 6.0, ph * 0.4, ph * 0.2), &solid(rgba(255, 255, 255, 160)));
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Sombras de janela (4 faixas em volta, recalculadas só ao redimensionar)
// ---------------------------------------------------------------------------

pub const SHADOW_M: i32 = 22;

pub fn shadow_spec(active: bool) -> ShadowSpec {
    if crate::theme::modern() {
        // Moderno: sombra maior e mais macia, cantos mais redondos.
        return ShadowSpec { blur: if active { 21.0 } else { 15.0 }, offset_y: if active { 7.0 } else { 4.0 }, radius: 10.0, alpha: if active { 0.55 } else { 0.28 } };
    }
    ShadowSpec {
        blur: if active { 20.0 } else { 14.0 },
        offset_y: if active { 5.0 } else { 3.0 },
        radius: 6.0,
        alpha: if active { 0.5 } else { 0.3 },
    }
}

/// Faixas [cima, baixo, esquerda, direita] e suas posições lógicas relativas
/// ao canto superior esquerdo da janela (incluindo a barra de título).
pub fn window_shadow(w: i32, h: i32, active: bool, scale: f32) -> Vec<(i32, i32, Canvas)> {
    let m = SHADOW_M;
    let s = shadow_spec(active);
    // A faixa de cima entra um pouco na janela: os cantos de cima são arredondados
    // e, sem sombra no pedaço recortado, o fundo aparecia ali e o canto parecia vivo.
    // (Fica atrás da barra de título, então só aparece nesses cantos.)
    let r = 7;
    // A de baixo também entra na janela: as de metal têm os cantos de baixo arredondados.
    let parts = [(-m, -m, w + 2 * m, m + r), (-m, h - r, w + 2 * m, m + r), (-m, r, m, h - 2 * r), (w, r, m, h - 2 * r)];
    parts
        .into_iter()
        .filter(|p| p.2 > 0 && p.3 > 0)
        .filter_map(|(x, y, pw, ph)| {
            Canvas::shadow(x as f32, y as f32, pw as f32, ph as f32, w as f32, h as f32, &s, scale).map(|c| (x, y, c))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Barra de menu
// ---------------------------------------------------------------------------

pub struct MenuBarLayout {
    /// Faixa horizontal do botão do logo.
    pub logo: (f32, f32),
    /// Faixa de cada título de menu (o primeiro é o nome do app).
    pub titles: Vec<(f32, f32)>,
    /// Faixa de cada extra (volume, bateria, monitor), à esquerda do relógio.
    pub extras: Vec<(f32, f32)>,
    /// Lupa da Busca Rápida (canto direito).
    pub search: (f32, f32),
}

pub const MENUBAR_PX: f32 = 14.0;

/// `titles[0]` é o nome do app (em negrito). `open`: título aberto (0 = logo, 1.. = títulos);
/// `open_extra`: extra aberto.
/// `search_open`: a Busca Rápida está aberta (lupa realçada).
#[allow(clippy::too_many_arguments)]
pub fn menubar(fonts: &Fonts, theme: &Theme, w: f32, titles: &[String], extras: &[Extra], clock: &str, open: Option<usize>, open_extra: Option<usize>, search_open: bool, scale: f32) -> Option<(Canvas, MenuBarLayout)> {
    let h = MENUBAR_H as f32;
    let mut c = Canvas::new(w, h, scale)?;
    let claro = theme.variant == Variant::Claro;
    let modern = theme.modern;
    if modern {
        // Moderno: translúcida (o papel de parede aparece por trás), sem brilho nem linha forte.
        c.fill_rect(0.0, 0.0, w, h, &solid(if theme.dark { rgba(30, 30, 34, 178) } else { rgba(246, 246, 248, 196) }));
        c.hline(0.0, h - 1.0 / scale, w, if theme.dark { rgba(0, 0, 0, 90) } else { rgba(0, 0, 0, 30) });
    } else if claro {
        // Degradê brilhante e liso: branco, um pouco mais escuro no meio, branco de novo.
        c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, rgba(255, 255, 255, 255)), (0.45, rgba(243, 243, 244, 255)), (0.9, rgba(255, 255, 255, 255))]));
        c.hline(0.0, h - 1.0 / scale, w, rgba(200, 200, 202, 255));
    } else {
        c.pinstripes(0.0, 0.0, w, h, theme.menu_bg_a, theme.menu_bg_b);
        c.hline(0.0, h - 1.0 / scale, w, rgba(0, 0, 0, 110));
    }
    // No moderno, o aberto ganha um realce arredondado e discreto (o texto não muda de cor).
    let open_ink = if modern { theme.menu_text } else { Color::WHITE };
    let hot = |c: &mut Canvas, a: f32, b: f32| {
        if modern {
            c.fill(rrect(a + 1.0, 2.0, b - a - 2.0, h - 5.0, 4.0), &solid(if theme.dark { rgba(255, 255, 255, 46) } else { rgba(0, 0, 0, 34) }));
        } else if claro {
            c.fill_rect(a, 0.0, b - a, h - 1.0, &solid(theme.menu_hot));
        } else {
            c.fill_rect(a, 0.0, b - a, h - 1.0, &vgrad(0.0, h, &[(0.0, lighten(theme.accent, 0.15)), (1.0, darken(theme.accent, 0.15))]));
        }
    };

    let logo = (6.0, 34.0);
    if open == Some(0) {
        hot(&mut c, logo.0, logo.1);
    }
    // Moderno: a luva chapada, preta (claro) ou branca (escuro); clássico: o gel.
    if modern {
        let cor = if theme.dark { Color::WHITE } else { Color::BLACK };
        icons::logo_solid(&mut c, 11.0, 2.0, 18.0, cor);
    } else {
        icons::logo(&mut c, 11.0, 2.0, 18.0, theme.accent);
    }

    let mut x = 40.0;
    let mut ranges = Vec::with_capacity(titles.len());
    for (i, t) in titles.iter().enumerate() {
        let bold = i == 0;
        let tw = fonts.measure(t, MENUBAR_PX, bold);
        let (a, b) = (x, x + tw + 20.0);
        let is_open = open == Some(i + 1);
        if is_open {
            hot(&mut c, a, b);
        }
        fonts.draw(&mut c, t, a + 10.0, 16.0, MENUBAR_PX, bold, if is_open { open_ink } else { theme.menu_text });
        ranges.push((a, b));
        x = b;
    }
    // Lupa da Busca Rápida no canto, num quadrado azul quando aberta (como no 10.4).
    let search = (w - 32.0, w);
    if search_open {
        hot(&mut c, search.0, search.1);
    }
    magnifier_glyph(&mut c, search.0 + 9.0, 4.0, 14.0, if search_open { open_ink } else { theme.menu_text });
    let right = search.0 - 4.0;
    // O relógio pode vir à parte (texto fixo) ou como o último extra (clicável).
    let extras_right = if clock.is_empty() {
        right
    } else {
        let cw = fonts.measure(clock, MENUBAR_PX, false);
        fonts.draw(&mut c, clock, right - cw - 8.0, 16.0, MENUBAR_PX, false, theme.menu_text);
        right - cw - 16.0
    };
    let extra_ranges = draw_extras(&mut c, fonts, theme, extras, extras_right, open_extra, &hot);
    Some((c, MenuBarLayout { logo, titles: ranges, extras: extra_ranges, search }))
}

// ---------------------------------------------------------------------------
// Menu suspenso
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub checked: bool,
    pub separator: bool,
    /// Atalho mostrado à direita (ex.: "Ctrl+S").
    pub shortcut: String,
    pub enabled: bool,
    /// Abre um submenu (mostra ▸ à direita).
    pub submenu: bool,
    /// ◆ na coluna da marca (janela minimizada no menu Janela).
    pub diamond: bool,
    /// Bolinha de cor antes do nome (rótulos do Arquivos).
    pub swatch: Option<Color>,
}

impl MenuItem {
    pub fn item(label: impl Into<String>) -> Self {
        Self { label: label.into(), checked: false, separator: false, shortcut: String::new(), enabled: true, submenu: false, diamond: false, swatch: None }
    }
    /// Item que abre um submenu.
    pub fn sub(label: impl Into<String>) -> Self {
        Self { submenu: true, ..Self::item(label) }
    }
    pub fn check(label: impl Into<String>, checked: bool) -> Self {
        Self { checked, ..Self::item(label) }
    }
    pub fn with_swatch(mut self, c: Color) -> Self {
        self.swatch = Some(c);
        self
    }
    pub fn sep() -> Self {
        Self { separator: true, ..Self::item("") }
    }
    pub fn with_shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = s.into();
        self
    }
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Atalho em duas partes, como nos menus da era: modificadores em símbolos (na ordem
/// ⌃ ⌥ ⇧ ❖) e a tecla, que fica numa coluna própria. "Ctrl+Shift+S" → ("⌃⇧", "S").
fn shortcut_parts(s: &str) -> (String, String) {
    let parts: Vec<&str> = s.split('+').collect();
    let (mods, key) = parts.split_at(parts.len().saturating_sub(1));
    let mut m = String::new();
    for (name, glyph) in [("Ctrl", "⌃"), ("Alt", "⌥"), ("Shift", "⇧"), ("Super", "❖")] {
        if mods.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            m.push_str(glyph);
        }
    }
    let key = match key.first().copied().unwrap_or("") {
        "Return" => "↩",
        "BackSpace" => "⌫",
        "Delete" => "⌦",
        // A fonte não tem o ⎋.
        "Escape" => "Esc",
        "Tab" => "⇥",
        "Up" => "↑",
        "Down" => "↓",
        "Left" => "←",
        "Right" => "→",
        "Comma" | "comma" => ",",
        "period" => ".",
        "equal" => "=",
        "minus" => "-",
        "plus" => "+",
        "space" => tr!("Espaço"),
        other => other,
    };
    let key = if key.chars().count() == 1 { key.to_uppercase() } else { key.to_string() };
    (m, key)
}

pub const MENU_ITEM_H: f32 = 19.0;
/// Separador: um vão (no tema Listras sem linha, como no 10.2).
const SEP_H: f32 = 11.0;
const MENU_PAD: f32 = 4.0;
/// Margem de sombra em volta do menu (esquerda, direita e embaixo).
pub const MENU_SHADOW: i32 = 12;
const MENU_PX: f32 = 14.0;

/// Larguras das colunas de atalho (modificadores, tecla).
fn shortcut_columns(fonts: &Fonts, items: &[MenuItem]) -> (f32, f32) {
    items.iter().filter(|i| !i.shortcut.is_empty()).fold((0.0f32, 0.0f32), |(mw, kw), i| {
        let (m, k) = shortcut_parts(&i.shortcut);
        (mw.max(fonts.measure(&m, MENU_PX, false)), kw.max(fonts.measure(&k, MENU_PX, false)))
    })
}

pub fn menu_content_size(fonts: &Fonts, items: &[MenuItem]) -> (f32, f32) {
    let (mw, kw) = shortcut_columns(fonts, items);
    let mut shortcuts = if mw + kw > 0.0 { mw + kw + 30.0 } else { 0.0 };
    if items.iter().any(|i| i.submenu) {
        shortcuts = shortcuts.max(SUB_ARROW_W);
    }
    let w = items.iter().map(|i| fonts.measure(&i.label, MENU_PX, false) + if i.swatch.is_some() { 16.0 } else { 0.0 }).fold(0.0, f32::max) + shortcuts + 44.0;
    let h = items.iter().map(|i| if i.separator { SEP_H } else { MENU_ITEM_H }).sum::<f32>() + MENU_PAD * 2.0;
    (w.max(180.0).ceil(), h.ceil())
}

/// Espaço à direita reservado para o ▸ dos submenus.
const SUB_ARROW_W: f32 = 24.0;

/// Topo do item `i` (relativo ao topo do conteúdo do menu).
pub fn menu_item_top(items: &[MenuItem], i: usize) -> f32 {
    MENU_PAD + items.iter().take(i).map(|it| if it.separator { SEP_H } else { MENU_ITEM_H }).sum::<f32>()
}

/// Item sob a coordenada `y` (relativa ao topo do conteúdo do menu).
pub fn menu_item_at(items: &[MenuItem], y: f32) -> Option<usize> {
    let mut cy = MENU_PAD;
    for (i, it) in items.iter().enumerate() {
        let h = if it.separator { SEP_H } else { MENU_ITEM_H };
        if y >= cy && y < cy + h {
            return (!it.separator).then_some(i);
        }
        cy += h;
    }
    None
}

/// Menu completo (com margem de sombra). O conteúdo começa em (MENU_SHADOW, 0).
pub fn menu(fonts: &Fonts, theme: &Theme, items: &[MenuItem], hover: Option<usize>, scale: f32) -> Option<Canvas> {
    let (w, h) = menu_content_size(fonts, items);
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;

    // Sombra forte para baixo e cantos retos, como nos menus da era (no moderno, macia e
    // com os cantos arredondados).
    let modern = theme.modern;
    let rad = if modern { 7.0 } else { 0.0 };
    let s = if modern { ShadowSpec { blur: 10.0, offset_y: 4.0, radius: rad, alpha: 0.32 } } else { ShadowSpec { blur: 8.0, offset_y: 4.0, radius: 0.0, alpha: 0.45 } };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    let claro = theme.variant == Variant::Claro;
    if modern {
        c.fill(rrect(x0, 0.0, w, h, rad), &solid(theme.menu_bg_a));
    } else if claro {
        // 10.4: listras quase invisíveis.
        c.fill_rect(x0, 0.0, w, h, &solid(rgba(237, 240, 241, 250)));
        c.pinstripes(x0, 0.0, w, h, rgba(235, 237, 239, 250), rgba(237, 240, 241, 0));
    } else {
        c.fill_rect(x0, 0.0, w, h, &solid(rgba(246, 247, 249, 248)));
        c.pinstripes(x0, 0.0, w, h, rgba(246, 247, 249, 0), rgba(230, 232, 234, 235));
    }
    if modern {
        c.stroke(rrect(x0 + 0.5, 0.5, w - 1.0, h - 1.0, rad), &solid(if theme.dark { rgba(255, 255, 255, 36) } else { rgba(0, 0, 0, 46) }), 1.0 / scale);
    } else {
        c.stroke(rrect(x0, 0.0, w, h, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    }

    let (mw, kw) = shortcut_columns(fonts, items);
    let key_x = x0 + w - 14.0 - kw;
    let mut cy = MENU_PAD;
    for (i, it) in items.iter().enumerate() {
        if it.separator {
            if modern {
                c.hline(x0 + 10.0, (cy + SEP_H / 2.0).floor(), w - 20.0, theme.separator);
            } else if theme.variant != Variant::Listras {
                let line = if claro { rgba(217, 218, 220, 255) } else { rgba(212, 212, 212, 255) };
                c.hline(x0 + 1.0, (cy + SEP_H / 2.0).floor(), w - 2.0, line);
            }
            cy += SEP_H;
            continue;
        }
        let hot = hover == Some(i) && it.enabled;
        if hot {
            if modern {
                c.fill(rrect(x0 + 5.0, cy, w - 10.0, MENU_ITEM_H, 4.0), &solid(theme.menu_hot));
            } else if claro {
                c.fill_rect(x0, cy, w, MENU_ITEM_H, &solid(theme.menu_hot));
            } else {
                c.fill_rect(x0, cy, w, MENU_ITEM_H, &vgrad(cy, cy + MENU_ITEM_H, &[(0.0, lighten(theme.accent, 0.2)), (1.0, darken(theme.accent, 0.1))]));
            }
        }
        let color = if hot {
            Color::WHITE
        } else if it.enabled {
            theme.menu_text
        } else if theme.dark {
            rgba(112, 112, 118, 255)
        } else {
            rgba(160, 160, 160, 255)
        };
        let base = cy + 14.5;
        if it.checked {
            fonts.draw(&mut c, "✓", x0 + 7.0, base, MENU_PX, false, color);
        } else if it.diamond {
            fonts.draw(&mut c, "◆", x0 + 6.0, base, MENU_PX - 2.0, false, color);
        }
        let mut lx = x0 + 22.0;
        if let Some(sw) = it.swatch {
            c.fill(circle(lx + 5.0, cy + MENU_ITEM_H / 2.0, 5.5), &solid(sw));
            c.stroke(circle(lx + 5.0, cy + MENU_ITEM_H / 2.0, 5.5), &solid(rgba(0, 0, 0, 70)), 1.0);
            lx += 16.0;
        }
        fonts.draw(&mut c, &it.label, lx, base, MENU_PX, false, color);
        if !it.shortcut.is_empty() {
            let (m, k) = shortcut_parts(&it.shortcut);
            let msw = fonts.measure(&m, MENU_PX, false);
            fonts.draw(&mut c, &m, key_x - 2.0 - msw, base, MENU_PX, false, color);
            fonts.draw(&mut c, &k, key_x, base, MENU_PX, false, color);
        }
        if it.submenu {
            // Triângulo cheio apontando para a direita.
            let (tx, ty) = (x0 + w - 16.0, cy + MENU_ITEM_H / 2.0);
            let mut pb = tiny_skia::PathBuilder::new();
            pb.move_to(tx, ty - 4.5);
            pb.line_to(tx + 6.5, ty);
            pb.line_to(tx, ty + 4.5);
            pb.close();
            c.fill(pb.finish(), &solid(color));
        }
        cy += MENU_ITEM_H;
    }
    let _ = mw;
    Some(c)
}

// ---------------------------------------------------------------------------
// Dock
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct DockItem {
    pub icon: Icon,
    pub label: String,
    pub letter: Option<char>,
    pub running: bool,
    pub separator_before: bool,
    /// Altura do pulo agora (app abrindo), em pontos.
    pub bounce: f32,
    /// Janela minimizada: o compositor desenha a miniatura viva no lugar do ícone.
    pub thumbnail: bool,
}

pub const DOCK_BASE: f32 = 56.0;
pub const DOCK_MAX: f32 = 80.0;
/// Espaço entre os ícones do Dock (os do estilo Tiger ocupam quase todo o quadrado).
const DOCK_GAP: f32 = 8.0;
const DOCK_PAD_X_CLASSIC: f32 = 8.0;
const DOCK_PAD_Y: f32 = 4.0;

/// Margem dos lados da prateleira: no Moderno, mais fina (como em cima).
fn dock_pad_x() -> f32 {
    if crate::theme::modern() { 6.0 } else { DOCK_PAD_X_CLASSIC }
}

/// No Moderno a faixa de baixo é mais alta que as bordas de cima e dos lados: o
/// pontinho de app aberto fica nela, com folga do ícone e da borda.
fn dock_bottom_extra() -> f32 {
    if crate::theme::modern() { 8.0 } else { 0.0 }
}
const DOCK_SEP: f32 = 14.0;
const DOCK_LABEL_H: f32 = 26.0;

/// Borda da tela em que o Dock fica.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DockSide {
    #[default]
    Bottom,
    Left,
    Right,
}

impl DockSide {
    pub fn from_config(s: &str) -> Self {
        match s {
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::Bottom,
        }
    }
    pub fn vertical(self) -> bool {
        self != Self::Bottom
    }
}

/// Espaço ao lado dos ícones para o nome, com o Dock na lateral.
const DOCK_SIDE_LABEL_W: f32 = 180.0;

#[derive(Clone, Debug, PartialEq)]
pub struct DockLayout {
    /// Tamanho lógico.
    pub width: u32,
    pub height: u32,
    /// (x, y, tamanho) de cada ícone, relativos ao Dock.
    pub items: Vec<(f32, f32, f32)>,
    /// Prateleira de vidro: (x, y, w, h).
    pub shelf: (f32, f32, f32, f32),
    pub side: DockSide,
}

impl DockLayout {
    /// Coordenada ao longo do Dock (x embaixo, y nas laterais).
    pub fn along(&self, x: f32, y: f32) -> f32 {
        if self.side.vertical() { y } else { x }
    }
}

/// Espessura da prateleira do Dock (o que as janelas não podem cobrir), para ícones de `base`.
pub fn dock_shelf_thickness(base: f32) -> f32 {
    base + DOCK_PAD_Y + 11.0 + dock_bottom_extra()
}

/// Posição de cada ícone com o Dock em `side`. `hover` é a coordenada ao longo do Dock.
pub fn dock_layout_on(items: &[DockItem], hover: Option<f32>, zoom: f32, base: f32, max: f32, side: DockSide) -> DockLayout {
    if side == DockSide::Bottom {
        return dock_layout(items, hover, zoom, base, max);
    }
    // Mede como se estivesse embaixo (com espaço para o nome ao lado) e gira.
    let h = dock_layout_room(items, hover, zoom, base, max, DOCK_SIDE_LABEL_W);
    let thick = h.height as f32;
    let items = h
        .items
        .iter()
        .map(|&(x, y, s)| {
            // Distância da borda da tela até o ícone, igual à de baixo.
            let from_edge = thick - (y + s);
            let nx = if side == DockSide::Left { from_edge } else { thick - from_edge - s };
            (nx, x, s)
        })
        .collect();
    let (sx, _, sw, sh) = h.shelf;
    let shelf = if side == DockSide::Left { (0.0, sx, sh, sw) } else { (thick - sh, sx, sh, sw) };
    DockLayout { width: h.height, height: h.width, items, shelf, side }
}

/// Posição de cada ícone. `hover_x` é relativo ao Dock; `zoom` (0 a 1) é a força
/// da ampliação, animada ao entrar e sair do Dock para não pular de uma vez.
pub fn dock_layout(items: &[DockItem], hover_x: Option<f32>, zoom: f32, base: f32, max: f32) -> DockLayout {
    dock_layout_room(items, hover_x, zoom, base, max, DOCK_LABEL_H)
}

/// Comprimento do Dock (ao longo da borda) para esses ícones e tamanhos.
pub fn dock_length(items: &[DockItem], base: f32, max: f32) -> f32 {
    let n = items.len() as f32;
    let seps = items.iter().filter(|i| i.separator_before).count() as f32;
    n * (base + DOCK_GAP) - DOCK_GAP + seps * DOCK_SEP + (max - base).max(0.0) * 3.2 + 2.0 * dock_pad_x() + 4.0
}

/// Dock embaixo, com `label_room` de espaço acima dos ícones para o nome.
fn dock_layout_room(items: &[DockItem], hover_x: Option<f32>, zoom: f32, base: f32, max: f32, label_room: f32) -> DockLayout {
    let n = items.len() as f32;
    let seps = items.iter().filter(|i| i.separator_before).count() as f32;
    let base_w = n * (base + DOCK_GAP) - DOCK_GAP + seps * DOCK_SEP;
    let extra = (max - base).max(0.0) * 3.2;
    let width = (base_w + extra + 2.0 * dock_pad_x()).ceil() as u32 + 4;
    let height = (label_room + max.max(base + 6.0) + 2.0 * DOCK_PAD_Y + 6.0 + dock_bottom_extra()).ceil() as u32;
    let center = width as f32 / 2.0;

    let mut base_centers = Vec::with_capacity(items.len());
    let mut x = center - base_w / 2.0;
    for it in items {
        if it.separator_before {
            x += DOCK_SEP;
        }
        base_centers.push(x + base / 2.0);
        x += base + DOCK_GAP;
    }

    let range = base * 2.6;
    let sizes: Vec<f32> = base_centers
        .iter()
        .map(|c| match hover_x {
            Some(hx) if max > base => {
                let d = ((hx - c).abs() / range).min(1.0);
                base + (max - base) * zoom.clamp(0.0, 1.0) * 0.5 * (1.0 + (std::f32::consts::PI * d).cos())
            }
            _ => base,
        })
        .collect();
    let total: f32 = sizes.iter().sum::<f32>() + DOCK_GAP * (n - 1.0).max(0.0) + seps * DOCK_SEP;

    let bottom = height as f32 - DOCK_PAD_Y - 6.0 - dock_bottom_extra();
    let mut out = Vec::with_capacity(items.len());
    let mut x = center - total / 2.0;
    for (it, s) in items.iter().zip(&sizes) {
        if it.separator_before {
            x += DOCK_SEP;
        }
        out.push((x, bottom - s, *s));
        x += s + DOCK_GAP;
    }
    // A prateleira envolve os ícones no tamanho normal; ampliados, eles saem por cima.
    // Folga em cima: o ícone não encosta na borda da prateleira (como no Tiger).
    let shelf_h = dock_shelf_thickness(base);
    let shelf = (center - total / 2.0 - dock_pad_x(), height as f32 - shelf_h, total + 2.0 * dock_pad_x(), shelf_h);
    DockLayout { width, height, items: out, shelf, side: DockSide::Bottom }
}

/// Item do Dock sob (x, y) relativos ao Dock.
pub fn dock_item_at(layout: &DockLayout, x: f32, y: f32) -> Option<usize> {
    if layout.side.vertical() {
        let (sx, _, sw, _) = layout.shelf;
        return layout.items.iter().position(|(ix, iy, s)| {
            let across = if layout.side == DockSide::Left { x >= 0.0 && x < (ix + s).max(sx + sw) } else { x >= ix.min(sx) && x < layout.width as f32 };
            across && y >= *iy - DOCK_GAP / 2.0 && y < iy + s + DOCK_GAP / 2.0
        });
    }
    let sy = layout.shelf.1;
    layout
        .items
        .iter()
        .position(|(ix, iy, s)| x >= *ix - DOCK_GAP / 2.0 && x < ix + s + DOCK_GAP / 2.0 && y >= iy.min(sy) && y < layout.height as f32)
}

/// O mouse está sobre o Dock (prateleira ou ícones)?
pub fn dock_contains(layout: &DockLayout, x: f32, y: f32) -> bool {
    if layout.side.vertical() {
        let (sx, sy, sw, sh) = layout.shelf;
        let (lo, hi) = if layout.side == DockSide::Left {
            (0.0, layout.items.iter().map(|i| i.0 + i.2).fold(sx + sw, f32::max))
        } else {
            (layout.items.iter().map(|i| i.0).fold(sx, f32::min), layout.width as f32)
        };
        return x >= lo && x < hi && y >= sy && y < sy + sh;
    }
    let (sx, sy, sw, _) = layout.shelf;
    let top = layout.items.iter().map(|i| i.1).fold(sy, f32::min);
    x >= sx && x < sx + sw && y >= top && y < layout.height as f32
}

/// Nome do app sobre o Dock, como na era: branco em negrito com contorno escuro
/// (cópias deslocadas em volta) e uma sombra leve, sem fundo.
fn outlined_label(fonts: &Fonts, c: &mut Canvas, text: &str, x: f32, y: f32, px: f32) {
    fonts.draw(c, text, x + 0.5, y + 1.6, px, true, rgba(0, 0, 0, 90));
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.8, -0.8), (0.8, -0.8), (-0.8, 0.8), (0.8, 0.8)] {
        fonts.draw(c, text, x + dx, y + dy, px, true, rgba(0, 0, 0, 150));
    }
    fonts.draw(c, text, x, y, px, true, Color::WHITE);
}

thread_local! {
    /// Sombras dos ícones do Dock já feitas: (ícone, letra, tamanho em pixels) → sombra.
    static ICON_SHADOWS: std::cell::RefCell<Vec<((String, u32), std::rc::Rc<Canvas>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Bytes das sombras de ícone guardadas (para medir a memória).
pub fn shadow_bytes() -> (usize, usize) {
    ICON_SHADOWS.with(|m| (m.borrow().len(), m.borrow().iter().map(|(_, c)| c.data().len()).sum()))
}

/// Sombra com a forma do ícone: o contorno dele, desfocado e escuro (como os
/// ícones do Dock do Tiger). Devolve a sombra e a margem em volta do ícone.
fn icon_shadow(fonts: &Fonts, icon: icons::Icon, letter: Option<char>, size: f32, scale: f32) -> Option<(std::rc::Rc<Canvas>, f32)> {
    // Tamanhos de 2 em 2 pixels: a ampliação não refaz a sombra a cada quadro.
    let px = ((size * scale / 2.0).round() * 2.0) as u32;
    // O tema entra na chave: a sombra tem a forma do desenho, e trocando de tema o
    // desenho é outro (com a chave sem o tema, ficava a sombra do ícone antigo).
    let key = (format!("{}:{icon:?}{letter:?}", icons::theme_name()), px);
    if let Some(c) = ICON_SHADOWS.with(|m| m.borrow().iter().find(|(k, _)| *k == key).map(|(_, c)| c.clone())) {
        return Some((c, (size * 0.08).max(3.0)));
    }
    let pad = (size * 0.08).max(3.0);
    let side = px as f32 / scale;
    let mut c = Canvas::new(side + 2.0 * pad, side + 2.0 * pad, scale)?;
    icons::draw_bare(&mut c, fonts, icon, pad, pad, side, letter);
    let (w, h) = (c.pm.width() as usize, c.pm.height() as usize);
    // Só o alfa, desfocado com duas passadas de caixa em cada direção.
    let mut a: Vec<f32> = c.pm.data().chunks_exact(4).map(|p| p[3] as f32).collect();
    let r = ((size * 0.035 * scale).round() as usize).max(1);
    let mut tmp = vec![0f32; a.len()];
    for _ in 0..2 {
        for y in 0..h {
            for x in 0..w {
                let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
                tmp[y * w + x] = a[y * w + lo..=y * w + hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
            }
        }
        for x in 0..w {
            for y in 0..h {
                let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
                a[y * w + x] = (lo..=hi).map(|yy| tmp[yy * w + x]).sum::<f32>() / (hi - lo + 1) as f32;
            }
        }
    }
    for (p, v) in c.pm.data_mut().chunks_exact_mut(4).zip(&a) {
        let al = (v * 0.45) as u8;
        p.copy_from_slice(&[0, 0, 0, al]);
    }
    let c = std::rc::Rc::new(c);
    ICON_SHADOWS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 96 {
            m.remove(0);
        }
        m.push((key, c.clone()));
    });
    Some((c, pad))
}

/// Onde cada peça do Dock vai, em coordenadas lógicas relativas ao Dock. O
/// compositor desenha cada peça como uma textura própria (a GPU só muda posição
/// e tamanho na ampliação); [`dock`] junta as mesmas peças num canvas só.
#[derive(Clone, Debug, PartialEq)]
pub struct DockScene {
    /// Retângulo da textura da prateleira (com as bordas finas em volta).
    pub shelf: (f32, f32, f32, f32),
    /// (item, x, y, lado) do quadrado do desenho de cada ícone (sem a margem da sombra).
    pub icons: Vec<(usize, f32, f32, f32)>,
    /// Canto de cada divisória.
    pub separators: Vec<(f32, f32)>,
    /// Canto de cada indicador de app aberto.
    pub indicators: Vec<(f32, f32)>,
    /// O nome sob o mouse: (item, canto da textura do nome).
    pub label: Option<(usize, f32, f32)>,
}

/// Tamanho do nome do ícone sob o mouse e a margem da textura dele.
const DOCK_LABEL_PX: f32 = 14.0;
const DOCK_LABEL_PAD: f32 = 4.0;

/// Calcula a cena do Dock (só contas, nada desenhado).
pub fn dock_scene(fonts: &Fonts, items: &[DockItem], layout: &DockLayout, hover: Option<usize>) -> DockScene {
    let (sx, sy, sw, sh) = layout.shelf;
    let side = layout.side;
    // A prateleira e as bordas dela, sem passar das bordas do Dock.
    let (x0, y0) = ((sx - 2.0).max(0.0), (sy - 2.0).max(0.0));
    let (x1, y1) = ((sx + sw + 2.0).min(layout.width as f32), (sy + sh + 2.0).min(layout.height as f32));
    let modern = crate::theme::modern();
    let mut sc = DockScene { shelf: (x0, y0, x1 - x0, y1 - y0), icons: Vec::new(), separators: Vec::new(), indicators: Vec::new(), label: None };
    for (i, (it, &(x, y, s))) in items.iter().zip(&layout.items).enumerate() {
        if it.separator_before {
            sc.separators.push(if side.vertical() { (sx, y - DOCK_GAP / 2.0 - DOCK_SEP / 2.0) } else { (x - DOCK_GAP / 2.0 - DOCK_SEP / 2.0, sy) });
        }
        // App abrindo: o ícone pula para longe da borda (até o fim do Dock, no máximo).
        let (x, y) = match side {
            DockSide::Bottom => (x, y - it.bounce.clamp(0.0, (y - 2.0).max(0.0))),
            DockSide::Left => (x + it.bounce.clamp(0.0, (layout.width as f32 - x - s - 2.0).max(0.0)), y),
            DockSide::Right => (x - it.bounce.clamp(0.0, (x - 2.0).max(0.0)), y),
        };
        if !it.thumbnail {
            // O desenho dos ícones tem margem dentro do quadro; no Dock ele cresce
            // para encher a vaga (como os ícones do Tiger, quase encostados).
            // Cresce para o lado da borda da tela (a margem de baixo do desenho fica
            // sobre o espaço do indicador), para o ícone caber na prateleira.
            let k = 1.14;
            let (dx, dy) = match side {
                DockSide::Bottom => (s * (k - 1.0) / 2.0, s * (k - 1.0) * 0.1),
                DockSide::Left => (s * (k - 1.0) * 0.7, s * (k - 1.0) / 2.0),
                DockSide::Right => (s * (k - 1.0) * 0.3, s * (k - 1.0) / 2.0),
            };
            sc.icons.push((i, x - dx, y - dy, s * k));
        }
        if it.running {
            // Indicador de app aberto: pequeno triângulo escuro junto à borda da tela.
            sc.indicators.push(match side {
                // Moderno: o pontinho no meio da faixa de baixo, longe do ícone e da borda.
                DockSide::Bottom if modern => (x + s / 2.0 - 5.0, layout.height as f32 - 11.0),
                DockSide::Left if modern => (6.0, y + s / 2.0 - 5.0),
                DockSide::Right if modern => (layout.width as f32 - 13.0, y + s / 2.0 - 5.0),
                DockSide::Bottom => (x + s / 2.0 - 5.0, layout.height as f32 - 7.0),
                DockSide::Left => (0.0, y + s / 2.0 - 5.0),
                DockSide::Right => (layout.width as f32 - 7.0, y + s / 2.0 - 5.0),
            });
        }
        if hover == Some(i) {
            let tw = fonts.measure(&it.label, DOCK_LABEL_PX, true);
            let (lx, ly) = match side {
                DockSide::Bottom => ((x + s / 2.0 - tw / 2.0).clamp(10.0, layout.width as f32 - tw - 10.0), (y - 8.0).max(16.0)),
                // Nas laterais, o nome fica ao lado do ícone.
                DockSide::Left => (x + s + 10.0, y + s / 2.0 + 5.0),
                DockSide::Right => ((x - 10.0 - tw).max(4.0), y + s / 2.0 + 5.0),
            };
            sc.label = Some((i, lx - DOCK_LABEL_PAD, ly - DOCK_LABEL_PX - DOCK_LABEL_PAD));
        }
    }
    sc
}

/// A prateleira de vidro do Dock, do tamanho de `DockScene::shelf`.
pub fn dock_shelf(theme: &Theme, layout: &DockLayout, scene: &DockScene, scale: f32) -> Option<Canvas> {
    let (ox, oy, w, h) = scene.shelf;
    let mut c = Canvas::new(w.max(1.0), h.max(1.0), scale)?;
    let (sx, sy, sw, sh) = layout.shelf;
    let (sx, sy) = (sx - ox, sy - oy);
    let side = layout.side;
    // Cantos arredondados só do lado de dentro (longe da borda da tela).
    let shelf = match side {
        DockSide::Bottom => rrect4(sx, sy, sw, sh + 2.0, [2.0, 2.0, 0.0, 0.0]),
        DockSide::Left => rrect4(sx - 2.0, sy, sw + 2.0, sh, [0.0, 2.0, 2.0, 0.0]),
        DockSide::Right => rrect4(sx, sy, sw + 2.0, sh, [2.0, 0.0, 0.0, 2.0]),
    };
    if theme.modern {
        // Moderno: vidro fosco com os cantos todos arredondados e um fio de luz em volta.
        let inset = 2.0;
        let glass = match side {
            DockSide::Bottom => rrect(sx, sy, sw, sh - inset, 14.0),
            DockSide::Left => rrect(sx + inset, sy, sw - inset, sh, 14.0),
            DockSide::Right => rrect(sx, sy, sw - inset, sh, 14.0),
        };
        c.fill(glass.clone(), &solid(if theme.dark { rgba(44, 44, 48, 170) } else { rgba(246, 246, 248, 150) }));
        c.stroke(glass, &solid(if theme.dark { rgba(255, 255, 255, 44) } else { rgba(255, 255, 255, 150) }), 1.0 / scale);
        return Some(c);
    }
    if theme.variant == Variant::Claro {
        // 10.4: vidro fosco cinza-lilás, quase opaco (o tom não depende do papel
        // de parede), liso e um pouco mais claro em cima.
        c.fill(shelf.clone(), &vgrad(sy, sy + sh, &[(0.0, rgba(170, 178, 216, 222)), (1.0, rgba(146, 156, 202, 228))]));
    } else {
        // Vidro branco translúcido com listras finas, encostado na borda de baixo.
        c.fill(shelf.clone(), &vgrad(sy, sy + sh, &[(0.0, rgba(255, 255, 255, 120)), (0.5, rgba(240, 244, 250, 100)), (1.0, rgba(225, 232, 242, 112))]));
        c.pinstripes(sx + 1.0, sy + 1.0, sw - 2.0, sh - 1.0, rgba(255, 255, 255, 0), rgba(255, 255, 255, 34));
    }
    // Borda clara fina e suave (no 10.4 é só um fio de luz em volta do vidro).
    let claro = theme.variant == Variant::Claro;
    c.stroke(shelf, &solid(rgba(255, 255, 255, if claro { 80 } else { 170 })), 1.0 / scale);
    match side {
        DockSide::Bottom => {
            c.hline(sx, sy - 1.0, sw, rgba(0, 0, 0, if claro { 35 } else { 50 }));
            if !claro {
                c.hline(sx + 2.0, sy + 1.0, sw - 4.0, rgba(255, 255, 255, 210));
            }
        }
        DockSide::Left => c.vline(sx + sw, sy, sh, rgba(0, 0, 0, 50)),
        DockSide::Right => c.vline(sx - 1.0, sy, sh, rgba(0, 0, 0, 50)),
    }
    Some(c)
}

/// Divisória entre os apps e o resto (comprimento = espessura da prateleira).
pub fn dock_separator(side: DockSide, len: f32, scale: f32) -> Option<Canvas> {
    let mut c = if side.vertical() { Canvas::new(len.max(1.0), 2.0, scale)? } else { Canvas::new(2.0, len.max(1.0), scale)? };
    if side.vertical() {
        c.hline(0.0, 0.0, len, rgba(0, 0, 0, 70));
        c.hline(0.0, 1.0, len, rgba(255, 255, 255, 190));
    } else {
        c.vline(0.0, 0.0, len, rgba(0, 0, 0, 70));
        c.vline(1.0, 0.0, len, rgba(255, 255, 255, 190));
    }
    Some(c)
}

/// Indicador de app aberto (10 × 7 ou 7 × 10, apontando para longe da borda).
pub fn dock_indicator(side: DockSide, scale: f32) -> Option<Canvas> {
    let mut c = if side.vertical() { Canvas::new(7.0, 10.0, scale)? } else { Canvas::new(10.0, 7.0, scale)? };
    if crate::theme::modern() {
        // Moderno: um pontinho.
        let col = if crate::theme::dark() { rgba(230, 230, 236, 230) } else { rgba(30, 30, 34, 200) };
        let (cx, cy) = match side {
            DockSide::Bottom => (5.0, 4.0),
            DockSide::Left => (3.0, 5.0),
            DockSide::Right => (4.0, 5.0),
        };
        c.fill(circle(cx, cy, 1.9), &solid(col));
        return Some(c);
    }
    let mut pb = tiny_skia::PathBuilder::new();
    match side {
        DockSide::Bottom => {
            pb.move_to(1.0, 5.5);
            pb.line_to(9.0, 5.5);
            pb.line_to(5.0, 1.0);
        }
        DockSide::Left => {
            pb.move_to(1.5, 1.0);
            pb.line_to(1.5, 9.0);
            pb.line_to(6.0, 5.0);
        }
        DockSide::Right => {
            pb.move_to(5.5, 1.0);
            pb.line_to(5.5, 9.0);
            pb.line_to(1.0, 5.0);
        }
    }
    pb.close();
    c.fill(pb.finish(), &solid(rgba(15, 20, 30, 220)));
    Some(c)
}

/// O nome do ícone sob o mouse, branco com contorno escuro.
pub fn dock_label(fonts: &Fonts, text: &str, scale: f32) -> Option<Canvas> {
    let tw = fonts.measure(text, DOCK_LABEL_PX, true);
    let mut c = Canvas::new(tw + DOCK_LABEL_PAD * 2.0 + 1.0, DOCK_LABEL_PX + DOCK_LABEL_PAD * 2.0 + 2.0, scale)?;
    outlined_label(fonts, &mut c, text, DOCK_LABEL_PAD, DOCK_LABEL_PAD + DOCK_LABEL_PX, DOCK_LABEL_PX);
    Some(c)
}

/// O ícone com a sombra no formato dele, para um quadrado de lado `side`.
/// Devolve o canvas e a margem (o desenho começa em (margem, margem)).
pub fn dock_icon(fonts: &Fonts, icon: Icon, letter: Option<char>, side: f32, scale: f32) -> Option<(Canvas, f32)> {
    let pad = (side * 0.08).max(3.0);
    let mut c = Canvas::new(side + 2.0 * pad, side + 2.0 * pad + (side * 0.035).ceil(), scale)?;
    if let Some((sh, spad)) = icon_shadow(fonts, icon, letter, side, scale) {
        c.draw_canvas(&sh, pad - spad, pad - spad + side * 0.035);
    }
    icons::draw_bare(&mut c, fonts, icon, pad, pad, side, letter);
    Some((c, pad))
}

/// O Dock inteiro num canvas (as mesmas peças que o compositor junta na GPU).
pub fn dock(fonts: &Fonts, theme: &Theme, items: &[DockItem], layout: &DockLayout, hover: Option<usize>, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(layout.width as f32, layout.height as f32, scale)?;
    let scene = dock_scene(fonts, items, layout, hover);
    if let Some(sh) = dock_shelf(theme, layout, &scene, scale) {
        c.draw_canvas(&sh, scene.shelf.0, scene.shelf.1);
    }
    let thick = if layout.side.vertical() { layout.shelf.2 } else { layout.shelf.3 };
    if let Some(sep) = dock_separator(layout.side, thick, scale) {
        for &(x, y) in &scene.separators {
            c.draw_canvas(&sep, x, y);
        }
    }
    for &(i, x, y, s) in &scene.icons {
        let it = &items[i];
        if let Some((sh, pad)) = icon_shadow(fonts, it.icon, it.letter, s, scale) {
            c.draw_canvas(&sh, x - pad, y - pad + s * 0.035);
        }
        // Os ícones novos vêm prontos do cache do glove-paint (a ampliação não redesenha).
        icons::draw_bare(&mut c, fonts, it.icon, x, y, s, it.letter);
    }
    if let Some(ind) = dock_indicator(layout.side, scale) {
        for &(x, y) in &scene.indicators {
            c.draw_canvas(&ind, x, y);
        }
    }
    if let Some((i, x, y)) = scene.label {
        if let Some(l) = dock_label(fonts, &items[i].label, scale) {
            c.draw_canvas(&l, x, y);
        }
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Ícones da mesa
// ---------------------------------------------------------------------------

/// Medidas dos ícones da mesa (Opções de Visualização da mesa).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeskStyle {
    /// Lado do ícone.
    pub icon: f32,
    /// Tamanho do nome.
    pub text: f32,
    /// Nome à direita do ícone.
    pub right: bool,
}

/// Largura do nome à direita do ícone.
const DESK_SIDE_TEXT: f32 = 130.0;

impl DeskStyle {
    /// Tamanho da célula de um ícone (a grade da mesa usa a mesma).
    pub fn cell(&self) -> (f32, f32) {
        let line = self.text + 3.0;
        if self.right {
            (self.icon + 10.0 + DESK_SIDE_TEXT, self.icon.max(2.0 * line) + 8.0)
        } else {
            ((self.icon + 40.0).max(90.0), self.icon + 10.0 + 2.0 * line + 4.0)
        }
    }

    fn text_width(&self) -> f32 {
        if self.right { DESK_SIDE_TEXT - 6.0 } else { self.cell().0 - 8.0 }
    }

    /// Onde fica o desenho do ícone dentro da célula: (x, y).
    pub fn icon_at(&self) -> (f32, f32) {
        let (w, h) = self.cell();
        if self.right { (4.0, ((h - self.icon) / 2.0).floor()) } else { ((w - self.icon) / 2.0, 4.0) }
    }
}

/// O texto em linhas que cabem em `max` (quebra nas palavras; palavra grande, nas letras).
pub fn wrap_lines(fonts: &Fonts, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut out = vec![String::new()];
    for word in text.split_inclusive(' ') {
        let cur = out.last_mut().unwrap();
        if fonts.measure(&format!("{cur}{word}"), px, bold) <= max || cur.is_empty() && fonts.measure(word, px, bold) <= max {
            cur.push_str(word);
            continue;
        }
        if !cur.is_empty() {
            out.push(String::new());
        }
        for ch in word.chars() {
            let cur = out.last_mut().unwrap();
            if !cur.is_empty() && fonts.measure(&format!("{cur}{ch}"), px, bold) > max {
                out.push(ch.to_string());
            } else {
                cur.push(ch);
            }
        }
    }
    out
}

/// O nome em até duas linhas (quebra nas palavras; a segunda termina em "…").
pub fn wrap_two_lines(fonts: &Fonts, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    if fonts.measure(text, px, bold) <= max {
        return vec![text.to_string()];
    }
    // Primeira linha: o máximo de palavras que cabem (ou letras, se a palavra não cabe).
    let words: Vec<&str> = text.split(' ').collect();
    let mut first = String::new();
    let mut used = 0;
    for (k, w) in words.iter().enumerate() {
        let cand = if first.is_empty() { w.to_string() } else { format!("{first} {w}") };
        if fonts.measure(&cand, px, bold) > max {
            break;
        }
        first = cand;
        used = k + 1;
    }
    let rest = if used == 0 {
        let mut cut = 0;
        for (bi, _) in text.char_indices().skip(1) {
            if fonts.measure(&text[..bi], px, bold) > max {
                break;
            }
            cut = bi;
        }
        first = text[..cut].to_string();
        text[cut..].to_string()
    } else {
        words[used..].join(" ")
    };
    vec![first, fonts.ellipsize(&rest, px, bold, max)]
}

/// Um ícone da mesa (do tamanho da célula de `style`): desenho, atalho, nome em
/// branco com sombra; escolhido = fundo escuro no ícone e nome em pílula de
/// destaque; `hot` = destino de um arrastar; `rename` = (texto, tudo escolhido).
#[allow(clippy::too_many_arguments)]
pub fn desk_icon(fonts: &Fonts, theme: &Theme, style: &DeskStyle, icon: Icon, link: bool, label: &str, tag: Option<Color>, selected: bool, hot: bool, rename: Option<(&str, bool)>, scale: f32) -> Option<Canvas> {
    let (w, h) = style.cell();
    let mut c = Canvas::new(w, h, scale)?;
    let s = style.icon;
    let (ix, iy) = style.icon_at();
    if selected || hot {
        c.fill(rrect(ix - 4.0, iy - 3.0, s + 8.0, s + 6.0, 6.0), &solid(rgba(0, 0, 0, if hot { 110 } else { 70 })));
    }
    icons::draw(&mut c, fonts, icon, ix, iy, s, None);
    if link {
        icons::alias_badge(&mut c, ix, iy, s);
    }
    let px = style.text;
    let line = px + 3.0;
    let max = style.text_width();
    let (tx0, ty0) = if style.right { (ix + s + 6.0, (h - 2.0 * line) / 2.0 + px) } else { (0.0, iy + s + 6.0 + px) };
    // Renomeando: campo branco com o texto em até duas linhas (as últimas, onde se
    // digita), escolhido por inteiro no começo.
    if let Some((text, all)) = rename {
        let mut lines = wrap_lines(fonts, text, px, false, max - 8.0);
        if lines.len() > 2 {
            lines.drain(..lines.len() - 2);
        }
        let widest = lines.iter().map(|l| fonts.measure(l, px, false)).fold(0.0f32, f32::max);
        let fw = (widest + 10.0).max(40.0).min(max + 4.0);
        let fh = lines.len() as f32 * line + 4.0;
        let fx = if style.right { tx0 - 3.0 } else { ((w - fw) / 2.0).max(1.0) };
        let fy = ty0 - px - 1.0;
        c.fill(rrect(fx + 1.0, fy + 1.0, fw, fh, 2.0), &solid(rgba(0, 0, 0, 80)));
        c.fill(rrect(fx, fy, fw, fh, 2.0), &solid(Color::WHITE));
        let x = fx + 5.0;
        for (k, l) in lines.iter().enumerate() {
            let y = ty0 + 1.0 + k as f32 * line;
            let lw = fonts.measure(l, px, false);
            if all && !text.is_empty() {
                c.fill(rrect(x - 1.0, y - px, lw + 2.0, px + 3.0, 1.0), &solid(theme.accent));
            }
            fonts.draw(&mut c, l, x, y, px, false, rgba(10, 10, 10, 255));
            if !all && k == lines.len() - 1 {
                c.fill_rect((x + lw + 1.0).round(), y - px, 1.0, px + 3.0, &solid(rgba(10, 10, 10, 255)));
            }
        }
        return Some(c);
    }
    for (k, t) in wrap_two_lines(fonts, label, px, true, max).iter().enumerate() {
        let tw = fonts.measure(t, px, true);
        let x = if style.right { tx0 } else { ((w - tw) / 2.0).floor() };
        let y = ty0 + k as f32 * line;
        let pill = rrect(x - 5.0, y - px, tw + 10.0, px + 4.0, (px + 4.0) / 2.0);
        if selected {
            c.fill(pill, &solid(theme.accent));
        } else if let Some(tc) = tag {
            // Rótulo de cor: o nome numa pílula da cor dele, em preto (como no Tiger).
            c.fill(pill, &solid(tc));
            fonts.draw(&mut c, t, x, y, px, true, rgba(20, 20, 20, 255));
            continue;
        } else {
            fonts.draw(&mut c, t, x + 1.0, y + 1.0, px, true, rgba(0, 0, 0, 190));
        }
        fonts.draw(&mut c, t, x, y, px, true, Color::WHITE);
    }
    Some(c)
}

/// Retângulo de escolher na mesa: cinza translúcido com borda clara.
pub fn rubber_band(w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w.max(1.0), h.max(1.0), scale)?;
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(180, 190, 210, 70)));
    c.stroke(rrect(0.5, 0.5, (w - 1.0).max(0.5), (h - 1.0).max(0.5), 0.0), &solid(rgba(235, 240, 250, 210)), 1.0);
    Some(c)
}

/// Ponta do cursor (ponto quente), em pontos lógicos.
pub const CURSOR_HOTSPOT: (f32, f32) = (4.0, 3.0);

/// Cursor de seta da era: preto, contorno branco e sombra suave. Desenhado aqui
/// (a sessão própria não tem o cursor do desktop por baixo).
pub fn cursor_arrow(scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(24.0, 28.0, scale)?;
    let (ox, oy) = CURSOR_HOTSPOT;
    let pts: [(f32, f32); 7] = [(0.0, 0.0), (0.0, 16.5), (4.0, 12.8), (6.8, 19.2), (9.6, 18.0), (6.9, 11.8), (12.2, 11.8)];
    let arrow = |dx: f32, dy: f32| {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(ox + dx + pts[0].0, oy + dy + pts[0].1);
        for p in &pts[1..] {
            pb.line_to(ox + dx + p.0, oy + dy + p.1);
        }
        pb.close();
        pb.finish()
    };
    // Sombra deslocada para baixo e para a direita.
    c.fill(arrow(1.0, 1.6), &solid(rgba(0, 0, 0, 60)));
    c.stroke(arrow(0.0, 0.0), &solid(rgba(255, 255, 255, 255)), 2.2);
    c.fill(arrow(0.0, 0.0), &solid(rgba(0, 0, 0, 255)));
    Some(c)
}

/// Ponta do dedo da mãozinha (ponto quente), em pontos lógicos.
pub const HAND_HOTSPOT: (f32, f32) = (9.5, 2.0);

/// Cursor de mãozinha (links e botões de páginas): uma luva branca apontando com o
/// indicador, contorno preto e o punho da luva cinza. Desenho próprio do Glove.
pub fn cursor_hand(scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(26.0, 30.0, scale)?;
    let shapes = |dx: f32, dy: f32| -> Vec<Option<tiny_skia::Path>> {
        vec![
            // Indicador em pé.
            rrect(7.5 + dx, 1.0 + dy, 4.2, 13.0, 2.1),
            // Os outros dedos dobrados (três nós) e a palma.
            rrect(11.2 + dx, 8.6 + dy, 3.6, 6.0, 1.8),
            rrect(14.4 + dx, 9.4 + dy, 3.6, 5.6, 1.8),
            rrect(17.6 + dx, 10.4 + dy, 3.4, 5.2, 1.7),
            rrect(6.2 + dx, 11.0 + dy, 14.8, 10.6, 4.0),
            // Polegar, de lado.
            rrect(2.6 + dx, 12.6 + dy, 6.6, 4.0, 2.0),
            // O punho da luva.
            rrect(7.0 + dx, 20.6 + dy, 12.0, 5.2, 1.2),
        ]
    };
    for p in shapes(0.8, 1.4) {
        c.fill(p, &solid(rgba(0, 0, 0, 55)));
    }
    for p in shapes(0.0, 0.0) {
        c.stroke(p, &solid(rgba(0, 0, 0, 255)), 1.7);
    }
    let all = shapes(0.0, 0.0);
    for (i, p) in all.into_iter().enumerate() {
        let col = if i == 6 { rgba(205, 208, 214, 255) } else { Color::WHITE };
        c.fill(p, &solid(col));
    }
    // Vincos entre os dedos e a costura do punho.
    let mut pb = tiny_skia::PathBuilder::new();
    for x in [14.6f32, 17.8] {
        pb.move_to(x, 10.6);
        pb.line_to(x, 14.0);
    }
    pb.move_to(7.6, 22.6);
    pb.line_to(18.4, 22.6);
    c.stroke(pb.finish(), &solid(rgba(90, 90, 96, 255)), 0.8);
    Some(c)
}

/// Meio do cursor de texto (ponto quente).
pub const IBEAM_HOTSPOT: (f32, f32) = (6.0, 11.0);

/// Cursor de texto (a barra em I), preto com contorno branco.
pub fn cursor_ibeam(scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(12.0, 22.0, scale)?;
    let beam = || {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(2.5, 2.0);
        pb.quad_to(6.0, 2.0, 6.0, 4.5);
        pb.quad_to(6.0, 2.0, 9.5, 2.0);
        pb.move_to(6.0, 4.5);
        pb.line_to(6.0, 17.5);
        pb.move_to(2.5, 20.0);
        pb.quad_to(6.0, 20.0, 6.0, 17.5);
        pb.quad_to(6.0, 20.0, 9.5, 20.0);
        pb.move_to(3.8, 11.0);
        pb.line_to(8.2, 11.0);
        pb.finish()
    };
    c.stroke_round(beam(), &solid(Color::WHITE), 3.4);
    c.stroke_round(beam(), &solid(Color::BLACK), 1.4);
    Some(c)
}

/// Lado do cursor de redimensionar (o ponto quente é o centro).
pub const RESIZE_CURSOR_SIZE: f32 = 26.0;

/// Cursor de redimensionar: seta de duas pontas, preta com contorno branco e
/// sombra, girada `angle` graus (0 = ↔, 90 = ↕, 45 = ⤡, −45 = ⤢).
pub fn cursor_resize(angle: f32, scale: f32) -> Option<Canvas> {
    let side = RESIZE_CURSOR_SIZE;
    let mut c = Canvas::new(side, side, scale)?;
    let (sin, cos) = angle.to_radians().sin_cos();
    // Seta deitada, centrada na origem.
    let pts: [(f32, f32); 10] = [(-9.5, 0.0), (-5.0, -4.6), (-5.0, -1.6), (5.0, -1.6), (5.0, -4.6), (9.5, 0.0), (5.0, 4.6), (5.0, 1.6), (-5.0, 1.6), (-5.0, 4.6)];
    let arrow = |dx: f32, dy: f32| {
        let mut pb = tiny_skia::PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            let (px, py) = (side / 2.0 + dx + x * cos - y * sin, side / 2.0 + dy + x * sin + y * cos);
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        pb.close();
        pb.finish()
    };
    c.fill(arrow(1.0, 1.6), &solid(rgba(0, 0, 0, 60)));
    c.stroke(arrow(0.0, 0.0), &solid(rgba(255, 255, 255, 255)), 2.2);
    c.fill(arrow(0.0, 0.0), &solid(rgba(0, 0, 0, 255)));
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atalhos_em_duas_partes() {
        assert_eq!(shortcut_parts("Ctrl+Shift+S"), ("⌃⇧".into(), "S".into()));
        assert_eq!(shortcut_parts("Ctrl+o"), ("⌃".into(), "O".into()));
        assert_eq!(shortcut_parts("Super+Return"), ("❖".into(), "↩".into()));
        assert_eq!(shortcut_parts("F5"), ("".into(), "F5".into()));
    }
}

// ---------------------------------------------------------------------------
// Tela de encerramento (Desligando…, Reiniciando…, Encerrando sessão…)
// ---------------------------------------------------------------------------

/// Tamanho do giratório da tela de encerramento.
pub const ENDING_SPINNER: f32 = 32.0;

/// Centro do giratório na tela de encerramento de altura `h`.
pub fn ending_spinner_center(w: f32, h: f32) -> (f32, f32) {
    (w / 2.0, h / 2.0 + 70.0)
}

/// Fundo cinza-claro de inicialização, com a logo e o texto (sem o giratório,
/// que é uma peça separada para não redesenhar a tela inteira a cada passo).
/// Com "a aparência em todo o sistema" no Moderno: fundo liso (claro ou grafite) e a
/// luva chapada (preta ou branca), como a abertura dos sistemas de hoje.
pub fn ending_screen(fonts: &Fonts, theme: &Theme, w: f32, h: f32, label: &str, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let modern = crate::theme::system_modern(theme);
    let (top, bot, ink) = match (modern, theme.dark) {
        (true, true) => (rgba(30, 30, 33, 255), rgba(22, 22, 25, 255), rgba(214, 214, 220, 255)),
        (true, false) => (rgba(242, 242, 244, 255), rgba(230, 230, 234, 255), rgba(60, 60, 66, 255)),
        _ => (rgba(214, 216, 219, 255), rgba(188, 191, 196, 255), rgba(60, 62, 66, 255)),
    };
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, top), (1.0, bot)]));
    let (cx, cy) = (w / 2.0, h / 2.0 - 30.0);
    if modern {
        icons::logo_for(&mut c, theme, cx - 52.0, cy - 52.0, 104.0);
    } else {
        icons::logo(&mut c, cx - 52.0, cy - 52.0, 104.0, theme.accent);
    }
    let px = 15.0;
    let tw = fonts.measure(label, px, false);
    let (_, sy) = ending_spinner_center(w, h);
    fonts.draw(&mut c, label, (w - tw) / 2.0, sy + ENDING_SPINNER / 2.0 + 30.0, px, false, ink);
    Some(c)
}

/// Giratório da tela de encerramento (`phase` de 0 a 1 = uma volta), sobre o fundo dela
/// (claro no fundo grafite do Moderno Escuro).
pub fn ending_spinner(theme: &Theme, phase: f32, scale: f32) -> Option<Canvas> {
    let s = ENDING_SPINNER;
    let mut c = Canvas::new(s, s, scale)?;
    let cor = if crate::theme::system_modern(theme) && theme.dark { rgba(210, 210, 216, 255) } else { rgba(70, 72, 76, 255) };
    spinner(&mut c, s / 2.0, s / 2.0, s / 2.0, phase, cor);
    Some(c)
}

// ---------------------------------------------------------------------------
// Forçar Encerrar (janela desenhada pelo compositor)
// ---------------------------------------------------------------------------

/// Um app na lista do Forçar Encerrar.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceQuitRow {
    pub icon: Icon,
    pub name: String,
    /// Não respondeu ao último ping.
    pub hung: bool,
}

/// O que está embaixo do mouse no painel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FqHit {
    Title(Option<TitleButton>),
    Row(usize),
    /// Botão da direita (Forçar Encerrar).
    Confirm,
    /// Botão "Cancelar" (só na pergunta de confirmação).
    Cancel,
    Body,
}

pub const FQ_W: f32 = 380.0;
/// Margem de sombra em volta do painel.
pub const FQ_SHADOW: f32 = 24.0;
const FQ_PAD: f32 = 18.0;
const FQ_ROW_H: f32 = 22.0;
const FQ_ROWS_MIN: usize = 6;
const FQ_ROWS_MAX: usize = 10;
const FQ_TEXT_PX: f32 = 11.0;
const FQ_BTN_H: f32 = 22.0;
const FQ_INFO: &str = "Se um aplicativo não responder por algum tempo, selecione o nome dele e clique em Forçar Encerrar.";

fn fq_wrap(fonts: &Fonts, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let try_line = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if !line.is_empty() && fonts.measure(&try_line, px, bold) > max {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = try_line;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Partes do painel, em coordenadas do conteúdo (0,0 = canto da barra de título).
struct FqLayout {
    h: f32,
    text_y: f32,
    list: (f32, f32, f32, f32),
    confirm: (f32, f32, f32, f32),
    cancel: Option<(f32, f32, f32, f32)>,
}

fn fq_layout(fonts: &Fonts, rows: usize, question: Option<&str>) -> FqLayout {
    let top = TITLE_H as f32;
    let text_lines = match question {
        Some(q) => fq_wrap(fonts, q, 13.0, true, FQ_W - 2.0 * FQ_PAD).len() + 1,
        None => fq_wrap(fonts, glove_i18n::tr(FQ_INFO), FQ_TEXT_PX, false, FQ_W - 2.0 * FQ_PAD).len(),
    };
    let text_y = top + 14.0;
    let text_h = text_lines as f32 * 16.0;
    let list_y = text_y + text_h + 8.0;
    let list_h = rows.clamp(FQ_ROWS_MIN, FQ_ROWS_MAX) as f32 * FQ_ROW_H + 2.0;
    let btn_y = list_y + list_h + 16.0;
    let label_w = |t: &str| (fonts.measure(t, 13.0, false) + 36.0).max(90.0).ceil();
    let cw = label_w(tr!("Forçar Encerrar"));
    let confirm = (FQ_W - FQ_PAD - cw, btn_y, cw, FQ_BTN_H);
    let cancel = question.map(|_| {
        let w = label_w(tr!("Cancelar"));
        (confirm.0 - 12.0 - w, btn_y, w, FQ_BTN_H)
    });
    FqLayout { h: btn_y + FQ_BTN_H + FQ_PAD, text_y, list: (FQ_PAD, list_y, FQ_W - 2.0 * FQ_PAD, list_h), confirm, cancel }
}

/// Tamanho do conteúdo do painel (sem a sombra).
pub fn force_quit_size(fonts: &Fonts, rows: usize, question: Option<&str>) -> (f32, f32) {
    (FQ_W, fq_layout(fonts, rows, question).h.ceil())
}

fn inside(r: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

/// O que há em (x, y), relativo ao conteúdo do painel.
pub fn force_quit_hit(fonts: &Fonts, rows: usize, question: Option<&str>, x: f32, y: f32) -> FqHit {
    if y < TITLE_H as f32 {
        return FqHit::Title(title_button_at(x, y));
    }
    let l = fq_layout(fonts, rows, question);
    if inside(l.confirm, x, y) {
        return FqHit::Confirm;
    }
    if l.cancel.is_some_and(|c| inside(c, x, y)) {
        return FqHit::Cancel;
    }
    if question.is_none() && inside(l.list, x, y) {
        let i = ((y - l.list.1 - 1.0) / FQ_ROW_H).floor();
        if i >= 0.0 && (i as usize) < rows.min(FQ_ROWS_MAX) {
            return FqHit::Row(i as usize);
        }
    }
    FqHit::Body
}

/// Painel completo, com margem de sombra (o conteúdo começa em FQ_SHADOW, FQ_SHADOW).
/// `question`: pergunta de confirmação no lugar do texto ("Deseja forçar…?").
#[allow(clippy::too_many_arguments)]
pub fn force_quit_panel(fonts: &Fonts, theme: &Theme, rows: &[ForceQuitRow], selected: Option<usize>, question: Option<&str>, pressed: Option<FqHit>, title_hover: bool, scale: f32) -> Option<Canvas> {
    let l = fq_layout(fonts, rows.len(), question);
    let (w, h) = (FQ_W, l.h.ceil());
    let m = FQ_SHADOW;
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    let _style = PanelStyle::begin(&mut c);
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &shadow_spec(true), scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    c.fill_rect(x0, y0, w, h, &solid(theme.body_b));
    c.pinstripes(x0, y0, w, h, theme.body_a, theme.body_b);
    let st = TitleBarState { active: true, hover: title_hover, pressed: match pressed { Some(FqHit::Title(b)) => b, _ => None }, zoom_enabled: false, edited: false, pill: false };
    if let Some(t) = titlebar(fonts, theme, w, tr!("Forçar Encerrar Aplicativos"), &st, None, scale) {
        c.draw_canvas(&t, x0, y0);
    }
    let ink = rgba(20, 20, 20, 255);

    // Texto (ou a pergunta)
    let mut ty = y0 + l.text_y + 11.0;
    match question {
        Some(q) => {
            for line in fq_wrap(fonts, q, 13.0, true, w - 2.0 * FQ_PAD) {
                fonts.draw(&mut c, &line, x0 + FQ_PAD, ty, 13.0, true, ink);
                ty += 16.0;
            }
            fonts.draw(&mut c, tr!("Alterações não salvas serão perdidas."), x0 + FQ_PAD, ty, FQ_TEXT_PX, false, rgba(40, 40, 40, 255));
        }
        None => {
            for line in fq_wrap(fonts, glove_i18n::tr(FQ_INFO), FQ_TEXT_PX, false, w - 2.0 * FQ_PAD) {
                fonts.draw(&mut c, &line, x0 + FQ_PAD, ty, FQ_TEXT_PX, false, ink);
                ty += 16.0;
            }
        }
    }

    // Lista
    let (lx, ly, lw, lh) = l.list;
    let (lx, ly) = (x0 + lx, y0 + ly);
    c.fill_rect(lx, ly, lw, lh, &solid(Color::WHITE));
    c.stroke(rrect(lx + 0.5, ly + 0.5, lw - 1.0, lh - 1.0, 0.0), &solid(rgba(140, 140, 140, 255)), 1.0);
    for (i, r) in rows.iter().take(FQ_ROWS_MAX).enumerate() {
        let ry = ly + 1.0 + i as f32 * FQ_ROW_H;
        let sel = selected == Some(i);
        if sel {
            let col = if question.is_some() { rgba(200, 200, 200, 255) } else { theme.menu_hot };
            c.fill_rect(lx + 1.0, ry, lw - 2.0, FQ_ROW_H, &solid(col));
        }
        icons::draw_bare(&mut c, fonts, r.icon, lx + 6.0, ry + 3.0, 16.0, None);
        let text_c = if sel && question.is_none() { Color::WHITE } else { ink };
        fonts.draw(&mut c, &r.name, lx + 28.0, ry + 15.5, 13.0, false, text_c);
        if r.hung {
            let nw = fonts.measure(&r.name, 13.0, false);
            let red = if sel && question.is_none() { rgba(255, 210, 210, 255) } else { rgba(200, 30, 30, 255) };
            fonts.draw(&mut c, tr!("(não responde)"), lx + 28.0 + nw + 6.0, ry + 15.5, 13.0, false, red);
        }
    }

    // Botões
    let button = |c: &mut Canvas, r: (f32, f32, f32, f32), label: &str, default: bool, down: bool| {
        let base = if default { theme.accent } else { rgba(238, 238, 240, 255) };
        let base = if down { darken(base, 0.2) } else { base };
        gel_rect(c, x0 + r.0, y0 + r.1, r.2, r.3, r.3 / 2.0, base);
        let tw = fonts.measure(label, 13.0, false);
        fonts.draw(c, label, x0 + r.0 + (r.2 - tw) / 2.0, y0 + r.1 + 15.5, 13.0, false, ink);
    };
    let can_quit = selected.is_some_and(|s| s < rows.len());
    if let Some(cr) = l.cancel {
        button(&mut c, cr, tr!("Cancelar"), false, pressed == Some(FqHit::Cancel));
    }
    if can_quit {
        button(&mut c, l.confirm, tr!("Forçar Encerrar"), true, pressed == Some(FqHit::Confirm));
    } else {
        let r = l.confirm;
        gel_rect(&mut c, x0 + r.0, y0 + r.1, r.2, r.3, r.3 / 2.0, rgba(238, 238, 240, 160));
        let tw = fonts.measure(tr!("Forçar Encerrar"), 13.0, false);
        fonts.draw(&mut c, tr!("Forçar Encerrar"), x0 + r.0 + (r.2 - tw) / 2.0, y0 + r.1 + 15.5, 13.0, false, rgba(150, 150, 150, 255));
    }
    Some(c)
}

/// Cursor de espera (app sem responder): disco claro com o giratório. `step` de 0 a 11.
pub const WAIT_CURSOR_SIZE: f32 = 24.0;
pub fn cursor_wait(step: i32, scale: f32) -> Option<Canvas> {
    let s = WAIT_CURSOR_SIZE;
    let mut c = Canvas::new(s, s, scale)?;
    let r = s / 2.0 - 1.5;
    c.fill(circle(s / 2.0, s / 2.0, r), &solid(rgba(250, 250, 250, 240)));
    c.stroke(circle(s / 2.0, s / 2.0, r), &solid(rgba(0, 0, 0, 150)), 1.0);
    spinner(&mut c, s / 2.0, s / 2.0, r - 2.0, step as f32 / 12.0, rgba(30, 60, 140, 255));
    Some(c)
}

// ---------------------------------------------------------------------------
// Extras da barra de menu (volume, bateria, monitor), controle de volume e
// aviso na tela (OSD) das teclas de volume e brilho
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    /// Volume de 0 a 1.
    Volume { level: f32, muted: bool },
    Battery { percent: u8, charging: bool, show_percent: bool },
    Display,
    /// Wi-Fi: ligado e quantas ondas acesas (0 = sem rede).
    Wifi { on: bool, bars: u8 },
    /// Menu de Entrada: a sigla do layout ("BR").
    Input { badge: String },
    /// Relógio: o texto, ou (hora, minuto) no mostrador de ponteiros.
    Clock { text: String, analog: Option<(u8, u8)> },
    /// Troca rápida de usuário (o último à direita): o nome de quem está usando.
    User { name: String },
    /// Central de avisos: quantos não lidos e "não perturbe".
    Notifications { unread: u32, quiet: bool },
    /// Gravação de tela em andamento: o tempo e a bolinha (acesa ou apagada, piscando).
    Recording { tempo: String, aceso: bool },
    /// Central de controle (Wi-Fi, Bluetooth, brilho, luz do teclado, som).
    Control,
    /// Ícone de um app na bandeja (StatusNotifierItem: Telegram, Discord, Steam…); sem
    /// imagem, a inicial do nome numa plaquinha.
    Tray { icon: Option<TrayIcon>, title: String },
}

/// A imagem de um ícone da bandeja: RGBA pré-multiplicado, `w`×`h` pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct TrayIcon {
    pub w: u32,
    pub h: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
}

/// Lado do ícone da bandeja na barra (pontos).
pub const TRAY_ICON: f32 = 16.0;

/// O ícone da bandeja em (x, y), com `TRAY_ICON` de lado (a imagem do app, reduzida
/// mantendo a proporção; sem ela, a inicial do nome).
fn tray_glyph(c: &mut Canvas, fonts: &Fonts, icon: Option<&TrayIcon>, title: &str, x: f32, y: f32, col: Color) {
    let s = TRAY_ICON;
    if let Some(img) = icon.and_then(|i| tiny_skia::PixmapRef::from_bytes(&i.rgba, i.w, i.h)) {
        let k = c.scale * s / img.width().max(img.height()).max(1) as f32;
        let (dw, dh) = (img.width() as f32 * k, img.height() as f32 * k);
        let (px, py) = (x * c.scale + (s * c.scale - dw) / 2.0, y * c.scale + (s * c.scale - dh) / 2.0);
        let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
        c.draw_pixmap_t(img, &paint, tiny_skia::Transform::from_row(k, 0.0, 0.0, k, px.round(), py.round()));
        return;
    }
    let letra: String = title.chars().find(|ch| ch.is_alphanumeric()).map(|ch| ch.to_uppercase().collect()).unwrap_or_else(|| "?".into());
    c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, 4.0), &solid(col), 1.2);
    let tw = fonts.measure(&letra, 10.0, true);
    fonts.draw(c, &letra, x + (s - tw) / 2.0, y + s / 2.0 + 3.5, 10.0, true, col);
}

/// Largura do extra na barra.
fn extra_width(fonts: &Fonts, e: &Extra) -> f32 {
    match e {
        Extra::Volume { .. } => 30.0,
        Extra::Battery { percent, show_percent: true, .. } => 36.0 + fonts.measure(&format!("{percent}%"), MENUBAR_PX - 1.0, false),
        Extra::Battery { .. } => 36.0,
        Extra::Display => 30.0,
        Extra::Wifi { .. } => 30.0,
        Extra::Input { badge } => fonts.measure(badge, 10.0, true) + 22.0,
        Extra::Clock { analog: Some(_), .. } => 28.0,
        Extra::Clock { text, .. } => fonts.measure(text, MENUBAR_PX, false) + 16.0,
        Extra::User { name } => fonts.measure(name, MENUBAR_PX, true) + 16.0,
        Extra::Notifications { unread: 0, .. } => 28.0,
        Extra::Notifications { unread, .. } => 30.0 + fonts.measure(&(*unread).min(99).to_string(), 9.0, true),
        Extra::Recording { tempo, .. } => 22.0 + fonts.measure(tempo, MENUBAR_PX, false),
        Extra::Control => 30.0,
        Extra::Tray { .. } => TRAY_ICON + 10.0,
    }
}

/// Dois botões de ligar (o ícone da central de controle), como nos sistemas de hoje: o de
/// cima só no contorno, com a bolinha cheia à esquerda (desligado); o de baixo cheio, com
/// a bolinha à direita RECORTADA (ligado). O recorte é vazado de verdade (regra par-ímpar),
/// não pintado: mostra o fundo (a barra clara, a escura ou o destaque com o menu aberto);
/// pintado de branco, o ícone virava um bloco no tema escuro.
fn control_glyph(c: &mut Canvas, x: f32, y: f32, color: Color) {
    c.stroke(rrect(x + 0.6, y + 0.6, 15.0, 6.4, 3.2), &solid(color), 1.2);
    c.fill(circle(x + 3.8, y + 3.8, 2.0), &solid(color));
    let yy = y + 8.6;
    let mut pb = tiny_skia::PathBuilder::new();
    if let Some(p) = rrect(x, yy - 0.4, 16.2, 7.6, 3.8) {
        pb.push_path(&p);
    }
    pb.push_circle(x + 12.2, yy + 3.4, 2.3);
    c.fill_rule(pb.finish(), &solid(color), tiny_skia::FillRule::EvenOdd);
}

/// Central de controle: o que ela mostra (None = o computador não tem).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ControlInfo {
    /// Wi-Fi: (ligado, nome da rede em uso).
    pub wifi: Option<(bool, String)>,
    pub bluetooth: Option<bool>,
    pub quiet: bool,
    /// Níveis de 0 a 1.
    pub brightness: Option<f32>,
    pub keyboard: Option<f32>,
    pub volume: Option<(f32, bool)>,
}

/// As partes da central de controle que respondem ao clique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlPart {
    Wifi,
    Bluetooth,
    Quiet,
    Brightness,
    Keyboard,
    Volume,
}

pub const CONTROL_W: f32 = 300.0;
const CC_PAD: f32 = 12.0;
const CC_TILE_H: f32 = 70.0;
const CC_SLIDER_H: f32 = 46.0;

/// Onde fica cada parte (x, y, largura, altura, relativos ao conteúdo) e a altura total.
pub fn control_layout(info: &ControlInfo) -> (Vec<(ControlPart, [f32; 4])>, f32) {
    let mut v = Vec::new();
    let tw = (CONTROL_W - 2.0 * CC_PAD - 2.0 * 8.0) / 3.0;
    for (i, p) in [ControlPart::Wifi, ControlPart::Bluetooth, ControlPart::Quiet].into_iter().enumerate() {
        v.push((p, [CC_PAD + i as f32 * (tw + 8.0), CC_PAD, tw, CC_TILE_H]));
    }
    let mut y = CC_PAD + CC_TILE_H + 10.0;
    for (p, has) in [(ControlPart::Brightness, info.brightness.is_some()), (ControlPart::Keyboard, info.keyboard.is_some()), (ControlPart::Volume, info.volume.is_some())] {
        if has {
            v.push((p, [CC_PAD, y, CONTROL_W - 2.0 * CC_PAD, CC_SLIDER_H]));
            y += CC_SLIDER_H + 4.0;
        }
    }
    (v, y + CC_PAD - 4.0)
}

/// Nível de um trilho pela posição x (relativa ao conteúdo).
pub fn control_level(part_rect: [f32; 4], x: f32) -> f32 {
    let (x0, x1) = (part_rect[0] + 28.0, part_rect[0] + part_rect[2] - 10.0);
    ((x - x0) / (x1 - x0)).clamp(0.0, 1.0)
}

/// Lua crescente (não perturbe): o arco de fora pela esquerda e o de dentro de volta.
fn moon_glyph(c: &mut Canvas, cx: f32, cy: f32, r: f32, col: Color) {
    use std::f32::consts::PI;
    let n = 20;
    let mut pb = tiny_skia::PathBuilder::new();
    for i in 0..=n {
        let a = 0.3 * PI + 1.4 * PI * i as f32 / n as f32;
        let (x, y) = (cx + r * a.cos(), cy - r * a.sin());
        if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
    }
    let (ix, ir) = (cx + r * 0.42, r * 0.8);
    for i in 0..=n {
        let a = 1.62 * PI - 1.24 * PI * i as f32 / n as f32;
        pb.line_to(ix + ir * a.cos(), cy - ir * a.sin());
    }
    pb.close();
    c.fill(pb.finish(), &solid(col));
}

/// Tecladinho com a luz (o símbolo pequeno do trilho da luz do teclado).
fn kbd_small_glyph(c: &mut Canvas, cx: f32, cy: f32, col: Color) {
    c.stroke(rrect(cx - 8.0, cy - 2.0, 16.0, 8.0, 1.5), &solid(col), 1.2);
    for k in 0..4 {
        c.fill_rect(cx - 6.0 + k as f32 * 3.4, cy, 2.0, 1.5, &solid(col));
    }
    c.fill_rect(cx - 4.0, cy + 2.8, 8.0, 1.4, &solid(col));
    let mut pb = tiny_skia::PathBuilder::new();
    for dx in [-5.0, 0.0, 5.0] {
        pb.move_to(cx + dx, cy - 4.5);
        pb.line_to(cx + dx * 1.3, cy - 7.5);
    }
    c.stroke_round(pb.finish(), &solid(col), 1.2);
}

/// A runa do Bluetooth.
fn bluetooth_glyph(c: &mut Canvas, cx: f32, cy: f32, h: f32, col: Color) {
    let (t, b, w) = (cy - h / 2.0, cy + h / 2.0, h * 0.28);
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(cx - w, cy - h * 0.25);
    pb.line_to(cx + w, cy + h * 0.25);
    pb.line_to(cx, b);
    pb.line_to(cx, t);
    pb.line_to(cx + w, cy - h * 0.25);
    pb.line_to(cx - w, cy + h * 0.25);
    c.stroke_round(pb.finish(), &solid(col), 2.0);
}

/// A central de controle que desce do ícone: três botões redondos e os trilhos.
pub fn control_panel(theme: &Theme, fonts: &Fonts, info: &ControlInfo, scale: f32) -> Option<Canvas> {
    let (parts, h) = control_layout(info);
    let w = CONTROL_W;
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;
    let _style = PanelStyle::begin(&mut c);
    let s = ShadowSpec { blur: 10.0, offset_y: 4.0, radius: 10.0, alpha: 0.45 };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    c.fill(rrect(x0, 0.0, w, h, 10.0), &solid(rgba(246, 247, 249, 250)));
    c.stroke(rrect(x0 + 0.5, 0.5, w - 1.0, h - 1.0, 10.0), &solid(rgba(0, 0, 0, 70)), 1.0);
    let ink = rgba(25, 25, 28, 255);
    let soft = rgba(100, 100, 106, 255);
    for (p, r) in &parts {
        let (x, y, pw, ph) = (x0 + r[0], r[1], r[2], r[3]);
        match p {
            ControlPart::Wifi | ControlPart::Bluetooth | ControlPart::Quiet => {
                let (on, avail, name, sub) = match p {
                    ControlPart::Wifi => {
                        let (on, net) = info.wifi.clone().unwrap_or((false, String::new()));
                        let sub = if info.wifi.is_none() { "—".to_string() } else if !on { glove_i18n::tr!("Desligado").into() } else if net.is_empty() { glove_i18n::tr!("Ligado").into() } else { net };
                        (on, info.wifi.is_some(), "Wi-Fi", sub)
                    }
                    ControlPart::Bluetooth => {
                        let on = info.bluetooth.unwrap_or(false);
                        (on, info.bluetooth.is_some(), "Bluetooth", if info.bluetooth.is_none() { "—".into() } else if on { glove_i18n::tr!("Ligado").into() } else { glove_i18n::tr!("Desligado").into() })
                    }
                    _ => (info.quiet, true, glove_i18n::tr!("Não perturbe"), if info.quiet { glove_i18n::tr!("Ligado").into() } else { glove_i18n::tr!("Desligado").into() }),
                };
                c.fill(rrect(x, y, pw, ph, 8.0), &solid(rgba(255, 255, 255, 230)));
                c.stroke(rrect(x + 0.5, y + 0.5, pw - 1.0, ph - 1.0, 8.0), &solid(rgba(0, 0, 0, 40)), 1.0);
                let (bx, by) = (x + pw / 2.0, y + 20.0);
                let base = if !avail { rgba(215, 215, 218, 255) } else if on { theme.accent } else { rgba(200, 202, 206, 255) };
                gel_circle(&mut c, bx, by, 13.0, base);
                let gcol = if on && avail { Color::WHITE } else { rgba(70, 70, 76, 255) };
                match p {
                    ControlPart::Wifi => wifi_glyph(&mut c, bx, by + 6.0, true, 3, gcol),
                    ControlPart::Bluetooth => bluetooth_glyph(&mut c, bx, by, 13.0, gcol),
                    _ => moon_glyph(&mut c, bx - 1.0, by, 6.5, gcol),
                }
                let t = fonts.ellipsize(name, 10.5, true, pw - 6.0);
                let tw = fonts.measure(&t, 10.5, true);
                fonts.draw(&mut c, &t, x + (pw - tw) / 2.0, y + 49.0, 10.5, true, if avail { ink } else { soft });
                let st = fonts.ellipsize(&sub, 10.0, false, pw - 8.0);
                let sw = fonts.measure(&st, 10.0, false);
                fonts.draw(&mut c, &st, x + (pw - sw) / 2.0, y + 62.0, 10.0, false, soft);
            }
            ControlPart::Brightness | ControlPart::Keyboard | ControlPart::Volume => {
                let (label, level, muted) = match p {
                    ControlPart::Brightness => (glove_i18n::tr!("Tela"), info.brightness.unwrap_or(0.0), false),
                    ControlPart::Keyboard => (glove_i18n::tr!("Luz do teclado"), info.keyboard.unwrap_or(0.0), false),
                    _ => (glove_i18n::tr!("Som"), info.volume.map(|v| v.0).unwrap_or(0.0), info.volume.is_some_and(|v| v.1)),
                };
                fonts.draw(&mut c, label, x + 2.0, y + 13.0, 11.0, true, ink);
                // O símbolo à esquerda do trilho.
                let gy = y + 31.0;
                match p {
                    ControlPart::Brightness => sun_glyph(&mut c, x + 10.0, gy, 6.0, soft),
                    ControlPart::Keyboard => kbd_small_glyph(&mut c, x + 10.0, gy, soft),
                    _ => speaker_glyph(&mut c, x + 2.0, gy - 8.0, 16.0, if muted { 0 } else { 2 }, soft),
                }
                let r = [x - x0, y, pw, ph];
                let (tx0, tx1) = (x0 + r[0] + 28.0, x0 + r[0] + r[2] - 10.0);
                let th = 6.0;
                gel_rect(&mut c, tx0, gy - th / 2.0, tx1 - tx0, th, th / 2.0, rgba(200, 202, 206, 255));
                let kx = tx0 + level.clamp(0.0, 1.0) * (tx1 - tx0);
                if !muted {
                    gel_rect(&mut c, tx0, gy - th / 2.0, (kx - tx0).max(th), th, th / 2.0, theme.accent);
                }
                gel_circle(&mut c, kx, gy, 8.0, if muted { rgba(225, 225, 228, 255) } else { lighten(theme.accent, 0.1) });
            }
        }
    }
    Some(c)
}

/// Alto-falante com 0 a 3 ondas (sem som = sem ondas, como na era).
pub fn speaker_glyph(c: &mut Canvas, x: f32, y: f32, s: f32, waves: u8, color: Color) {
    // Desenhado numa caixa de s × s.
    let k = s / 16.0;
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x + 1.0 * k, y + 5.5 * k);
    pb.line_to(x + 4.0 * k, y + 5.5 * k);
    pb.line_to(x + 8.0 * k, y + 2.0 * k);
    pb.line_to(x + 8.0 * k, y + 14.0 * k);
    pb.line_to(x + 4.0 * k, y + 10.5 * k);
    pb.line_to(x + 1.0 * k, y + 10.5 * k);
    pb.close();
    c.fill(pb.finish(), &solid(color));
    for i in 0..waves.min(3) {
        let r = (3.0 + 2.6 * i as f32) * k;
        let (cx, cy) = (x + 8.5 * k, y + 8.0 * k);
        let mut pb = tiny_skia::PathBuilder::new();
        let a = 0.85f32;
        pb.move_to(cx + r * a.cos(), cy - r * a.sin());
        pb.quad_to(cx + r * 1.25, cy, cx + r * a.cos(), cy + r * a.sin());
        c.stroke_round(pb.finish(), &solid(color), 1.3 * k);
    }
}

/// Sol (brilho): disco com raios.
pub fn sun_glyph(c: &mut Canvas, cx: f32, cy: f32, r: f32, color: Color) {
    c.fill(circle(cx, cy, r * 0.45), &solid(color));
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let (s, co) = a.sin_cos();
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(cx + co * r * 0.65, cy + s * r * 0.65);
        pb.line_to(cx + co * r, cy + s * r);
        c.stroke_round(pb.finish(), &solid(color), r * 0.14);
    }
}

fn battery_glyph(c: &mut Canvas, x: f32, y: f32, percent: u8, charging: bool, color: Color) {
    let (w, h) = (22.0, 10.0);
    c.stroke(rrect(x + 0.5, y + 0.5, w, h, 2.0), &solid(color), 1.0);
    c.fill_rect(x + w + 0.5, y + 3.0, 2.0, 5.0, &solid(color));
    let fill = (w - 3.0) * (percent.min(100) as f32 / 100.0);
    let fc = if percent <= 10 && !charging { rgba(210, 40, 30, 255) } else { color };
    if fill > 0.0 {
        c.fill_rect(x + 2.0, y + 2.0, fill, h - 3.0, &solid(fc));
    }
    if charging {
        // Raio por cima, branco com contorno escuro.
        let (bx, by) = (x + w / 2.0, y + h / 2.0 + 0.5);
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(bx + 1.5, by - 6.5);
        pb.line_to(bx - 3.5, by + 0.8);
        pb.line_to(bx - 0.2, by + 0.8);
        pb.line_to(bx - 1.5, by + 6.5);
        pb.line_to(bx + 3.5, by - 0.8);
        pb.line_to(bx + 0.2, by - 0.8);
        pb.close();
        let p = pb.finish();
        c.stroke(p.clone(), &solid(rgba(40, 40, 40, 255)), 1.2);
        c.fill(p, &solid(Color::WHITE));
    }
}

/// Mostradorzinho de ponteiros (relógio analógico da barra).
fn clock_glyph(c: &mut Canvas, cx: f32, cy: f32, h: u8, m: u8, color: Color) {
    c.stroke(circle(cx, cy, 7.5), &solid(color), 1.2);
    for k in 0..12 {
        let a = k as f32 * std::f32::consts::TAU / 12.0;
        if k % 3 == 0 {
            c.fill(circle(cx + 5.8 * a.sin(), cy - 5.8 * a.cos(), 0.7), &solid(color));
        }
    }
    let hand = |c: &mut Canvas, a: f32, len: f32, w: f32| {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(cx, cy);
        pb.line_to(cx + len * a.sin(), cy - len * a.cos());
        c.stroke_round(pb.finish(), &solid(color), w);
    };
    let ma = m as f32 / 60.0 * std::f32::consts::TAU;
    let ha = ((h % 12) as f32 + m as f32 / 60.0) / 12.0 * std::f32::consts::TAU;
    hand(c, ha, 3.8, 1.6);
    hand(c, ma, 5.6, 1.1);
}

/// Leque do Wi-Fi: três arcos sobre um ponto; os apagados ficam claros.
/// Desligado: só o contorno.
fn wifi_glyph(c: &mut Canvas, cx: f32, base: f32, on: bool, bars: u8, color: Color) {
    let dim = with_alpha(color, color.alpha() * 0.28);
    for k in 0..3u8 {
        let r = 4.5 + k as f32 * 4.0;
        let mut pb = tiny_skia::PathBuilder::new();
        let a0 = std::f32::consts::PI * 1.25;
        let steps = 12;
        for i in 0..=steps {
            let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / steps as f32;
            let (x, y) = (cx + r * a.cos(), base + r * a.sin());
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        let lit = on && k < bars;
        c.stroke(pb.finish(), &solid(if lit { color } else { dim }), if on { 2.0 } else { 1.0 });
    }
    c.fill(circle(cx, base - 1.0, 1.6), &solid(if on && bars > 0 { color } else { dim }));
}

/// Balão de conversa (central de avisos); "não perturbe" leva um risco.
fn notif_glyph(c: &mut Canvas, x: f32, y: f32, quiet: bool, color: Color) {
    c.stroke(rrect(x + 0.6, y + 0.6, 14.0, 10.0, 4.0), &solid(color), 1.2);
    // A pontinha embaixo à esquerda.
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x + 3.5, y + 10.0);
    pb.line_to(x + 2.5, y + 14.5);
    pb.line_to(x + 7.5, y + 10.4);
    pb.close();
    c.fill(pb.finish(), &solid(color));
    for k in 0..3 {
        c.fill(circle(x + 4.4 + k as f32 * 3.2, y + 5.6, 0.95), &solid(color));
    }
    if quiet {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(x - 0.5, y + 13.5);
        pb.line_to(x + 15.5, y - 1.0);
        c.stroke(pb.finish(), &solid(color), 1.4);
    }
}

fn display_glyph(c: &mut Canvas, x: f32, y: f32, color: Color) {
    c.stroke(rrect(x + 0.5, y + 0.5, 16.0, 11.0, 1.5), &solid(color), 1.2);
    c.fill_rect(x + 2.5, y + 2.5, 12.0, 7.0, &solid(with_alpha(color, color.alpha() * 0.35)));
    c.fill_rect(x + 7.0, y + 12.0, 3.0, 2.0, &solid(color));
    c.fill_rect(x + 4.5, y + 14.0, 8.0, 1.3, &solid(color));
}

/// Desenha os extras da direita para a esquerda, terminando em `right`. Devolve as faixas.
fn draw_extras(c: &mut Canvas, fonts: &Fonts, theme: &Theme, extras: &[Extra], right: f32, open: Option<usize>, hot: &dyn Fn(&mut Canvas, f32, f32)) -> Vec<(f32, f32)> {
    let mut ranges = vec![(0.0, 0.0); extras.len()];
    let mut x = right;
    for (i, e) in extras.iter().enumerate().rev() {
        let w = extra_width(fonts, e);
        let (a, b) = (x - w, x);
        let is_open = open == Some(i);
        if is_open {
            hot(c, a, b);
        }
        let col = if is_open && !theme.modern { Color::WHITE } else { theme.menu_text };
        match e {
            Extra::Volume { level, muted } => {
                let waves = if *muted || *level <= 0.0 { 0 } else { (1.0 + level * 2.99) as u8 };
                speaker_glyph(c, a + 7.0, 3.0, 16.0, waves, col);
            }
            Extra::Battery { percent, charging, show_percent } => {
                let mut bx = a + 6.0;
                if *show_percent {
                    let t = format!("{percent}%");
                    fonts.draw(c, &t, bx, 16.0, MENUBAR_PX - 1.0, false, col);
                    bx += fonts.measure(&t, MENUBAR_PX - 1.0, false) + 5.0;
                }
                battery_glyph(c, bx, 6.0, *percent, *charging, col);
            }
            Extra::Display => display_glyph(c, a + 7.0, 3.0, col),
            Extra::Wifi { on, bars } => wifi_glyph(c, a + 15.0, 16.0, *on, *bars, col),
            Extra::Input { badge } => {
                // Sigla numa plaquinha (como a bandeirinha do 10.4, sem bandeira).
                let tw = fonts.measure(badge, 10.0, true);
                c.fill(rrect(a + 7.0, 4.0, tw + 8.0, 14.0, 3.0), &solid(col));
                // A sigla contrasta com a plaquinha (no escuro ela é clara: sigla escura).
                let light = 0.3 * col.red() + 0.59 * col.green() + 0.11 * col.blue() > 0.6;
                let ink = if is_open { theme.accent } else if light { rgba(40, 40, 44, 255) } else { Color::WHITE };
                fonts.draw(c, badge, a + 11.0, 15.0, 10.0, true, ink);
            }
            Extra::Clock { analog: Some((h, m)), .. } => clock_glyph(c, a + 14.0, 11.0, *h, *m, col),
            Extra::Clock { text, .. } => fonts.draw(c, text, a + 8.0, 16.0, MENUBAR_PX, false, col),
            Extra::User { name } => fonts.draw(c, name, a + 8.0, 16.0, MENUBAR_PX, true, col),
            Extra::Control => control_glyph(c, a + 7.0, 3.0, col),
            Extra::Tray { icon, title } => tray_glyph(c, fonts, icon.as_ref(), title, a + 5.0, (MENUBAR_H as f32 - TRAY_ICON) / 2.0, col),
            Extra::Recording { tempo, aceso } => {
                // Bolinha vermelha (piscando) e o tempo decorrido ao lado.
                let (c1, c2) = if *aceso {
                    (rgba(255, 110, 100, 255), rgba(200, 30, 25, 255))
                } else {
                    (rgba(255, 110, 100, 90), rgba(200, 30, 25, 90))
                };
                c.fill(rrect(a + 7.0, 6.0, 9.0, 9.0, 4.5), &vgrad(6.0, 15.0, &[(0.0, c1), (1.0, c2)]));
                fonts.draw(c, tempo, a + 20.0, 16.0, MENUBAR_PX, false, col);
            }
            Extra::Notifications { unread, quiet } => {
                notif_glyph(c, a + 7.0, 4.0, *quiet, col);
                if *unread > 0 {
                    // A contagem numa pastilha vermelha ao lado.
                    let t = (*unread).min(99).to_string();
                    let tw = fonts.measure(&t, 9.0, true);
                    let (bx, bw) = (a + 21.0, tw + 7.0);
                    c.fill(rrect(bx, 5.0, bw, 12.0, 6.0), &vgrad(5.0, 17.0, &[(0.0, rgba(240, 90, 80, 255)), (1.0, rgba(200, 30, 25, 255))]));
                    fonts.draw(c, &t, bx + 3.5, 14.5, 9.0, true, Color::WHITE);
                }
            }
        }
        ranges[i] = (a, b);
        x = a;
    }
    ranges
}

/// Controle de volume que desce do ícone: trilho vertical em gel.
pub const VOLUME_SLIDER_W: f32 = 30.0;
pub const VOLUME_SLIDER_H: f32 = 132.0;
const VS_TOP: f32 = 14.0;
const VS_BOTTOM: f32 = 118.0;

/// Nível (0 a 1) no ponto `y` do conteúdo do controle.
pub fn volume_slider_level(y: f32) -> f32 {
    ((VS_BOTTOM - y) / (VS_BOTTOM - VS_TOP)).clamp(0.0, 1.0)
}

/// Controle completo, com margem de sombra igual à dos menus (conteúdo em MENU_SHADOW, 0).
pub fn volume_slider(theme: &Theme, level: f32, muted: bool, scale: f32) -> Option<Canvas> {
    let (w, h) = (VOLUME_SLIDER_W, VOLUME_SLIDER_H);
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;
    let _style = PanelStyle::begin(&mut c);
    let s = ShadowSpec { blur: 8.0, offset_y: 4.0, radius: 0.0, alpha: 0.45 };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    c.fill_rect(x0, 0.0, w, h, &solid(rgba(246, 247, 249, 248)));
    c.pinstripes(x0, 0.0, w, h, rgba(246, 247, 249, 0), rgba(232, 234, 236, 235));
    c.stroke(rrect(x0, 0.0, w, h, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    let cx = x0 + w / 2.0;
    // Trilho: cinza em cima, cor de destaque embaixo (até o polegar).
    let tw = 7.0;
    let ky = VS_BOTTOM - level.clamp(0.0, 1.0) * (VS_BOTTOM - VS_TOP);
    gel_rect(&mut c, cx - tw / 2.0, VS_TOP, tw, VS_BOTTOM - VS_TOP, tw / 2.0, rgba(200, 202, 206, 255));
    if !muted {
        gel_rect(&mut c, cx - tw / 2.0, ky, tw, VS_BOTTOM - ky, tw / 2.0, theme.accent);
    }
    gel_circle(&mut c, cx, ky, 7.5, if muted { rgba(225, 225, 228, 255) } else { lighten(theme.accent, 0.1) });
    Some(c)
}

/// Aviso na tela das teclas de volume e brilho: quadrado translúcido com o
/// símbolo e 16 quadradinhos de nível.
pub const OSD_SIZE: f32 = 170.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OsdKind {
    Volume,
    Brightness,
    /// Luz do teclado (Ctrl + teclas de brilho, ou as teclas próprias).
    Keyboard,
}

/// Teclado com a luz saindo dele (o aviso da luz do teclado).
fn keyboard_glyph(c: &mut Canvas, cx: f32, cy: f32, w: f32, col: Color) {
    let h = w * 0.42;
    let (x, y) = (cx - w / 2.0, cy - h / 2.0 + 10.0);
    c.stroke(rrect(x, y, w, h, 6.0), &solid(col), 4.0);
    // Três fileiras de teclas e a barra de espaço.
    let (kw, kh, gap) = (w * 0.085, h * 0.17, w * 0.028);
    for row in 0..3 {
        let n = 8 - row;
        let ry = y + 7.0 + row as f32 * (kh + gap);
        let rw = n as f32 * kw + (n - 1) as f32 * gap;
        for k in 0..n {
            c.fill_rect(cx - rw / 2.0 + k as f32 * (kw + gap), ry, kw, kh, &solid(col));
        }
    }
    let sw = w * 0.42;
    c.fill_rect(cx - sw / 2.0, y + 7.0 + 3.0 * (kh + gap), sw, kh, &solid(col));
    // A luz: raios acima do teclado.
    let mut pb = tiny_skia::PathBuilder::new();
    for i in -2..=2 {
        let a = i as f32 * 0.38;
        let (sx, sy) = (cx + a.sin() * w * 0.30, y - 8.0 - a.cos() * 2.0);
        pb.move_to(sx, sy);
        pb.line_to(sx + a.sin() * 12.0, sy - a.cos() * 12.0);
    }
    c.stroke_round(pb.finish(), &solid(col), 4.0);
}

pub fn osd(kind: OsdKind, level: f32, muted: bool, alpha: f32, scale: f32) -> Option<Canvas> {
    let s = OSD_SIZE;
    let mut c = Canvas::new(s, s, scale)?;
    let a = |v: u8| ((v as f32) * alpha.clamp(0.0, 1.0)) as u8;
    c.fill(rrect(0.0, 0.0, s, s, 22.0), &solid(rgba(30, 32, 36, a(120))));
    let white = rgba(255, 255, 255, a(235));
    match kind {
        OsdKind::Volume => {
            let waves = if muted || level <= 0.0 { 0 } else { 3 };
            speaker_glyph(&mut c, s / 2.0 - 46.0, 26.0, 88.0, waves, white);
            if muted {
                // Traço de "sem som" sobre as ondas ausentes.
                let mut pb = tiny_skia::PathBuilder::new();
                pb.move_to(s / 2.0 + 8.0, 52.0);
                pb.line_to(s / 2.0 + 38.0, 88.0);
                pb.move_to(s / 2.0 + 38.0, 52.0);
                pb.line_to(s / 2.0 + 8.0, 88.0);
                c.stroke_round(pb.finish(), &solid(white), 6.0);
            }
        }
        OsdKind::Brightness => sun_glyph(&mut c, s / 2.0, 70.0, 42.0, white),
        OsdKind::Keyboard => keyboard_glyph(&mut c, s / 2.0, 70.0, 108.0, white),
    }
    let n = 16;
    let (bx, by, bw, bh, gap) = (20.0, s - 34.0, (s - 40.0 - 15.0 * 2.0) / 16.0, 9.0, 2.0);
    let lit = if muted { 0 } else { (level.clamp(0.0, 1.0) * n as f32).round() as i32 };
    for i in 0..n {
        let x = bx + i as f32 * (bw + gap);
        let col = if i < lit { white } else { rgba(255, 255, 255, a(55)) };
        c.fill_rect(x, by, bw, bh, &solid(col));
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Tela bloqueada
// ---------------------------------------------------------------------------

pub const LOCK_W: f32 = 380.0;
pub const LOCK_H: f32 = 196.0;
/// Margem de sombra em volta do painel de desbloqueio.
pub const LOCK_SHADOW: f32 = 24.0;
const LOCK_FIELD: (f32, f32, f32, f32) = (112.0, 104.0, 248.0, 24.0);
const LOCK_BUTTON_W: f32 = 116.0;

/// Retângulo do botão Desbloquear (relativo ao conteúdo do painel).
pub fn lock_button_rect() -> (f32, f32, f32, f32) {
    (LOCK_W - 20.0 - LOCK_BUTTON_W, LOCK_H - 20.0 - 22.0, LOCK_BUTTON_W, 22.0)
}

/// Retrato genérico de usuário: cabeça e ombros em gel sobre um fundo claro.
pub fn user_picture(c: &mut Canvas, x: f32, y: f32, s: f32, accent: Color) {
    c.fill(rrect(x, y, s, s, s * 0.12), &vgrad(y, y + s, &[(0.0, rgba(236, 240, 246, 255)), (1.0, rgba(200, 210, 226, 255))]));
    c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, s * 0.12), &solid(rgba(0, 0, 0, 90)), 1.0);
    let body = darken(accent, 0.1);
    // Ombros: meia elipse cortada pela borda de baixo.
    let sh = s * 0.36;
    c.fill(rrect4(x + s * 0.16, y + s - sh - 1.0, s * 0.68, sh, [s * 0.3, s * 0.3, 0.0, 0.0]), &vgrad(y + s - sh, y + s, &[(0.0, lighten(body, 0.35)), (1.0, body)]));
    gel_circle(c, x + s / 2.0, y + s * 0.4, s * 0.2, body);
}

/// Retrato genérico no Moderno: um círculo cinza liso com a silhueta clara (cabeça e
/// ombros), como nas telas de login de hoje.
pub fn user_picture_modern(c: &mut Canvas, x: f32, y: f32, s: f32, dark: bool) {
    let (cx, cy, r) = (x + s / 2.0, y + s / 2.0, s / 2.0);
    let (fundo, figura) = if dark { (rgba(92, 92, 98, 255), rgba(200, 200, 206, 255)) } else { (rgba(168, 168, 176, 255), rgba(242, 242, 245, 255)) };
    c.fill(circle(cx, cy, r), &solid(fundo));
    c.fill(circle(cx, cy - r * 0.2, r * 0.36), &solid(figura));
    // Ombros: uma elipse larga cortada pelo círculo.
    let mut pb = tiny_skia::PathBuilder::new();
    let n = 48;
    for i in 0..=n {
        let a = std::f32::consts::PI + i as f32 / n as f32 * std::f32::consts::PI;
        let (px, py) = (cx + r * 0.66 * a.cos(), cy + r * 0.92 + r * 0.62 * a.sin());
        if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
    }
    pb.close();
    // A figura não sai do círculo.
    c.fill_clipped(pb.finish(), &solid(figura), circle(cx, cy, r - 0.5));
}

/// Painel de desbloqueio, com sombra. `dots`: quantos caracteres digitados;
/// `message`: aviso em vermelho ("Senha incorreta.") ou None; `busy`: verificando.
#[allow(clippy::too_many_arguments)]
pub fn lock_panel(fonts: &Fonts, theme: &Theme, name: &str, dots: usize, message: Option<&str>, busy: bool, caret: bool, pressed: bool, spin: f32, scale: f32) -> Option<Canvas> {
    let (w, h, m) = (LOCK_W, LOCK_H, LOCK_SHADOW);
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &ShadowSpec { blur: 16.0, offset_y: 6.0, radius: 8.0, alpha: 0.5 }, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    // Moderno (com a aparência em todo o sistema): painel liso de cantos largos.
    let modern = crate::theme::system_modern(theme);
    let shape = || rrect(x0, y0, w, h, if modern { 12.0 } else { 8.0 });
    c.fill(shape(), &solid(theme.body_b));
    if !modern {
        // Listras longe dos cantos arredondados.
        c.pinstripes(x0, y0 + 8.0, w, h - 16.0, theme.body_a, theme.body_b);
    }
    c.stroke(shape(), &solid(if modern && theme.dark { rgba(255, 255, 255, 40) } else { rgba(0, 0, 0, 90) }), 1.0 / scale);

    if modern {
        user_picture_modern(&mut c, x0 + 20.0, y0 + 22.0, 76.0, theme.dark);
    } else {
        user_picture(&mut c, x0 + 20.0, y0 + 22.0, 76.0, theme.accent);
    }
    let (ink, dim) = if modern { (theme.text, theme.text_dim) } else { (rgba(20, 20, 20, 255), rgba(50, 50, 50, 255)) };
    fonts.draw(&mut c, name, x0 + 112.0, y0 + 40.0, 15.0, true, ink);
    fonts.draw(&mut c, tr!("Esta tela está bloqueada."), x0 + 112.0, y0 + 62.0, 11.0, false, dim);
    fonts.draw(&mut c, tr!("Digite sua senha para desbloquear."), x0 + 112.0, y0 + 78.0, 11.0, false, dim);

    // Campo de senha
    let (fx, fy, fw, fh) = LOCK_FIELD;
    let (fx, fy) = (x0 + fx, y0 + fy);
    if modern {
        c.fill(rrect(fx, fy, fw, fh, 6.0), &solid(theme.control_bg));
        c.stroke(rrect(fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0, 6.0), &solid(theme.control_border), 1.0);
    } else {
        c.fill(rrect(fx, fy, fw, fh, 3.0), &solid(Color::WHITE));
        c.stroke(rrect(fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0, 3.0), &solid(rgba(120, 120, 120, 255)), 1.0);
        c.hline(fx + 1.0, fy + 1.5, fw - 2.0, rgba(0, 0, 0, 40));
    }
    let mut dx = fx + 10.0;
    for _ in 0..dots.min(28) {
        c.fill(circle(dx + 3.0, fy + fh / 2.0, 3.2), &solid(ink));
        dx += 8.5;
    }
    if caret && !busy {
        c.fill_rect(dx + 1.0, fy + 5.0, 1.0, fh - 10.0, &solid(ink));
    }
    if busy {
        spinner(&mut c, fx + fw - 13.0, fy + fh / 2.0, 7.5, spin, rgba(70, 70, 70, 255));
    }

    if let Some(msg) = message {
        fonts.draw(&mut c, msg, x0 + 112.0, y0 + 146.0, 11.0, true, rgba(200, 30, 30, 255));
    }

    let (bx, by, bw, bh) = lock_button_rect();
    let base = if busy { rgba(225, 225, 228, 255) } else { theme.accent };
    let label = tr!("Desbloquear");
    let tw = fonts.measure(label, 13.0, false);
    if modern {
        // Botão liso azul com o texto branco.
        c.fill(rrect(x0 + bx, y0 + by, bw, bh, 6.0), &solid(if pressed { darken(base, 0.2) } else { base }));
        fonts.draw(&mut c, label, x0 + bx + (bw - tw) / 2.0, y0 + by + 15.5, 13.0, false, if busy { rgba(140, 140, 140, 255) } else { Color::WHITE });
    } else {
        gel_rect(&mut c, x0 + bx, y0 + by, bw, bh, bh / 2.0, if pressed { darken(base, 0.2) } else { base });
        fonts.draw(&mut c, label, x0 + bx + (bw - tw) / 2.0, y0 + by + 15.5, 13.0, false, if busy { rgba(140, 140, 140, 255) } else { ink });
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Iniciando sessão
// ---------------------------------------------------------------------------

pub const STARTUP_W: f32 = 340.0;
pub const STARTUP_H: f32 = 190.0;
pub const STARTUP_SHADOW: f32 = 24.0;

/// Painel "Iniciando sessão": logo, nome, o passo atual e a barra de progresso.
pub fn startup_panel(fonts: &Fonts, theme: &Theme, status: &str, progress: f32, scale: f32) -> Option<Canvas> {
    let (w, h, m) = (STARTUP_W, STARTUP_H, STARTUP_SHADOW);
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &ShadowSpec { blur: 16.0, offset_y: 6.0, radius: 8.0, alpha: 0.45 }, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    let modern = crate::theme::system_modern(theme);
    let r = if modern { 12.0 } else { 8.0 };
    c.fill(rrect(x0, y0, w, h, r), &solid(theme.body_b));
    if modern {
        c.stroke(rrect(x0, y0, w, h, r), &solid(if theme.dark { rgba(255, 255, 255, 40) } else { rgba(0, 0, 0, 60) }), 1.0 / scale);
        icons::logo_for(&mut c, theme, x0 + w / 2.0 - 32.0, y0 + 14.0, 64.0);
    } else {
        c.pinstripes(x0, y0 + 8.0, w, h - 16.0, theme.body_a, theme.body_b);
        c.stroke(rrect(x0, y0, w, h, r), &solid(rgba(0, 0, 0, 90)), 1.0 / scale);
        icons::logo(&mut c, x0 + w / 2.0 - 32.0, y0 + 14.0, 64.0, theme.accent);
    }
    let center = |c: &mut Canvas, t: &str, y: f32, px: f32, bold: bool, col: Color| {
        let tw = fonts.measure(t, px, bold);
        fonts.draw(c, t, x0 + (w - tw) / 2.0, y, px, bold, col);
    };
    let (ink, dim) = if modern { (theme.text, theme.text_dim) } else { (rgba(20, 20, 20, 255), rgba(50, 50, 50, 255)) };
    center(&mut c, "Glove", y0 + 104.0, 22.0, true, ink);
    center(&mut c, status, y0 + 132.0, 11.0, false, dim);
    progress_bar(&mut c, x0 + 40.0, y0 + 148.0, w - 80.0, 12.0, Some(progress.clamp(0.0, 1.0)), 0.0, theme.accent);
    Some(c)
}

// ---------------------------------------------------------------------------
// Busca Rápida
// ---------------------------------------------------------------------------

/// Lupa: aro e cabo, numa caixa de s × s.
pub fn magnifier_glyph(c: &mut Canvas, x: f32, y: f32, s: f32, color: Color) {
    let r = s * 0.32;
    let (cx, cy) = (x + r + 1.0, y + r + 1.0);
    c.stroke(circle(cx, cy, r), &solid(color), s * 0.13);
    let mut pb = tiny_skia::PathBuilder::new();
    let k = std::f32::consts::FRAC_1_SQRT_2;
    pb.move_to(cx + r * k + 0.5, cy + r * k + 0.5);
    pb.line_to(x + s - 0.5, y + s - 0.5);
    c.stroke_round(pb.finish(), &solid(color), s * 0.18);
}

/// Uma linha de resultado.
#[derive(Clone, Debug, PartialEq)]
pub struct QsRow {
    /// Nome da seção (só na primeira linha de cada uma).
    pub section: Option<String>,
    pub icon: Icon,
    pub name: String,
}

pub const QS_W: f32 = 430.0;
const QS_HEADER: f32 = 38.0;
const QS_ROW_H: f32 = 21.0;
const QS_LABEL_W: f32 = 118.0;
const QS_PAD: f32 = 6.0;

/// Busca Rápida no Moderno: no centro da tela, campo grande e cantos arredondados.
const QSM_W: f32 = 640.0;
const QSM_HEADER: f32 = 52.0;
const QSM_ROW_H: f32 = 28.0;
const QSM_PAD: f32 = 8.0;
const QSM_SHADOW: f32 = 30.0;
const QSM_R: f32 = 12.0;

/// (cabeçalho, linha, margem) da aparência atual.
fn qs_metrics() -> (f32, f32, f32) {
    if crate::theme::modern() { (QSM_HEADER, QSM_ROW_H, QSM_PAD) } else { (QS_HEADER, QS_ROW_H, QS_PAD) }
}

/// Largura do conteúdo do painel.
pub fn qs_w() -> f32 {
    if crate::theme::modern() { QSM_W } else { QS_W }
}

/// Onde o conteúdo começa dentro da imagem do painel (a margem da sombra): (x, y).
pub fn quick_search_margin() -> (f32, f32) {
    if crate::theme::modern() { (QSM_SHADOW, QSM_SHADOW) } else { (MENU_SHADOW as f32, 0.0) }
}

/// Altura do conteúdo do painel.
pub fn quick_search_height(rows: usize, query_empty: bool, none: bool) -> f32 {
    let (head, row, pad) = qs_metrics();
    if query_empty {
        head
    } else if none {
        head + 2.0 * pad + row
    } else {
        head + 2.0 * pad + rows as f32 * row
    }
}

/// Linha sob o y do conteúdo.
pub fn quick_search_row_at(rows: usize, y: f32) -> Option<usize> {
    let (head, row, pad) = qs_metrics();
    let i = ((y - head - pad) / row).floor();
    (i >= 0.0 && (i as usize) < rows).then_some(i as usize)
}

/// A Busca Rápida do Moderno: um campo grande no centro da tela e os resultados embaixo,
/// numa peça arredondada com sombra macia (o escuro vem do `PanelStyle`).
#[allow(clippy::too_many_arguments)]
fn quick_search_modern(fonts: &Fonts, theme: &Theme, query: &str, caret: bool, rows: &[QsRow], selected: Option<usize>, searching: bool, scale: f32) -> Option<Canvas> {
    let none = rows.is_empty();
    let (w, h) = (QSM_W, quick_search_height(rows.len(), query.is_empty(), none).ceil());
    let m = QSM_SHADOW;
    let mut c = Canvas::new(w + 2.0 * m, h + 2.0 * m, scale)?;
    let _style = PanelStyle::begin(&mut c);
    let s = ShadowSpec { blur: 22.0, offset_y: 8.0, radius: QSM_R, alpha: 0.30 };
    if let Some(sh) = Canvas::shadow(-m, -m, w + 2.0 * m, h + 2.0 * m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let (x0, y0) = (m, m);
    c.fill(rrect(x0, y0, w, h, QSM_R), &solid(rgba(246, 246, 248, 250)));
    // O campo: a lupa e o texto grandes, sem moldura (o painel é o campo).
    let ink = rgba(25, 25, 28, 255);
    magnifier_glyph(&mut c, x0 + 18.0, y0 + 15.0, 22.0, rgba(120, 120, 126, 255));
    let tx = x0 + 54.0;
    let px = 22.0;
    let fw = w - 54.0 - 46.0;
    if query.is_empty() {
        fonts.draw(&mut c, tr!("Busca Rápida"), tx, y0 + 34.0, px, false, rgba(160, 160, 166, 255));
    }
    let mut shown = query.to_string();
    while fonts.measure(&shown, px, false) > fw && !shown.is_empty() {
        shown.remove(0);
    }
    fonts.draw(&mut c, &shown, tx, y0 + 34.0, px, false, ink);
    if caret {
        let cx = tx + fonts.measure(&shown, px, false) + 1.0;
        c.fill_rect(cx, y0 + 14.0, 1.5, 25.0, &solid(theme.accent));
    }
    if searching {
        spinner(&mut c, x0 + w - 26.0, y0 + QSM_HEADER / 2.0, 8.0, 0.0, rgba(120, 120, 126, 255));
    }
    if !query.is_empty() {
        c.hline(x0, y0 + QSM_HEADER - 0.5, w, rgba(218, 218, 222, 255));
        let label_w = rows.iter().filter_map(|r| r.section.as_ref()).map(|t| fonts.measure(t, 11.0, true) + 30.0).fold(QS_LABEL_W, f32::max);
        let top = y0 + QSM_HEADER + QSM_PAD;
        if none {
            fonts.draw(&mut c, tr!("Nenhum resultado"), x0 + label_w + 28.0, top + 19.0, 14.0, false, rgba(130, 130, 136, 255));
        }
        for (i, r) in rows.iter().enumerate() {
            let y = top + i as f32 * QSM_ROW_H;
            if let Some(sec) = &r.section {
                if i > 0 {
                    c.hline(x0 + 14.0, y - 0.5, w - 28.0, rgba(228, 228, 232, 255));
                }
                let sw = fonts.measure(sec, 11.0, true);
                fonts.draw(&mut c, sec, x0 + label_w - 16.0 - sw, y + 18.0, 11.0, true, rgba(130, 130, 136, 255));
            }
            let sel = selected == Some(i);
            if sel {
                c.fill(rrect(x0 + label_w - 8.0, y + 1.0, w - label_w + 8.0 - 10.0, QSM_ROW_H - 2.0, 6.0), &solid(theme.accent));
            }
            icons::draw_bare(&mut c, fonts, r.icon, x0 + label_w, y + 4.0, 20.0, None);
            let max = w - label_w - 44.0;
            let name = fonts.ellipsize(&r.name, 14.0, false, max);
            fonts.draw(&mut c, &name, x0 + label_w + 28.0, y + 19.0, 14.0, false, if sel { Color::WHITE } else { ink });
        }
    }
    Some(c)
}

/// Painel da Busca Rápida com margem de sombra de menu (conteúdo em MENU_SHADOW, 0).
#[allow(clippy::too_many_arguments)]
pub fn quick_search(fonts: &Fonts, theme: &Theme, query: &str, caret: bool, rows: &[QsRow], selected: Option<usize>, searching: bool, scale: f32) -> Option<Canvas> {
    if theme.modern {
        return quick_search_modern(fonts, theme, query, caret, rows, selected, searching, scale);
    }
    let none = rows.is_empty();
    let (w, h) = (QS_W, quick_search_height(rows.len(), query.is_empty(), none).ceil());
    let m = MENU_SHADOW as f32;
    let mut c = Canvas::new(w + 2.0 * m, h + m, scale)?;
    let _style = PanelStyle::begin(&mut c);
    let s = ShadowSpec { blur: 8.0, offset_y: 4.0, radius: 0.0, alpha: 0.45 };
    if let Some(sh) = Canvas::shadow(-m, 0.0, w + 2.0 * m, h + m, w, h, &s, scale) {
        c.draw_canvas(&sh, 0.0, 0.0);
    }
    let x0 = m;
    // Coluna das categorias: cabe o nome maior (os nomes mudam com o idioma).
    let label_w = rows.iter().filter_map(|r| r.section.as_ref()).map(|t| fonts.measure(t, 11.0, true) + 26.0).fold(QS_LABEL_W, f32::max);
    c.fill_rect(x0, 0.0, w, h, &solid(rgba(250, 250, 251, 250)));

    // Faixa de cima em azul, com o nome e o campo em cápsula.
    let a = theme.accent;
    c.fill_rect(x0, 0.0, w, QS_HEADER, &vgrad(0.0, QS_HEADER, &[(0.0, lighten(a, 0.35)), (1.0, lighten(a, 0.05))]));
    c.hline(x0, QS_HEADER - 1.0, w, darken(a, 0.25));
    fonts.draw(&mut c, tr!("Busca Rápida"), x0 + 12.0, 24.0, 13.0, true, Color::WHITE);
    let (fx, fy, fw, fh) = (x0 + label_w, 8.0, w - label_w - 10.0, 22.0);
    c.fill(rrect(fx, fy, fw, fh, fh / 2.0), &solid(Color::WHITE));
    c.stroke(rrect(fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0, fh / 2.0), &solid(darken(a, 0.35)), 1.0);
    magnifier_glyph(&mut c, fx + 7.0, fy + 5.0, 12.0, rgba(110, 110, 110, 255));
    let tx = fx + 25.0;
    let ink = rgba(20, 20, 20, 255);
    // Texto longo: mostra o fim (onde se digita).
    let mut shown = query.to_string();
    while fonts.measure(&shown, 13.0, false) > fw - 40.0 && !shown.is_empty() {
        shown.remove(0);
    }
    fonts.draw(&mut c, &shown, tx, fy + 15.5, 13.0, false, ink);
    if caret {
        let cx = tx + fonts.measure(&shown, 13.0, false) + 1.0;
        c.fill_rect(cx, fy + 4.0, 1.0, fh - 8.0, &solid(ink));
    }
    if searching {
        spinner(&mut c, fx + fw - 12.0, fy + fh / 2.0, 6.5, 0.0, rgba(110, 110, 110, 255));
    }

    if !query.is_empty() {
        let y0 = QS_HEADER + QS_PAD;
        if none {
            fonts.draw(&mut c, tr!("Nenhum resultado"), x0 + label_w + 26.0, y0 + 15.0, 13.0, false, rgba(120, 120, 120, 255));
        }
        // Coluna das seções em cinza claro.
        c.fill_rect(x0, QS_HEADER, label_w - 8.0, h - QS_HEADER, &solid(rgba(238, 240, 243, 255)));
        for (i, r) in rows.iter().enumerate() {
            let y = y0 + i as f32 * QS_ROW_H;
            if let Some(sec) = &r.section {
                if i > 0 {
                    c.hline(x0, y - 0.5, w, rgba(222, 224, 228, 255));
                }
                let sw = fonts.measure(sec, 11.0, true);
                fonts.draw(&mut c, sec, x0 + label_w - 16.0 - sw, y + 14.5, 11.0, true, rgba(110, 112, 118, 255));
            }
            let sel = selected == Some(i);
            if sel {
                c.fill_rect(x0 + label_w - 8.0, y, w - label_w + 8.0, QS_ROW_H, &solid(theme.menu_hot));
            }
            icons::draw_bare(&mut c, fonts, r.icon, x0 + label_w, y + 2.5, 16.0, None);
            let mut name = r.name.clone();
            let max = w - label_w - 34.0;
            if fonts.measure(&name, 13.0, false) > max {
                while fonts.measure(&format!("{name}…"), 13.0, false) > max && !name.is_empty() {
                    name.pop();
                }
                name.push('…');
            }
            fonts.draw(&mut c, &name, x0 + label_w + 22.0, y + 15.0, 13.0, false, if sel { Color::WHITE } else { ink });
        }
    }
    c.stroke(rrect(x0, 0.0, w, h, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    Some(c)
}

// ---------------------------------------------------------------------------
// Exposé
// ---------------------------------------------------------------------------

/// Título da janela sob o mouse no Exposé: cápsula escura translúcida, texto branco.
pub fn expose_label(fonts: &Fonts, text: &str, scale: f32) -> Option<Canvas> {
    let px = 13.0;
    let mut t = text.to_string();
    while fonts.measure(&t, px, true) > 420.0 && t.chars().count() > 1 {
        t.pop();
    }
    if t.len() < text.len() {
        t.push('…');
    }
    let (w, h) = ((fonts.measure(&t, px, true) + 28.0).ceil(), 26.0);
    let mut c = Canvas::new(w, h, scale)?;
    c.fill(rrect(0.0, 0.0, w, h, h / 2.0), &solid(rgba(20, 20, 24, 170)));
    fonts.draw(&mut c, &t, 14.0, 17.5, px, true, Color::WHITE);
    Some(c)
}

/// Faixa das mesas virtuais no alto do Exposé: a moldura de cada miniatura
/// (`rects`, já no espaço da faixa) e o nome embaixo. A atual tem a moldura azul;
/// `target` (sob o mouse, ou onde a janela vai cair) fica com a branca.
/// As miniaturas em si (o fundo e as janelas) são desenhadas por cima pelo compositor.
pub fn desk_strip(fonts: &Fonts, rects: &[(f32, f32, f32, f32)], cur: usize, target: Option<usize>, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    for (i, &(x, y, rw, rh)) in rects.iter().enumerate() {
        // Sombra suave e a moldura, um pouco para fora da miniatura.
        for (k, a) in [(5.0, 18), (3.0, 30), (1.5, 50)] {
            c.fill(rrect(x - k, y - k + 2.0, rw + 2.0 * k, rh + 2.0 * k, 4.0 + k), &solid(rgba(0, 0, 0, a)));
        }
        let (col, width) = if i == cur {
            (rgba(90, 160, 255, 255), 3.0)
        } else if target == Some(i) {
            (rgba(255, 255, 255, 240), 2.5)
        } else {
            (rgba(255, 255, 255, 110), 1.0)
        };
        let o = width / 2.0 + 0.5;
        c.stroke(rrect(x - o, y - o, rw + 2.0 * o, rh + 2.0 * o, 4.0), &solid(col), width);
        let label = tr!("Mesa {}", i + 1);
        let px = 11.5;
        let lw = fonts.measure(&label, px, true);
        let (lx, ly) = (x + (rw - lw) / 2.0, y + rh + 19.0);
        fonts.draw(&mut c, &label, lx, ly + 1.0, px, true, rgba(0, 0, 0, 160));
        fonts.draw(&mut c, &label, lx, ly, px, true, Color::WHITE);
    }
    Some(c)
}

// ---------------------------------------------------------------------------
// Troca de apps (Super+Tab, como no 10.4)
// ---------------------------------------------------------------------------

const SW_ICON: f32 = 96.0;
const SW_CELL: f32 = 124.0;
const SW_PAD: f32 = 18.0;
const SW_LABEL_H: f32 = 34.0;

/// Tamanho da faixa para `n` apps (encolhe os ícones se não couber em `max_w`).
pub fn app_switcher_size(n: usize, max_w: f32) -> (f32, f32, f32) {
    let k = ((max_w - 2.0 * SW_PAD) / (n.max(1) as f32 * SW_CELL)).min(1.0);
    let cell = SW_CELL * k;
    (n.max(1) as f32 * cell + 2.0 * SW_PAD, cell + SW_LABEL_H + SW_PAD, k)
}

/// Faixa escura translúcida com os ícones; o escolhido num quadrado claro e o
/// nome dele embaixo.
pub fn app_switcher(fonts: &Fonts, apps: &[(Icon, String)], selected: usize, max_w: f32, scale: f32) -> Option<Canvas> {
    let (w, h, k) = app_switcher_size(apps.len(), max_w);
    let mut c = Canvas::new(w, h, scale)?;
    c.fill(rrect(0.0, 0.0, w, h, 20.0 * k.max(0.5)), &solid(rgba(24, 26, 30, 165)));
    let cell = SW_CELL * k;
    let icon = SW_ICON * k;
    for (i, (ic, _)) in apps.iter().enumerate() {
        let x = SW_PAD + i as f32 * cell;
        let y = SW_PAD * 0.8;
        if i == selected {
            c.fill(rrect(x + 2.0, y, cell - 4.0, cell - 4.0, 12.0 * k.max(0.5)), &solid(rgba(255, 255, 255, 70)));
        }
        icons::draw(&mut c, fonts, *ic, x + (cell - icon) / 2.0, y + (cell - 4.0 - icon) / 2.0, icon, None);
    }
    if let Some((_, name)) = apps.get(selected) {
        let px = 15.0;
        let tw = fonts.measure(name, px, true);
        fonts.draw(&mut c, name, ((w - tw) / 2.0).max(8.0), h - SW_LABEL_H / 2.0 + 2.0, px, true, Color::WHITE);
    }
    Some(c)
}

const WS_CELL: (f32, f32) = (220.0, 150.0);
const WS_GAP: f32 = 10.0;

/// A faixa das janelas de um app (a troca entre as janelas do mesmo app): o tamanho
/// dela e, em cada casa, a área onde a miniatura da janela cabe (relativa à faixa).
/// As casas encolhem se não couberem em `max_w`. Os tamanhos das janelas dão a
/// proporção de cada miniatura.
pub fn window_switcher_layout(sizes: &[(f32, f32)], max_w: f32) -> ((f32, f32), Vec<[f32; 4]>) {
    let n = sizes.len().max(1) as f32;
    let k = ((max_w - 2.0 * SW_PAD - (n - 1.0) * WS_GAP) / (n * WS_CELL.0)).min(1.0).max(0.1);
    let (cw, ch) = (WS_CELL.0 * k, WS_CELL.1 * k);
    let w = n * cw + (n - 1.0) * WS_GAP + 2.0 * SW_PAD;
    let h = ch + SW_PAD * 1.6 + SW_LABEL_H;
    let slots = sizes
        .iter()
        .enumerate()
        .map(|(i, &(ww, wh))| {
            let (cx, cy) = (SW_PAD + i as f32 * (cw + WS_GAP), SW_PAD * 0.8);
            // A miniatura inteira dentro da casa, com uma folga, na proporção da janela.
            let (aw, ah) = (cw - 16.0, ch - 16.0);
            let s = (aw / ww.max(1.0)).min(ah / wh.max(1.0));
            let (tw, th) = (ww * s, wh * s);
            [cx + (cw - tw) / 2.0, cy + (ch - th) / 2.0, tw, th]
        })
        .collect();
    ((w, h), slots)
}

/// O fundo da faixa das janelas: escuro e translúcido como o da troca de apps, a casa
/// escolhida num retângulo claro e o título dela embaixo. As miniaturas são desenhadas
/// pelo compositor por cima, nas áreas de `window_switcher_layout`.
pub fn window_switcher(fonts: &Fonts, sizes: &[(f32, f32)], selected: usize, title: &str, max_w: f32, scale: f32) -> Option<Canvas> {
    let ((w, h), slots) = window_switcher_layout(sizes, max_w);
    let mut c = Canvas::new(w, h, scale)?;
    c.fill(rrect(0.0, 0.0, w, h, 20.0), &solid(rgba(24, 26, 30, 165)));
    let n = sizes.len().max(1) as f32;
    let cw = (w - 2.0 * SW_PAD - (n - 1.0) * WS_GAP) / n;
    let ch = h - SW_PAD * 1.6 - SW_LABEL_H;
    if selected < slots.len() {
        let x = SW_PAD + selected as f32 * (cw + WS_GAP);
        c.fill(rrect(x, SW_PAD * 0.8, cw, ch, 10.0), &solid(rgba(255, 255, 255, 70)));
    }
    let px = 15.0;
    let t = fonts.ellipsize(title, px, true, w - 2.0 * SW_PAD);
    let tw = fonts.measure(&t, px, true);
    fonts.draw(&mut c, &t, ((w - tw) / 2.0).max(8.0), h - SW_LABEL_H / 2.0 + 2.0, px, true, Color::WHITE);
    Some(c)
}

/// Menu com bico embaixo apontando para `pointer_x` (relativo ao conteúdo), para
/// os menus do Dock. O bico ocupa a margem de sombra de baixo.
pub fn menu_with_pointer(fonts: &Fonts, theme: &Theme, items: &[MenuItem], hover: Option<usize>, pointer_x: f32, scale: f32) -> Option<Canvas> {
    let mut c = menu(fonts, theme, items, hover, scale)?;
    let (w, h) = menu_content_size(fonts, items);
    let m = MENU_SHADOW as f32;
    let px = m + pointer_x.clamp(14.0, w - 14.0);
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(px - 9.0, h - 0.5);
    pb.line_to(px, h + 9.0);
    pb.line_to(px + 9.0, h - 0.5);
    pb.close();
    let bg = if theme.variant == Variant::Claro { rgba(237, 240, 241, 255) } else { rgba(244, 245, 247, 255) };
    c.fill(pb.finish(), &solid(bg));
    // Só as duas bordas do bico (a de cima se funde com o menu).
    let mut edge = tiny_skia::PathBuilder::new();
    edge.move_to(px - 9.0, h - 0.5);
    edge.line_to(px, h + 9.0);
    edge.line_to(px + 9.0, h - 0.5);
    c.stroke(edge.finish(), &solid(rgba(0, 0, 0, 70)), 1.0 / scale);
    Some(c)
}

// ---------------------------------------------------------------------------
// Avatares originais (usuários sem foto)
// ---------------------------------------------------------------------------

/// Número estável a partir de um texto (FNV-1a), para escolher o avatar.
pub fn avatar_seed(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// Avatar desenhado: azulejo colorido com um desenho (gota, folha, bola,
/// estrela, flor ou nuvem), escolhido por `seed`.
pub fn avatar(c: &mut Canvas, x: f32, y: f32, s: f32, seed: u64) {
    const TILES: [(u8, u8, u8); 6] = [(86, 150, 222), (98, 178, 112), (232, 150, 64), (164, 110, 196), (226, 96, 110), (70, 172, 176)];
    let (r, g, b) = TILES[(seed % 6) as usize];
    let base = rgba(r, g, b, 255);
    let tile = || rrect(x, y, s, s, s * 0.14);
    c.fill(tile(), &vgrad(y, y + s, &[(0.0, lighten(base, 0.35)), (1.0, darken(base, 0.1))]));
    c.stroke(rrect(x + 0.5, y + 0.5, s - 1.0, s - 1.0, s * 0.14), &solid(rgba(0, 0, 0, 80)), 1.0);
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    let white = rgba(255, 255, 255, 235);
    let k = s / 100.0;
    let mut pb = tiny_skia::PathBuilder::new();
    match (seed / 6) % 6 {
        0 => {
            // Gota em gel.
            pb.move_to(cx, cy - 34.0 * k);
            pb.cubic_to(cx + 8.0 * k, cy - 18.0 * k, cx + 26.0 * k, cy + 2.0 * k, cx + 24.0 * k, cy + 16.0 * k);
            pb.cubic_to(cx + 22.0 * k, cy + 34.0 * k, cx - 22.0 * k, cy + 34.0 * k, cx - 24.0 * k, cy + 16.0 * k);
            pb.cubic_to(cx - 26.0 * k, cy + 2.0 * k, cx - 8.0 * k, cy - 18.0 * k, cx, cy - 34.0 * k);
            pb.close();
            let p = pb.finish();
            c.fill(p.clone(), &vgrad(cy - 34.0 * k, cy + 30.0 * k, &[(0.0, rgba(190, 225, 255, 255)), (1.0, rgba(40, 110, 210, 255))]));
            c.stroke(p, &solid(rgba(20, 60, 140, 200)), 1.2 * k);
            c.fill(circle(cx - 9.0 * k, cy + 8.0 * k, 5.0 * k), &solid(rgba(255, 255, 255, 170)));
        }
        1 => {
            // Folha com nervura.
            pb.move_to(cx - 26.0 * k, cy + 26.0 * k);
            pb.cubic_to(cx - 30.0 * k, cy - 18.0 * k, cx + 6.0 * k, cy - 34.0 * k, cx + 30.0 * k, cy - 30.0 * k);
            pb.cubic_to(cx + 32.0 * k, cy - 2.0 * k, cx + 14.0 * k, cy + 30.0 * k, cx - 26.0 * k, cy + 26.0 * k);
            pb.close();
            c.fill(pb.finish(), &vgrad(cy - 30.0 * k, cy + 28.0 * k, &[(0.0, rgba(170, 230, 120, 255)), (1.0, rgba(50, 140, 60, 255))]));
            let mut v = tiny_skia::PathBuilder::new();
            v.move_to(cx - 26.0 * k, cy + 26.0 * k);
            v.quad_to(cx + 2.0 * k, cy - 2.0 * k, cx + 26.0 * k, cy - 26.0 * k);
            c.stroke_round(v.finish(), &solid(rgba(30, 90, 40, 220)), 2.2 * k);
        }
        2 => {
            // Bola de praia em gel.
            gel_circle(c, cx, cy, 28.0 * k, rgba(240, 240, 240, 255));
            for (i, col) in [(0, rgba(230, 60, 60, 230)), (2, rgba(50, 120, 220, 230)), (4, rgba(250, 200, 40, 230))] {
                let a0 = i as f32 * std::f32::consts::TAU / 6.0;
                let mut w = tiny_skia::PathBuilder::new();
                w.move_to(cx, cy);
                for j in 0..=8 {
                    let a = a0 + j as f32 / 8.0 * std::f32::consts::TAU / 6.0;
                    w.line_to(cx + a.cos() * 26.5 * k, cy + a.sin() * 26.5 * k);
                }
                w.close();
                c.fill(w.finish(), &solid(col));
            }
            c.fill(circle(cx - 9.0 * k, cy - 11.0 * k, 7.0 * k), &solid(rgba(255, 255, 255, 120)));
        }
        3 => {
            // Estrela de cinco pontas.
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rr = if i % 2 == 0 { 32.0 } else { 13.0 } * k;
                let (px, py) = (cx + a.cos() * rr, cy + 2.0 * k + a.sin() * rr);
                if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
            }
            pb.close();
            let p = pb.finish();
            c.fill(p.clone(), &vgrad(cy - 30.0 * k, cy + 30.0 * k, &[(0.0, rgba(255, 240, 150, 255)), (1.0, rgba(240, 170, 30, 255))]));
            c.stroke(p, &solid(rgba(150, 90, 10, 200)), 1.2 * k);
        }
        4 => {
            // Flor: cinco pétalas e o miolo.
            for i in 0..5 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                c.fill(circle(cx + a.cos() * 15.0 * k, cy + a.sin() * 15.0 * k, 12.0 * k), &solid(white));
            }
            gel_circle(c, cx, cy, 9.0 * k, rgba(250, 190, 40, 255));
        }
        _ => {
            // Nuvem.
            for (dx, dy, r) in [(-14.0, 6.0, 13.0), (0.0, -4.0, 17.0), (15.0, 5.0, 12.0)] {
                c.fill(circle(cx + dx * k, cy + dy * k, r * k), &solid(white));
            }
            c.fill(rrect(cx - 27.0 * k, cy + 4.0 * k, 54.0 * k, 15.0 * k, 7.5 * k), &solid(white));
        }
    }
}

#[cfg(test)]
mod control_tests {
    /// A faixa das janelas do app: as casas encolhem para caber e cada miniatura fica
    /// inteira dentro da casa dela, na proporção da janela.
    #[test]
    fn faixa_das_janelas() {
        let sizes = [(800.0, 600.0), (300.0, 900.0), (1600.0, 400.0)];
        let ((w, h), slots) = super::window_switcher_layout(&sizes, 2000.0);
        assert_eq!(slots.len(), 3);
        assert!(w <= 2000.0 && h > 150.0);
        for (s, (ww, wh)) in slots.iter().zip(sizes) {
            assert!(s[0] >= 0.0 && s[0] + s[2] <= w && s[1] + s[3] <= h, "{s:?}");
            assert!((s[2] / s[3] - ww / wh).abs() < 0.01, "proporção {s:?}");
            assert!(s[2] <= 204.5 && s[3] <= 134.5, "{s:?}");
        }
        // Muitas janelas numa tela estreita: a faixa cabe.
        let many = vec![(800.0, 600.0); 12];
        let ((w, _), slots) = super::window_switcher_layout(&many, 1200.0);
        assert!(w <= 1200.5, "{w}");
        assert!(slots.windows(2).all(|p| p[0][0] + p[0][2] <= p[1][0] + 0.5));
    }

    use super::*;

    /// O ícone da central de controle é VAZADO: as duas chaves só no contorno, com o miolo
    /// mostrando o fundo (a barra escura no tema escuro), nunca uma cor fixa. Antes partes
    /// eram pintadas de branco e o ícone virava um bloco branco no tema escuro.
    #[test]
    fn icone_da_central_e_vazado() {
        let fundo = rgba(30, 30, 34, 255);
        let mut c = Canvas::new(24.0, 20.0, 4.0).unwrap();
        c.fill_rect(0.0, 0.0, 24.0, 20.0, &solid(fundo));
        control_glyph(&mut c, 2.0, 2.0, rgba(235, 235, 240, 255));
        if let Ok(dir) = std::env::var("GLOVE_TEST_PNG") {
            c.pm.save_png(std::path::Path::new(&dir).join("icone-central-escuro.png")).unwrap();
        }
        let px = |x: f32, y: f32| {
            let p = c.pm.pixel((x * 4.0) as u32, (y * 4.0) as u32).unwrap();
            (p.red(), p.green(), p.blue())
        };
        // A chave de cima: o miolo mostra o fundo e a bolinha (à esquerda) é cheia.
        assert_eq!(px(2.0 + 10.0, 2.0 + 3.8), (30, 30, 34));
        assert_ne!(px(2.0 + 3.8, 2.0 + 3.8), (30, 30, 34));
        // A de baixo é cheia, e a bolinha (à direita) é um furo que mostra o fundo.
        assert_ne!(px(2.0 + 5.0, 2.0 + 8.6 + 3.4), (30, 30, 34));
        assert_eq!(px(2.0 + 12.2, 2.0 + 8.6 + 3.4), (30, 30, 34));
    }

    /// A central com tudo (botões e os três trilhos) cabe na altura medida.
    #[test]
    fn central_de_controle() {
        let info = ControlInfo { wifi: Some((true, "Casa".into())), bluetooth: Some(false), quiet: true, brightness: Some(0.6), keyboard: Some(0.3), volume: Some((0.8, false)) };
        let (parts, h) = control_layout(&info);
        assert_eq!(parts.len(), 6);
        assert!(parts.iter().all(|(_, r)| r[1] + r[3] <= h));
        assert_eq!(control_level(parts[3].1, parts[3].1[0] + 28.0), 0.0);
        let theme = crate::theme::Theme::new(crate::theme::Variant::Claro);
        if let Some(fonts) = crate::Fonts::load_system() {
            let c = control_panel(&theme, &fonts, &info, 1.0).unwrap();
            if let Ok(dir) = std::env::var("GLOVE_TEST_PNG") {
                c.pm.save_png(std::path::Path::new(&dir).join("central.png")).unwrap();
            }
        }
    }
}

