//! Janela Wayland do kit: escala fracionada (nítido em 1.5×), mouse, teclado,
//! e redesenho só quando há evento (zero CPU parado).

use std::os::fd::AsRawFd;
use std::time::Duration;

use glove_paint::{draw::Canvas, Fonts, Theme, Variant};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    data_device_manager::{
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        data_source::{CopyPasteSource, DataSourceHandler, DragSource},
        DataDeviceManagerState, WritePipe,
    },
    delegate_registry,
    dispatch2::Dispatch2,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            generic::Generic,
            timer::{TimeoutAction, Timer},
            EventLoop, Interest, LoopHandle, Mode, PostAction,
        },
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
            Connection, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            popup::{Popup, PopupConfigure, PopupHandler},
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgPositioner, XdgShell, XdgSurface,
        },
        WaylandSurface,
    },
    shm::{slot::{Buffer, SlotPool}, Shm, ShmHandler},
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, wp_fractional_scale_v1::{self, WpFractionalScaleV1}},
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use crate::ui::{Input, Key, KeyPress, Memory, Ui};

/// Um app do Glove: descreve a interface a cada quadro.
pub trait App: 'static {
    fn ui(&mut self, ui: &mut Ui);
    /// Chamado quando a configuração do Glove muda (tema etc.).
    fn config_changed(&mut self) {}
    /// Se devolver um intervalo, a janela é redesenhada nesse ritmo (ex.: relógio).
    /// Sem isso, nada acontece enquanto não houver evento.
    fn tick(&self) -> Option<Duration> {
        None
    }
    /// Chamado a cada batida do timer de `tick`, antes de redesenhar.
    fn on_tick(&mut self) {}
    /// Depois de `on_tick`: as janelas precisam ser redesenhadas? (Padrão: sim.) Apps
    /// com batida rápida (navegador) evitam refazer a janela quando nada mudou.
    fn tick_redraw(&mut self) -> bool {
        true
    }
    /// O usuário pediu para fechar a janela. Devolva `false` para impedir
    /// (ex.: perguntar se quer salvar) e feche depois com `ui.close = true`.
    fn close_requested(&mut self) -> bool {
        true
    }
    /// O botão de fechar da janela `id` (apps com várias janelas). Devolva
    /// verdadeiro para fechar. Padrão: o mesmo que `close_requested`.
    fn close_window(&mut self, id: u32) -> bool {
        let _ = id;
        self.close_requested()
    }
    /// A janela `id` foi fechada (pelo botão ou por `ui.close`).
    fn window_closed(&mut self, id: u32) {
        let _ = id;
    }
    /// Descritor que o kit deve observar (ex.: saída do shell no terminal).
    fn watch_fd(&self) -> Option<std::os::fd::RawFd> {
        None
    }
    /// O descritor de `watch_fd` tem dados. Devolva `false` para parar de observar.
    fn fd_ready(&mut self) -> bool {
        false
    }
    /// Um descritor observado (o de `watch_fd` ou um de `ui.watch_fd`) tem dados.
    /// Padrão: `fd_ready`. Devolva `false` para parar de observar este.
    fn fd_ready_on(&mut self, fd: std::os::fd::RawFd) -> bool {
        let _ = fd;
        self.fd_ready()
    }
    /// Menus do app para a barra de menu global (enviados só quando mudam).
    fn menus(&self) -> Vec<crate::menu::Menu> {
        Vec::new()
    }
    /// O app pinta o fundo inteiro sozinho (ex.: a partir de um cache) e o kit
    /// pode pular o fundo listrado padrão.
    fn paints_background(&self) -> bool {
        false
    }
    /// Backend GTK: uma tecla apertada com o teclado num widget do sistema da janela
    /// (`Ui::native_host`), no formato dos atalhos ("Ctrl+T"). Devolva verdadeiro para
    /// ficar com ela (o widget não a recebe); a janela é redesenhada.
    fn native_key(&mut self, combo: &str) -> bool {
        let _ = combo;
        false
    }
}

/// Hora local: (ano, mês 1-12, dia, dia da semana 0=domingo, hora, minuto, segundo).
pub fn local_time() -> (i32, u32, u32, u32, u32, u32, u32) {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_year + 1900, tm.tm_mon as u32 + 1, tm.tm_mday as u32, tm.tm_wday as u32, tm.tm_hour as u32, tm.tm_min as u32, tm.tm_sec as u32)
    }
}

#[derive(Clone, Debug)]
pub struct WindowOptions {
    pub title: String,
    pub app_id: String,
    pub size: (u32, u32),
    /// Tamanho fixo (sem zoom nem redimensionar).
    pub fixed: bool,
    /// Menor tamanho ao redimensionar (o que cabe a barra de ferramentas sem sobrepor).
    /// Sem ele, a janela não fica menor que 3/4 do tamanho inicial.
    pub min_size: Option<(u32, u32)>,
}

/// Ícone do app pelo app_id (a mesma lista do Dock).
pub fn app_icon(app_id: &str) -> glove_paint::icons::Icon {
    glove_paint::icons::Icon::from_name(&glove_config::app_icon_name(&[], app_id))
}

pub fn theme_from_config() -> Theme {
    let c = glove_config::Config::load();
    glove_paint::icons::set_theme(&c.appearance.icon_theme);
    let mut t = Theme::new(glove_paint::theme::visual().apply(Variant::from_config(&c.theme)));
    glove_paint::theme::set_style(&t);
    let (r, g, b) = glove_config::highlight_rgb(&c.appearance.highlight);
    t.highlight = glove_paint::draw::rgba(r, g, b, 255);
    // Ajustes que o kit lê a cada clique (clique duplo, trilho da rolagem).
    DOUBLE_CLICK_MS.store(c.pointer.double_click_ms.clamp(150, 1500), std::sync::atomic::Ordering::Relaxed);
    SCROLL_JUMP.store(c.appearance.scroll_jump, std::sync::atomic::Ordering::Relaxed);
    t
}

/// Tempo do clique duplo (Preferências ▸ Mouse).
pub(crate) static DOUBLE_CLICK_MS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(400);
/// Clicar no trilho da barra de rolagem vai até ali (Preferências ▸ Aparência).
pub(crate) static SCROLL_JUMP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Estado de uma janela. Um app pode ter várias (ex.: Lembretes); a que está
/// recebendo o evento ou sendo desenhada fica em `Runner::w` e as outras em
/// `Runner::others` (trocadas de lugar quando o evento é de outra).
struct Win {
    /// Número dado pelo app (0 = a primeira janela).
    id: u32,
    window: Window,
    viewport: Option<WpViewport>,
    _frac: Option<WpFractionalScaleV1>,
    /// Escala fracionada vinda do compositor (ou inteira, sem o protocolo).
    scale: f64,
    int_scale: i32,
    width: u32,
    height: u32,
    fixed: bool,
    configured: bool,
    dirty: bool,
    /// Só uma animação pediu o quadro (pode ser redesenho parcial).
    anim_dirty: bool,
    /// Um configure foi confirmado: o compositor só aplica o novo estado
    /// (bordas, tamanho) no próximo commit, mesmo que nenhum pixel mude.
    need_commit: bool,
    frame_pending: bool,
    input: Input,
    mem: Memory,
    /// Buffers reaproveitados entre quadros, cada um com o número do quadro que
    /// guarda (só cria outro se o compositor ainda estiver usando todos).
    buffers: Vec<(Buffer, u64)>,
    /// Número do último quadro enviado e as áreas que mudaram nos últimos quadros
    /// (um buffer antigo recebe só o que mudou desde o quadro que ele guarda).
    frame_no: u64,
    history: std::collections::VecDeque<(u64, Vec<[i32; 4]>)>,
    /// Cópia do último quadro enviado, para achar o que mudou.
    prev: Vec<u8>,
    prev_key: (u32, u32, u32),
    /// Áreas cruzadas pelo mouse que já se viu não mudarem nada na tela (só clique):
    /// cruzá-las de novo não redesenha.
    silent: std::collections::HashSet<[u32; 4]>,
    /// O quadro pedido só porque o mouse cruzou estas áreas (para aprender as mudas).
    probe: Option<Vec<[u32; 4]>>,
    /// O cursor do mouse mudou neste quadro (a área cruzada não é muda).
    cursor_moved: bool,
    /// Canvas reaproveitado enquanto o tamanho/escala não mudam.
    canvas: Option<Canvas>,
    /// Operações de desenho do quadro que está no canvas (redesenho parcial).
    ops: Vec<glove_paint::draw::Op>,
    ops_valid: bool,
    /// Áreas pintadas fora das operações anotadas (animações): entram no próximo quadro.
    forced: Vec<[i32; 4]>,
    /// Já há um timer de "acordar" pendente (ui.wake_after).
    wake_pending: bool,
    /// Mudança de tamanho pedida pelo app, animada: (de, para, início, duração).
    size_anim: Option<((u32, u32), (u32, u32), std::time::Instant, Duration)>,
    /// Mínimo pedido pelo app com `set_window_min_size` (o último enviado ao compositor).
    min: Option<(u32, u32)>,
    /// O que o usuário aumentou além do mínimo, por eixo (sem valor = acompanha o mínimo).
    chosen: (Option<u32>, Option<u32>),
    /// Último tamanho que o compositor pediu.
    cfg_size: Option<(u32, u32)>,
    /// O usuário está arrastando a borda; o tamanho no começo do arraste e os eixos em
    /// que ele já mexeu (só esses contam como escolhidos).
    resizing: bool,
    drag_base: (u32, u32),
    touched: (bool, bool),
    /// Algum quadro já foi para a tela (antes disso o tamanho muda sem animar).
    shown: bool,
    // Clique duplo/triplo
    last_click: (u32, (f32, f32), u8),
    app_id: String,
    /// Janela de alerta aberta pelo glove-ui (não pelo app): (janela dona, o alerta).
    alert_of: Option<(u32, crate::ui::AlertReq)>,
    /// Open or Save in a window of its own (opened by glove-ui): (owner, request, browser).
    file_of: Option<(u32, u64, crate::filebrowser::FileBrowser)>,
}

/// O que o usuário aumentou além do mínimo, nos eixos em que ele mexeu (`touched`); os
/// outros ficam como estavam. O eixo que voltou ao mínimo acompanha o mínimo (sem valor).
fn chosen_size(prev: (Option<u32>, Option<u32>), touched: (bool, bool), size: (u32, u32), min: (u32, u32)) -> (Option<u32>, Option<u32>) {
    (if touched.0 { (size.0 > min.0).then_some(size.0) } else { prev.0 }, if touched.1 { (size.1 > min.1).then_some(size.1) } else { prev.1 })
}

/// Tamanho da janela para um mínimo: o que o usuário escolheu, nunca abaixo do mínimo.
fn floor_size(chosen: (Option<u32>, Option<u32>), min: (u32, u32)) -> (u32, u32) {
    (chosen.0.map_or(min.0, |c| c.max(min.0)), chosen.1.map_or(min.1, |c| c.max(min.1)))
}

/// Janelas sem a barra de título do Glove: o app desenha a própria moldura
/// (ex.: Lembretes) e usa `ui.start_move()` para arrastar.
pub(crate) static OWN_FRAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Chame antes de `run` para o app desenhar a própria moldura.
pub fn use_own_frame() {
    OWN_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Só a primeira janela (número 0) sem a barra de título do Glove: o app põe os botões
/// da janela na barra de ferramentas (`Ui::window_buttons`) e arrasta por ela
/// (`Ui::toolbar_drag`). As outras janelas (Preferências etc.) seguem com a barra.
pub(crate) static OWN_FRAME_MAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Chame antes de `run`: a janela principal desenha a própria moldura.
pub fn use_own_frame_main() {
    OWN_FRAME_MAIN.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// O que é preciso para abrir janelas novas depois da primeira.
struct Shells {
    compositor: CompositorState,
    xdg: XdgShell,
    frac_mgr: Option<WpFractionalScaleManagerV1>,
    viewporter: Option<WpViewporter>,
}

impl Shells {
    fn open<A: App>(&self, qh: &QueueHandle<Runner<A>>, id: u32, opts: &WindowOptions) -> Win {
        self.open_with(qh, id, opts, false)
    }

    /// `bare`: sem a barra de título do Glove (a janela do alerta desenha tudo).
    fn open_with<A: App>(&self, qh: &QueueHandle<Runner<A>>, id: u32, opts: &WindowOptions, bare: bool) -> Win {
        let surface = self.compositor.create_surface(qh);
        let (viewport, frac) = match (&self.frac_mgr, &self.viewporter) {
            (Some(m), Some(v)) => (Some(v.get_viewport(&surface, qh, NoEvents)), Some(m.get_fractional_scale(&surface, qh, FracData))),
            _ => (None, None),
        };
        let own = OWN_FRAME.load(std::sync::atomic::Ordering::Relaxed) || (id == 0 && OWN_FRAME_MAIN.load(std::sync::atomic::Ordering::Relaxed));
        let deco = if bare || own { WindowDecorations::RequestClient } else { WindowDecorations::RequestServer };
        let window = self.xdg.create_window(surface, deco, qh);
        window.set_title(opts.title.clone());
        window.set_app_id(opts.app_id.clone());
        if opts.fixed {
            window.set_min_size(Some(opts.size));
            window.set_max_size(Some(opts.size));
        } else {
            let min = opts.min_size.unwrap_or((opts.size.0 * 3 / 4, opts.size.1 * 3 / 4));
            window.set_min_size(Some((min.0.min(opts.size.0), min.1.min(opts.size.1))));
        }
        window.commit();
        let mut mem = Memory::with_app_icon(app_icon(&opts.app_id));
        mem.window_id = id;
        // GLOVE_POPUP_IN_WINDOW=1: a lista suspensa desenhada dentro da janela (o jeito antigo).
        mem.native_popups = std::env::var_os("GLOVE_POPUP_IN_WINDOW").is_none();
        // GLOVE_ALERT_SHEET=1: os alertas como folha dentro da janela (o jeito antigo).
        mem.alert_native = crate::ui::native_menus() && std::env::var_os("GLOVE_ALERT_SHEET").is_none();
        // GLOVE_FILE_SHEET=1: Open and Save as a sheet inside the window (the old way).
        mem.file_native = mem.alert_native && std::env::var_os("GLOVE_FILE_SHEET").is_none();
        Win {
            id,
            window,
            viewport,
            _frac: frac,
            scale: 1.0,
            int_scale: 1,
            width: opts.size.0,
            height: opts.size.1,
            fixed: opts.fixed,
            configured: false,
            dirty: true,
            anim_dirty: false,
            need_commit: false,
            frame_pending: false,
            input: Input::default(),
            mem,
            buffers: Vec::new(),
            frame_no: 0,
            history: std::collections::VecDeque::new(),
            prev: Vec::new(),
            prev_key: (0, 0, 0),
            silent: Default::default(),
            probe: None,
            cursor_moved: false,
            canvas: None,
            ops: Vec::new(),
            ops_valid: false,
            forced: Vec::new(),
            wake_pending: false,
            size_anim: None,
            min: None,
            chosen: (None, None),
            cfg_size: None,
            resizing: false,
            drag_base: (0, 0),
            touched: (false, false),
            shown: false,
            last_click: (0, (0.0, 0.0), 0),
            app_id: opts.app_id.clone(),
            alert_of: None,
            file_of: None,
        }
    }
}

struct Runner<A: App> {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    /// A janela atual e as outras do app.
    w: Win,
    others: Vec<Win>,
    shells: Shells,
    /// Superfície com o foco do teclado.
    kbd_surface: Option<wl_surface::WlSurface>,
    exit: bool,
    fonts: Fonts,
    doc_fonts: Fonts,
    theme: Theme,
    app: A,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    /// O ponteiro com os formatos de cursor (seta, mãozinha, barra de texto) e o que
    /// está valendo agora (`None` = mandar de novo, ex.: o mouse acabou de entrar).
    themed: Option<smithay_client_toolkit::seat::pointer::ThemedPointer>,
    cursor_on: Option<crate::ui::Cursor>,
    conn: Connection,
    loop_handle: LoopHandle<'static, Runner<A>>,
    qh: QueueHandle<Runner<A>>,
    /// O relógio do `App::tick` está rodando (para de rodar quando o app devolve None).
    tick_active: std::rc::Rc<std::cell::Cell<bool>>,
    // Área de transferência
    data_manager: Option<DataDeviceManagerState>,
    data_device: Option<DataDevice>,
    clip_source: Option<CopyPasteSource>,
    clip_text: String,
    /// Arquivos copiados (e se foi "recortar"): a seleção é de arquivos.
    clip_files: Option<(Vec<std::path::PathBuf>, bool)>,
    /// Arrastando arquivos para fora: a fonte (viva até o fim) e os arquivos.
    drag_source: Option<DragSource>,
    drag_files: Vec<std::path::PathBuf>,
    drag_icon: Option<wl_surface::WlSurface>,
    /// Janela por onde algo está sendo arrastado.
    dnd_win: Option<u32>,
    /// Último "serial" de clique/tecla (o compositor exige para copiar).
    last_serial: u32,
    /// Serial do último botão do mouse apertado: arrastar exige o do clique
    /// (o de uma tecla apertada no meio faria o compositor recusar).
    press_serial: u32,
    seat: Option<wl_seat::WlSeat>,
    /// Lista suspensa aberta numa superfície própria (no máximo uma por vez).
    list_popup: Option<ListPopup>,
    /// Respostas de alertas escolhidas neste quadro: (janela dona, alerta, botão).
    alert_done: Vec<(u32, u64, usize)>,
    /// The choices of Open/Save windows going back to their owners.
    file_done: Vec<(u32, u64, crate::filebrowser::Choice)>,
    /// O timer da rolagem pelas setas da lista está rodando.
    list_scroll_timer: bool,
    // Menus na barra global
    menu_stream: Option<std::os::unix::net::UnixStream>,
    menu_sent: String,
    menu_shortcuts: std::collections::HashMap<String, String>,
}

/// Folga transparente em volta da lista suspensa para a sombra (esquerda, direita,
/// embaixo); a geometria da janela do popup é só a lista.
const LIST_SHADOW: (f32, f32, f32) = (10.0, 10.0, 12.0);

/// Lista suspensa (`Ui::popup`) aberta numa superfície própria (xdg_popup): o
/// compositor a põe ao lado do botão, pode passar das bordas da janela e, se não
/// couber na tela, ela encolhe e rola (roda do mouse, setas, teclado).
struct ListPopup {
    popup: Popup,
    viewport: Option<WpViewport>,
    /// Janela dona e o widget que abriu (a escolha volta para ele).
    win: u32,
    id: crate::ui::Id,
    items: Vec<String>,
    selected: usize,
    /// O botão, em coordenadas da janela: o item atual fica em cima dele.
    anchor: crate::ui::Rect,
    /// O pedido de posição ao compositor (para refazer sem deslizar, ver `configure`).
    place: ListPlace,
    /// Já se tentou a lista alta alinhada ao botão.
    tall_tried: bool,
    /// Largura da lista, altura da lista inteira e a altura que o compositor deu.
    w: f32,
    full_h: f32,
    h: f32,
    configured: bool,
    /// Escala da janela dona (a lista sai igual, nítida em 1.5×).
    scale: f32,
    int_scale: i32,
    scroll: f32,
    /// Item aceso (pelo mouse ou pelo teclado) e onde está o mouse (coordenadas da lista).
    hover: Option<usize>,
    pos: Option<(f32, f32)>,
    buffers: Vec<Buffer>,
}

/// Onde pedir a lista: tamanho, retângulo do botão na janela e deslocamento.
#[derive(Clone, Copy)]
struct ListPlace {
    size: (i32, i32),
    anchor: [i32; 4],
    offset: (i32, i32),
}

impl ListPlace {
    /// O posicionador: o canto da lista no botão, deslocado para o item atual ficar
    /// em cima dele. Sem "virar" (o alinhamento do Tiger não tem lado certo). Com
    /// `slide`, desliza para dentro da tela e, se nem assim couber, encolhe; sem, só
    /// encolhe (a lista alta fica alinhada ao botão e rola).
    fn positioner(&self, xdg: &XdgShell, slide: bool) -> Option<XdgPositioner> {
        use smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_positioner::{Anchor, ConstraintAdjustment, Gravity};
        let p = XdgPositioner::new(xdg).ok()?;
        let [ax, ay, aw, ah] = self.anchor;
        p.set_size(self.size.0, self.size.1);
        p.set_anchor_rect(ax, ay, aw, ah);
        p.set_anchor(Anchor::TopLeft);
        p.set_gravity(Gravity::BottomRight);
        p.set_offset(self.offset.0, self.offset.1);
        let mut adj = ConstraintAdjustment::SlideX | ConstraintAdjustment::ResizeY;
        if slide {
            adj |= ConstraintAdjustment::SlideY;
        }
        p.set_constraint_adjustment(adj);
        Some(p)
    }
}

impl ListPopup {
    fn max_scroll(&self) -> f32 {
        (self.full_h - self.h).max(0.0)
    }

    /// Setas de rolagem em cima e embaixo (há mais itens naquela direção).
    fn arrows(&self) -> (bool, bool) {
        (self.scroll > 0.5, self.scroll < self.max_scroll() - 0.5)
    }

    fn set_scroll(&mut self, s: f32) {
        self.scroll = s.clamp(0.0, self.max_scroll()).round();
    }

    /// Seta de rolagem sob o ponto: -1 (em cima), 1 (embaixo).
    fn arrow_at(&self, (x, y): (f32, f32)) -> Option<f32> {
        if x < 0.0 || x >= self.w {
            return None;
        }
        let (up, down) = self.arrows();
        if up && (0.0..crate::ui::POPUP_ARROW_H).contains(&y) {
            Some(-1.0)
        } else if down && y >= self.h - crate::ui::POPUP_ARROW_H && y < self.h {
            Some(1.0)
        } else {
            None
        }
    }

    /// Item sob o ponto (fora das setas e das margens, nenhum).
    fn item_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        if x < 0.0 || x >= self.w || y < 0.0 || y >= self.h || self.arrow_at((x, y)).is_some() {
            return None;
        }
        let i = ((y - crate::ui::POPUP_PAD + self.scroll) / crate::ui::POPUP_ITEM_H).floor();
        (i >= 0.0 && (i as usize) < self.items.len()).then_some(i as usize)
    }

    /// Rola o mínimo para o item ficar à vista (entre as setas).
    fn reveal(&mut self, i: usize) {
        for _ in 0..3 {
            let (up, down) = self.arrows();
            let top = crate::ui::POPUP_PAD + i as f32 * crate::ui::POPUP_ITEM_H - self.scroll;
            let lo = if up { crate::ui::POPUP_ARROW_H } else { 0.0 };
            let hi = self.h - if down { crate::ui::POPUP_ARROW_H } else { 0.0 };
            if top < lo {
                self.set_scroll(self.scroll - (lo - top));
            } else if top + crate::ui::POPUP_ITEM_H > hi {
                self.set_scroll(self.scroll + (top + crate::ui::POPUP_ITEM_H - hi));
            } else {
                break;
            }
        }
    }
}

impl Drop for ListPopup {
    fn drop(&mut self) {
        if let Some(v) = self.viewport.take() {
            v.destroy();
        }
    }
}

const TEXT_MIMES: [&str; 5] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"];

/// Dados para objetos que não recebem eventos.
pub struct NoEvents;
/// Recebe a escala preferida do compositor.
pub struct FracData;

impl<A: App> Dispatch2<WpFractionalScaleManagerV1, Runner<A>> for NoEvents {
    fn event(&self, _: &mut Runner<A>, _: &WpFractionalScaleManagerV1, _: <WpFractionalScaleManagerV1 as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &Connection, _: &QueueHandle<Runner<A>>) {}
}
impl<A: App> Dispatch2<WpViewporter, Runner<A>> for NoEvents {
    fn event(&self, _: &mut Runner<A>, _: &WpViewporter, _: <WpViewporter as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &Connection, _: &QueueHandle<Runner<A>>) {}
}
impl<A: App> Dispatch2<WpViewport, Runner<A>> for NoEvents {
    fn event(&self, _: &mut Runner<A>, _: &WpViewport, _: <WpViewport as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &Connection, _: &QueueHandle<Runner<A>>) {}
}
impl<A: App> Dispatch2<WpFractionalScaleV1, Runner<A>> for FracData {
    fn event(&self, st: &mut Runner<A>, proxy: &WpFractionalScaleV1, ev: wp_fractional_scale_v1::Event, _: &Connection, qh: &QueueHandle<Runner<A>>) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = ev {
            if st.w._frac.as_ref() != Some(proxy) {
                match st.others.iter().position(|o| o._frac.as_ref() == Some(proxy)) {
                    Some(i) => std::mem::swap(&mut st.w, &mut st.others[i]),
                    None => return,
                }
            }
            let s = scale as f64 / 120.0;
            if (s - st.w.scale).abs() > 1e-6 {
                st.w.scale = s;
                st.request_redraw(qh);
            }
        }
    }
}

/// Abre a janela e roda até ela ser fechada.
pub fn run<A: App>(opts: WindowOptions, app: A) -> Result<(), Box<dyn std::error::Error>> {
    glove_config::tune_memory();
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<Runner<A>> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), queue).insert(event_loop.handle()).map_err(|e| e.to_string())?;

    let compositor = CompositorState::bind(&globals, &qh)?;
    let xdg = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let frac_mgr: Option<WpFractionalScaleManagerV1> = globals.bind(&qh, 1..=1, NoEvents).ok();
    let viewporter: Option<WpViewporter> = globals.bind(&qh, 1..=1, NoEvents).ok();

    let shells = Shells { compositor, xdg, frac_mgr, viewporter };
    let first = shells.open(&qh, 0, &opts);

    // Recarrega o tema quando a configuração muda.
    if let Some(fd) = glove_config::watch_fd() {
        let raw = fd.as_raw_fd();
        let _ = raw;
        event_loop
            .handle()
            .insert_source(Generic::new(fd, Interest::READ, Mode::Level), |_, fd, st: &mut Runner<A>| {
                if glove_config::drain_watch(fd) {
                    st.theme = theme_from_config();
                    st.app.config_changed();
                    st.redraw_all();
                }
                Ok(PostAction::Continue)
            })
            .map_err(|e| e.to_string())?;
    }

    // Timer só para apps que pedem (ex.: relógio); acorda alinhado ao próximo segundo.
    // Se o app só pedir depois (um trabalho começou), `ensure_tick` liga o timer.
    let tick_active = std::rc::Rc::new(std::cell::Cell::new(false));
    if let Some(first) = app.tick() {
        tick_active.set(true);
        let flag = tick_active.clone();
        event_loop
            .handle()
            .insert_source(Timer::from_duration(first), move |_, _, st: &mut Runner<A>| {
                st.app.on_tick();
                if st.app.tick_redraw() {
                    st.redraw_all();
                }
                match st.app.tick() {
                    Some(d) => TimeoutAction::ToDuration(d),
                    None => {
                        flag.set(false);
                        TimeoutAction::Drop
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }

    let data_manager = DataDeviceManagerState::bind(&globals, &qh).ok();
    let pool = SlotPool::new((opts.size.0 * opts.size.1 * 4 * 4) as usize, &shm)?;
    let mut runner = Runner {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        w: first,
        others: Vec::new(),
        shells,
        kbd_surface: None,
        exit: false,
        fonts: Fonts::load_system().ok_or("nenhuma fonte encontrada")?,
        doc_fonts: Fonts::load_document().ok_or("nenhuma fonte encontrada")?,
        theme: theme_from_config(),
        app,
        keyboard: None,
        pointer: None,
        themed: None,
        cursor_on: None,
        conn: conn.clone(),
        loop_handle: event_loop.handle(),
        qh: qh.clone(),
        tick_active,
        data_manager,
        data_device: None,
        clip_source: None,
        clip_text: String::new(),
        clip_files: None,
        drag_source: None,
        drag_files: Vec::new(),
        drag_icon: None,
        dnd_win: None,
        last_serial: 0,
        press_serial: 0,
        seat: None,
        list_popup: None,
        alert_done: Vec::new(),
        file_done: Vec::new(),
        list_scroll_timer: false,
        menu_stream: None,
        menu_sent: String::new(),
        menu_shortcuts: std::collections::HashMap::new(),
    };
    runner.connect_menus();
    if let Some(fd) = runner.app.watch_fd() {
        // Segurança: o app garante que o descritor vive enquanto a janela existir.
        let src = Generic::new(unsafe { smithay_client_toolkit::reexports::calloop::generic::FdWrapper::new(fd) }, Interest::READ, Mode::Level);
        event_loop
            .handle()
            .insert_source(src, move |_, _, st: &mut Runner<A>| {
                let keep = st.app.fd_ready_on(fd);
                st.redraw_all();
                Ok(if keep { PostAction::Continue } else { PostAction::Remove })
            })
            .map_err(|e| e.to_string())?;
    }

    while !runner.exit {
        event_loop.dispatch(None::<Duration>, &mut runner)?;
    }
    Ok(())
}

/// União dos retângulos lógicos em pixels físicos, com folga para a
/// suavização das bordas.
/// GLOVE_FULL_REDRAW=1: o jeito antigo (cada quadro pintado inteiro, sem anotar).
fn classic_redraw() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("GLOVE_FULL_REDRAW").is_some())
}

fn check_damage() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("GLOVE_CHECK_DAMAGE").is_some())
}

fn px_union(rects: &[crate::ui::Rect], scale: f32) -> [i32; 4] {
    let mut u = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for r in rects {
        u[0] = u[0].min(((r.x - 2.0) * scale).floor() as i32);
        u[1] = u[1].min(((r.y - 2.0) * scale).floor() as i32);
        u[2] = u[2].max(((r.x + r.w + 2.0) * scale).ceil() as i32);
        u[3] = u[3].max(((r.y + r.h + 2.0) * scale).ceil() as i32);
    }
    u
}

/// As áreas a copiar sem as que já estão dentro de outra (as repetidas inclusive).
fn sem_repetidas(mut v: Vec<[i32; 4]>) -> Vec<[i32; 4]> {
    let area = |r: &[i32; 4]| (r[2] - r[0]).max(0) as i64 * (r[3] - r[1]).max(0) as i64;
    v.sort_by_key(|r| std::cmp::Reverse(area(r)));
    let mut out: Vec<[i32; 4]> = Vec::with_capacity(v.len());
    for r in v {
        if area(&r) > 0 && !out.iter().any(|o| o[0] <= r[0] && o[1] <= r[1] && o[2] >= r[2] && o[3] >= r[3]) {
            out.push(r);
        }
    }
    out
}

/// Compara o quadro novo (`cur`, RGBA w×h) com o anterior (`prev`) dentro de
/// `region` e devolve as áreas que mudaram, já copiadas para `prev`. Linhas
/// vizinhas que mudaram viram um só retângulo (até 8 linhas de folga).
pub(crate) fn frame_damage(prev: &mut Vec<u8>, cur: &[u8], w: i32, h: i32, region: [i32; 4]) -> Vec<[i32; 4]> {
    if prev.len() != cur.len() {
        *prev = cur.to_vec();
        return vec![[0, 0, w, h]];
    }
    let [rx0, ry0, rx1, ry1] = [region[0].clamp(0, w), region[1].clamp(0, h), region[2].clamp(0, w), region[3].clamp(0, h)];
    if rx0 >= rx1 || ry0 >= ry1 {
        return Vec::new();
    }
    let row = |y: i32| ((y * w + rx0) * 4) as usize..((y * w + rx1) * 4) as usize;
    let mut out = Vec::new();
    let mut band: Option<[i32; 4]> = None;
    let mut gap = 0;
    for y in ry0..ry1 {
        let (a, b) = (&cur[row(y)], &prev[row(y)]);
        if a == b {
            if band.is_some() {
                gap += 1;
                if gap > 8 {
                    out.extend(band.take());
                }
            }
            continue;
        }
        gap = 0;
        let n = (rx1 - rx0) as usize;
        let first = a.chunks_exact(4).zip(b.chunks_exact(4)).position(|(p, q)| p != q).unwrap_or(0) as i32;
        let last = (n - a.chunks_exact(4).rev().zip(b.chunks_exact(4).rev()).position(|(p, q)| p != q).unwrap_or(0)) as i32;
        let (x0, x1) = (rx0 + first, rx0 + last);
        band = Some(match band {
            Some([bx0, by0, bx1, _]) => [bx0.min(x0), by0, bx1.max(x1), y + 1],
            None => [x0, y, x1, y + 1],
        });
    }
    out.extend(band);
    if out.len() > 16 {
        let u = out.iter().fold([w, h, 0, 0], |u, r| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]);
        out = vec![u];
    }
    for &[x0, y0, x1, y1] in &out {
        for y in y0..y1 {
            let r = ((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize;
            prev[r.clone()].copy_from_slice(&cur[r]);
        }
    }
    out
}

impl<A: App> Runner<A> {
    /// Traz para `self.w` a janela desta superfície. Devolve se achou.
    fn focus_surface(&mut self, s: &wl_surface::WlSurface) -> bool {
        if self.w.window.wl_surface() == s {
            return true;
        }
        match self.others.iter().position(|o| o.window.wl_surface() == s) {
            Some(i) => {
                std::mem::swap(&mut self.w, &mut self.others[i]);
                true
            }
            None => false,
        }
    }

    /// Traz para `self.w` a janela de número `id`. Devolve se achou.
    fn focus_id(&mut self, id: u32) -> bool {
        if self.w.id == id {
            return true;
        }
        match self.others.iter().position(|o| o.id == id) {
            Some(i) => {
                std::mem::swap(&mut self.w, &mut self.others[i]);
                true
            }
            None => false,
        }
    }

    /// Traz para `self.w` a janela com o foco do teclado (se houver).
    fn focus_keyboard(&mut self) {
        if let Some(s) = self.kbd_surface.clone() {
            self.focus_surface(&s);
        }
    }

    /// Redesenha todas as janelas (tema, relógio, saída do terminal…).
    fn redraw_all(&mut self) {
        let ids: Vec<u32> = std::iter::once(self.w.id).chain(self.others.iter().map(|o| o.id)).collect();
        for id in ids {
            if self.focus_id(id) {
                self.w.dirty = true;
                self.draw_now();
            }
        }
    }

    /// Fecha a janela atual; sem nenhuma, o app termina.
    fn close_current(&mut self) {
        let id = self.w.id;
        if self.list_popup.as_ref().is_some_and(|l| l.win == id) {
            self.list_popup = None;
        }
        // Open/Save window: it goes away and the choice goes to the owner.
        if let Some(parent) = self.w.file_of.as_ref().map(|f| f.0) {
            if let Some(next) = self.others.pop() {
                drop(std::mem::replace(&mut self.w, next));
            }
            self.deliver_files(parent);
            return;
        }
        // Janela de alerta: some e a resposta vai para a dona (o app não sabe dela).
        if let Some((parent, _)) = self.w.alert_of.clone() {
            if let Some(next) = self.others.pop() {
                drop(std::mem::replace(&mut self.w, next));
            }
            self.deliver_alerts(parent);
            return;
        }
        // As janelas de alerta desta vão junto.
        self.others.retain(|o| o.alert_of.as_ref().is_none_or(|(p, _)| *p != id) && o.file_of.as_ref().is_none_or(|f| f.0 != id));
        match self.others.pop() {
            Some(next) => {
                // Soltar a janela fecha o toplevel no compositor.
                drop(std::mem::replace(&mut self.w, next));
                self.app.window_closed(id);
            }
            None => {
                self.app.window_closed(id);
                self.exit = true;
            }
        }
    }

    /// Gives the window `parent` the choices of its Open/Save windows and redraws it.
    fn deliver_files(&mut self, parent: u32) {
        let mut done: Vec<(u64, crate::filebrowser::Choice)> = Vec::new();
        self.file_done.retain(|d| {
            if d.0 == parent {
                done.push((d.1, d.2.clone()));
                false
            } else {
                true
            }
        });
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(0)), move |_, _, st: &mut Runner<A>| {
            if st.focus_id(parent) {
                st.w.mem.file_results.extend(std::mem::take(&mut done));
                st.w.mem.alert_open = false;
                st.w.dirty = true;
                st.draw_now();
            }
            TimeoutAction::Drop
        });
    }

    /// After the frame: opens the Open/Save window the app asked for (or closes it, if it
    /// stopped asking), tied to this one, which stays still while it is open.
    fn sync_file_window(&mut self) {
        if self.w.alert_of.is_some() || self.w.file_of.is_some() {
            return;
        }
        let pid = self.w.id;
        let want = self.w.mem.file_want;
        let open = self.others.iter().position(|o| o.file_of.as_ref().is_some_and(|f| f.0 == pid));
        match (want, open) {
            (Some(key), None) => {
                let Some((k, fb)) = self.w.mem.file_new.take() else { return };
                if k != key {
                    return;
                }
                let opts = WindowOptions { title: fb.window_title(), app_id: self.w.app_id.clone(), size: (640, 440), fixed: false, min_size: Some((480, 320)) };
                let id = 0x2000_0000 | pid;
                let mut win = self.shells.open_with(&self.qh, id, &opts, true);
                win.window.set_parent(Some(&self.w.window));
                win.window.commit();
                win.mem.app_icon = self.w.mem.app_icon;
                win.file_of = Some((pid, key, fb));
                self.w.mem.alert_open = true;
                self.others.push(win);
            }
            (Some(_), Some(_)) => self.w.mem.alert_open = true,
            (None, Some(i)) => {
                drop(self.others.remove(i));
                self.w.mem.alert_open = false;
            }
            (None, None) => {}
        }
    }

    /// Entrega à janela `parent` as respostas dos alertas dela e a redesenha.
    fn deliver_alerts(&mut self, parent: u32) {
        let done: Vec<(u64, usize)> = self.alert_done.iter().filter(|d| d.0 == parent).map(|d| (d.1, d.2)).collect();
        self.alert_done.retain(|d| d.0 != parent);
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(0)), move |_, _, st: &mut Runner<A>| {
            if st.focus_id(parent) {
                st.w.mem.alert_results.extend(done.iter().copied());
                st.w.mem.alert_open = false;
                st.w.dirty = true;
                st.draw_now();
            }
            TimeoutAction::Drop
        });
    }

    /// Depois do quadro: abre a janela do alerta que o app pediu (ou a fecha, se ele
    /// parou de pedir).
    fn sync_alert_window(&mut self) {
        if self.w.alert_of.is_some() {
            return;
        }
        let pid = self.w.id;
        let want = self.w.mem.alert_req.clone();
        let open = self.others.iter().position(|o| o.alert_of.as_ref().is_some_and(|(p, _)| *p == pid));
        match (want, open) {
            (Some(req), None) => {
                let opts = WindowOptions { title: req.title.clone(), app_id: self.w.app_id.clone(), size: (440, 150), fixed: true, min_size: None };
                let id = 0x4000_0000 | pid;
                let mut win = self.shells.open_with(&self.qh, id, &opts, true);
                win.window.set_parent(Some(&self.w.window));
                win.window.commit();
                win.mem.app_icon = self.w.mem.app_icon;
                win.alert_of = Some((pid, req));
                self.w.mem.alert_open = true;
                self.others.push(win);
            }
            (Some(req), Some(i)) => {
                if self.others[i].alert_of.as_ref().is_some_and(|(_, r)| *r != req) {
                    self.others[i].alert_of = Some((pid, req));
                    self.others[i].dirty = true;
                }
                self.w.mem.alert_open = true;
            }
            (None, Some(i)) => {
                drop(self.others.remove(i));
                self.w.mem.alert_open = false;
            }
            (None, None) => {}
        }
    }

    fn request_redraw(&mut self, _qh: &QueueHandle<Self>) {
        self.w.dirty = true;
        if !self.w.frame_pending {
            self.draw_now();
        }
    }

    /// O app passou a pedir o relógio (`tick`) depois de aberto: liga o timer.
    fn ensure_tick(&mut self) {
        if self.tick_active.get() {
            return;
        }
        let Some(first) = self.app.tick() else { return };
        self.tick_active.set(true);
        let flag = self.tick_active.clone();
        let _ = self.loop_handle.insert_source(Timer::from_duration(first), move |_, _, st: &mut Runner<A>| {
            st.app.on_tick();
            if st.app.tick_redraw() {
                st.redraw_all();
            }
            match st.app.tick() {
                Some(d) => TimeoutAction::ToDuration(d),
                None => {
                    flag.set(false);
                    TimeoutAction::Drop
                }
            }
        });
    }

    /// Another frame for this window as soon as the loop is free (state that changed
    /// after the last pass of a frame, outside any event).
    fn redraw_later(&mut self) {
        let win = self.w.id;
        self.w.dirty = true;
        self.loop_handle.insert_idle(move |st: &mut Runner<A>| {
            if st.focus_id(win) {
                st.w.dirty = true;
                st.draw_now();
            }
        });
    }

    fn draw_now(&mut self) {
        self.ensure_tick();
        // Espera o compositor mostrar o quadro anterior: nunca desenha mais
        // rápido que a tela (importante quando chega muita saída de uma vez).
        if !self.w.configured || !(self.w.dirty || self.w.anim_dirty) || self.w.frame_pending {
            return;
        }
        self.step_size_anim();
        let t0 = std::time::Instant::now();
        // Redesenho parcial: se só as animações pediram este quadro (nenhum
        // evento, nada do app), refaz só a área delas; o resto do canvas já
        // está certo desde o quadro anterior.
        let anim_rects = std::mem::take(&mut self.w.mem.anim_rects);
        let mut partial = !self.w.dirty
            && self.w.anim_dirty
            && !self.w.mem.anim_full
            && !anim_rects.is_empty()
            && self.w.size_anim.is_none()
            && !self.w.input.has_events()
            && !self.w.input.moved;
        self.w.anim_dirty = false;
        // Com o quadro anterior anotado e o mesmo canvas: redesenho parcial. Sem
        // isso (janela nova, tamanho novo), pinta tudo de uma vez, já anotando.
        let noted = !partial && !classic_redraw();
        if noted && self.w.ops_valid {
            let scale = if self.w.viewport.is_some() { self.w.scale } else { self.w.int_scale as f64 } as f32;
            if self.w.canvas.as_ref().is_some_and(|c| c.w == self.w.width as f32 && c.h == self.w.height as f32 && c.scale == scale) {
                self.draw_noted(t0);
                return;
            }
        }
        // Até 3 passadas seguidas se a interface pedir (ex.: pop-up que acabou de abrir).
        for pass in 0..3 {
            self.w.dirty = false;
            self.w.mem.anim_rects.clear();
            self.w.mem.anim_full = false;
            let scale = if self.w.viewport.is_some() { self.w.scale } else { self.w.int_scale as f64 } as f32;
            let reuse = self.w.canvas.take().filter(|c| c.w == self.w.width as f32 && c.h == self.w.height as f32 && c.scale == scale);
            partial &= reuse.is_some();
            let Some(mut canvas) = reuse.or_else(|| Canvas::new(self.w.width as f32, self.w.height as f32, scale)) else { return };
            canvas.dark_remap = glove_paint::theme::dark();
            glove_paint::draw::set_grad_remap((canvas.dark_remap, glove_paint::theme::modern()));
            let region = partial.then(|| px_union(&anim_rects, scale));
            canvas.set_clip(None);
            canvas.set_region(region);
            if noted {
                canvas.start_record(false);
            }
            self.background(&mut canvas);
            let alert = self.w.alert_of.clone();
            let mut file = self.w.file_of.take();
            let mut file_choice = None;
            let (redraw, close) = {
                let mut ui = Ui::new(&mut canvas, &self.theme, &self.fonts, &self.doc_fonts, !self.w.fixed, &self.w.input, &mut self.w.mem);
                match (&alert, file.as_mut()) {
                    // Janela de alerta: o alerta ocupa a janela; a escolha volta para a dona.
                    (Some((parent, req)), _) => {
                        let b: Vec<&str> = req.buttons.iter().map(String::as_str).collect();
                        if let Some(i) = ui.alert_window(req.kind, &req.title, &req.text, &b) {
                            self.alert_done.push((*parent, req.key, i));
                            ui.close = true;
                        }
                    }
                    // Open/Save window: the browser fills it; the choice goes to the owner.
                    (None, Some((_, _, fb))) => {
                        if let Some(c) = ui.file_sheet(fb) {
                            file_choice = Some(c);
                            ui.close = true;
                        }
                    }
                    (None, None) => self.app.ui(&mut ui),
                }
                ui.finish()
            };
            if let (Some(c), Some((p, k, _))) = (file_choice, file.as_ref()) {
                self.file_done.push((*p, *k, c));
            }
            self.w.file_of = file;
            // Um clique muda o estado no meio do quadro (ex.: o visor já foi
            // desenhado antes do botão). Mais uma passada, sem eventos, mostra
            // o resultado na hora. Só a última passada vai para a tela.
            let ops = canvas.take_record();
            canvas.set_clip(None);
            canvas.set_region(None);
            let resized = self.after_frame();
            let had_events = self.w.input.has_events();
            self.w.input.end_frame();
            if close {
                // Fecha só esta janela (o app termina quando não sobra nenhuma).
                self.close_current();
                return;
            }
            let settled = std::mem::take(&mut self.w.mem.settled);
            if resized && pass == 2 {
                // No passes left: what changed after this one is drawn right after.
                self.redraw_later();
            }
            if (redraw || (had_events && !settled) || resized) && pass < 2 {
                // A próxima passada pode mudar qualquer coisa: refaz tudo.
                partial = false;
                self.w.canvas = Some(canvas);
                self.w.dirty = true;
                continue;
            }
            let t1 = std::time::Instant::now();
            let key = (canvas.px_width(), canvas.px_height(), canvas.scale.to_bits());
            if key != self.w.prev_key {
                self.w.prev.clear();
                self.w.prev_key = key;
            }
            let (pw, ph) = (canvas.px_width() as i32, canvas.px_height() as i32);
            let damage = frame_damage(&mut self.w.prev, canvas.data(), pw, ph, region.unwrap_or([0, 0, pw, ph]));
            // Quadro só do mouse cruzando áreas, e nada mudou: essas áreas são mudas.
            // (Uma troca de cursor também conta como mudança: a área não é muda.)
            let cursor_moved = std::mem::take(&mut self.w.cursor_moved);
            if let Some(crossed) = self.w.probe.take() {
                if damage.is_empty() && pass == 0 && !had_events && !cursor_moved {
                    if self.w.silent.len() > 4096 {
                        self.w.silent.clear();
                    }
                    self.w.silent.extend(crossed);
                }
            }
            self.present(&canvas, &damage);
            if std::mem::take(&mut self.w.need_commit) && damage.is_empty() {
                self.w.window.commit();
            }
            if std::env::var_os("GLOVE_PROF").is_some() {
                let px: i32 = damage.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
                eprintln!("PROF desenho={:?} envio={:?} passadas={} parcial={} mudou={}px em {} áreas", t1 - t0, t1.elapsed(), pass + 1, region.is_some(), px, damage.len());
            }
            // O que a animação pintou não está nas operações guardadas: o próximo
            // quadro anotado repinta essa área. Um quadro inteiro invalida tudo.
            match region {
                Some(r) => self.w.forced.push(r),
                None => {
                    self.w.ops_valid = noted;
                    self.w.ops = ops;
                    self.w.forced.clear();
                }
            }
            self.w.canvas = Some(canvas);
            break;
        }
    }

    /// Fundo listrado da janela (antes do app, se ele não pinta o próprio).
    fn background(&self, canvas: &mut Canvas) {
        if !self.app.paints_background() {
            canvas.clear(self.theme.body_a);
            canvas.pinstripes(0.0, 0.0, self.w.width as f32, self.w.height as f32, self.theme.body_a, self.theme.body_b);
        }
    }

    /// Quadro com redesenho parcial: primeiro o app roda só anotando o que
    /// desenharia (sem pintar nada, e quantas passadas precisar); depois, onde a
    /// lista de operações difere da do quadro anterior, roda de novo pintando só
    /// ali. Vale para todo app, sem ele precisar dizer o que mudou.
    fn draw_noted(&mut self, t0: std::time::Instant) {
        let mut ops = Vec::new();
        let mut passes = 0;
        let mut first_events = false;
        for pass in 0..3 {
            self.w.dirty = false;
            self.w.mem.anim_rects.clear();
            self.w.mem.anim_full = false;
            let scale = if self.w.viewport.is_some() { self.w.scale } else { self.w.int_scale as f64 } as f32;
            let reuse = self.w.canvas.take().filter(|c| c.w == self.w.width as f32 && c.h == self.w.height as f32 && c.scale == scale);
            if reuse.is_none() {
                self.w.ops_valid = false;
            }
            let Some(mut canvas) = reuse.or_else(|| Canvas::new(self.w.width as f32, self.w.height as f32, scale)) else { return };
            canvas.dark_remap = glove_paint::theme::dark();
            glove_paint::draw::set_grad_remap((canvas.dark_remap, glove_paint::theme::modern()));
            canvas.set_clip(None);
            canvas.set_region(None);
            canvas.start_record(true);
            self.background(&mut canvas);
            let alert = self.w.alert_of.clone();
            let mut file = self.w.file_of.take();
            let mut file_choice = None;
            let (redraw, close) = {
                let mut ui = Ui::new(&mut canvas, &self.theme, &self.fonts, &self.doc_fonts, !self.w.fixed, &self.w.input, &mut self.w.mem);
                match (&alert, file.as_mut()) {
                    // Janela de alerta: o alerta ocupa a janela; a escolha volta para a dona.
                    (Some((parent, req)), _) => {
                        let b: Vec<&str> = req.buttons.iter().map(String::as_str).collect();
                        if let Some(i) = ui.alert_window(req.kind, &req.title, &req.text, &b) {
                            self.alert_done.push((*parent, req.key, i));
                            ui.close = true;
                        }
                    }
                    // Open/Save window: the browser fills it; the choice goes to the owner.
                    (None, Some((_, _, fb))) => {
                        if let Some(c) = ui.file_sheet(fb) {
                            file_choice = Some(c);
                            ui.close = true;
                        }
                    }
                    (None, None) => self.app.ui(&mut ui),
                }
                ui.finish()
            };
            if let (Some(c), Some((p, k, _))) = (file_choice, file.as_ref()) {
                self.file_done.push((*p, *k, c));
            }
            self.w.file_of = file;
            ops = canvas.take_record();
            canvas.set_clip(None);
            self.w.canvas = Some(canvas);
            let resized = self.after_frame();
            let had_events = self.w.input.has_events();
            if pass == 0 {
                first_events = had_events;
            }
            self.w.input.end_frame();
            if close {
                self.close_current();
                return;
            }
            passes = pass + 1;
            let settled = std::mem::take(&mut self.w.mem.settled);
            if resized && pass == 2 {
                // No passes left: what changed after this one is drawn right after.
                self.redraw_later();
            }
            if (redraw || (had_events && !settled) || resized) && pass < 2 {
                self.w.dirty = true;
                continue;
            }
            break;
        }
        let t1 = std::time::Instant::now();
        let Some(mut canvas) = self.w.canvas.take() else { return };
        // O tamanho pode ter mudado na última passada.
        if canvas.w != self.w.width as f32 || canvas.h != self.w.height as f32 {
            self.w.ops_valid = false;
            self.w.dirty = true;
        }
        let (pw, ph) = (canvas.px_width() as i32, canvas.px_height() as i32);
        let full = [0, 0, pw, ph];
        let mut rects = if self.w.ops_valid { glove_paint::draw::diff_ops(&self.w.ops, &ops) } else { vec![full] };
        rects.append(&mut self.w.forced);
        let clamp = |r: [i32; 4]| [r[0].max(0), r[1].max(0), r[2].min(pw), r[3].min(ph)];
        let rects = glove_paint::draw::merge_rects(rects.into_iter().map(clamp).collect(), 3);
        self.w.ops = ops;
        self.w.ops_valid = true;
        let mut t2 = t1;
        if !rects.is_empty() {
            // Passadas só para pintar: os pedidos e o estado do quadro já vieram
            // da passada anotada (o app vê o mesmo estado, sem eventos).
            let keep = (std::mem::take(&mut self.w.mem.hover_rects), self.w.mem.pointer_sensitive, std::mem::take(&mut self.w.mem.anim_rects), self.w.mem.anim_full);
            let alert = self.w.alert_of.clone();
            for r in &rects {
                canvas.set_region(Some(*r));
                self.background(&mut canvas);
                let mut file = self.w.file_of.take();
                let _ = {
                    let mut ui = Ui::new(&mut canvas, &self.theme, &self.fonts, &self.doc_fonts, !self.w.fixed, &self.w.input, &mut self.w.mem);
                    // A janela do alerta só pinta o alerta (o app nunca desenha nela: ele
                    // pintaria a própria tela por cima e mudaria o estado dele com o
                    // tamanho errado).
                    match (&alert, file.as_mut()) {
                        (Some((_, req)), _) => {
                            let b: Vec<&str> = req.buttons.iter().map(String::as_str).collect();
                            let _ = ui.alert_window(req.kind, &req.title, &req.text, &b);
                        }
                        (None, Some((_, _, fb))) => {
                            let _ = ui.file_sheet(fb);
                        }
                        (None, None) => self.app.ui(&mut ui),
                    }
                    ui.finish()
                };
                self.w.file_of = file;
                canvas.set_clip(None);
                canvas.set_region(None);
            }
            t2 = std::time::Instant::now();
            if check_damage() {
                self.check_against_full(&canvas, &rects);
            }
            self.w.mem.drop_requests();
            self.w.mem.settled = false;
            (self.w.mem.hover_rects, self.w.mem.pointer_sensitive, self.w.mem.anim_rects, self.w.mem.anim_full) = keep;
        }
        let t3 = std::time::Instant::now();
        let key = (canvas.px_width(), canvas.px_height(), canvas.scale.to_bits());
        if key != self.w.prev_key {
            self.w.prev.clear();
            self.w.prev_key = key;
        }
        let damage = if rects.is_empty() {
            Vec::new()
        } else {
            let u = rects.iter().fold([pw, ph, 0, 0], |u, r| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]);
            frame_damage(&mut self.w.prev, canvas.data(), pw, ph, u)
        };
        // Quadro só do mouse cruzando áreas, e nada mudou: essas áreas são mudas.
        let cursor_moved = std::mem::take(&mut self.w.cursor_moved);
        if let Some(crossed) = self.w.probe.take() {
            if damage.is_empty() && passes == 1 && !first_events && !cursor_moved {
                if self.w.silent.len() > 4096 {
                    self.w.silent.clear();
                }
                self.w.silent.extend(crossed);
            }
        }
        self.present(&canvas, &damage);
        if std::mem::take(&mut self.w.need_commit) && damage.is_empty() {
            self.w.window.commit();
        }
        if std::env::var_os("GLOVE_PROF").is_some() {
            let px: i64 = rects.iter().map(|r| (r[2] - r[0]) as i64 * (r[3] - r[1]) as i64).sum();
            let sent: i32 = damage.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
            eprintln!(
                "PROF desenho={:?} (anotar={:?} ×{} pintar={:?} ×{}) envio={:?} pintou={}px de {} mudou={}px",
                t2 - t0,
                t1 - t0,
                passes,
                t2 - t1,
                rects.len(),
                t3.elapsed(),
                px,
                pw as i64 * ph as i64,
                sent
            );
        }
        self.w.canvas = Some(canvas);
    }

    /// GLOVE_CHECK_DAMAGE=1: pinta o quadro inteiro à parte e compara com o
    /// parcial; avisa no terminal onde diferem (para achar desenho não anotado).
    fn check_against_full(&mut self, canvas: &Canvas, rects: &[[i32; 4]]) {
        let Some(mut full) = Canvas::new(canvas.w, canvas.h, canvas.scale) else { return };
        self.background(&mut full);
        {
            let alert = self.w.alert_of.clone();
            let mut file = self.w.file_of.take();
            let mut ui = Ui::new(&mut full, &self.theme, &self.fonts, &self.doc_fonts, !self.w.fixed, &self.w.input, &mut self.w.mem);
            match (&alert, file.as_mut()) {
                (Some((_, req)), _) => {
                    let b: Vec<&str> = req.buttons.iter().map(String::as_str).collect();
                    let _ = ui.alert_window(req.kind, &req.title, &req.text, &b);
                }
                (None, Some((_, _, fb))) => {
                    let _ = ui.file_sheet(fb);
                }
                (None, None) => self.app.ui(&mut ui),
            }
            let _ = ui.finish();
            self.w.file_of = file;
        }
        self.w.mem.drop_requests();
        let w = canvas.px_width() as usize;
        let (a, b) = (canvas.data(), full.data());
        let mut bad = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        let mut n = 0;
        for i in 0..a.len() / 4 {
            // Diferença de 1 nível (arredondamento) não conta.
            if a[i * 4..i * 4 + 4].iter().zip(&b[i * 4..i * 4 + 4]).any(|(p, q)| p.abs_diff(*q) > 2) {
                let (x, y) = ((i % w) as i32, (i / w) as i32);
                bad = [bad[0].min(x), bad[1].min(y), bad[2].max(x + 1), bad[3].max(y + 1)];
                n += 1;
            }
        }
        // Até uns poucos pixels soltos é do tiny-skia: traços finos e compridos saem
        // com um pixel diferente conforme o recorte (ele os divide em pedaços).
        if n > 12 {
            eprintln!("CHECK redesenho parcial difere em {n} pixels, área {bad:?} (pintou {rects:?})");
            // GLOVE_CHECK_DAMAGE=pasta: guarda as duas imagens para comparar.
            if let Some(dir) = std::env::var_os("GLOVE_CHECK_DAMAGE").filter(|d| std::path::Path::new(d).is_dir()) {
                let dir = std::path::PathBuf::from(dir);
                let _ = canvas.pm.save_png(dir.join("partial.png"));
                let _ = full.pm.save_png(dir.join("full.png"));
            }
        } else {
            eprintln!("CHECK ok (pintou {rects:?})");
        }
    }

    /// Conecta ao Glove para mostrar os menus na barra global (se houver).
    fn connect_menus(&mut self) {
        use std::io::Write;
        let Some(path) = std::env::var_os("GLOVE_MENU_SOCKET").filter(|p| !p.is_empty()) else { return };
        let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) else { return };
        if stream.write_all(format!("HELLO\t{}\n", std::process::id()).as_bytes()).is_err() {
            return;
        }
        let Ok(reader) = stream.try_clone() else { return };
        let _ = reader.set_nonblocking(true);
        let mut buf = Vec::new();
        let _ = self.loop_handle.insert_source(Generic::new(reader, Interest::READ, Mode::Level), move |_, s, st: &mut Runner<A>| {
            use std::io::Read;
            let mut tmp = [0u8; 1024];
            loop {
                // Segurança: o socket pertence a esta fonte.
                match unsafe { s.get_mut() }.read(&mut tmp) {
                    Ok(0) => return Ok(PostAction::Remove),
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return Ok(PostAction::Remove),
                }
            }
            while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                if let Some(id) = line.strip_prefix("ACT\t") {
                    st.menu_activated(id);
                }
            }
            Ok(PostAction::Continue)
        });
        self.menu_stream = Some(stream);
    }

    /// Item escolhido na barra de menu: com atalho, vira o atalho; sem, vai para o app.
    fn menu_activated(&mut self, id: &str) {
        self.focus_keyboard();
        match self.menu_shortcuts.get(id).filter(|s| !s.is_empty()).cloned() {
            Some(combo) => self.w.input.keys.push(KeyPress { key: Key::Other, combo }),
            None => self.w.input.menu_actions.push(id.to_string()),
        }
        self.w.dirty = true;
        self.draw_now();
    }

    fn send_menus(&mut self) {
        use std::io::Write;
        if self.menu_stream.is_none() {
            return;
        }
        let menus = self.app.menus();
        let text = crate::menu::serialize(&menus);
        if text == self.menu_sent {
            return;
        }
        self.menu_shortcuts.clear();
        for m in &menus {
            let mut all = Vec::new();
            for it in &m.items {
                it.walk(&mut all);
            }
            for it in all {
                if let crate::menu::MenuItem::Item { id, shortcut, .. } = it {
                    self.menu_shortcuts.insert(id.clone(), shortcut.clone());
                }
            }
        }
        if let Some(s) = &mut self.menu_stream {
            if s.write_all(text.as_bytes()).is_err() {
                self.menu_stream = None;
            }
        }
        self.menu_sent = text;
    }

    /// Pedidos que os widgets deixaram durante o quadro.
    /// Aplica o tamanho da janela (o canto de cima fica parado; o compositor ancora
    /// a janela pelo canto superior esquerdo).
    fn set_size(&mut self, w: u32, h: u32) {
        if (w, h) == (self.w.width, self.w.height) {
            return;
        }
        self.w.width = w;
        self.w.height = h;
        if self.w.fixed {
            self.w.window.set_min_size(Some((w, h)));
            self.w.window.set_max_size(Some((w, h)));
        }
    }

    /// Avança a animação de tamanho e pede o próximo quadro enquanto ela durar.
    fn step_size_anim(&mut self) {
        let Some((from, to, start, dur)) = self.w.size_anim else { return };
        let t = (start.elapsed().as_secs_f32() / dur.as_secs_f32()).min(1.0);
        // Começa rápido e freia no fim.
        let e = 1.0 - (1.0 - t).powi(3);
        let lerp = |a: u32, b: u32| (a as f32 + (b as f32 - a as f32) * e).round() as u32;
        self.set_size(lerp(from.0, to.0), lerp(from.1, to.1));
        if t >= 1.0 {
            self.w.size_anim = None;
        } else {
            self.w.mem.wake_after = Some(self.w.mem.wake_after.map_or(Duration::from_millis(16), |w| w.min(Duration::from_millis(16))));
        }
    }

    /// Devolve verdadeiro se o tamanho mudou na hora (o quadro precisa ser refeito).
    fn after_frame(&mut self) -> bool {
        let mut resized = false;
        self.sync_alert_window();
        self.sync_file_window();
        // O cursor que o quadro pediu (só manda quando muda, com o mouse sobre a janela).
        let want = self.w.mem.mouse_cursor;
        if self.w.input.inside && self.cursor_on != Some(want) {
            if let Some(t) = &self.themed {
                use smithay_client_toolkit::seat::pointer::CursorIcon;
                let icon = match want {
                    crate::ui::Cursor::Arrow => CursorIcon::Default,
                    crate::ui::Cursor::Hand => CursorIcon::Pointer,
                    crate::ui::Cursor::Text => CursorIcon::Text,
                };
                if t.set_cursor(&self.conn, icon).is_ok() {
                    self.cursor_on = Some(want);
                    self.w.cursor_moved = true;
                }
            }
        }
        for fd in std::mem::take(&mut self.w.mem.watch_req) {
            // Segurança: o app garante que o descritor vive enquanto observado.
            let src = Generic::new(unsafe { smithay_client_toolkit::reexports::calloop::generic::FdWrapper::new(fd) }, Interest::READ, Mode::Level);
            let _ = self.loop_handle.insert_source(src, move |_, _, st: &mut Runner<A>| {
                let keep = st.app.fd_ready_on(fd);
                st.redraw_all();
                Ok(if keep { PostAction::Continue } else { PostAction::Remove })
            });
        }
        if std::mem::take(&mut self.w.mem.redraw_all) && !self.others.is_empty() {
            let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(0)), |_, _, st: &mut Runner<A>| {
                st.redraw_all();
                TimeoutAction::Drop
            });
        }
        if let Some((mw, mh, animate)) = self.w.mem.min_req.take() {
            if self.w.min != Some((mw, mh)) {
                self.w.min = Some((mw, mh));
                self.w.window.set_min_size(Some((mw, mh)));
                self.w.need_commit = true;
            }
            if self.w.resizing {
                // Durante o arraste o tamanho é o do compositor; só não fica abaixo do
                // mínimo (que pode crescer no meio: a grade refaz as linhas). Ao soltar,
                // a janela vai para o lugar dela, animada.
                let grown = (self.w.width.max(mw), self.w.height.max(mh));
                if grown != (self.w.width, self.w.height) {
                    self.w.mem.size_req = Some((grown.0, grown.1, false));
                }
            } else {
                let (w, h) = floor_size(self.w.chosen, (mw, mh));
                self.w.mem.size_req = Some((w, h, animate));
            }
        }
        if let Some((w, h, animate)) = self.w.mem.size_req.take() {
            let target = self.w.size_anim.map(|a| a.1).unwrap_or((self.w.width, self.w.height));
            if (w, h) != target {
                if animate && self.w.shown && !glove_config::reduce_effects() {
                    let d = (w as f32 - self.w.width as f32).abs().max((h as f32 - self.w.height as f32).abs());
                    let dur = Duration::from_millis((d * 0.8).clamp(120.0, 300.0) as u64);
                    self.w.size_anim = Some(((self.w.width, self.w.height), (w, h), std::time::Instant::now(), dur));
                    self.step_size_anim();
                } else {
                    self.w.size_anim = None;
                    self.set_size(w, h);
                    resized = true;
                }
            }
        }
        self.send_menus();
        for cmd in std::mem::take(&mut self.w.mem.glove_cmds) {
            use std::io::Write;
            if let Some(s) = &mut self.menu_stream {
                let _ = s.write_all(format!("{cmd}\n").as_bytes());
            }
        }
        if std::mem::take(&mut self.w.mem.resize_req) {
            if let Some(seat) = &self.seat {
                self.w.window.resize(seat, self.press_serial, smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge::BottomRight);
            }
        }
        for (id, opts) in std::mem::take(&mut self.w.mem.open_windows) {
            if self.w.id != id && !self.others.iter().any(|o| o.id == id) {
                let win = self.shells.open(&self.qh, id, &opts);
                self.others.push(win);
            }
        }
        if let Some(d) = self.w.mem.wake_after.take() {
            if !self.w.wake_pending {
                self.w.wake_pending = true;
                let id = self.w.id;
                let _ = self.loop_handle.insert_source(Timer::from_duration(d), move |_, _, st: &mut Runner<A>| {
                    if !st.focus_id(id) {
                        return TimeoutAction::Drop;
                    }
                    st.w.wake_pending = false;
                    // A animação de tamanho refaz tudo; as outras podem ser parciais.
                    if st.w.size_anim.is_some() {
                        st.w.dirty = true;
                    } else {
                        st.w.anim_dirty = true;
                    }
                    st.draw_now();
                    TimeoutAction::Drop
                });
            }
        }
        if let Some(f) = self.w.mem.doc_font_req.take() {
            if let Some(fonts) = Fonts::load_family(&f) {
                self.doc_fonts = fonts;
                self.w.dirty = true;
                // Another pass now: an app that marked the frame settled would otherwise
                // show the new font only on the next event.
                resized = true;
            }
        }
        if std::mem::take(&mut self.w.mem.move_req) {
            if let Some(seat) = &self.seat {
                self.w.window.move_(seat, self.press_serial);
            }
        }
        if std::mem::take(&mut self.w.mem.minimize_req) {
            self.w.window.set_minimized();
        }
        if std::mem::take(&mut self.w.mem.zoom_req) {
            if self.w.input.maximized {
                self.w.window.unset_maximized();
            } else {
                self.w.window.set_maximized();
            }
        }
        if let Some(t) = self.w.mem.title.take() {
            self.w.window.set_title(t);
        }
        if let Some(text) = self.w.mem.clip_out.take() {
            self.clip_text = text;
            self.clip_files = None;
            if let (Some(m), Some(d)) = (&self.data_manager, &self.data_device) {
                let src = m.create_copy_paste_source(&self.qh, TEXT_MIMES);
                src.set_selection(d, self.last_serial);
                self.clip_source = Some(src);
            }
        }
        if let Some((paths, cut)) = self.w.mem.clip_files.take() {
            // Colar neste mesmo app recebe a lista de URIs.
            self.clip_text = String::from_utf8_lossy(&crate::clipfiles::payload(&paths, cut, crate::clipfiles::URI_LIST)).into_owned();
            if let (Some(m), Some(d)) = (&self.data_manager, &self.data_device) {
                let src = m.create_copy_paste_source(&self.qh, crate::clipfiles::mimes(&paths));
                src.set_selection(d, self.last_serial);
                self.clip_source = Some(src);
            }
            self.clip_files = Some((paths, cut));
        }
        if let Some(paths) = self.w.mem.drag_out.take() {
            self.start_drag(paths);
        }
        // Não deu para abrir a lista na superfície própria: mais uma passada a
        // desenha dentro da janela.
        resized |= self.sync_list_popup();
        if let Some(id) = self.w.mem.paste_req.take() {
            let files = std::mem::take(&mut self.w.mem.paste_files);
            // Our own selection is pasted right away: one more pass shows it in this same
            // frame (the editor marks its frames settled, and the text would only appear
            // on the next key).
            resized |= self.request_paste(id, files);
        }
        resized
    }

    /// Acompanha a lista suspensa da janela atual: abre a superfície própria para a
    /// lista que o `Ui` acabou de abrir e some com ela quando o `Ui` a fechou (clique
    /// na janela). Devolve verdadeiro se a lista vai ser desenhada dentro da janela
    /// (não deu para abrir a superfície) e o quadro precisa ser refeito.
    fn sync_list_popup(&mut self) -> bool {
        let want = self.w.mem.popup.as_ref().map(|p| (p.id, p.native));
        if self.list_popup.as_ref().is_some_and(|l| l.win == self.w.id && want.map(|w| w.0) != Some(l.id)) {
            self.list_popup = None;
        }
        if let Some((_, None)) = want {
            let ok = self.open_list_popup();
            if let Some(p) = &mut self.w.mem.popup {
                p.native = Some(ok);
            }
            return !ok;
        }
        false
    }

    /// Abre a lista suspensa numa superfície própria, ancorada no botão: o item atual
    /// em cima dele, como no Tiger; o compositor desliza/encolhe para caber na tela.
    fn open_list_popup(&mut self) -> bool {
        use crate::ui::{POPUP_ITEM_H, POPUP_PAD};
        // Uma lista por vez (outra janela pode ter deixado uma aberta).
        if let Some(old) = self.list_popup.take() {
            let (win, id) = (old.win, old.id);
            drop(old);
            let cur = self.w.id;
            if win != cur && self.focus_id(win) {
                if self.w.mem.popup.as_ref().is_some_and(|p| p.id == id) {
                    self.w.mem.popup = None;
                }
                self.w.dirty = true;
                self.focus_id(cur);
            }
        }
        let Some(seat) = self.seat.clone() else { return false };
        let Some(p) = self.w.mem.popup.as_ref() else { return false };
        let (id, items, selected, anchor) = (p.id, p.items.clone(), p.selected, p.anchor);
        if items.is_empty() {
            return false;
        }
        // Larga o bastante para o nome mais comprido (com o ✓ à esquerda).
        let text_w = items.iter().map(|t| self.fonts.measure(t, 13.0, false)).fold(0.0f32, f32::max);
        let w = anchor.w.max(text_w + 22.0 + 16.0).ceil();
        let full_h = (items.len() as f32 * POPUP_ITEM_H + 2.0 * POPUP_PAD).ceil();
        // O retângulo do botão precisa estar dentro da janela.
        let (ww, wh) = (self.w.width.max(1) as i32, self.w.height.max(1) as i32);
        let ax = (anchor.x.floor() as i32).clamp(0, ww - 1);
        let ay = (anchor.y.floor() as i32).clamp(0, wh - 1);
        let aw = (anchor.w.ceil() as i32).clamp(1, ww - ax);
        let ah = (anchor.h.ceil() as i32).clamp(1, wh - ay);
        // O item atual na altura do botão (a lista começa acima dele).
        let offset = ((anchor.x.floor() as i32) - ax, (anchor.y.floor() as i32) - ay - (POPUP_PAD + selected as f32 * POPUP_ITEM_H).round() as i32);
        let place = ListPlace { size: (w as i32, full_h as i32), anchor: [ax, ay, aw, ah], offset };
        let Some(positioner) = place.positioner(&self.shells.xdg, true) else { return false };
        let surface = self.shells.compositor.create_surface(&self.qh);
        let viewport = match (&self.shells.viewporter, &self.w.viewport) {
            (Some(v), Some(_)) => Some(v.get_viewport(&surface, &self.qh, NoEvents)),
            _ => None,
        };
        let popup = match Popup::from_surface(Some(self.w.window.xdg_surface()), &positioner, &self.qh, surface, &self.shells.xdg) {
            Ok(p) => p,
            Err(_) => {
                if let Some(v) = viewport {
                    v.destroy();
                }
                return false;
            }
        };
        let (sl, _, _) = LIST_SHADOW;
        popup.xdg_surface().set_window_geometry(sl as i32, 0, w as i32, full_h as i32);
        // O grab (antes do primeiro commit): clicar fora fecha, o teclado vem para a lista.
        popup.xdg_popup().grab(&seat, self.last_serial);
        popup.wl_surface().commit();
        let scale = if self.w.viewport.is_some() { self.w.scale } else { self.w.int_scale as f64 } as f32;
        self.list_popup = Some(ListPopup {
            popup,
            viewport,
            win: self.w.id,
            id,
            items,
            selected,
            anchor,
            place,
            tall_tried: false,
            w,
            full_h,
            h: full_h,
            configured: false,
            scale,
            int_scale: self.w.int_scale.max(1),
            scroll: 0.0,
            hover: None,
            pos: None,
            buffers: Vec::new(),
        });
        true
    }

    /// Pinta a lista na superfície dela (só quando algo muda: item aceso, rolagem).
    fn draw_list_popup(&mut self) {
        let Some(mut lp) = self.list_popup.take() else { return };
        if lp.configured {
            self.paint_list_popup(&mut lp);
        }
        self.list_popup = Some(lp);
    }

    fn paint_list_popup(&mut self, lp: &mut ListPopup) {
        let (sl, sr, sb) = LIST_SHADOW;
        let (lw, lh) = (lp.w + sl + sr, lp.h + sb);
        let Some(mut c) = Canvas::new(lw, lh, lp.scale) else { return };
        c.dark_remap = glove_paint::theme::dark();
        glove_paint::draw::set_grad_remap((c.dark_remap, glove_paint::theme::modern()));
        crate::ui::draw_popup_list(&mut c, &self.fonts, &self.theme, crate::ui::Rect::new(sl, 0.0, lp.w, lp.h), &lp.items, lp.selected, lp.hover, lp.scroll, lp.arrows());
        let (pw, ph) = (c.px_width() as i32, c.px_height() as i32);
        lp.buffers.retain(|b| b.height() == ph && b.stride() == pw * 4);
        let idx = match lp.buffers.iter().position(|b| self.pool.canvas(b).is_some()) {
            Some(i) => i,
            None => {
                let Ok((b, _)) = self.pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else { return };
                if lp.buffers.len() >= 2 {
                    lp.buffers.remove(0);
                }
                lp.buffers.push(b);
                lp.buffers.len() - 1
            }
        };
        let buffer = &lp.buffers[idx];
        let Some(dst) = self.pool.canvas(buffer) else { return };
        for (d, s) in dst.chunks_exact_mut(4).zip(c.data().chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        let surface = lp.popup.wl_surface();
        match &lp.viewport {
            Some(v) => v.set_destination(lw as i32, lh as i32),
            None => surface.set_buffer_scale(lp.int_scale),
        }
        lp.popup.xdg_surface().set_window_geometry(sl as i32, 0, lp.w as i32, lp.h as i32);
        // Só a lista recebe o mouse (a sombra deixa passar: lá é "fora").
        if let Ok(region) = smithay_client_toolkit::compositor::Region::new(&self.shells.compositor) {
            region.add(sl as i32, 0, lp.w as i32, lp.h as i32);
            surface.set_input_region(Some(region.wl_region()));
        }
        if buffer.attach_to(surface).is_ok() {
            surface.damage_buffer(0, 0, pw, ph);
            surface.commit();
        }
    }

    /// Fecha a lista suspensa; com `choice`, o item escolhido volta para o widget.
    fn finish_list_popup(&mut self, choice: Option<usize>) {
        let Some(lp) = self.list_popup.take() else { return };
        let (win, id) = (lp.win, lp.id);
        drop(lp);
        if self.focus_id(win) {
            if let Some(i) = choice {
                self.w.mem.popup_choice = Some((id, i));
            }
            if self.w.mem.popup.as_ref().is_some_and(|p| p.id == id) {
                self.w.mem.popup = None;
            }
            self.w.dirty = true;
            self.draw_now();
        }
    }

    /// Mouse na lista suspensa: acende o item, rola, escolhe ao soltar o botão.
    fn list_popup_pointer(&mut self, e: &PointerEvent) {
        let Some(lp) = self.list_popup.as_mut() else { return };
        let p = (e.position.0 as f32 - LIST_SHADOW.0, e.position.1 as f32);
        let before = (lp.hover, lp.scroll);
        let mut choose = None;
        match e.kind {
            PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                lp.pos = Some(p);
                lp.hover = lp.item_at(p);
            }
            PointerEventKind::Leave { .. } => {
                lp.pos = None;
                lp.hover = None;
            }
            PointerEventKind::Press { serial, .. } => {
                self.last_serial = serial;
                self.press_serial = serial;
            }
            // Botão direito: nada (como no Mac).
            PointerEventKind::Release { button: 0x111, .. } => {}
            PointerEventKind::Release { .. } => {
                lp.pos = Some(p);
                // Na margem ou numa seta: nada (a lista continua aberta).
                choose = lp.item_at(p);
            }
            PointerEventKind::Axis { vertical, .. } => {
                let d = if vertical.absolute != 0.0 { vertical.absolute as f32 } else { vertical.discrete as f32 * 3.0 * crate::ui::POPUP_ITEM_H };
                lp.set_scroll(lp.scroll + d);
                if let Some(at) = lp.pos {
                    lp.hover = lp.item_at(at);
                }
            }
        }
        let over_arrow = lp.pos.is_some_and(|at| lp.arrow_at(at).is_some());
        let changed = before != (lp.hover, lp.scroll);
        if let Some(i) = choose {
            self.finish_list_popup(Some(i));
            return;
        }
        if over_arrow {
            self.start_list_scroll();
        }
        if changed {
            self.draw_list_popup();
        }
    }

    /// Mouse parado numa seta de rolagem: a lista rola sozinha enquanto ele estiver lá.
    fn start_list_scroll(&mut self) {
        if self.list_scroll_timer {
            return;
        }
        self.list_scroll_timer = true;
        let step = Duration::from_millis(30);
        let _ = self.loop_handle.insert_source(Timer::from_duration(step), move |_, _, st: &mut Runner<A>| {
            let Some(lp) = st.list_popup.as_mut() else {
                st.list_scroll_timer = false;
                return TimeoutAction::Drop;
            };
            let Some(dir) = lp.pos.and_then(|at| lp.arrow_at(at)) else {
                st.list_scroll_timer = false;
                return TimeoutAction::Drop;
            };
            lp.set_scroll(lp.scroll + dir * 8.0);
            lp.hover = lp.pos.and_then(|at| lp.item_at(at));
            st.draw_list_popup();
            TimeoutAction::ToDuration(step)
        });
    }

    /// Teclado com a lista aberta: ↑/↓ andam, Enter escolhe, Esc fecha sem mudar e
    /// uma letra pula para o próximo item que começa com ela.
    fn list_popup_key(&mut self, ev: &KeyEvent) {
        let Some(lp) = self.list_popup.as_mut() else { return };
        let n = lp.items.len();
        let cur = lp.hover.unwrap_or(lp.selected).min(n.saturating_sub(1));
        let page = ((lp.h / crate::ui::POPUP_ITEM_H).floor() as usize).saturating_sub(2).max(1);
        let to = match ev.keysym {
            Keysym::Escape => return self.finish_list_popup(None),
            Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                let c = lp.hover;
                return self.finish_list_popup(c);
            }
            Keysym::Up => Some(if lp.hover.is_none() { cur } else { cur.saturating_sub(1) }),
            Keysym::Down => Some(if lp.hover.is_none() { cur } else { (cur + 1).min(n - 1) }),
            Keysym::Home => Some(0),
            Keysym::End => Some(n - 1),
            Keysym::Page_Up => Some(cur.saturating_sub(page)),
            Keysym::Page_Down => Some((cur + page).min(n - 1)),
            _ => {
                let typed = ev.utf8.as_deref().and_then(|t| t.chars().next()).filter(|c| !c.is_control());
                typed.and_then(|ch| {
                    let ch = ch.to_lowercase().to_string();
                    let start = lp.hover.map_or(0, |h| h + 1);
                    (0..n).map(|k| (start + k) % n).find(|&i| lp.items[i].to_lowercase().starts_with(&ch))
                })
            }
        };
        if let Some(i) = to {
            lp.hover = Some(i);
            lp.reveal(i);
            self.draw_list_popup();
        }
    }

    /// Começa a arrastar arquivos (para outra janela, outro app, a mesa), com um
    /// ícone do arquivo seguindo o mouse.
    fn start_drag(&mut self, paths: Vec<std::path::PathBuf>) {
        if paths.is_empty() || self.data_manager.is_none() || self.data_device.is_none() {
            return;
        }
        let icon = self.drag_icon_surface(&paths);
        let (Some(m), Some(d)) = (&self.data_manager, &self.data_device) else { return };
        let src = m.create_drag_and_drop_source(&self.qh, crate::clipfiles::mimes(&paths), DndAction::Copy | DndAction::Move);
        src.start_drag(d, self.w.window.wl_surface(), icon.as_ref(), self.press_serial);
        self.drag_source = Some(src);
        self.drag_icon = icon;
        self.drag_files = paths;
        // O compositor fica com o mouse até soltar: o botão não volta para esta janela.
        self.w.input.down = false;
    }

    /// Ícone do arrasto: o desenho do tipo do arquivo (e quantos são, se mais de um).
    fn drag_icon_surface(&mut self, paths: &[std::path::PathBuf]) -> Option<wl_surface::WlSurface> {
        use glove_paint::icons::Icon;
        let k = self.w.int_scale.max(1);
        let size = 40.0;
        let mut c = Canvas::new(size, size, k as f32)?;
        let p = &paths[0];
        let icon = if p.is_dir() {
            Icon::Folder
        } else if crate::image::format_of(p).is_some() {
            Icon::Picture
        } else {
            Icon::Document
        };
        glove_paint::icons::draw_bare(&mut c, &self.fonts, icon, 2.0, 2.0, 34.0, None);
        if paths.len() > 1 {
            let t = paths.len().to_string();
            let tw = self.fonts.measure(&t, 11.0, true);
            let bw = (tw + 10.0).max(18.0);
            c.fill(glove_paint::draw::rrect(size - bw, 0.0, bw, 16.0, 8.0), &glove_paint::draw::solid(glove_paint::draw::rgba(220, 40, 40, 240)));
            self.fonts.draw(&mut c, &t, size - bw / 2.0 - tw / 2.0, 12.0, 11.0, true, glove_paint::Color::WHITE);
        }
        let (pw, ph) = (c.px_width() as i32, c.px_height() as i32);
        let (buffer, dst) = self.pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888).ok()?;
        for (d, s) in dst.chunks_exact_mut(4).zip(c.data().chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            // Um pouco transparente, como o ícone arrastado na era.
            d[3] = s[3];
        }
        let surface = self.shells.compositor.create_surface(&self.qh);
        surface.set_buffer_scale(k);
        buffer.attach_to(&surface).ok()?;
        surface.damage_buffer(0, 0, pw, ph);
        surface.commit();
        Some(surface)
    }

    fn end_drag(&mut self) {
        self.drag_source = None;
        self.drag_files.clear();
        if let Some(s) = self.drag_icon.take() {
            s.destroy();
        }
    }

    /// Lê a área de transferência sem travar (o texto chega por um pipe).
    /// `files`: prefere a lista de arquivos (text/uri-list) ao texto. Devolve verdadeiro
    /// quando o texto já chegou (a seleção é nossa) e o quadro precisa de outra passada.
    fn request_paste(&mut self, id: crate::ui::Id, files: bool) -> bool {
        // Se a seleção é nossa, nem precisa ir ao compositor.
        if self.clip_source.is_some() || self.data_device.is_none() {
            if !self.clip_text.is_empty() {
                self.w.mem.paste_in = Some((id, self.clip_text.clone()));
                self.w.dirty = true;
                return true;
            }
            return false;
        }
        let Some(offer) = self.data_device.as_ref().and_then(|d| d.data().selection_offer()) else { return false };
        let file_mimes = [crate::clipfiles::URI_LIST, crate::clipfiles::GNOME_FILES];
        let order: Vec<&str> = if files { file_mimes.iter().chain(TEXT_MIMES.iter()).copied().collect() } else { TEXT_MIMES.to_vec() };
        let Some(mime) = offer.with_mime_types(|m| order.iter().find(|t| m.iter().any(|x| x == *t)).map(|t| t.to_string())) else { return false };
        let Ok(pipe) = offer.receive(mime) else { return false };
        let mut data = Vec::new();
        let win = self.w.id;
        let _ = self.loop_handle.insert_source(pipe, move |_, f, st: &mut Runner<A>| {
            use std::io::Read;
            // Segurança: o descritor pertence à fonte enquanto ela existir.
            let f: &mut std::fs::File = unsafe { f.get_mut() };
            let mut buf = [0u8; 8192];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => {
                        if !st.focus_id(win) {
                            return PostAction::Remove;
                        }
                        st.w.mem.paste_in = Some((id, String::from_utf8_lossy(&data).into_owned()));
                        st.w.dirty = true;
                        st.draw_now();
                        return PostAction::Remove;
                    }
                    Ok(n) => data.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return PostAction::Continue,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return PostAction::Remove,
                }
            }
        });
        false
    }

    /// Manda o quadro para a tela: só as áreas em `damage` são copiadas e
    /// anunciadas ao compositor. Sem mudança, nada é enviado.
    fn present(&mut self, c: &Canvas, damage: &[[i32; 4]]) {
        if damage.is_empty() {
            return;
        }
        let (pw, ph) = (c.px_width() as i32, c.px_height() as i32);
        // Um buffer livre do mesmo tamanho; senão, um novo (no máximo 3).
        self.w.buffers.retain(|(b, _)| b.height() == ph && b.stride() == pw * 4);
        let free = self.w.buffers.iter().position(|(b, _)| self.pool.canvas(b).is_some());
        let idx = match free {
            Some(i) => i,
            None => {
                let Ok((b, _)) = self.pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else { return };
                if self.w.buffers.len() >= 3 {
                    self.w.buffers.remove(0);
                }
                self.w.buffers.push((b, 0));
                self.w.buffers.len() - 1
            }
        };
        self.w.frame_no += 1;
        let n = self.w.frame_no;
        // O que copiar: o que mudou desde o quadro que o buffer guarda.
        let held = self.w.buffers[idx].1;
        let mut copy: Vec<[i32; 4]> = damage.to_vec();
        let known = held > 0 && (held + 1 == n || self.w.history.front().is_some_and(|(f, _)| *f <= held + 1));
        if known {
            for (f, rects) in &self.w.history {
                if *f > held {
                    copy.extend_from_slice(rects);
                }
            }
        } else {
            copy = vec![[0, 0, pw, ph]];
        }
        // Rolando uma página, cada quadro muda a área inteira: as áreas dos quadros
        // anteriores repetiam a mesma cópia (e a troca de cores) duas ou três vezes.
        let copy = sem_repetidas(copy);
        let (buffer, _) = &self.w.buffers[idx];
        let Some(dst) = self.pool.canvas(buffer) else { return };
        let src = c.data();
        // RGBA (tiny-skia) → BGRA (wl_shm Argb8888 em little-endian)
        for [x0, y0, x1, y1] in copy {
            for y in y0..y1 {
                let a = ((y * pw + x0) * 4) as usize;
                let b = ((y * pw + x1) * 4) as usize;
                for (d, s) in dst[a..b].chunks_exact_mut(4).zip(src[a..b].chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
            }
        }
        let surface = self.w.window.wl_surface();
        if let Some(v) = &self.w.viewport {
            v.set_destination(self.w.width as i32, self.w.height as i32);
        } else {
            surface.set_buffer_scale(self.w.int_scale);
        }
        for [x0, y0, x1, y1] in damage {
            surface.damage_buffer(*x0, *y0, x1 - x0, y1 - y0);
        }
        surface.frame(&self.qh, smithay_client_toolkit::compositor::FrameCallbackData(surface.clone()));
        self.w.frame_pending = true;
        if buffer.attach_to(surface).is_ok() {
            self.w.window.commit();
            self.w.shown = true;
        }
        self.w.buffers[idx].1 = n;
        self.w.history.push_back((n, damage.to_vec()));
        while self.w.history.len() > 4 {
            self.w.history.pop_front();
        }
    }

    fn key(&mut self, ev: &KeyEvent) {
        let key = match ev.keysym {
            Keysym::BackSpace => Key::Backspace,
            Keysym::Delete => Key::Delete,
            Keysym::Return | Keysym::KP_Enter => Key::Enter,
            Keysym::Escape => Key::Escape,
            Keysym::Tab | Keysym::ISO_Left_Tab => Key::Tab,
            Keysym::Left => Key::Left,
            Keysym::Right => Key::Right,
            Keysym::Up => Key::Up,
            Keysym::Down => Key::Down,
            Keysym::Home => Key::Home,
            Keysym::End => Key::End,
            Keysym::Page_Up => Key::PageUp,
            Keysym::Page_Down => Key::PageDown,
            _ => Key::Other,
        };
        // Combinação no formato dos atalhos ("Super+Shift+Q").
        let name = ev.keysym.name().map(|n| n.trim_start_matches("XK_").to_string()).unwrap_or_default();
        let is_mod = matches!(
            ev.keysym,
            Keysym::Shift_L | Keysym::Shift_R | Keysym::Control_L | Keysym::Control_R | Keysym::Alt_L | Keysym::Alt_R | Keysym::Super_L | Keysym::Super_R | Keysym::Meta_L | Keysym::Meta_R
        );
        let combo = if is_mod || name.is_empty() {
            String::new()
        } else {
            let m = &self.w.input.mods;
            let mut parts = Vec::new();
            if m.ctrl {
                parts.push("Ctrl".to_string());
            }
            if m.alt {
                parts.push("Alt".to_string());
            }
            if m.shift {
                parts.push("Shift".to_string());
            }
            if m.logo {
                parts.push("Super".to_string());
            }
            let n = if name.chars().count() == 1 { name.to_uppercase() } else { name };
            parts.push(n);
            parts.join("+")
        };
        self.w.input.keys.push(KeyPress { key, combo });
        if let Some(t) = &ev.utf8 {
            if !self.w.input.mods.ctrl && !self.w.input.mods.logo {
                self.w.input.text.push_str(t);
            }
        }
        self.w.dirty = true;
        self.draw_now();
    }
}

impl<A: App> CompositorHandler for Runner<A> {
    fn scale_factor_changed(&mut self, _: &Connection, qh: &QueueHandle<Self>, surface: &wl_surface::WlSurface, factor: i32) {
        if !self.focus_surface(surface) {
            return;
        }
        if self.w.int_scale != factor {
            self.w.int_scale = factor;
            if self.w.viewport.is_none() {
                self.request_redraw(qh);
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, _: u32) {
        if !self.focus_surface(surface) {
            return;
        }
        self.w.frame_pending = false;
        self.draw_now();
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl<A: App> OutputHandler for Runner<A> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl<A: App> WindowHandler for Runner<A> {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        if !self.focus_surface(window.wl_surface()) {
            return;
        }
        // Closing the Open/Save window = Cancel.
        if let Some((parent, key, _)) = self.w.file_of.as_ref().map(|f| (f.0, f.1, ())) {
            self.file_done.push((parent, key, crate::filebrowser::Choice::Cancelled));
            self.close_current();
            return;
        }
        // Fechar a janela do alerta = Cancelar (ou o primeiro botão).
        if let Some((parent, req)) = self.w.alert_of.clone() {
            let i = req.buttons.iter().position(|b| b.eq_ignore_ascii_case("Cancelar") || b.eq_ignore_ascii_case(crate::tr!("Cancelar"))).unwrap_or(0);
            self.alert_done.push((parent, req.key, i));
            self.close_current();
            return;
        }
        if self.app.close_window(self.w.id) {
            self.close_current();
        } else {
            self.w.dirty = true;
            self.draw_now();
        }
    }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, window: &Window, cfg: WindowConfigure, _: u32) {
        if !self.focus_surface(window.wl_surface()) {
            return;
        }
        self.w.input.maximized = cfg.is_maximized();
        if !self.w.fixed {
            let resizing = cfg.is_resizing();
            if resizing && !self.w.resizing {
                self.w.drag_base = self.w.size_anim.map(|a| a.1).unwrap_or((self.w.width, self.w.height));
                self.w.touched = (false, false);
            }
            self.w.resizing = resizing;
            if let (Some(w), Some(h)) = cfg.new_size {
                let size = (w.get(), h.get());
                // O compositor repete o último tamanho que pediu a cada configuração (ao
                // mudar o foco, por exemplo). Só vale quando muda: repetido, desfaria o
                // tamanho que o app escolheu depois (`set_window_size`).
                if self.w.cfg_size != Some(size) {
                    self.w.cfg_size = Some(size);
                    self.w.size_anim = None;
                    self.w.width = size.0;
                    self.w.height = size.1;
                    if let Some(min) = self.w.min {
                        // Arrastando, só o eixo em que o usuário mexeu passa a ser escolha
                        // dele (quem só alarga não prende a altura). Zoom e afins: os dois.
                        if resizing {
                            self.w.touched.0 |= size.0 != self.w.drag_base.0;
                            self.w.touched.1 |= size.1 != self.w.drag_base.1;
                        } else {
                            self.w.touched = (true, true);
                        }
                        self.w.chosen = chosen_size(self.w.chosen, self.w.touched, size, min);
                        self.w.width = size.0.max(min.0);
                        self.w.height = size.1.max(min.1);
                    }
                }
            }
        }
        self.w.configured = true;
        self.w.need_commit = true;
        self.request_redraw(qh);
    }
}

impl<A: App> PopupHandler for Runner<A> {
    /// O compositor pôs a lista (talvez deslizada ou encolhida para caber na tela).
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup, cfg: PopupConfigure) {
        let Some(lp) = self.list_popup.as_mut().filter(|l| &l.popup == popup) else { return };
        // A lista não coube na tela e foi deslizada até o topo e cortada: como no Tiger,
        // pede de novo sem deslizar (o item atual fica no botão e a lista rola), se
        // assim sobrar espaço para uns itens. A primeira resposta diz onde a tela começa
        // e acaba (em coordenadas da janela).
        if !lp.configured && !lp.tall_tried && cfg.height > 0 && (cfg.height as f32) < lp.full_h && smithay_client_toolkit::reexports::client::Proxy::version(lp.popup.xdg_popup()) >= 3 {
            lp.tall_tried = true;
            let (st, sb) = (cfg.position.1 as f32, (cfg.position.1 + cfg.height) as f32);
            let top = lp.anchor.y - crate::ui::POPUP_PAD - lp.selected as f32 * crate::ui::POPUP_ITEM_H;
            let visible = (top + lp.full_h).min(sb) - top.max(st);
            if visible >= (8.0 * crate::ui::POPUP_ITEM_H).min(cfg.height as f32) {
                if let Some(p) = lp.place.positioner(&self.shells.xdg, false) {
                    lp.popup.reposition(&p, 1);
                    return;
                }
            }
        }
        if cfg.height > 0 {
            lp.h = (cfg.height as f32).min(lp.full_h);
        }
        if cfg.width > 0 {
            lp.w = cfg.width as f32;
        }
        if !lp.configured {
            // O item atual em cima do botão, mesmo se a lista encolheu (rola até ele).
            let button_y = lp.anchor.y - cfg.position.1 as f32;
            let s = crate::ui::POPUP_PAD + lp.selected as f32 * crate::ui::POPUP_ITEM_H - button_y;
            lp.set_scroll(s);
            lp.reveal(lp.selected);
        } else {
            let s = lp.scroll;
            lp.set_scroll(s);
        }
        lp.configured = true;
        self.draw_list_popup();
    }

    /// Clique fora (noutro app, na mesa): fecha sem mudar o valor.
    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup) {
        if self.list_popup.as_ref().is_some_and(|l| &l.popup == popup) {
            self.finish_list_popup(None);
        }
    }
}

impl<A: App> SeatHandler for Runner<A> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer && self.pointer.is_none() {
            let surface = self.shells.compositor.create_surface(qh);
            self.themed = self.seat_state.get_pointer_with_theme::<_, ()>(qh, &seat, self.shm.wl_shm(), surface, smithay_client_toolkit::seat::pointer::ThemeSpec::default()).ok();
            self.pointer = match &self.themed {
                Some(t) => Some(t.pointer().clone()),
                None => self.seat_state.get_pointer(qh, &seat).ok(),
            };
            self.cursor_on = None;
        }
        self.seat = Some(seat.clone());
        if self.data_device.is_none() {
            if let Some(m) = &self.data_manager {
                self.data_device = Some(m.get_data_device(qh, &seat));
            }
        }
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self
                .seat_state
                .get_keyboard_with_repeat(qh, &seat, None, self.loop_handle.clone(), Box::new(|st: &mut Self, _, ev| st.key(&ev)))
                .ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer {
            self.themed = None;
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<A: App> KeyboardHandler for Runner<A> {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        self.kbd_surface = Some(surface.clone());
        if !self.focus_surface(surface) {
            return;
        }
        self.w.input.active = true;
        self.w.dirty = true;
        self.draw_now();
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
        if self.kbd_surface.as_ref() == Some(surface) {
            self.kbd_surface = None;
        }
        if !self.focus_surface(surface) {
            return;
        }
        // O teclado foi para a lista suspensa desta janela: ela continua ativa.
        if self.list_popup.as_ref().is_some_and(|l| l.win == self.w.id) {
            return;
        }
        self.w.input.active = false;
        self.w.dirty = true;
        self.draw_now();
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, serial: u32, ev: KeyEvent) {
        self.last_serial = serial;
        // Lista suspensa aberta: o teclado é dela (como um menu).
        if self.list_popup.is_some() {
            return self.list_popup_key(&ev);
        }
        self.focus_keyboard();
        self.key(&ev);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        if self.list_popup.is_some() {
            return self.list_popup_key(&ev);
        }
        self.focus_keyboard();
        self.key(&ev);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.focus_keyboard();
        self.w.input.mods = crate::ui::Mods { ctrl: m.ctrl, alt: m.alt, shift: m.shift, logo: m.logo };
    }
}

impl<A: App> PointerHandler for Runner<A> {
    fn pointer_frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        // Cada evento vai para a janela dele; depois, redesenha as que receberam.
        let mut touched: Vec<u32> = Vec::new();
        // Por janela: onde o mouse estava e se só houve movimento (sem botão, entrar, sair, roda).
        let mut motion_only: Vec<(u32, (f32, f32), bool)> = Vec::new();
        for e in events {
            if self.list_popup.as_ref().is_some_and(|l| l.popup.wl_surface() == &e.surface) {
                self.list_popup_pointer(e);
                continue;
            }
            if !self.focus_surface(&e.surface) {
                continue;
            }
            if !touched.contains(&self.w.id) {
                touched.push(self.w.id);
                motion_only.push((self.w.id, self.w.input.pos, true));
            }
            if !matches!(e.kind, PointerEventKind::Motion { .. }) {
                if let Some(m) = motion_only.iter_mut().find(|m| m.0 == self.w.id) {
                    m.2 = false;
                }
            }
            self.w.input.pos = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } => {
                    self.w.input.inside = true;
                    self.cursor_on = None;
                }
                PointerEventKind::Leave { .. } => self.w.input.inside = false,
                PointerEventKind::Motion { .. } => self.w.input.moved = true,
                // Botão direito: só para menus de contexto (não é um clique comum).
                PointerEventKind::Press { button: 0x111, serial, .. } => {
                    self.last_serial = serial;
                    self.w.input.right_pressed = true;
                }
                PointerEventKind::Release { button: 0x111, .. } => {}
                // Botão do meio: à parte (links em aba nova, colar), não é um clique comum.
                PointerEventKind::Press { button: 0x112, serial, .. } => {
                    self.last_serial = serial;
                    self.w.input.middle_pressed = true;
                }
                PointerEventKind::Release { button: 0x112, .. } => self.w.input.middle_released = true,
                PointerEventKind::Press { serial, time, .. } => {
                    self.last_serial = serial;
                    self.press_serial = serial;
                    self.w.input.down = true;
                    self.w.input.pressed = true;
                    // Clique duplo/triplo: dentro do tempo das Preferências e perto do anterior.
                    let (t0, p0, n) = self.w.last_click;
                    let near = (p0.0 - self.w.input.pos.0).abs() < 4.0 && (p0.1 - self.w.input.pos.1).abs() < 4.0;
                    let count = if time.wrapping_sub(t0) < DOUBLE_CLICK_MS.load(std::sync::atomic::Ordering::Relaxed) && near { (n % 3) + 1 } else { 1 };
                    self.w.last_click = (time, self.w.input.pos, count);
                    self.w.input.clicks = count;
                }
                PointerEventKind::Release { .. } => {
                    self.w.input.down = false;
                    self.w.input.released = true;
                }
                PointerEventKind::Axis { vertical, horizontal, .. } => {
                    self.w.input.scroll += vertical.absolute as f32;
                    self.w.input.scroll_x += horizontal.absolute as f32;
                }
            }
        }
        for id in touched {
            if self.focus_id(id) {
                // Só o mouse passando, sem entrar nem sair de nada que acende: o quadro
                // sairia igual ao anterior. Não redesenha (num processador fraco, redesenhar
                // a janela inteira a cada movimento é o que mais pesa).
                let (old, only) = motion_only.iter().find(|m| m.0 == id).map_or(((0.0, 0.0), false), |m| (m.1, m.2));
                let new = self.w.input.pos;
                let quiet = only && !self.w.input.down && !self.w.mem.pointer_sensitive && self.w.input.inside && !self.w.dirty;
                if quiet {
                    let key = |r: &crate::ui::Rect| [r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()];
                    let crossed: Vec<[u32; 4]> = self.w.mem.hover_rects.iter().filter(|r| r.contains(old) != r.contains(new)).map(key).collect();
                    if crossed.iter().all(|k| self.w.silent.contains(k)) {
                        continue;
                    }
                    self.w.probe = Some(crossed);
                } else {
                    self.w.probe = None;
                }
                self.request_redraw(qh);
            }
        }
    }
}

impl<A: App> DataDeviceHandler for Runner<A> {
    /// Algo arrastado entrou numa janela: aceita se forem arquivos; o ponto vira a
    /// posição do mouse para o app (que acende a pasta de destino).
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, dev: &WlDataDevice, x: f64, y: f64, surface: &wl_surface::WlSurface) {
        if !self.focus_surface(surface) {
            return;
        }
        self.dnd_win = Some(self.w.id);
        let offer = self.data_device.as_ref().filter(|d| d.inner() == dev).and_then(|d| d.data().drag_offer());
        if let Some(o) = &offer {
            let files = o.with_mime_types(|m| m.iter().any(|t| t == crate::clipfiles::URI_LIST));
            o.accept_mime_type(o.serial, files.then(|| crate::clipfiles::URI_LIST.to_string()));
            let (all, preferred) = match (files, self.w.mem.drop_copy) {
                (false, _) => (DndAction::empty(), DndAction::empty()),
                (true, true) => (DndAction::Copy, DndAction::Copy),
                (true, false) => (DndAction::Copy | DndAction::Move, DndAction::Move),
            };
            o.set_actions(all, preferred);
            self.w.mem.dnd_hover = files;
        }
        self.w.input.pos = (x as f32, y as f32);
        self.w.input.inside = true;
        self.w.input.moved = true;
        self.w.dirty = true;
        self.draw_now();
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        if let Some(id) = self.dnd_win.take() {
            if self.focus_id(id) {
                self.w.mem.dnd_hover = false;
                self.w.input.inside = false;
                self.w.dirty = true;
                self.draw_now();
            }
        }
    }
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, x: f64, y: f64) {
        let Some(id) = self.dnd_win else { return };
        if self.focus_id(id) {
            self.w.input.pos = (x as f32, y as f32);
            self.w.input.moved = true;
            self.w.dirty = true;
            self.draw_now();
        }
    }
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    /// Soltou aqui: lê a lista de arquivos sem travar (a fonte pode ser este app).
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, dev: &WlDataDevice) {
        let Some(id) = self.dnd_win.take() else { return };
        let Some(offer) = self.data_device.as_ref().filter(|d| d.inner() == dev).and_then(|d| d.data().drag_offer()) else { return };
        if !self.focus_id(id) || !offer.with_mime_types(|m| m.iter().any(|t| t == crate::clipfiles::URI_LIST)) {
            return;
        }
        self.w.mem.dnd_hover = false;
        let pos = self.w.input.pos;
        let Ok(pipe) = offer.receive(crate::clipfiles::URI_LIST.to_string()) else { return };
        let mut data = Vec::new();
        let _ = self.loop_handle.insert_source(pipe, move |_, f, st: &mut Runner<A>| {
            use std::io::Read;
            let f: &mut std::fs::File = unsafe { f.get_mut() };
            let mut buf = [0u8; 8192];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => {
                        offer.finish();
                        offer.destroy();
                        if st.focus_id(id) {
                            let paths = crate::clipfiles::parse(&String::from_utf8_lossy(&data));
                            if !paths.is_empty() {
                                st.w.mem.dropped = Some((paths, pos));
                            }
                            st.w.dirty = true;
                            st.draw_now();
                        }
                        return PostAction::Remove;
                    }
                    Ok(n) => data.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return PostAction::Continue,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return PostAction::Remove,
                }
            }
        });
    }
}

impl<A: App> DataOfferHandler for Runner<A> {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl<A: App> DataSourceHandler for Runner<A> {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource, mime: String, fd: WritePipe) {
        use std::io::Write;
        let mut f = std::fs::File::from(std::os::fd::OwnedFd::from(fd));
        // Arquivos (do arrasto ou copiados): o conteúdo é montado e escrito numa
        // thread (uma imagem pode ser grande e quem lê pode ser este mesmo app).
        let files = if self.drag_source.as_ref().is_some_and(|d| d.inner() == source) {
            Some((self.drag_files.clone(), false))
        } else {
            self.clip_files.clone()
        };
        match files {
            Some((paths, cut)) => {
                std::thread::spawn(move || {
                    let _ = f.write_all(&crate::clipfiles::payload(&paths, cut, &mime));
                });
            }
            None => {
                let text = self.clip_text.clone();
                std::thread::spawn(move || {
                    let _ = f.write_all(text.as_bytes());
                });
            }
        }
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        // Outro app copiou algo: a seleção deixou de ser nossa.
        if self.clip_source.as_ref().is_some_and(|s| s.inner() == source) {
            self.clip_source = None;
            self.clip_files = None;
        }
        if self.drag_source.as_ref().is_some_and(|d| d.inner() == source) {
            self.end_drag();
        }
        source.destroy();
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self.drag_source.as_ref().is_some_and(|d| d.inner() == source) {
            self.end_drag();
        }
    }
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl<A: App> ShmHandler for Runner<A> {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(@<A: App + 'static> Runner<A>);

impl<A: App> ProvidesRegistryState for Runner<A> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState,];
}

smithay_client_toolkit::delegate_dispatch2!(@<A: App + 'static> Runner<A>);

#[cfg(test)]
mod tests {
    /// Janela com mínimo que muda (Preferências): acompanha o mínimo até o usuário
    /// aumentar; o que ele aumentou continua, e nunca fica abaixo do mínimo novo.
    #[test]
    fn minimo_que_muda() {
        use super::{chosen_size, floor_size};
        // Sem mexer: a janela é o mínimo de cada painel.
        assert_eq!(floor_size((None, None), (660, 480)), (660, 480));
        assert_eq!(floor_size((None, None), (660, 300)), (660, 300));
        // Só alargou: a altura continua acompanhando o painel, mesmo que o mínimo tenha
        // caído no meio do arraste (a grade com menos linhas).
        let c = chosen_size((None, None), (true, false), (800, 480), (660, 400));
        assert_eq!(c, (Some(800), None));
        assert_eq!(floor_size(c, (660, 300)), (800, 300));
        // Aumentou os dois: vale nos outros painéis, e cresce se o painel pedir mais.
        let c = chosen_size(c, (true, true), (800, 560), (660, 480));
        assert_eq!(floor_size(c, (660, 300)), (800, 560));
        assert_eq!(floor_size(c, (660, 640)), (800, 640));
        // Estreitou sem mexer na altura: a altura escolhida continua.
        assert_eq!(chosen_size(c, (true, false), (700, 560), (660, 480)), (Some(700), Some(560)));
        // Voltou ao mínimo: acompanha de novo.
        assert_eq!(chosen_size(c, (true, true), (660, 480), (660, 480)), (None, None));
    }

    use super::frame_damage;

    #[test]
    fn areas_repetidas_saem() {
        let v = vec![[0, 0, 100, 50], [10, 10, 20, 20], [0, 0, 100, 50], [90, 40, 120, 60], [5, 5, 5, 9]];
        assert_eq!(super::sem_repetidas(v), vec![[0, 0, 100, 50], [90, 40, 120, 60]]);
    }

    #[test]
    fn so_o_que_mudou() {
        let (w, h) = (20, 16);
        let mut prev = Vec::new();
        let mut cur = vec![0u8; (w * h * 4) as usize];
        assert_eq!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]), vec![[0, 0, w, h]]);
        assert!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]).is_empty());
        let set = |cur: &mut Vec<u8>, x: i32, y: i32| cur[((y * w + x) * 4) as usize] = 255;
        set(&mut cur, 5, 3);
        assert_eq!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]), vec![[5, 3, 6, 4]]);
        assert_eq!(prev, cur);
        // Duas mudanças próximas viram uma área; uma distante, outra.
        set(&mut cur, 2, 0);
        set(&mut cur, 9, 2);
        set(&mut cur, 7, 15);
        assert_eq!(frame_damage(&mut prev, &cur, w, h, [0, 0, w, h]), vec![[2, 0, 10, 3], [7, 15, 8, 16]]);
        // Fora da região comparada não conta.
        set(&mut cur, 0, 10);
        assert!(frame_damage(&mut prev, &cur, w, h, [5, 0, w, h]).is_empty());
    }
}
