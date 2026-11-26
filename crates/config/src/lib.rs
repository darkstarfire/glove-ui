//! Configuração do Glove (~/.config/glove/config.toml), compartilhada entre
//! o compositor e os apps (Preferências, etc.).

/// Traduções (ver o glove-i18n): a macro `tr!` com o texto em português.
pub use glove_i18n::{idioma, tr, tr_dyn, virgula};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod arquivos;
pub mod audio;
pub mod comando;
pub mod discos;
pub mod inicio;
pub mod login_wallpaper;
pub mod rede;
pub mod senha;
pub mod sons;

/// Segurança (Preferências ▸ Segurança).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Security {
    /// Sair da sessão depois de N minutos parado (0 = nunca), com o aviso de 2 minutos.
    pub logout_minutes: u32,
}

/// Busca Rápida (Preferências ▸ Busca Rápida): o que aparece e onde não procurar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchPrefs {
    pub apps: bool,
    pub prefs: bool,
    pub files: bool,
    /// Pastas que a busca não olha (Privacidade).
    pub excluded: Vec<String>,
}

impl Default for SearchPrefs {
    fn default() -> Self {
        Self { apps: true, prefs: true, files: true, excluded: Vec::new() }
    }
}

/// CDs e DVDs (Preferências ▸ Mídia Removível).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Media {
    /// Ao inserir um CD ou DVD de dados: "open" (no Arquivos) ou "nothing".
    pub data_disc: String,
    /// Ao inserir um DVD de vídeo: "video" (no Vídeo), "open" (no Arquivos) ou "nothing".
    pub video_dvd: String,
}

impl Default for Media {
    fn default() -> Self {
        Self { data_disc: "open".into(), video_dvd: "video".into() }
    }
}

/// Mesa (Opções de Visualização da mesa, Ctrl+J com a mesa em foco).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Desk {
    /// Lado do ícone, em pontos (32 a 128).
    pub icon_size: u32,
    /// Tamanho do nome (10 a 16).
    pub text_size: u32,
    /// Nome à direita do ícone (em vez de embaixo).
    pub label_right: bool,
    /// Mostrar o disco do sistema e os pendrives/discos externos.
    pub show_disks: bool,
    pub show_removable: bool,
    /// Manter organizado por: "" (livre), "name", "date", "size" ou "kind".
    pub arrange: String,
    /// Clicar numa área livre da mesa mostra a mesa (afasta as janelas, como o F11).
    pub click_shows_desktop: bool,
}

impl Default for Desk {
    fn default() -> Self {
        Self { icon_size: 52, text_size: 12, label_right: false, show_disks: true, show_removable: true, arrange: String::new(), click_shows_desktop: false }
    }
}

/// A pasta Mesa do usuário (`XDG_DESKTOP_DIR`), criada se faltar.
pub fn desktop_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
    let from_dirs = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).ok().and_then(|t| {
        let line = t.lines().find(|l| l.starts_with("XDG_DESKTOP_DIR="))?.to_string();
        let v = line.split_once('=')?.1.trim().trim_matches('"').to_string();
        Some(match v.strip_prefix("$HOME") {
            Some(rest) => home.join(rest.trim_start_matches('/')),
            None => PathBuf::from(v),
        })
    });
    let dir = from_dirs.filter(|p| *p != home).unwrap_or_else(|| home.join("Desktop"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Relógio da barra de menu (painel Data e Hora, aba Relógio).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clock {
    pub show: bool,
    /// Mostrador de ponteiros em vez dos números.
    pub analog: bool,
    pub seconds: bool,
    pub weekday: bool,
    /// Data curta ("5 nov") antes da hora.
    pub date: bool,
    pub h24: bool,
    /// Os dois-pontos piscam a cada segundo.
    pub blink: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self { show: true, analog: false, seconds: false, weekday: true, date: false, h24: true, blink: false }
    }
}

impl Clock {
    /// Texto do relógio para (dia da semana 0=domingo, dia, mês 1-12, hora, minuto, segundo).
    pub fn text(&self, wday: u32, day: u32, month: u32, h: u32, m: u32, s: u32) -> String {
        let dias = [tr!("Dom"), tr!("Seg"), tr!("Ter"), tr!("Qua"), tr!("Qui"), tr!("Sex"), tr!("Sáb")];
        let meses = [tr!("jan"), tr!("fev"), tr!("mar"), tr!("abr"), tr!("mai"), tr!("jun"), tr!("jul"), tr!("ago"), tr!("set"), tr!("out"), tr!("nov"), tr!("dez")];
        let colon = if self.blink && s % 2 == 1 { " " } else { ":" };
        let (hh, suffix) = if self.h24 {
            (format!("{h:02}"), "")
        } else {
            let h12 = if h % 12 == 0 { 12 } else { h % 12 };
            (h12.to_string(), if h < 12 { " AM" } else { " PM" })
        };
        let mut t = String::new();
        if self.weekday {
            t.push_str(dias[(wday % 7) as usize]);
            t.push(' ');
        }
        if self.date {
            // {0} dia, {1} mês abreviado (cada idioma na sua ordem).
            t.push_str(&tr!("{0} {1} ", day, meses[((month + 11) % 12) as usize]));
        }
        t.push_str(&format!("{hh}{colon}{m:02}"));
        if self.seconds {
            t.push_str(&format!("{colon}{s:02}"));
        }
        t.push_str(suffix);
        t
    }
}

/// Protetor de tela.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saver {
    /// Qual (ver `glove_paint::saver::SAVERS`).
    pub kind: String,
    /// Minutos parado até começar; 0 = nunca.
    pub minutes: u32,
}

impl Default for Saver {
    fn default() -> Self {
        Self { kind: "aurora".into(), minutes: 5 }
    }
}

/// Acesso Universal (feito pelo compositor).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Access {
    /// Zoom da tela inteira, seguindo o mouse.
    pub zoom: bool,
    /// Quanto o zoom aproxima (1,5 a 8).
    pub zoom_level: f32,
    /// Branco no preto.
    pub invert: bool,
    /// Escala de cinza.
    pub grayscale: bool,
    /// Contraste a mais, de 0 (normal) a 1 (máximo).
    pub contrast: f32,
    /// Tamanho do cursor (1 = normal, até 4).
    pub cursor_size: f32,
}

impl Default for Access {
    fn default() -> Self {
        Self { zoom: false, zoom_level: 2.0, invert: false, grayscale: false, contrast: 0.0, cursor_size: 1.0 }
    }
}

impl Access {
    /// Algum efeito de tela ligado (precisa do passo extra de desenho)?
    pub fn screen_effects(&self) -> bool {
        (self.zoom && self.zoom_level > 1.01) || self.invert || self.grayscale || self.contrast > 0.01
    }
}

/// Painel Som.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sound {
    /// Nome do alerta (ver `sons::ALERTS`).
    pub alert: String,
    /// Volume do alerta, de 0 a 1.
    pub alert_volume: f32,
    /// "Pop" ao mudar o volume pelas teclas.
    pub volume_feedback: bool,
    /// Volume na barra de menu.
    pub menu_extra: bool,
}

impl Default for Sound {
    fn default() -> Self {
        Self { alert: "glass".into(), alert_volume: 0.7, volume_feedback: true, menu_extra: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockApp {
    pub name: String,
    pub icon: String,
    pub command: String,
    /// Usado para saber se o app já está aberto.
    #[serde(default)]
    pub app_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dock {
    pub size: u32,
    pub magnification: bool,
    pub max_size: u32,
    /// "bottom", "left" ou "right".
    pub position: String,
    /// Some e reaparece quando o mouse encosta na borda.
    pub autohide: bool,
    /// Ícone pula enquanto o app abre.
    pub animate_launch: bool,
    /// Efeito de minimizar: "scale" ou "genie".
    pub minimize_effect: String,
}

impl Default for Dock {
    fn default() -> Self {
        Self { size: 56, magnification: true, max_size: 80, position: "bottom".into(), autohide: false, animate_launch: true, minimize_effect: "scale".into() }
    }
}

/// Layouts de teclado conhecidos: (layout, variante, nome, sigla da barra).
pub const KEYBOARD_LAYOUTS: [(&str, &str, &str, &str); 10] = [
    ("br", "", "Português (Brasil, ABNT2)", "BR"),
    ("us", "", "Inglês (EUA)", "US"),
    ("us", "intl", "Inglês (EUA, internacional)", "US+"),
    ("pt", "", "Português (Portugal)", "PT"),
    ("latam", "", "Espanhol (América Latina)", "LA"),
    ("es", "", "Espanhol (Espanha)", "ES"),
    ("gb", "", "Inglês (Reino Unido)", "GB"),
    ("fr", "", "Francês", "FR"),
    ("de", "", "Alemão", "DE"),
    ("it", "", "Italiano", "IT"),
];

/// Um layout do catálogo do sistema (xkeyboard-config): (layout, variante, nome).
pub type Catalogo = (String, String, String);

/// O catálogo de layouts e variantes instalado (base.lst do xkeyboard-config), sem os que
/// já estão na lista de sempre. Vazio se o sistema não tem o catálogo.
pub fn catalogo_teclado() -> Vec<Catalogo> {
    static C: std::sync::OnceLock<Vec<Catalogo>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let Ok(t) = std::fs::read_to_string("/usr/share/X11/xkb/rules/base.lst") else { return Vec::new() };
        let mut v = Vec::new();
        let mut secao = "";
        for l in t.lines() {
            if let Some(s) = l.strip_prefix("! ") {
                secao = s.trim();
                continue;
            }
            let l = l.trim();
            if l.is_empty() {
                continue;
            }
            match secao {
                "layout" => {
                    if let Some((cod, nome)) = l.split_once(char::is_whitespace) {
                        v.push((cod.to_string(), String::new(), nome.trim().to_string()));
                    }
                }
                "variant" => {
                    // "intl            us: English (US, intl., with dead keys)"
                    let mut it = l.splitn(2, char::is_whitespace);
                    let (Some(var), Some(resto)) = (it.next(), it.next()) else { continue };
                    let Some((lay, nome)) = resto.trim().split_once(':') else { continue };
                    v.push((lay.trim().to_string(), var.to_string(), nome.trim().to_string()));
                }
                _ => {}
            }
        }
        v
    })
    .clone()
}

/// O nome de um layout pelo id ("us:intl"), no catálogo do sistema ou nos conhecidos.
pub fn nome_layout(id: &str) -> String {
    let (l, v) = split_layout(id);
    if let Some(k) = KEYBOARD_LAYOUTS.iter().find(|k| k.0 == l && k.1 == v) {
        return k.2.to_string();
    }
    catalogo_teclado().into_iter().find(|c| c.0 == l && c.1 == v).map(|c| c.2).unwrap_or_else(|| id.to_string())
}

/// "us:intl" → ("us", "intl").
pub fn split_layout(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or((id, ""))
}

/// ("us", "intl") → "us:intl".
pub fn layout_id(layout: &str, variant: &str) -> String {
    if variant.is_empty() { layout.to_string() } else { format!("{layout}:{variant}") }
}

/// Sigla curta para a barra ("BR", "US+").
pub fn layout_badge(layout: &str, variant: &str) -> String {
    KEYBOARD_LAYOUTS.iter().find(|l| l.0 == layout && l.1 == variant).map(|l| l.3.to_string()).unwrap_or_else(|| layout.to_uppercase())
}

/// O próximo layout do Menu de Entrada depois do atual.
pub fn next_layout(k: &Keyboard) -> Option<(String, String)> {
    if k.input_menu.len() < 2 {
        return None;
    }
    let cur = layout_id(&k.layout, &k.variant);
    let i = k.input_menu.iter().position(|l| *l == cur).map_or(0, |i| (i + 1) % k.input_menu.len());
    let (l, v) = split_layout(&k.input_menu[i]);
    Some((l.to_string(), v.to_string()))
}

/// Painel Aparência.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Cor do texto selecionado (ver `HIGHLIGHTS`).
    pub highlight: String,
    /// Clique duplo na barra de título minimiza a janela.
    pub double_click_minimizes: bool,
    /// Clique no trilho da barra de rolagem vai até ali (em vez de uma página).
    pub scroll_jump: bool,
    /// Quantos itens em Itens Recentes (apps e documentos).
    pub recent_apps: u32,
    pub recent_docs: u32,
    /// Tema de ícones (pasta em /usr/share/icons); vazio = os desenhados pelo Glove.
    #[serde(default)]
    pub icon_theme: String,
    /// Reduzir efeitos (computadores mais simples): sem animações, sem ampliação do
    /// Dock, sem sombras e sem o botão padrão pulsando.
    #[serde(default)]
    pub reduce_effects: bool,
    /// A aparência também nas telas do sistema: a abertura (do sistema e da sessão), a
    /// tela de login, o bloqueio e a saída. Desligado, elas ficam no visual clássico.
    #[serde(default)]
    pub system_theme: bool,
    /// O tema de cada tela do sistema (Preferências ▸ Telas do Sistema): "" = segue a
    /// opção acima (a aparência, se ligada; senão o clássico), "appearance" = sempre a
    /// aparência escolhida, ou o nome de um tema ("light", "modern-dark"…).
    /// A sessão: abertura da sessão, bloqueio de tela e saída.
    #[serde(default)]
    pub screen_session: String,
    /// A tela de login.
    #[serde(default)]
    pub screen_login: String,
    /// A abertura e o desligamento do sistema (o Plymouth).
    #[serde(default)]
    pub screen_boot: String,
}

/// As telas do sistema que têm tema próprio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Session,
    Login,
    Boot,
}

static REDUCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// "Reduzir efeitos" ligado (lido a cada `Config::load`).
pub fn reduce_effects() -> bool {
    REDUCE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Liga ou desliga "Reduzir efeitos" neste processo (o `Config::load` já faz isso).
pub fn set_reduce_effects(on: bool) {
    REDUCE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Duração de uma animação: com "Reduzir efeitos", quase nada (termina no quadro seguinte).
pub fn anim(d: std::time::Duration) -> std::time::Duration {
    if reduce_effects() { std::time::Duration::from_millis(1) } else { d }
}

impl Default for Appearance {
    fn default() -> Self {
        Self { highlight: "blue".into(), double_click_minimizes: false, scroll_jump: false, recent_apps: 10, recent_docs: 10, icon_theme: String::new(), reduce_effects: false, system_theme: false, screen_session: String::new(), screen_login: String::new(), screen_boot: String::new() }
    }
}

/// Nome do ícone (Icon=) do app de fora com este app_id: o .desktop com o mesmo
/// nome ou com StartupWMClass igual. Guardado depois da primeira busca.
pub fn desktop_icon(app_id: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let key = app_id.to_lowercase();
    if key.is_empty() {
        return None;
    }
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().ok()?.get(&key) {
        return v.clone();
    }
    let mut dirs = vec![std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/applications")).unwrap_or_default()];
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    let mut found = None;
    'all: for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let stem = e.path().file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)).map(|v| v.trim().to_string());
            let wm = field("StartupWMClass=").map(|v| v.to_lowercase());
            let last = stem.rsplit('.').next().unwrap_or(&stem).to_string();
            if stem == key || wm.as_deref() == Some(key.as_str()) || last == key {
                if let Some(i) = field("Icon=") {
                    found = Some(i);
                    break 'all;
                }
            }
        }
    }
    if let Ok(mut c) = cache.lock() {
        c.insert(key, found.clone());
    }
    found
}

/// Cores de destaque do texto selecionado: (id, nome, r, g, b).
pub const HIGHLIGHTS: [(&str, &str, u8, u8, u8); 8] = [
    ("blue", "Azul", 181, 213, 255),
    ("graphite", "Grafite", 199, 208, 219),
    ("silver", "Prata", 214, 214, 220),
    ("gold", "Dourado", 252, 236, 152),
    ("orange", "Laranja", 255, 210, 160),
    ("purple", "Roxo", 226, 200, 245),
    ("red", "Vermelho", 255, 190, 190),
    ("green", "Verde", 196, 240, 180),
];

/// Cor de destaque pelo id (azul se não conhecer).
pub fn highlight_rgb(id: &str) -> (u8, u8, u8) {
    let h = HIGHLIGHTS.iter().find(|h| h.0 == id).unwrap_or(&HIGHLIGHTS[0]);
    (h.2, h.3, h.4)
}

/// Cantos ativos: "all", "app" ou "desktop" (Exposé), "dashboard", "apps", "screensaver"…; vazio = nada.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotCorners {
    pub top_left: String,
    pub top_right: String,
    pub bottom_left: String,
    pub bottom_right: String,
}

/// Economia de energia e bloqueio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Energy {
    /// Na tomada (ou sem bateria): minutos parado até apagar a tela; 0 = nunca.
    pub screen_off_minutes: u32,
    /// Na tomada: minutos parado até o computador entrar em repouso; 0 = nunca.
    pub sleep_minutes: u32,
    /// Na bateria: o mesmo, valores próprios.
    pub battery_screen_off_minutes: u32,
    pub battery_sleep_minutes: u32,
    /// Pedir a senha ao voltar (tela apagada, repouso ou tampa fechada).
    pub require_password: bool,
    /// Diminuir um pouco o brilho ao passar para a bateria.
    pub dim_on_battery: bool,
    /// Bateria na barra de menu.
    pub battery_menu: bool,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            screen_off_minutes: 10,
            sleep_minutes: 0,
            battery_screen_off_minutes: 5,
            battery_sleep_minutes: 15,
            require_password: true,
            dim_on_battery: true,
            battery_menu: true,
        }
    }
}

impl Energy {
    /// (minutos até apagar a tela, minutos até o repouso) para a fonte de energia.
    pub fn timeouts(&self, on_battery: bool) -> (u32, u32) {
        if on_battery {
            (self.battery_screen_off_minutes, self.battery_sleep_minutes)
        } else {
            (self.screen_off_minutes, self.sleep_minutes)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    pub layout: String,
    pub variant: String,
    /// Layouts do Menu de Entrada ("br", "us:intl"…); a tecla de troca passa por eles.
    pub input_menu: Vec<String>,
    /// Layouts acrescentados pelo catálogo do sistema ("Adicionar Layout…"), além dos de sempre.
    #[serde(default)]
    pub extra_layouts: Vec<String>,
    /// Menu de Entrada na barra de menu.
    pub show_input_menu: bool,
    /// Atraso antes de repetir, em ms.
    pub repeat_delay: u32,
    /// Repetições por segundo.
    pub repeat_rate: u32,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self { layout: "br".into(), variant: String::new(), input_menu: vec!["br".into(), "us".into()], extra_layouts: Vec::new(), show_input_menu: false, repeat_delay: 300, repeat_rate: 30 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pointer {
    /// Rolagem natural da roda do mouse.
    pub natural_scroll: bool,
    /// Velocidade do cursor, de -1 (lenta) a 1 (rápida).
    pub speed: f64,
    /// Aceleração: o cursor anda mais quando o mouse se move rápido.
    pub acceleration: bool,
    /// Botões trocados (canhoto).
    pub left_handed: bool,
    /// Multiplica a rolagem (0,25 a 4).
    pub scroll_speed: f64,
    /// Tempo máximo entre os cliques de um clique duplo, em ms (mouse e trackpad).
    pub double_click_ms: u32,
}

impl Default for Pointer {
    fn default() -> Self {
        Self { natural_scroll: false, speed: 0.0, acceleration: true, left_handed: false, scroll_speed: 1.0, double_click_ms: 400 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Touchpad {
    pub tap_to_click: bool,
    pub natural_scroll: bool,
    /// De -1 (lenta) a 1 (rápida).
    pub speed: f64,
    /// Ignora o touchpad enquanto se digita.
    pub disable_while_typing: bool,
    /// Clique com dois dedos = botão direito (em vez de canto inferior direito).
    pub two_finger_right_click: bool,
    /// Tocar duas vezes e arrastar.
    pub tap_drag: bool,
    /// Gestos com três e quatro dedos (Exposé, mesas virtuais, Todos os Apps).
    pub gestures: bool,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self { tap_to_click: true, natural_scroll: true, speed: 0.0, disable_while_typing: true, two_finger_right_click: true, tap_drag: true, gestures: true }
    }
}

/// Papel de parede animado (vídeo ou GIF, tocado pelo ffmpeg).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallpaperAnim {
    /// Quadros por segundo (o vídeo é reduzido a isso).
    pub fps: u32,
    /// Leve: a imagem com metade da resolução (a placa de vídeo amplia).
    pub light: bool,
    /// Parar na bateria (fica o quadro atual).
    pub pause_on_battery: bool,
    /// Parar com um app em tela cheia.
    pub pause_fullscreen: bool,
}

impl Default for WallpaperAnim {
    fn default() -> Self {
        Self { fps: 24, light: true, pause_on_battery: true, pause_fullscreen: true }
    }
}

impl Config {
    /// O que está gravado para uma tela do sistema.
    pub fn screen_choice(&self, which: Screen) -> &str {
        match which {
            Screen::Session => &self.appearance.screen_session,
            Screen::Login => &self.appearance.screen_login,
            Screen::Boot => &self.appearance.screen_boot,
        }
    }

    pub fn set_screen_choice(&mut self, which: Screen, v: &str) {
        let c = match which {
            Screen::Session => &mut self.appearance.screen_session,
            Screen::Login => &mut self.appearance.screen_login,
            Screen::Boot => &mut self.appearance.screen_boot,
        };
        *c = v.to_string();
    }

    /// O tema (o nome) que uma tela do sistema usa de fato. Sem escolha própria: a
    /// aparência com "Usar a aparência em todo o sistema" ligada; senão o clássico (a
    /// própria aparência se ela é clássica, o Claro se é moderna).
    pub fn screen_theme(&self, which: Screen) -> String {
        match self.screen_choice(which) {
            "" if self.appearance.system_theme => self.theme.clone(),
            "" => {
                if self.theme.starts_with("modern") { "light".into() } else { self.theme.clone() }
            }
            "appearance" => self.theme.clone(),
            t => t.to_string(),
        }
    }

    /// O papel de parede de um monitor (pelo nome do conector).
    pub fn wallpaper_for(&self, output: &str) -> &str {
        if self.wallpaper_mode == "per-display" {
            if let Some(w) = self.wallpapers.get(output).filter(|w| !w.is_empty()) {
                return w;
            }
        }
        &self.wallpaper
    }
}

/// Monitor (só na sessão própria; dentro de outro desktop vale a janela).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Monitor {
    /// Conector preferido (ex.: "HDMI-A-1"); vazio = automático (externo, se houver).
    pub output: String,
    /// Resolução "LxA@Hz" (ex.: "1920x1080@60"); vazio = a recomendada pelo monitor.
    pub mode: String,
    /// Escala da interface; 0 = automática (pela densidade da tela).
    pub scale: f64,
    /// Menu de monitores (brilho, resolução) na barra de menu.
    pub menu_extra: bool,
    /// Com mais de um monitor: "extend" (mesa maior), "mirror" (a mesma
    /// imagem) ou "single" (só o principal).
    #[serde(default = "default_arrangement")]
    pub arrangement: String,
    /// Onde ficam os outros monitores, vistos do principal: "right",
    /// "left" ou "below".
    #[serde(default = "default_side")]
    pub side: String,
    /// Resolução e escala dos outros monitores (os que não são o principal), pelo
    /// nome do conector. Sem entrada, cada um usa a recomendada e a escala automática.
    #[serde(default)]
    pub others: std::collections::BTreeMap<String, OtherMonitor>,
    /// Mesa estendida: a barra de menu em todos os monitores (senão, só no principal).
    pub menubar_all: bool,
    /// Mesa estendida: o Dock em todos os monitores.
    pub dock_all: bool,
    /// A barra de menu dos monitores sem o foco (sem a janela ativa) fica esmaecida.
    pub dim_inactive: bool,
}

/// Ajustes de um monitor que não é o principal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OtherMonitor {
    /// "LxA@Hz"; vazio = a recomendada.
    pub mode: String,
    /// 0 = automática.
    pub scale: f64,
}

fn default_arrangement() -> String {
    "extend".into()
}

fn default_side() -> String {
    "right".into()
}

impl Default for Monitor {
    fn default() -> Self {
        Self { output: String::new(), mode: String::new(), scale: 0.0, menu_extra: true, arrangement: default_arrangement(), side: default_side(), others: Default::default(), menubar_all: true, dock_all: true, dim_inactive: true }
    }
}

/// Monitores conectados, escritos pelo compositor para as Preferências lerem
/// ($XDG_RUNTIME_DIR/glove-outputs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputsInfo {
    /// Rodando dentro de outro desktop.
    pub nested: bool,
    pub outputs: Vec<OutputInfo>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    pub description: String,
    /// Tamanho físico em mm.
    pub size_mm: (u32, u32),
    pub active: bool,
    /// O monitor principal (barra de menu e Dock).
    #[serde(default)]
    pub primary: bool,
    /// "LxA@Hz" em uso.
    pub current: String,
    pub preferred: String,
    pub modes: Vec<String>,
    pub scale: f64,
    pub auto_scale: f64,
    /// Onde ele fica na mesa e o tamanho lógico: (x, y, largura, altura).
    #[serde(default)]
    pub rect: (i32, i32, i32, i32),
}

impl OutputsInfo {
    pub fn path() -> Option<PathBuf> {
        std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("glove-outputs"))
    }

    pub fn load() -> Option<Self> {
        toml::from_str(&std::fs::read_to_string(Self::path()?).ok()?).ok()
    }

    pub fn save(&self) {
        if let (Some(p), Ok(s)) = (Self::path(), toml::to_string(self)) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }
}

/// Escala sugerida pela densidade da tela (pontos por polegada), em passos de 0,25.
pub fn auto_scale(px: (u32, u32), mm: (u32, u32)) -> f64 {
    if mm.0 < 50 || mm.1 < 50 {
        // Sem tamanho físico confiável (projetores, alguns adaptadores).
        return 1.0;
    }
    let dpi = px.0 as f64 / (mm.0 as f64 / 25.4);
    // Telas "minúsculas e densíssimas" (ex.: 160 × 90 mm numa 1920×1080) quase sempre
    // são adaptadores e TVs que gravam a proporção (16:9) no lugar do tamanho.
    if mm.0 < 200 && dpi > 200.0 {
        return 1.0;
    }
    ((dpi / 135.0) * 4.0).round().clamp(4.0, 12.0) / 4.0
}

/// "1920x1080@60" -> (1920, 1080, 60).
pub fn parse_mode(s: &str) -> Option<(u32, u32, u32)> {
    let (wh, hz) = s.split_once('@').unwrap_or((s, "0"));
    let (w, h) = wh.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?, hz.trim().parse().unwrap_or(0)))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortcut {
    pub action: String,
    /// Ex.: "Super+Return", "Ctrl+Alt+BackSpace".
    pub keys: String,
}

/// Ações que podem ter atalho, com o nome que aparece nas Preferências.
pub const ACTIONS: &[(&str, &str)] = &[
    ("terminal", "Abrir Terminal"),
    ("close", "Fechar janela"),
    ("minimize", "Minimizar janela"),
    ("hide", "Ocultar o app"),
    ("hide_others", "Ocultar os outros apps"),
    ("zoom", "Zoom da janela"),
    ("fullscreen", "Tela cheia"),
    ("cycle", "Trocar de app"),
    ("expose_all", "Exposé: todas as janelas"),
    ("expose_app", "Exposé: janelas do app"),
    ("expose_desktop", "Exposé: mostrar a mesa"),
    ("dashboard", "Painel de Widgets"),
    ("apps", "Todos os Apps"),
    ("desk_prev", "Mesa virtual anterior"),
    ("desk_next", "Próxima mesa virtual"),
    ("desk_move_prev", "Levar a janela para a mesa anterior"),
    ("desk_move_next", "Levar a janela para a próxima mesa"),
    ("cycle_windows", "Alternar janelas do app"),
    ("preferences", "Abrir Preferências"),
    ("screenshot", "Captura de tela"),
    ("screenshot_area", "Captura de uma área"),
    ("clipboard_history", "Histórico da área de transferência"),
    ("record_screen", "Gravar a tela"),
    ("record_area", "Gravar uma área"),
    ("record_toggle", "Gravação de tela: abrir ou parar"),
    ("force_quit", "Forçar Encerrar"),
    ("lock", "Bloquear a tela"),
    ("search", "Busca Rápida"),
    ("input_next", "Próximo layout de teclado"),
    ("zoom_toggle", "Zoom: ligar e desligar"),
    ("zoom_in", "Zoom: aproximar"),
    ("zoom_out", "Zoom: afastar"),
    ("invert_colors", "Inverter as cores"),
    ("quit", "Sair do Glove"),
];

pub fn action_label(id: &str) -> &str {
    ACTIONS.iter().find(|(a, _)| *a == id).map(|(_, l)| tr(l)).unwrap_or(id)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    /// "waves", "aurora", "plain", "color:R,G,B" ou o caminho de uma imagem (PNG,
    /// JPEG, WebP) ou de um vídeo/GIF (papel animado). Vale para todos os monitores,
    /// menos no modo "per-display" para os que têm o seu em `wallpapers`.
    pub wallpaper: String,
    /// Com mais de um monitor: "same" (a mesma em cada um), "per-display" (um papel
    /// para cada monitor) ou "panorama" (uma imagem só cobrindo todos, pela posição
    /// deles: lado a lado, um em cima do outro...).
    pub wallpaper_mode: String,
    /// Como a imagem ocupa a tela: "fill", "fit", "stretch", "center" ou "tile".
    pub wallpaper_fit: String,
    /// Modo "per-display": o papel de cada monitor, pelo nome do conector.
    pub wallpapers: std::collections::BTreeMap<String, String>,
    /// Papel animado (vídeo ou GIF).
    pub wallpaper_anim: WallpaperAnim,
    /// As imagens favoritas para papel de parede (caminhos, ou os nomes das do Glove),
    /// na ordem em que foram juntadas: a coleção "Favoritas" do painel Mesa.
    pub wallpaper_favorites: Vec<String>,
    pub terminal: String,
    /// Família da fonte do Terminal (ex.: "JetBrainsMono Nerd Font"); vazio = padrão.
    pub terminal_font: String,
    pub terminal_font_size: f32,
    /// Tema de cores do Terminal (ver `term_themes`).
    pub terminal_theme: String,
    pub dock: Dock,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub sound: Sound,
    #[serde(default)]
    pub clock: Clock,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub saver: Saver,
    /// Navegador do glove-browser ("" = pela memória: NetSurf ou Firefox).
    #[serde(default, alias = "navegador")]
    pub browser: String,
    /// Ao conectar um pendrive ou disco: "open" (no Arquivos) ou "nothing".
    #[serde(default = "default_removable")]
    pub removable_action: String,
    /// Menu Wi-Fi na barra de menu.
    #[serde(default = "yes")]
    pub wifi_menu: bool,
    /// The clipboard history in the menu bar.
    #[serde(default = "yes")]
    pub clipboard_menu: bool,
    pub keyboard: Keyboard,
    pub pointer: Pointer,
    pub touchpad: Touchpad,
    pub monitor: Monitor,
    pub shortcuts: Vec<Shortcut>,
    pub dock_apps: Vec<DockApp>,
    /// Comando para abrir a lixeira.
    pub trash_command: String,
    #[serde(default)]
    pub energy: Energy,
    #[serde(default)]
    pub hot_corners: HotCorners,
    /// O assistente de primeiro uso já foi visto. Configurações antigas (sem o
    /// campo) contam como vistas; só instalações novas abrem o assistente.
    #[serde(default = "yes")]
    pub welcome_done: bool,
    /// Porcentagem ao lado do ícone da bateria na barra de menu.
    #[serde(default)]
    pub battery_percent: bool,
    /// Versão da lista padrão do Dock já aplicada (para acrescentar apps novos).
    #[serde(default)]
    pub dock_rev: u32,
    /// Painéis na barra das Preferências (ao lado de "Mostrar Todos").
    #[serde(default = "default_prefs_favorites")]
    pub prefs_favorites: Vec<String>,
    /// Abrir apps X11 (liga o Xwayland, uns 20 MB a mais).
    #[serde(default = "yes")]
    pub x11_apps: bool,
    /// Troca rápida de usuário: o nome na ponta da barra de menu, com a lista de contas.
    #[serde(default)]
    pub user_menu: bool,
    #[serde(default)]
    pub desk: Desk,
    /// Preferências do Arquivos e Opções de Visualização das janelas.
    #[serde(default)]
    pub files: arquivos::Files,
    #[serde(default)]
    pub security: Security,
    #[serde(default)]
    pub search: SearchPrefs,
    #[serde(default)]
    pub media: Media,
    /// Quantas mesas virtuais (1 = só a mesa de sempre).
    pub desktops: u32,
}

/// Mesas virtuais: de 1 a este tanto.
pub const MAX_DESKTOPS: u32 = 9;

pub fn default_prefs_favorites() -> Vec<String> {
    ["displays", "dock", "keyboard"].map(String::from).to_vec()
}

/// Aumente quando a lista padrão do Dock ganhar apps novos.
pub const DOCK_REV: u32 = 10;

/// O item do Dock que abre a grade com todos os apps (o compositor a mostra; não é um programa).
pub const ALL_APPS_ID: &str = "glove.AllApps";
/// O papel de parede do Glove OS (a luva com o nome), onde o pacote o instalou
/// (`share/glove/wallpapers/glove-os.png`, ao lado da pasta dos programas). É o padrão.
pub fn glove_os_wallpaper() -> Option<String> {
    static P: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let mut c: Vec<PathBuf> = Vec::new();
        if let Some(bin) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
            c.push(bin.join("../share/glove/wallpapers/glove-os.png"));
        }
        c.push("/usr/share/glove/wallpapers/glove-os.png".into());
        c.push("/usr/local/share/glove/wallpapers/glove-os.png".into());
        c.into_iter().find(|p| p.exists()).and_then(|p| p.canonicalize().ok()).map(|p| p.to_string_lossy().into_owned())
    })
    .clone()
}

fn default_wallpaper() -> String {
    glove_os_wallpaper().unwrap_or_else(|| "waves".into())
}

fn default_removable() -> String {
    "open".into()
}

fn yes() -> bool {
    true
}

fn default_terminal() -> String {
    "glove-term".into()
}

fn app(name: &str, icon: &str, command: &str, app_id: &str) -> DockApp {
    DockApp { name: name.into(), icon: icon.into(), command: command.into(), app_id: app_id.into() }
}

pub fn default_shortcuts() -> Vec<Shortcut> {
    [
        ("terminal", "Super+Return"),
        ("close", "Super+Q"),
        ("minimize", "Super+M"),
        ("hide", "Super+H"),
        ("hide_others", "Super+Alt+H"),
        ("zoom", "Super+Up"),
        ("fullscreen", "Ctrl+Super+F"),
        ("cycle", "Super+Tab"),
        ("expose_all", "F9"),
        ("expose_app", "F10"),
        ("expose_desktop", "F11"),
        ("dashboard", "F12"),
        ("apps", "F4"),
        ("desk_prev", "Ctrl+Super+Left"),
        ("desk_next", "Ctrl+Super+Right"),
        ("desk_move_prev", "Ctrl+Super+Shift+Left"),
        ("desk_move_next", "Ctrl+Super+Shift+Right"),
        ("cycle_windows", "Alt+Tab"),
        ("preferences", "Super+Comma"),
        // As on the Mac (Command is the Super key); the Print key also works (the
        // compositor keeps Print and Shift+Print for them when nothing else uses them).
        ("screenshot", "Shift+Super+3"),
        ("screenshot_area", "Shift+Super+4"),
        ("clipboard_history", "Shift+Super+V"),
        ("record_screen", "Alt+Shift+Super+3"),
        ("record_area", "Alt+Shift+Super+4"),
        ("record_toggle", "Shift+Super+5"),
        ("force_quit", "Ctrl+Alt+Escape"),
        ("lock", "Super+L"),
        ("search", "Super+space"),
        ("input_next", "Super+Alt+space"),
        ("zoom_toggle", "Super+Alt+8"),
        ("zoom_in", "Super+Alt+equal"),
        ("zoom_out", "Super+Alt+minus"),
        ("invert_colors", "Ctrl+Super+Alt+8"),
        ("quit", "Ctrl+Alt+BackSpace"),
    ]
    .into_iter()
    .map(|(a, k)| Shortcut { action: a.into(), keys: k.into() })
    .collect()
}

/// The programs that had Portuguese names up to Glove 0.14: (then, now). Only used to bring
/// stored configuration forward (`Config::rename_old_names`).
const OLD_PROGRAMS: [(&str, &str); 23] = [
    ("glove-agenda", "glove-calendar"),
    ("glove-ajuda", "glove-help"),
    ("glove-arquivo", "glove-archive"),
    ("glove-boasvindas", "glove-welcome"),
    ("glove-central", "glove-appcenter"),
    ("glove-chaves", "glove-keychain"),
    ("glove-contatos", "glove-contacts"),
    ("glove-copias", "glove-backup"),
    ("glove-correio", "glove-mail"),
    ("glove-dicionario", "glove-dictionary"),
    ("glove-escolher", "glove-chooser"),
    ("glove-fontes", "glove-fonts"),
    ("glove-fotos", "glove-photos"),
    ("glove-gravacao", "glove-screenrec"),
    ("glove-gravador", "glove-voicerec"),
    ("glove-instalador", "glove-installer"),
    ("glove-mensagens", "glove-messages"),
    ("glove-musica", "glove-music"),
    ("glove-navegador", "glove-browser"),
    ("glove-registros", "glove-console"),
    ("glove-sair", "glove-logout"),
    ("glove-senha", "glove-auth"),
    ("glove-utilrede", "glove-netutil"),
];

/// The window ids that went with them: (then, now).
const OLD_WINDOW_IDS: [(&str, &str); 24] = [
    ("glove.Agenda", "glove.Calendar"),
    ("glove.Ajuda", "glove.Help"),
    ("glove.Arquivo", "glove.Archive"),
    ("glove.Boasvindas", "glove.Welcome"),
    ("glove.Central", "glove.AppCenter"),
    ("glove.Chaves", "glove.Keychain"),
    ("glove.Contatos", "glove.Contacts"),
    ("glove.Copias", "glove.Backup"),
    ("glove.Correio", "glove.Mail"),
    ("glove.Dicionario", "glove.Dictionary"),
    ("glove.Escolher", "glove.Chooser"),
    ("glove.Fontes", "glove.Fonts"),
    ("glove.Fotos", "glove.Photos"),
    ("glove.Grade", "glove.AllApps"),
    ("glove.Gravacao", "glove.ScreenRec"),
    ("glove.Gravador", "glove.VoiceRec"),
    ("glove.Instalador", "glove.Installer"),
    ("glove.Mensagens", "glove.Messages"),
    ("glove.Musica", "glove.Music"),
    ("glove.Navegador", "glove.Browser"),
    ("glove.Registros", "glove.Console"),
    ("glove.Sair", "glove.Logout"),
    ("glove.Senha", "glove.Auth"),
    ("glove.UtilRede", "glove.NetUtil"),
];

/// The appearances: (then, now).
const OLD_THEMES: [(&str, &str); 5] = [("claro", "light"), ("listras", "pinstripe"), ("grafite", "graphite"), ("moderno", "modern"), ("moderno-escuro", "modern-dark")];

/// What a hot corner does: (then, now).
const OLD_CORNERS: [(&str, &str); 4] = [("todas", "all"), ("mesa", "desktop"), ("painel", "dashboard"), ("protetor", "screensaver")];

/// The Preferences panes kept as favorites: (then, now).
const OLD_PANES: [(&str, &str); 22] = [
    ("mesa", "desktop"),
    ("telas", "system-screens"),
    ("aparencia", "appearance"),
    ("monitor", "displays"),
    ("som", "sound"),
    ("energia", "energy"),
    ("teclado", "keyboard"),
    ("rede", "network"),
    ("datahora", "date-time"),
    ("contas", "accounts"),
    ("inicio", "login-items"),
    ("internacional", "international"),
    ("acesso", "accessibility"),
    ("midia", "removable-media"),
    ("compartilhamento", "sharing"),
    ("padrao", "default-apps"),
    ("impressao", "printing"),
    ("atualizacao", "software-update"),
    ("inicializacao", "startup-disk"),
    ("fala", "speech"),
    ("seguranca", "security"),
    ("busca", "search"),
];

pub fn default_dock_apps() -> Vec<DockApp> {
    vec![
        app(tr!("Arquivos"), "files", "glove-files", "glove.Files"),
        app(tr!("Todos os Apps"), "apps-grid", "", ALL_APPS_ID),
        app("Terminal", "terminal", "glove-term", "glove.Terminal"),
        app("Jangada", "jangada", "glove-browser", "glove.Browser"),
        app(tr!("Correio"), "mail", "glove-mail", "glove.Mail"),
        app(tr!("Editor de Texto"), "editor", "glove-edit", "glove.TextEditor"),
        app(tr!("Lembretes"), "notes", "glove-notes", "glove.Notes"),
        app(tr!("Contatos"), "contacts", "glove-contacts", "glove.Contacts"),
        app(tr!("Agenda"), "calendar", "glove-calendar", "glove.Calendar"),
        app(tr!("Calculadora"), "calculator", "glove-calc", "glove.Calculator"),
        app(tr!("Relógio"), "clock", "glove-clock", "glove.Clock"),
        app(tr!("Visualizador"), "viewer", "glove-view", "glove.Viewer"),
        app(tr!("Música"), "music", "glove-music", "glove.Music"),
        app(tr!("Busca"), "search", "glove-find", "glove.Find"),
        app(tr!("Caracteres"), "chars", "glove-chars", "glove.Chars"),
        app(tr!("Captura"), "camera", "glove-shot", "glove.Shot"),
        app(tr!("Monitor"), "monitor", "glove-monitor", "glove.Monitor"),
        app(tr!("Discos"), "diskutil", "glove-disks", "glove.Disks"),
        app(tr!("Xadrez"), "chess", "glove-chess", "glove.Chess"),
        app(tr!("Preferências"), "settings", "glove-prefs", "glove.Preferences"),
    ]
}

/// Apps do Glove que não ficam no Dock de saída (acham-se na Busca e na pasta Aplicativos).
pub fn extra_apps() -> Vec<DockApp> {
    vec![
        app(tr!("Vídeo"), "video", "glove-video", "glove.Video"),
        app(tr!("Registros"), "logs", "glove-console", "glove.Console"),
        app(tr!("Informações do Sistema"), "info", "glove-info", "glove.Info"),
        app(tr!("Ajuda"), "help", "glove-help", "glove.Help"),
        app(tr!("Dicionário"), "dictionary", "glove-dictionary", "glove.Dictionary"),
        app(tr!("Livro de Fontes"), "fontbook", "glove-fonts", "glove.Fonts"),
        app(tr!("Acesso às Chaves"), "keychain", "glove-keychain", "glove.Keychain"),
        app(tr!("Utilitário de Rede"), "netutil", "glove-netutil", "glove.NetUtil"),
        app(tr!("Cópias de Segurança"), "backup", "glove-backup", "glove.Backup"),
        app(tr!("Central de Apps"), "appcenter", "glove-appcenter", "glove.AppCenter"),
        app(tr!("Mensagens"), "messages", "glove-messages", "glove.Messages"),
        app(tr!("Fotos"), "photos", "glove-photos", "glove.Photos"),
        app(tr!("Gravador"), "recorder", "glove-voicerec", "glove.VoiceRec"),
    ]
}

/// O app com esse app_id: o do Dock do usuário, senão um dos apps do Glove.
pub fn known_app(dock: &[DockApp], app_id: &str) -> Option<DockApp> {
    if app_id.is_empty() {
        return None;
    }
    dock.iter()
        .find(|a| a.app_id.eq_ignore_ascii_case(app_id))
        .cloned()
        .or_else(|| default_dock_apps().into_iter().chain(extra_apps()).find(|a| a.app_id.eq_ignore_ascii_case(app_id)))
        // O nome para mostrar, no idioma em uso (o gravado na configuração não muda).
        .map(|mut a| {
            a.name = tr_dyn(&a.name);
            a
        })
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "light".into(),
            wallpaper: default_wallpaper(),
            wallpaper_mode: "same".into(),
            wallpaper_fit: "fill".into(),
            wallpapers: Default::default(),
            wallpaper_anim: WallpaperAnim::default(),
            wallpaper_favorites: Vec::new(),
            terminal: default_terminal(),
            terminal_font: String::new(),
            terminal_font_size: 12.0,
            terminal_theme: "basic".into(),
            dock: Dock::default(),
            keyboard: Keyboard::default(),
            pointer: Pointer::default(),
            touchpad: Touchpad::default(),
            monitor: Monitor::default(),
            shortcuts: default_shortcuts(),
            dock_apps: default_dock_apps(),
            trash_command: "glove-files ~/.local/share/Trash/files".into(),
            battery_percent: false,
            energy: Energy::default(),
            hot_corners: HotCorners::default(),
            welcome_done: false,
            appearance: Appearance::default(),
            sound: Sound::default(),
            wifi_menu: true,
            clipboard_menu: true,
            clock: Clock::default(),
            access: Access::default(),
            saver: Saver::default(),
            removable_action: default_removable(),
            browser: String::new(),
            dock_rev: DOCK_REV,
            prefs_favorites: default_prefs_favorites(),
            x11_apps: true,
            user_menu: false,
            desk: Desk::default(),
            files: arquivos::Files::default(),
            security: Security::default(),
            search: SearchPrefs::default(),
            media: Media::default(),
            desktops: 4,
        }
    }
}

/// Moves what an older version stored under another name: when `old` exists and `new`
/// does not, `old` is renamed to `new` (the folder of `new` is created). Says whether it moved.
/// When both exist nothing is touched: the new one is the one in use.
pub fn move_stored(old: &std::path::Path, new: &std::path::Path) -> bool {
    if !old.exists() || new.exists() {
        return false;
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::rename(old, new).is_ok()
}

/// Today's name of a window id an older version stored ("glove.Agenda" → "glove.Calendar").
pub fn current_window_id(old: &str) -> Option<&'static str> {
    OLD_WINDOW_IDS.iter().find(|(o, _)| *o == old).map(|(_, new)| *new)
}

/// Today's name of a program an older version stored ("glove-agenda" → "glove-calendar").
pub fn current_program(old: &str) -> Option<&'static str> {
    OLD_PROGRAMS.iter().find(|(o, _)| *o == old).map(|(_, new)| *new)
}

pub fn dir() -> PathBuf {
    // A tela de login (glove --greeter) usa a configuração do sistema.
    if let Some(d) = std::env::var_os("GLOVE_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("glove")
}

pub fn path() -> PathBuf {
    dir().join("config.toml")
}

impl Config {
    /// Lê o arquivo; se não existir ou estiver inválido, usa o padrão.
    /// Na primeira execução, grava o padrão para o usuário poder editar.
    pub fn load() -> Self {
        match std::fs::read_to_string(path()) {
            Ok(s) => {
                let mut c: Config = toml::from_str(&s).unwrap_or_default();
                if c.dock_apps.is_empty() {
                    c.dock_apps = default_dock_apps();
                }
                if c.shortcuts.is_empty() {
                    c.shortcuts = default_shortcuts();
                }
                // First of all: the shortcuts and the rest are looked up by today's names.
                let renamed = c.rename_old_names();
                let added = c.add_missing_shortcuts();
                let gel = c.drop_removed_gel();
                if c.migrate() || added || gel || renamed {
                    let _ = c.save();
                }
                set_reduce_effects(c.appearance.reduce_effects);
                c
            }
            Err(_) => {
                let c = Self::default();
                let _ = c.save();
                c
            }
        }
    }

    /// Grava de forma atômica (arquivo temporário + renomear), para quem
    /// estiver observando a pasta nunca ler um arquivo pela metade.
    pub fn save(&self) -> std::io::Result<()> {
        let p = path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = p.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, p)
    }

    /// Ações novas ganham o atalho padrão em configurações antigas, e padrões
    /// que mudaram são trocados (só se o usuário não tiver mudado o atalho).
    pub fn add_missing_shortcuts(&mut self) -> bool {
        let mut added = false;
        // Alternar as janelas do mesmo app: era Ctrl+`, agora Alt+Tab.
        for s in self.shortcuts.iter_mut().filter(|s| s.action == "cycle_windows" && s.keys.eq_ignore_ascii_case("Ctrl+grave")) {
            s.keys = "Alt+Tab".into();
            added = true;
        }
        // Captures moved to Shift+Super+3 and 4 (as on the Mac) and the recordings that
        // were there to Alt+Shift+Super+3 and 4: only the shortcuts left as they came.
        for (action, old, new) in [
            ("record_screen", "Shift+Super+3", "Alt+Shift+Super+3"),
            ("record_area", "Shift+Super+4", "Alt+Shift+Super+4"),
            ("screenshot", "Print", "Shift+Super+3"),
            ("screenshot_area", "Shift+Print", "Shift+Super+4"),
        ] {
            for s in self.shortcuts.iter_mut().filter(|s| s.action == action && s.keys.eq_ignore_ascii_case(old)) {
                s.keys = new.into();
                added = true;
            }
        }
        for d in default_shortcuts() {
            if !self.shortcuts.iter().any(|s| s.action == d.action) {
                self.shortcuts.push(d);
                added = true;
            }
        }
        added
    }

    /// Atualiza configurações antigas: troca apps do GNOME pelos do Glove e
    /// acrescenta os apps novos do Dock. Devolve verdadeiro se mudou algo.
    /// Os papéis em gel (pacote glove-papeis) saíram do Glove OS: quem estava com um
    /// deles volta para o papel original, em vez de cair no desenhado.
    fn drop_removed_gel(&mut self) -> bool {
        let gone = |w: &str| (w.contains("/glove/papeis/gel/") || w.contains("/glove/wallpapers/gel/")) && !std::path::Path::new(w).exists();
        let mut changed = false;
        if gone(&self.wallpaper) {
            self.wallpaper = default_wallpaper();
            changed = true;
        }
        for w in self.wallpapers.values_mut() {
            if gone(w) {
                *w = default_wallpaper();
                changed = true;
            }
        }
        changed
    }

    /// Up to Glove 0.14 the programs, the window ids and the Preferences panes had Portuguese
    /// names. What the user's configuration still stores under those names (the Dock apps,
    /// the favorite panes) moves to the names of today. Returns whether anything changed.
    pub fn rename_old_names(&mut self) -> bool {
        let mut changed = false;
        for a in &mut self.dock_apps {
            if let Some((_, new)) = OLD_WINDOW_IDS.iter().find(|(old, _)| a.app_id == *old) {
                a.app_id = new.to_string();
                changed = true;
            }
            if a.icon == "chaves" {
                a.icon = "keychain".into();
                changed = true;
            }
            // The program is the first word of the command (maybe with a folder in front).
            let command: Vec<String> = a
                .command
                .split(' ')
                .enumerate()
                .map(|(i, word)| {
                    let (folder, name) = word.rsplit_once('/').map_or(("", word), |(f, n)| (f, n));
                    match OLD_PROGRAMS.iter().find(|(old, _)| i == 0 && name == *old) {
                        Some((_, new)) if folder.is_empty() => new.to_string(),
                        Some((_, new)) => format!("{folder}/{new}"),
                        None => word.to_string(),
                    }
                })
                .collect();
            let command = command.join(" ");
            if command != a.command {
                a.command = command;
                changed = true;
            }
        }
        for f in &mut self.prefs_favorites {
            if let Some((_, new)) = OLD_PANES.iter().find(|(old, _)| f == old) {
                *f = new.to_string();
                changed = true;
            }
        }
        // The stored choices that were Portuguese words up to Glove 0.15.
        let mut word = |value: &mut String, table: &[(&str, &str)]| {
            if let Some((_, new)) = table.iter().find(|(old, _)| value == old) {
                *value = new.to_string();
                changed = true;
            }
        };
        word(&mut self.theme, &OLD_THEMES);
        for screen in [&mut self.appearance.screen_session, &mut self.appearance.screen_login, &mut self.appearance.screen_boot] {
            word(screen, &OLD_THEMES);
            word(screen, &[("aparencia", "appearance")]);
        }
        word(&mut self.appearance.highlight, &[("azul", "blue"), ("grafite", "graphite"), ("prata", "silver"), ("dourado", "gold"), ("laranja", "orange"), ("roxo", "purple"), ("vermelho", "red"), ("verde", "green")]);
        word(&mut self.dock.position, &[("embaixo", "bottom"), ("esquerda", "left"), ("direita", "right")]);
        word(&mut self.dock.minimize_effect, &[("escala", "scale"), ("genio", "genie")]);
        word(&mut self.monitor.arrangement, &[("estender", "extend"), ("espelhar", "mirror"), ("um", "single")]);
        word(&mut self.monitor.side, &[("direita", "right"), ("esquerda", "left"), ("abaixo", "below")]);
        word(&mut self.saver.kind, &[("estrelas", "stars"), ("bolhas", "bubbles"), ("ondas", "waves"), ("relogio", "clock")]);
        word(&mut self.sound.alert, &[("Gota", "drop"), ("Sino", "bell"), ("Vidro", "glass"), ("Bolha", "bubble"), ("Toque", "tap"), ("Sopro", "blow")]);
        word(&mut self.terminal_theme, &[("basico", "basic"), ("oceano", "ocean"), ("grama", "grass"), ("novela", "novel"), ("areia", "red-sands"), ("noite", "night")]);
        word(&mut self.wallpaper_mode, &[("igual", "same"), ("por_tela", "per-display")]);
        word(&mut self.wallpaper_fit, &[("preencher", "fill"), ("ajustar", "fit"), ("esticar", "stretch"), ("centralizar", "center"), ("lado_a_lado", "tile")]);
        for corner in [&mut self.hot_corners.top_left, &mut self.hot_corners.top_right, &mut self.hot_corners.bottom_left, &mut self.hot_corners.bottom_right] {
            word(corner, &OLD_CORNERS);
        }
        for action in [&mut self.media.data_disc, &mut self.media.video_dvd, &mut self.removable_action] {
            word(action, &[("abrir", "open"), ("nada", "nothing")]);
        }
        let arrange = [("nome", "name"), ("data", "date"), ("tamanho", "size"), ("tipo", "kind")];
        word(&mut self.desk.arrange, &arrange);
        word(&mut self.files.arrange, &arrange);
        word(&mut self.files.new_window, &[("inicio", "home"), ("mesa", "desktop"), ("documentos", "documents"), ("computador", "computer")]);
        // The sidebar places hidden in Files, and the shortcut that opens All Apps.
        let places = [("disco", "disk"), ("mesa", "desktop"), ("inicio", "home"), ("aplicativos", "applications"), ("documentos", "documents"), ("imagens", "pictures"), ("musica", "music"), ("buscas", "smart-folders"), ("rede", "network")];
        for place in &mut self.files.sidebar_hidden {
            word(place, &places);
        }
        for shortcut in &mut self.shortcuts {
            word(&mut shortcut.action, &[("grade", "apps")]);
        }
        // A program built with an older kit may have put its own "grade" back next to
        // today's "apps": one of them is enough (the first, which is the one in use).
        let mut seen = false;
        let before = self.shortcuts.len();
        self.shortcuts.retain(|s| s.action != "apps" || !std::mem::replace(&mut seen, true));
        changed |= self.shortcuts.len() != before;
        // The wallpapers drawn by code and the plain colors ("cor:R,G,B").
        let mut wallpaper = |value: &mut String| {
            let new = match value.as_str() {
                "ondas" => "waves".to_string(),
                "liso" => "plain".to_string(),
                v => match v.strip_prefix("cor:") {
                    Some(rgb) => format!("color:{rgb}"),
                    // The wallpapers that come with Glove moved to share/glove/wallpapers.
                    None if v.contains("/share/glove/papeis/") => v.replacen("/share/glove/papeis/", "/share/glove/wallpapers/", 1),
                    None => return,
                },
            };
            *value = new;
            changed = true;
        };
        wallpaper(&mut self.wallpaper);
        self.wallpapers.values_mut().for_each(&mut wallpaper);
        self.wallpaper_favorites.iter_mut().for_each(&mut wallpaper);
        changed
    }

    pub fn migrate(&mut self) -> bool {
        if self.dock_rev >= DOCK_REV {
            return false;
        }
        let defaults = default_dock_apps();
        let replace = [
            ("org.gnome.Nautilus", "glove.Files"),
            ("org.gnome.Ptyxis", "glove.Terminal"),
            ("foot", "glove.Terminal"),
            ("org.gnome.TextEditor", "glove.TextEditor"),
            ("org.gnome.Calculator", "glove.Calculator"),
        ];
        for a in &mut self.dock_apps {
            // O Navegador virou a Jangada (o navegador do Glove), com o ícone dela.
            if a.app_id == "glove.Browser" && a.icon == "globe" {
                a.icon = "jangada".into();
                a.name = "Jangada".into();
            }
            // O Firefox direto (perfil do Glove) virou o glove-browser (NetSurf ou Firefox).
            if a.app_id == "firefox" && a.command.contains(".cache/glove/firefox") {
                if let Some(d) = defaults.iter().find(|d| d.app_id == "glove.Browser") {
                    *a = d.clone();
                }
            }
            if let Some((_, new_id)) = replace.iter().find(|(old, _)| a.app_id.eq_ignore_ascii_case(old)) {
                if let Some(d) = defaults.iter().find(|d| d.app_id == *new_id) {
                    *a = d.clone();
                }
            }
        }
        // Remove duplicados criados pela troca.
        let mut seen = Vec::new();
        self.dock_apps.retain(|a| {
            let dup = !a.app_id.is_empty() && seen.contains(&a.app_id);
            seen.push(a.app_id.clone());
            !dup
        });
        for d in defaults {
            if !self.dock_apps.iter().any(|a| a.app_id == d.app_id) {
                // A grade de apps vai logo depois do Arquivos, como no Dock novo.
                if d.app_id == ALL_APPS_ID {
                    let at = self.dock_apps.iter().position(|a| a.app_id == "glove.Files").map_or(0, |i| i + 1);
                    self.dock_apps.insert(at, d);
                } else {
                    self.dock_apps.push(d);
                }
            }
        }
        if self.terminal.contains("ptyxis") || self.terminal == "foot" {
            self.terminal = default_terminal();
        }
        if self.trash_command.contains("nautilus") || self.trash_command.is_empty() {
            self.trash_command = "glove-files ~/.local/share/Trash/files".into();
        }
        // O Arquivos ganhou ícone próprio (antes usava o da pasta).
        for a in &mut self.dock_apps {
            if a.app_id == "glove.Files" && a.icon == "folder" {
                a.icon = "files".into();
            }
            // Ícones próprios dos utilitários (antes reaproveitavam os de outros).
            for (id, old, new) in [("glove.Viewer", "picture", "viewer"), ("glove.Disks", "disk", "diskutil"), ("glove.NetUtil", "globe", "netutil")] {
                if a.app_id == id && a.icon == old {
                    a.icon = new.into();
                }
            }
        }
        self.dock_rev = DOCK_REV;
        true
    }

    pub fn shortcut(&self, action: &str) -> Option<&str> {
        self.shortcuts.iter().find(|s| s.action == action).map(|s| s.keys.as_str())
    }
}

/// Observa a pasta de configuração (inotify) sem acordar a CPU à toa.
/// Retorna o descritor; ele fica legível quando o arquivo muda.
/// Os ícones de cada aparência quando ela vale em todo o sistema: as modernas usam os do
/// Mide (o Moderno Claro, os "mide"; o Moderno Escuro, os "mide-dark"), as clássicas os
/// do Glove (vazio: o padrão). Quem chama confere se o tema de ícones está instalado.
pub fn icones_da_aparencia(tema: &str) -> &'static str {
    match tema {
        "modern" => "mide",
        "modern-dark" => "mide-dark",
        _ => "",
    }
}

pub fn watch_fd() -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    let d = dir();
    std::fs::create_dir_all(&d).ok()?;
    let cpath = std::ffi::CString::new(d.to_string_lossy().as_bytes()).ok()?;
    unsafe {
        let fd = libc_inotify::init();
        if fd < 0 {
            return None;
        }
        if libc_inotify::add_watch(fd, cpath.as_ptr()) < 0 {
            libc_inotify::close(fd);
            return None;
        }
        Some(std::os::fd::OwnedFd::from_raw_fd(fd))
    }
}

/// Esvazia os eventos pendentes do descritor de `watch_fd`.
/// Retorna verdadeiro se o config.toml foi tocado.
pub fn drain_watch(fd: &std::os::fd::OwnedFd) -> bool {
    use std::os::fd::AsRawFd;
    let mut buf = [0u8; 4096];
    let mut hit = false;
    loop {
        let n = unsafe { libc_inotify::read(fd.as_raw_fd(), buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }
        // Qualquer evento na pasta conta; o nome exato é verificado de forma simples.
        if buf[..n as usize].windows(11).any(|w| w == b"config.toml") {
            hit = true;
        }
    }
    hit
}

/// Chamadas mínimas ao inotify sem depender de outras bibliotecas.
mod libc_inotify {
    use std::os::raw::{c_char, c_int, c_void};
    const IN_NONBLOCK: c_int = 0o4000;
    const IN_CLOEXEC: c_int = 0o2000000;
    const IN_CLOSE_WRITE: u32 = 0x0000_0008;
    const IN_MOVED_TO: u32 = 0x0000_0080;
    const IN_CREATE: u32 = 0x0000_0100;
    unsafe extern "C" {
        fn inotify_init1(flags: c_int) -> c_int;
        fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;
        #[link_name = "read"]
        fn c_read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
        #[link_name = "close"]
        fn c_close(fd: c_int) -> c_int;
    }
    pub unsafe fn init() -> c_int {
        unsafe { inotify_init1(IN_NONBLOCK | IN_CLOEXEC) }
    }
    pub unsafe fn add_watch(fd: c_int, p: *const c_char) -> c_int {
        unsafe { inotify_add_watch(fd, p, IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE) }
    }
    pub unsafe fn read(fd: c_int, buf: *mut u8, n: usize) -> isize {
        unsafe { c_read(fd, buf as *mut c_void, n) }
    }
    pub unsafe fn close(fd: c_int) {
        unsafe {
            c_close(fd);
        }
    }
}

/// Faz o alocador devolver memória ao sistema assim que ela é liberada
/// (sem isso, o glibc guarda blocos grandes "para depois" e a RAM só cresce).
pub fn tune_memory() {
    unsafe {
        // Blocos acima de 64 KB vêm direto do sistema e voltam ao liberar.
        libc::mallopt(libc::M_MMAP_THRESHOLD, 64 * 1024);
        // Devolve sobras do topo do heap acima de 64 KB.
        libc::mallopt(libc::M_TRIM_THRESHOLD, 64 * 1024);
        // Uma arena só: programas do Glove são quase todos de uma thread.
        libc::mallopt(libc::M_ARENA_MAX, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capturas_como_no_mac() {
        let mut c = Config::default();
        c.shortcuts = vec![
            Shortcut { action: "screenshot".into(), keys: "Print".into() },
            Shortcut { action: "screenshot_area".into(), keys: "Shift+Print".into() },
            Shortcut { action: "record_screen".into(), keys: "Shift+Super+3".into() },
            Shortcut { action: "record_area".into(), keys: "Ctrl+F9".into() },
        ];
        assert!(c.add_missing_shortcuts());
        assert_eq!(c.shortcut("screenshot"), Some("Shift+Super+3"));
        assert_eq!(c.shortcut("screenshot_area"), Some("Shift+Super+4"));
        assert_eq!(c.shortcut("record_screen"), Some("Alt+Shift+Super+3"));
        // Changed by the user: stays.
        assert_eq!(c.shortcut("record_area"), Some("Ctrl+F9"));
    }

    #[test]
    fn old_names_in_stored_config() {
        let mut c = Config::default();
        c.dock_apps = vec![
            DockApp { name: "Música".into(), icon: "music".into(), command: "glove-musica".into(), app_id: "glove.Musica".into() },
            DockApp { name: "Jangada".into(), icon: "jangada".into(), command: "/usr/bin/glove-navegador https://exemplo.com".into(), app_id: "glove.Navegador".into() },
            DockApp { name: "Arquivos".into(), icon: "files".into(), command: "glove-files".into(), app_id: "glove.Files".into() },
            // A word that only looks like a program further along the command stays.
            DockApp { name: "Editor".into(), icon: "editor".into(), command: "glove-edit glove-sair".into(), app_id: "glove.TextEditor".into() },
            DockApp { name: "Todos os Apps".into(), icon: "apps-grid".into(), command: String::new(), app_id: "glove.Grade".into() },
            DockApp { name: "Chaves".into(), icon: "chaves".into(), command: "glove-chaves".into(), app_id: "glove.Chaves".into() },
        ];
        c.prefs_favorites = vec!["rede".into(), "dock".into(), "datahora".into()];
        assert!(c.rename_old_names());
        assert_eq!((c.dock_apps[0].command.as_str(), c.dock_apps[0].app_id.as_str()), ("glove-music", "glove.Music"));
        assert_eq!((c.dock_apps[1].command.as_str(), c.dock_apps[1].app_id.as_str()), ("/usr/bin/glove-browser https://exemplo.com", "glove.Browser"));
        assert_eq!(c.dock_apps[2].command, "glove-files");
        assert_eq!(c.dock_apps[3].command, "glove-edit glove-sair");
        assert_eq!(c.dock_apps[4].app_id, ALL_APPS_ID);
        assert_eq!((c.dock_apps[5].icon.as_str(), c.dock_apps[5].command.as_str(), c.dock_apps[5].app_id.as_str()), ("keychain", "glove-keychain", "glove.Keychain"));
        assert_eq!(c.prefs_favorites, ["network", "dock", "date-time"]);
        // Nothing left to rename the second time.
        assert!(!c.rename_old_names());
        // The stored choices that were Portuguese words.
        let mut v = Config::default();
        v.theme = "moderno-escuro".into();
        v.appearance.screen_login = "aparencia".into();
        v.appearance.highlight = "grafite".into();
        v.dock.position = "esquerda".into();
        v.monitor.arrangement = "espelhar".into();
        v.monitor.side = "abaixo".into();
        v.saver.kind = "estrelas".into();
        v.sound.alert = "Vidro".into();
        v.terminal_theme = "basico".into();
        v.wallpaper = "cor:10,20,30".into();
        v.wallpaper_mode = "por_tela".into();
        v.wallpaper_fit = "lado_a_lado".into();
        v.wallpapers.insert("eDP-1".into(), "ondas".into());
        v.wallpapers.insert("HDMI-A-1".into(), "/home/x/foto.jpg".into());
        v.wallpapers.insert("DP-1".into(), "/usr/share/glove/papeis/glove-os.png".into());
        v.wallpaper_favorites = vec!["/usr/local/share/glove/papeis/glove-os.png".into(), "/home/x/papeis/a.png".into()];
        v.hot_corners.top_left = "todas".into();
        v.hot_corners.bottom_right = "protetor".into();
        v.media.data_disc = "nada".into();
        v.removable_action = "abrir".into();
        v.desk.arrange = "tamanho".into();
        v.files.new_window = "documentos".into();
        v.files.sidebar_hidden = vec!["musica".into(), "downloads".into(), "buscas".into()];
        v.shortcuts = vec![Shortcut { action: "grade".into(), keys: "F4".into() }];
        assert!(v.rename_old_names());
        assert_eq!(v.files.sidebar_hidden, ["music", "downloads", "smart-folders"]);
        // The old name of the shortcut becomes today's, so no second one is added for it.
        assert_eq!(v.shortcuts[0].action, "apps");
        v.add_missing_shortcuts();
        assert_eq!(v.shortcuts.iter().filter(|s| s.action == "apps").count(), 1);
        // And an old one that came back beside today's goes away.
        v.shortcuts.push(Shortcut { action: "grade".into(), keys: "F4".into() });
        assert!(v.rename_old_names());
        assert_eq!(v.shortcuts.iter().filter(|s| s.action == "apps" || s.action == "grade").count(), 1);
        assert_eq!((v.theme.as_str(), v.appearance.screen_login.as_str(), v.appearance.highlight.as_str()), ("modern-dark", "appearance", "graphite"));
        assert_eq!((v.dock.position.as_str(), v.monitor.arrangement.as_str(), v.monitor.side.as_str()), ("left", "mirror", "below"));
        assert_eq!((v.saver.kind.as_str(), v.sound.alert.as_str(), v.terminal_theme.as_str()), ("stars", "glass", "basic"));
        assert_eq!((v.wallpaper.as_str(), v.wallpaper_mode.as_str(), v.wallpaper_fit.as_str()), ("color:10,20,30", "per-display", "tile"));
        assert_eq!((v.wallpapers["eDP-1"].as_str(), v.wallpapers["HDMI-A-1"].as_str()), ("waves", "/home/x/foto.jpg"));
        assert_eq!(v.wallpapers["DP-1"], "/usr/share/glove/wallpapers/glove-os.png");
        assert_eq!(v.wallpaper_favorites, ["/usr/local/share/glove/wallpapers/glove-os.png", "/home/x/papeis/a.png"]);
        assert_eq!((v.hot_corners.top_left.as_str(), v.hot_corners.bottom_right.as_str()), ("all", "screensaver"));
        assert_eq!((v.media.data_disc.as_str(), v.removable_action.as_str(), v.desk.arrange.as_str(), v.files.new_window.as_str()), ("nothing", "open", "size", "documents"));
        assert!(!v.rename_old_names());
        // Every default app and favorite pane already has today's name.
        let mut d = Config::default();
        d.dock_apps = default_dock_apps().into_iter().chain(extra_apps()).collect();
        assert!(!d.rename_old_names());
    }

    #[test]
    fn migra_config_antiga() {
        let old = r#"
theme = "metal"
terminal = "ptyxis --standalone"
trash_command = "dbus-run-session -- nautilus --new-window trash:///"
[[dock_apps]]
name = "Arquivos"
icon = "folder"
command = "dbus-run-session -- nautilus --new-window"
app_id = "org.gnome.Nautilus"
[[dock_apps]]
name = "Meu app"
icon = "x"
command = "meuapp"
app_id = "meu.app"
"#;
        let mut c: Config = toml::from_str(old).unwrap();
        assert!(c.migrate());
        assert_eq!(c.theme, "metal");
        assert_eq!(c.terminal, "glove-term");
        assert_eq!(c.dock_apps[0].app_id, "glove.Files");
        assert!(c.dock_apps.iter().any(|a| a.app_id == "meu.app"));
        assert!(c.dock_apps.iter().any(|a| a.app_id == "glove.Chess"));
        assert!(!c.migrate());
    }
}

#[cfg(test)]
mod monitor_tests {
    use super::*;

    #[test]
    fn escala_automatica() {
        // Notebook 13,5" 2256x1504 (≈201 ppp) -> 1,5.
        assert_eq!(auto_scale((2256, 1504), (285, 190)), 1.5);
        // 15,6" Full HD (≈141 ppp) -> 1.
        assert_eq!(auto_scale((1920, 1080), (344, 194)), 1.0);
        // 27" 4K (≈163 ppp) -> 1,25.
        assert_eq!(auto_scale((3840, 2160), (597, 336)), 1.25);
        // Sem tamanho físico.
        assert_eq!(auto_scale((1920, 1080), (0, 0)), 1.0);
        assert_eq!(parse_mode("1920x1080@60"), Some((1920, 1080, 60)));
        assert_eq!(parse_mode("1280x800"), Some((1280, 800, 0)));
    }
}

/// Apps do Glove que abrem arquivos: (programa, nome, ícone do sistema, tipos MIME).
const FILE_APPS: &[(&str, &str, &str, &str)] = &[
    // Pastas: o Arquivos (o "abrir pasta" dos apps de fora, como o Firefox).
    ("glove-files", "Arquivos", "system-file-manager", "inode/directory;"),
    ("glove-view", "Visualizador", "image-x-generic", "image/png;image/jpeg;image/pjpeg;image/webp;application/pdf;"),
    ("glove-edit", "Editor de Texto", "accessories-text-editor", "text/plain;text/markdown;text/x-csrc;text/x-chdr;text/x-python;text/x-shellscript;application/x-shellscript;text/x-rust;application/json;application/toml;application/xml;text/xml;text/css;text/html;"),
    ("glove-video", "Vídeo", "video-x-generic", "video/mp4;video/x-matroska;video/webm;video/quicktime;video/x-msvideo;video/mpeg;video/ogg;video/x-flv;video/3gpp;"),
    ("glove-fonts", "Livro de Fontes", "font-x-generic", "font/ttf;font/otf;font/collection;application/x-font-ttf;application/x-font-otf;"),
    ("glove-music", "Música", "audio-x-generic", "audio/mpeg;audio/flac;audio/ogg;audio/x-wav;audio/wav;audio/x-vorbis+ogg;"),
];

/// Registra os apps do Glove que abrem arquivos em `~/.local/share/applications`
/// (`glove-*.desktop`, fora dos menus): assim aparecem em "Abrir com" e podem ser o
/// app padrão de um tipo. `bin_dir` é a pasta dos programas. Na primeira vez, cada
/// app vira o padrão dos tipos que ainda não têm um. Só grava o que mudou.
pub fn register_file_apps(bin_dir: &std::path::Path) {
    let Some(home) = std::env::var_os("HOME") else { return };
    let dir = std::path::PathBuf::from(home).join(".local/share/applications");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let mut changed = false;
    let mut fresh = Vec::new();
    // Up to Glove 0.14 two of these programs had Portuguese names: their entries go away,
    // and the types that had them as the default app move to today's names.
    for (old, new) in [("glove-fontes", "glove-fonts"), ("glove-musica", "glove-music")] {
        changed |= std::fs::remove_file(dir.join(format!("{old}.desktop"))).is_ok();
        for list in [dir.join("mimeapps.list"), dir.join("../../../.config/mimeapps.list")] {
            if let Ok(text) = std::fs::read_to_string(&list) {
                let renamed = text.replace(&format!("{old}.desktop"), &format!("{new}.desktop"));
                if renamed != text {
                    changed |= std::fs::write(&list, renamed).is_ok();
                }
            }
        }
    }
    for (bin, name, icon, mimes) in FILE_APPS {
        let exe = bin_dir.join(bin);
        if !exe.is_file() {
            continue;
        }
        let path = dir.join(format!("{bin}.desktop"));
        let text = format!(
            "[Desktop Entry]\n# Generated by Glove (apps that open files).\nType=Application\nName={name}\nIcon={icon}\nExec=\"{}\" %f\nMimeType={mimes}\nNoDisplay=true\nTerminal=false\nOnlyShowIn=Glove;\n",
            exe.display()
        );
        let old = std::fs::read_to_string(&path).ok();
        if old.as_deref() != Some(text.as_str()) {
            if old.is_none() {
                fresh.push((*bin, *mimes));
            }
            changed = std::fs::write(&path, text).is_ok() || changed;
        }
    }
    if !changed {
        return;
    }
    std::thread::spawn(move || {
        let _ = std::process::Command::new("update-desktop-database").arg(&dir).status();
        for (bin, mimes) in fresh {
            for m in mimes.split(';').filter(|m| !m.is_empty()) {
                let def = std::process::Command::new("xdg-mime").args(["query", "default", m]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
                if def.is_empty() {
                    let _ = std::process::Command::new("xdg-mime").args(["default", &format!("{bin}.desktop"), m]).status();
                }
            }
        }
    });
}

/// Tema de cores do Terminal: fundo, texto, cursor, seleção e as 16 cores ANSI.
#[derive(Clone, Debug, PartialEq)]
pub struct TermTheme {
    pub id: &'static str,
    pub name: &'static str,
    pub bg: (u8, u8, u8),
    pub fg: (u8, u8, u8),
    pub cursor: (u8, u8, u8),
    pub palette: [(u8, u8, u8); 16],
}

/// Cores ANSI para fundo claro e para fundo escuro.
const ANSI_LIGHT: [(u8, u8, u8); 16] = [
    (0, 0, 0), (194, 54, 33), (37, 160, 38), (173, 140, 0), (40, 70, 200), (176, 50, 176), (20, 150, 170), (190, 190, 190),
    (102, 102, 102), (235, 70, 50), (49, 200, 50), (210, 180, 0), (80, 110, 240), (225, 70, 225), (40, 200, 220), (255, 255, 255),
];
const ANSI_DARK: [(u8, u8, u8); 16] = [
    (0, 0, 0), (205, 60, 50), (60, 190, 60), (205, 175, 40), (75, 125, 235), (200, 90, 200), (60, 185, 200), (200, 200, 200),
    (110, 110, 110), (245, 95, 80), (90, 225, 90), (240, 215, 80), (120, 160, 255), (235, 120, 235), (100, 225, 235), (255, 255, 255),
];

/// Temas do Terminal (inspirados nos do Terminal da era, com cores próprias).
pub fn term_themes() -> Vec<TermTheme> {
    let t = |id, name, bg, fg, cursor, palette| TermTheme { id, name, bg, fg, cursor, palette };
    vec![
        t("basic", "Básico", (255, 255, 255), (0, 0, 0), (60, 60, 60), ANSI_LIGHT),
        t("pro", "Pro", (14, 14, 16), (230, 230, 230), (200, 200, 200), ANSI_DARK),
        t("homebrew", "Homebrew", (0, 0, 0), (40, 254, 20), (40, 254, 20), ANSI_DARK),
        t("ocean", "Oceano", (34, 79, 188), (255, 255, 255), (220, 230, 255), ANSI_DARK),
        t("grass", "Grama", (19, 119, 61), (255, 240, 165), (142, 40, 0), ANSI_DARK),
        t("novel", "Novela", (223, 219, 195), (59, 35, 34), (58, 35, 34), ANSI_LIGHT),
        t("red-sands", "Areia Vermelha", (122, 37, 30), (215, 201, 167), (255, 255, 255), ANSI_DARK),
        t("manual", "Manual", (254, 244, 156), (0, 0, 0), (127, 127, 127), ANSI_LIGHT),
        t("night", "Noite", (30, 30, 46), (205, 214, 244), (245, 224, 220), ANSI_DARK),
    ]
}

/// O tema pelo nome (o Básico se não existir).
pub fn term_theme(id: &str) -> TermTheme {
    let all = term_themes();
    all.iter().find(|t| t.id == id).cloned().unwrap_or_else(|| all[0].clone())
}

/// Nome do ícone de um app pelo app_id: o dos apps do Glove (ou do Dock); para os
/// de fora, o Icon= do .desktop dele; senão "window".
pub fn app_icon_name(dock: &[DockApp], app_id: &str) -> String {
    // Utilitários que não aparecem na Busca, mas têm janela (e ícone próprio).
    if app_id == "glove.Archive" {
        return "archiver".into();
    }
    // A janela de senha do sistema (glove-auth): o chaveiro.
    if app_id == "glove.Auth" {
        return "keychain".into();
    }
    match known_app(dock, app_id) {
        Some(a) if a.icon != "window" && a.icon != "generic" && !a.icon.is_empty() => a.icon,
        _ => desktop_icon(app_id).unwrap_or_else(|| "window".into()),
    }
}
