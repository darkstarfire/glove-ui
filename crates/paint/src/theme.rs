//! Temas. Cada variante lembra uma fase da era "gel" dos anos 2000,
//! com nomes e arte próprios.

use glove_i18n::tr;
use crate::draw::rgba;
use tiny_skia::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Sóbrio e claro, o padrão (lembra o fim da era): barra de menu lisa, menus com
    /// separador em linha, Dock liso e janelas em cinza claro.
    Claro,
    /// Listras finas e gel azul (lembra o começo da era).
    Listras,
    /// Metal escovado nas barras de título.
    Metal,
    /// Listras com acentos em grafite, mais sóbrio.
    Grafite,
    /// Moderno (claro): liso, cantos arredondados, sem gel nem listras, barras
    /// translúcidas e controles planos (a geração de 2021, não a mais nova).
    Moderno,
    /// Moderno escuro: o mesmo, em cinza-grafite com o texto claro.
    ModernoEscuro,
}

impl Variant {
    pub const ALL: [Variant; 6] = [Variant::Claro, Variant::Listras, Variant::Metal, Variant::Grafite, Variant::Moderno, Variant::ModernoEscuro];

    pub fn id(self) -> &'static str {
        match self {
            Variant::Claro => "claro",
            Variant::Listras => "listras",
            Variant::Metal => "metal",
            Variant::Grafite => "grafite",
            Variant::Moderno => "moderno",
            Variant::ModernoEscuro => "moderno-escuro",
        }
    }

    /// Uma das aparências modernas (claras ou escura).
    pub fn modern(self) -> bool {
        matches!(self, Variant::Moderno | Variant::ModernoEscuro)
    }

    pub fn label(self) -> &'static str {
        match self {
            Variant::Claro => tr!("Claro"),
            Variant::Listras => tr!("Listras"),
            Variant::Metal => tr!("Metal"),
            Variant::Grafite => tr!("Grafite"),
            Variant::Moderno => tr!("Moderno Claro"),
            Variant::ModernoEscuro => tr!("Moderno Escuro"),
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id().eq_ignore_ascii_case(s))
    }

    /// Tema da configuração, com o padrão (Claro) quando o nome é desconhecido.
    pub fn from_config(s: &str) -> Self {
        Self::from_id(s).unwrap_or(Variant::Claro)
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub variant: Variant,
    /// Cor de destaque (gel de botões, seleção de menu, Dock ativo).
    pub accent: Color,
    pub close: Color,
    pub minimize: Color,
    pub zoom: Color,
    /// Botões de janelas inativas.
    pub inactive_button: Color,
    pub title_text: Color,
    pub title_text_inactive: Color,
    pub stripe_a: Color,
    pub stripe_b: Color,
    pub metal_top: Color,
    pub metal_bottom: Color,
    pub menu_bg_a: Color,
    pub menu_bg_b: Color,
    pub menu_text: Color,
    /// Item de menu sob o mouse (o Tiger usa um azul liso, sem degradê).
    pub menu_hot: Color,
    /// Fundo das janelas dos apps: listras alternadas.
    pub body_a: Color,
    pub body_b: Color,
    pub wallpaper: [Color; 3],
    /// Fundo do texto selecionado (cor de destaque do painel Aparência).
    pub highlight: Color,
    /// Aparência moderna (lisa) e escura.
    pub modern: bool,
    pub dark: bool,
    /// Texto comum, texto secundário e o fundo de listas, campos e tabelas.
    pub text: Color,
    pub text_dim: Color,
    pub list_bg: Color,
    /// Fundo dos controles (botões, listas suspensas) e a borda deles.
    pub control_bg: Color,
    pub control_border: Color,
    /// Linhas finas de separação.
    pub separator: Color,
}

/// Visual escolhido por um app feito com o glove-ui: seguir as Preferências (o
/// padrão) ou ficar sempre no clássico (a era do gel) ou sempre no moderno. A
/// estrutura do app é a mesma nos dois; só o desenho dos controles muda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Visual {
    /// O que o usuário escolheu em Preferências ▸ Aparência.
    #[default]
    System,
    /// Sempre clássico: o Claro, ou a variante clássica escolhida.
    Classic,
    /// Sempre moderno: o Moderno Claro, ou o Escuro se for o escolhido.
    Modern,
}

static VISUAL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Fixa o visual do app (chamar antes de abrir a janela).
pub fn set_visual(v: Visual) {
    VISUAL.store(v as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn visual() -> Visual {
    match VISUAL.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Visual::Classic,
        2 => Visual::Modern,
        _ => Visual::System,
    }
}

impl Visual {
    /// A variante das Preferências ajustada ao visual pedido pelo app.
    pub fn apply(self, chosen: Variant) -> Variant {
        match self {
            Visual::System => chosen,
            Visual::Classic if chosen.modern() => Variant::Claro,
            Visual::Modern if !chosen.modern() => Variant::Moderno,
            _ => chosen,
        }
    }
}

/// O estilo do processo (o tema principal que ele mostra): os desenhos que não recebem
/// o tema (sombras, controles avulsos, a troca de cores do modo escuro) consultam aqui.
static STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Guarda o estilo do tema principal (chamado por quem monta o tema da configuração).
pub fn set_style(t: &Theme) {
    STYLE.store(u8::from(t.modern) | (u8::from(t.dark) << 1), std::sync::atomic::Ordering::Relaxed);
}

/// O processo está numa aparência moderna?
pub fn modern() -> bool {
    STYLE.load(std::sync::atomic::Ordering::Relaxed) & 1 != 0
}

/// O processo está na aparência escura?
pub fn dark() -> bool {
    STYLE.load(std::sync::atomic::Ordering::Relaxed) & 2 != 0
}

static SYSTEM_SCREENS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// "Usar a aparência em todo o sistema" (Preferências ▸ Aparência): as telas do sistema
/// (abertura da sessão, bloqueio, saída) seguem o tema moderno. Chamado pelo Glove ao
/// ler a configuração.
pub fn set_system_screens(on: bool) {
    SYSTEM_SCREENS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// As telas do sistema no tema `t`: moderno (claro ou escuro) só com a opção ligada.
pub fn system_modern(t: &Theme) -> bool {
    t.modern && SYSTEM_SCREENS.load(std::sync::atomic::Ordering::Relaxed)
}

pub const TITLE_H: i32 = 22;
pub const MENUBAR_H: i32 = 22;

impl Theme {
    pub fn new(variant: Variant) -> Self {
        let graphite = variant == Variant::Grafite;
        let accent = if graphite { rgba(120, 132, 150, 255) } else { rgba(45, 120, 230, 255) };
        let (close, minimize, zoom) = if graphite {
            (rgba(140, 150, 165, 255), rgba(140, 150, 165, 255), rgba(140, 150, 165, 255))
        } else {
            (rgba(230, 70, 60, 255), rgba(240, 180, 40, 255), rgba(80, 190, 70, 255))
        };
        let wallpaper = match variant {
            Variant::Claro | Variant::Listras | Variant::Moderno | Variant::ModernoEscuro => [rgba(10, 40, 110, 255), rgba(30, 95, 200, 255), rgba(140, 200, 255, 255)],
            Variant::Metal => [rgba(8, 28, 70, 255), rgba(40, 80, 160, 255), rgba(170, 200, 240, 255)],
            Variant::Grafite => [rgba(35, 40, 52, 255), rgba(85, 95, 115, 255), rgba(190, 200, 215, 255)],
        };
        let claro = variant == Variant::Claro;
        Self {
            variant,
            accent,
            close,
            minimize,
            zoom,
            inactive_button: rgba(190, 190, 195, 255),
            title_text: rgba(20, 20, 20, 255),
            title_text_inactive: rgba(130, 130, 130, 255),
            stripe_a: rgba(236, 236, 236, 255),
            stripe_b: rgba(246, 246, 246, 255),
            // Metal do Tiger: claro e suave (de ~230 a ~200), riscos quase invisíveis.
            metal_top: rgba(232, 232, 232, 255),
            metal_bottom: rgba(203, 203, 205, 255),
            menu_bg_a: rgba(246, 246, 246, 238),
            menu_bg_b: rgba(255, 255, 255, 238),
            menu_text: rgba(0, 0, 0, 255),
            menu_hot: if claro { rgba(66, 132, 214, 255) } else { accent },
            body_a: if claro { rgba(240, 240, 240, 255) } else { rgba(232, 232, 232, 255) },
            body_b: if claro { rgba(243, 243, 243, 255) } else { rgba(242, 242, 242, 255) },
            wallpaper,
            highlight: if graphite { rgba(199, 208, 219, 255) } else { rgba(181, 213, 255, 255) },
            modern: false,
            dark: false,
            text: rgba(20, 20, 24, 255),
            text_dim: rgba(110, 110, 116, 255),
            list_bg: rgba(255, 255, 255, 255),
            control_bg: rgba(255, 255, 255, 255),
            control_border: rgba(150, 150, 156, 255),
            separator: rgba(190, 190, 194, 255),
        }
        .modernize(variant)
    }

    /// As aparências modernas: liso, sem listras nem gel; o escuro em grafite.
    fn modernize(mut self, variant: Variant) -> Self {
        if !variant.modern() {
            return self;
        }
        let dark = variant == Variant::ModernoEscuro;
        self.modern = true;
        self.dark = dark;
        self.accent = rgba(10, 122, 255, 255);
        self.close = rgba(255, 95, 87, 255);
        self.minimize = rgba(254, 188, 46, 255);
        self.zoom = rgba(40, 200, 64, 255);
        self.menu_hot = rgba(10, 122, 255, 255);
        self.highlight = if dark { rgba(30, 80, 150, 255) } else { rgba(179, 215, 255, 255) };
        if dark {
            self.inactive_button = rgba(88, 88, 92, 255);
            self.title_text = rgba(226, 226, 230, 255);
            self.title_text_inactive = rgba(140, 140, 146, 255);
            let bg = rgba(38, 38, 41, 255);
            (self.stripe_a, self.stripe_b, self.body_a, self.body_b) = (bg, bg, bg, bg);
            (self.metal_top, self.metal_bottom) = (rgba(56, 56, 60, 255), rgba(48, 48, 52, 255));
            (self.menu_bg_a, self.menu_bg_b) = (rgba(44, 44, 48, 246), rgba(44, 44, 48, 246));
            self.menu_text = rgba(236, 236, 240, 255);
            self.text = rgba(232, 232, 236, 255);
            self.text_dim = rgba(152, 152, 158, 255);
            self.list_bg = rgba(30, 30, 32, 255);
            self.control_bg = rgba(80, 80, 84, 255);
            self.control_border = rgba(96, 96, 102, 255);
            self.separator = rgba(70, 70, 74, 255);
            self.wallpaper = [rgba(12, 16, 32, 255), rgba(40, 52, 96, 255), rgba(120, 130, 190, 255)];
        } else {
            self.inactive_button = rgba(208, 208, 212, 255);
            self.title_text = rgba(38, 38, 42, 255);
            self.title_text_inactive = rgba(160, 160, 166, 255);
            let bg = rgba(236, 236, 238, 255);
            (self.stripe_a, self.stripe_b, self.body_a, self.body_b) = (bg, bg, bg, bg);
            (self.metal_top, self.metal_bottom) = (rgba(240, 240, 242, 255), rgba(230, 230, 233, 255));
            (self.menu_bg_a, self.menu_bg_b) = (rgba(246, 246, 248, 246), rgba(246, 246, 248, 246));
            self.menu_text = rgba(24, 24, 28, 255);
            self.text = rgba(24, 24, 28, 255);
            self.text_dim = rgba(120, 120, 126, 255);
            self.list_bg = rgba(255, 255, 255, 255);
            self.control_bg = rgba(255, 255, 255, 255);
            self.control_border = rgba(196, 196, 200, 255);
            self.separator = rgba(214, 214, 218, 255);
            self.wallpaper = [rgba(30, 60, 140, 255), rgba(90, 120, 210, 255), rgba(200, 180, 230, 255)];
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_do_app_ajusta_a_variante() {
        assert_eq!(Visual::System.apply(Variant::Metal), Variant::Metal);
        assert_eq!(Visual::Classic.apply(Variant::ModernoEscuro), Variant::Claro);
        assert_eq!(Visual::Classic.apply(Variant::Grafite), Variant::Grafite);
        assert_eq!(Visual::Modern.apply(Variant::Listras), Variant::Moderno);
        assert_eq!(Visual::Modern.apply(Variant::ModernoEscuro), Variant::ModernoEscuro);
    }
}
