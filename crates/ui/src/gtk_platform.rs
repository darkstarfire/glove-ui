//! O kit sobre o GTK 4 (feature `gtk`). Cada janela do kit é uma janela do GTK: o
//! desenho do kit vai por cima, em texturas (só as faixas que mudaram são refeitas), e
//! embaixo fica um lugar para widgets do sistema (`Ui::native_host`). Onde o app apaga o
//! desenho (`Canvas::clear_rect`), o que está embaixo aparece e recebe o mouse direto.
//!
//! É o que a Jangada usa para pôr a página (WebKitGTK) na janela com o visual do Glove.
//! O resto é o mesmo kit do backend Wayland: o `Ui`, o redesenho parcial anotado, os
//! alertas em janela própria e os menus do Glove (pelo canal dos menus). Fora do Glove,
//! o menu de contexto vira um menu do GTK. As listas suspensas abrem dentro da janela.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::rc::{Rc, Weak};
use std::time::Duration;

use glove_paint::{draw::Canvas, Fonts, Theme};
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk, prelude::*};
use gtk4 as gtk;

use crate::platform::{app_icon, frame_damage, theme_from_config, App, WindowOptions, DOUBLE_CLICK_MS, OWN_FRAME, OWN_FRAME_MAIN};
use crate::ui::{AlertReq, Input, Key, KeyPress, Memory, Ui};

/// Altura (em pixels) das faixas da imagem: só as que mudaram viram textura nova.
const FAIXA: i32 = 96;

// ---------------------------------------------------------------------------
// O widget do desenho
// ---------------------------------------------------------------------------

mod imp_pintura {
    use super::*;

    #[derive(Default)]
    pub struct Pintura {
        /// As faixas da imagem (do alto para baixo).
        pub faixas: RefCell<Vec<Option<gdk::MemoryTexture>>>,
        pub escala: Cell<f64>,
        /// Tamanho da imagem em pixels.
        pub px: Cell<(i32, i32)>,
        /// A transparência de cada pixel: onde não há desenho, o mouse passa para baixo.
        pub alfa: RefCell<Vec<u8>>,
        /// O tamanho mudou (largura, altura em pontos).
        pub ao_mudar: RefCell<Option<Box<dyn Fn(i32, i32)>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Pintura {
        const NAME: &'static str = "GlovePintura";
        type Type = super::Pintura;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Pintura {
        fn dispose(&self) {
            while let Some(c) = self.obj().first_child() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for Pintura {
        fn snapshot(&self, s: &gtk::Snapshot) {
            // A imagem já vem na escala da tela (1,25, 1,5, 2…): cada pixel dela vai num
            // pixel da tela.
            let k = self.escala.get().max(0.25) as f32;
            for (i, t) in self.faixas.borrow().iter().enumerate() {
                let Some(t) = t else { continue };
                let y = (i as i32 * FAIXA) as f32 / k;
                let r = graphene::Rect::new(0.0, y, t.width() as f32 / k, t.height() as f32 / k);
                // O nó de textura simples: na escala da tela o GTK o põe pixel a pixel. O
                // com filtro (append_scaled_texture) passa por outro caminho e borra em
                // escala fracionária.
                s.append_texture(t, &r);
            }
        }

        fn size_allocate(&self, w: i32, h: i32, b: i32) {
            self.parent_size_allocate(w, h, b);
            // Os menus de contexto (popovers) ficam presos a este widget.
            let mut c = self.obj().first_child();
            while let Some(w) = c {
                if let Some(p) = w.downcast_ref::<gtk::Popover>() {
                    p.present();
                }
                c = w.next_sibling();
            }
            if let Some(f) = self.ao_mudar.borrow().as_ref() {
                f(w, h);
            }
        }

        fn contains(&self, x: f64, y: f64) -> bool {
            let a = self.alfa.borrow();
            if a.is_empty() {
                return true;
            }
            let (pw, ph) = self.px.get();
            let k = self.escala.get();
            let (px, py) = ((x * k).floor() as i32, (y * k).floor() as i32);
            if px < 0 || py < 0 || px >= pw || py >= ph {
                return false;
            }
            a[(py * pw + px) as usize] != 0
        }
    }
}

glib::wrapper! {
    /// O desenho do kit numa janela do GTK.
    pub struct Pintura(ObjectSubclass<imp_pintura::Pintura>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Pintura {
    fn new() -> Self {
        glib::Object::new()
    }

    /// Mostra o quadro: refaz as faixas que mudaram (`dano`, em pixels).
    fn mostrar(&self, c: &Canvas, dano: &[[i32; 4]], escala: f64) {
        let imp = self.imp();
        let (pw, ph) = (c.px_width() as i32, c.px_height() as i32);
        let novo = imp.px.get() != (pw, ph) || (imp.escala.get() - escala).abs() > 1e-9;
        let nf = (ph as usize).div_ceil(FAIXA as usize);
        let data = c.data();
        let dano: Vec<[i32; 4]> = if novo { vec![[0, 0, pw, ph]] } else { dano.to_vec() };
        {
            let mut faixas = imp.faixas.borrow_mut();
            let mut alfa = imp.alfa.borrow_mut();
            if novo {
                faixas.clear();
                faixas.resize(nf, None);
                alfa.clear();
                alfa.resize((pw * ph) as usize, 0);
                imp.px.set((pw, ph));
                imp.escala.set(escala);
            }
            let mut sujas = vec![false; nf];
            for [x0, y0, x1, y1] in dano {
                let (x0, y0, x1, y1) = (x0.clamp(0, pw), y0.clamp(0, ph), x1.clamp(0, pw), y1.clamp(0, ph));
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                for f in (y0 / FAIXA) as usize..((y1 + FAIXA - 1) / FAIXA) as usize {
                    if f < nf {
                        sujas[f] = true;
                    }
                }
                for y in y0..y1 {
                    let linha = (y * pw) as usize;
                    for x in x0..x1 {
                        alfa[linha + x as usize] = data[(linha + x as usize) * 4 + 3];
                    }
                }
            }
            for (f, suja) in sujas.iter().enumerate() {
                if !suja {
                    continue;
                }
                let y0 = f as i32 * FAIXA;
                let h = FAIXA.min(ph - y0);
                let (a, b) = ((y0 * pw * 4) as usize, ((y0 + h) * pw * 4) as usize);
                let bytes = glib::Bytes::from(&data[a..b]);
                faixas[f] = Some(gdk::MemoryTexture::new(pw, h, gdk::MemoryFormat::R8g8b8a8Premultiplied, &bytes, (pw * 4) as usize));
            }
        }
        self.queue_draw();
    }
}

// ---------------------------------------------------------------------------
// O lugar dos widgets do sistema
// ---------------------------------------------------------------------------

mod imp_hospede {
    use super::*;

    #[derive(Default)]
    pub struct Hospede {
        /// Cada widget e onde ele fica (x, y, largura, altura, em pontos).
        pub lugares: RefCell<Vec<(gtk::Widget, (i32, i32, i32, i32))>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Hospede {
        const NAME: &'static str = "GloveHospede";
        type Type = super::NativeHost;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Hospede {
        fn dispose(&self) {
            for (w, _) in self.lugares.borrow_mut().drain(..) {
                w.unparent();
            }
        }
    }

    impl WidgetImpl for Hospede {
        /// Não pede tamanho nenhum: a janela pode encolher à vontade (o app põe os
        /// widgets onde quiser a cada quadro).
        fn measure(&self, _o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn size_allocate(&self, _w: i32, _h: i32, _b: i32) {
            for (w, (x, y, lw, lh)) in self.lugares.borrow().iter() {
                if !w.is_visible() {
                    continue;
                }
                // O GTK reclama de alocar menos que o mínimo do widget.
                let (mw, _) = w.preferred_size();
                let (lw, lh) = ((*lw).max(mw.width()).max(1), (*lh).max(mw.height()).max(1));
                let t = gsk::Transform::new().translate(&graphene::Point::new(*x as f32, *y as f32));
                w.allocate(lw, lh, -1, Some(t));
            }
        }

        fn snapshot(&self, s: &gtk::Snapshot) {
            for (w, _) in self.lugares.borrow().iter() {
                self.obj().snapshot_child(w, s);
            }
        }
    }
}

glib::wrapper! {
    /// Onde o app põe widgets do sistema (ex.: a página do navegador), embaixo do
    /// desenho do kit. Aparecem onde o app apaga o desenho (`Canvas::clear_rect`).
    pub struct NativeHost(ObjectSubclass<imp_hospede::Hospede>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl NativeHost {
    fn new() -> Self {
        glib::Object::new()
    }

    /// Põe (ou move) um widget no retângulo (em pontos, da janela).
    pub fn place(&self, w: &impl IsA<gtk::Widget>, x: f32, y: f32, width: f32, height: f32) {
        let w = w.as_ref();
        let r = (x.round() as i32, y.round() as i32, width.round() as i32, height.round() as i32);
        let mut l = self.imp().lugares.borrow_mut();
        match l.iter_mut().find(|(o, _)| o == w) {
            Some(e) => {
                if e.1 == r {
                    return;
                }
                e.1 = r;
            }
            None => {
                w.set_parent(self);
                l.push((w.clone(), r));
            }
        }
        drop(l);
        self.queue_allocate();
    }

    /// Tira um widget daqui.
    pub fn remove(&self, w: &impl IsA<gtk::Widget>) {
        let w = w.as_ref();
        let mut l = self.imp().lugares.borrow_mut();
        if let Some(i) = l.iter().position(|(o, _)| o == w) {
            l.remove(i);
            w.unparent();
        }
    }

    /// A janela do GTK deste lugar (para quem precisa dela: tela cheia, diálogos).
    pub fn window(&self) -> Option<gtk::Window> {
        self.root().and_then(|r| r.downcast::<gtk::Window>().ok())
    }
}

// ---------------------------------------------------------------------------
// As janelas e o estado
// ---------------------------------------------------------------------------

struct Win {
    id: u32,
    window: gtk::Window,
    pintura: Pintura,
    width: u32,
    height: u32,
    scale: f64,
    fixed: bool,
    dirty: bool,
    /// Já há um quadro pedido ao relógio da tela.
    agendado: bool,
    input: Input,
    mem: Memory,
    canvas: Option<Canvas>,
    ops: Vec<glove_paint::draw::Op>,
    ops_valid: bool,
    forced: Vec<[i32; 4]>,
    prev: Vec<u8>,
    prev_key: (u32, u32, u32),
    wake_pending: bool,
    min: Option<(u32, u32)>,
    last_click: (u32, (f32, f32), u8),
    app_id: String,
    alert_of: Option<(u32, AlertReq)>,
    /// O último botão apertado (dispositivo, botão, posição na superfície, hora), para
    /// arrastar a janela pela barra do app.
    aperto: Option<(gdk::Device, u32, f64, f64, u32)>,
    cursor_on: Option<crate::ui::Cursor>,
}

struct State<A: App> {
    app: A,
    wins: Vec<Win>,
    fonts: Fonts,
    doc_fonts: Fonts,
    theme: Theme,
    laco: glib::MainLoop,
    eu: Weak<Comum<A>>,
    tick_active: bool,
    menu_stream: Option<std::os::unix::net::UnixStream>,
    menu_sent: String,
    menu_shortcuts: HashMap<String, String>,
    alert_done: Vec<(u32, u64, usize)>,
    config_fd: Option<std::os::fd::OwnedFd>,
}

/// O estado e a fila do que chegou enquanto ele estava ocupado (um sinal do GTK no meio
/// do código do kit).
struct Comum<A: App> {
    st: RefCell<State<A>>,
    fila: RefCell<Vec<Box<dyn FnOnce(&mut State<A>)>>>,
}

fn com<A: App>(c: &Rc<Comum<A>>, f: impl FnOnce(&mut State<A>) + 'static) {
    c.fila.borrow_mut().push(Box::new(f));
    esvaziar(c);
}

fn com_fraco<A: App>(c: &Weak<Comum<A>>, f: impl FnOnce(&mut State<A>) + 'static) {
    if let Some(c) = c.upgrade() {
        com(&c, f);
    }
}

fn esvaziar<A: App>(c: &Rc<Comum<A>>) {
    let Ok(mut st) = c.st.try_borrow_mut() else {
        let c2 = c.clone();
        glib::idle_add_local_once(move || esvaziar(&c2));
        return;
    };
    loop {
        let fila = std::mem::take(&mut *c.fila.borrow_mut());
        if fila.is_empty() {
            break;
        }
        for f in fila {
            f(&mut st);
        }
    }
}

/// Fundo listrado da janela (antes do app, se ele não pinta o próprio).
fn fundo<A: App>(app: &A, theme: &Theme, canvas: &mut Canvas, w: u32, h: u32) {
    if !app.paints_background() {
        canvas.clear(theme.body_a);
        canvas.pinstripes(0.0, 0.0, w as f32, h as f32, theme.body_a, theme.body_b);
    }
}

/// A tecla no formato dos atalhos ("Ctrl+Shift+T"), e qual é.
fn modificadores(state: gdk::ModifierType) -> crate::ui::Mods {
    crate::ui::Mods {
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        logo: state.contains(gdk::ModifierType::SUPER_MASK),
    }
}

fn traduzir(keyval: gdk::Key, state: gdk::ModifierType) -> (Key, String, crate::ui::Mods) {
    use gdk::Key as K;
    let key = match keyval {
        K::BackSpace => Key::Backspace,
        K::Delete | K::KP_Delete => Key::Delete,
        K::Return | K::KP_Enter => Key::Enter,
        K::Escape => Key::Escape,
        K::Tab | K::ISO_Left_Tab => Key::Tab,
        K::Left | K::KP_Left => Key::Left,
        K::Right | K::KP_Right => Key::Right,
        K::Up | K::KP_Up => Key::Up,
        K::Down | K::KP_Down => Key::Down,
        K::Home | K::KP_Home => Key::Home,
        K::End | K::KP_End => Key::End,
        K::Page_Up | K::KP_Page_Up => Key::PageUp,
        K::Page_Down | K::KP_Page_Down => Key::PageDown,
        _ => Key::Other,
    };
    let mods = modificadores(state);
    let is_mod = matches!(keyval, K::Shift_L | K::Shift_R | K::Control_L | K::Control_R | K::Alt_L | K::Alt_R | K::Super_L | K::Super_R | K::Meta_L | K::Meta_R | K::ISO_Level3_Shift);
    // Shift+Tab chega como ISO_Left_Tab: o atalho é "Shift+Tab".
    let name = if keyval == K::ISO_Left_Tab { "Tab".to_string() } else { keyval.name().map(|n| n.to_string()).unwrap_or_default() };
    let combo = if is_mod || name.is_empty() {
        String::new()
    } else {
        let mut parts = Vec::new();
        if mods.ctrl {
            parts.push("Ctrl".to_string());
        }
        if mods.alt {
            parts.push("Alt".to_string());
        }
        if mods.shift {
            parts.push("Shift".to_string());
        }
        if mods.logo {
            parts.push("Super".to_string());
        }
        parts.push(if name.chars().count() == 1 { name.to_uppercase() } else { name });
        parts.join("+")
    };
    (key, combo, mods)
}

/// Sem a sombra e o raio que o GTK desenha nas janelas com barra própria: a sombra e os
/// cantos são do Glove (o compositor os desenha nas janelas sem moldura, como nos alertas).
/// O GTK reserva 12 px de margem para a sombra no nó `decoration`, dentro da janela.
const CSS: &str = "
window.glove-bare, window.glove-bare.csd, window.glove-bare.solid-csd, window.glove-bare decoration {
  box-shadow: none; margin: 0; border-radius: 0; outline: none; border: none; padding: 0;
}
";

thread_local! {
    /// Quem redesenha as janelas a pedido de fora do kit (`redraw_soon`).
    static ACORDAR: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    static ACORDAR_PEDIDO: Cell<bool> = const { Cell::new(false) };
}

/// Pede um quadro novo em todas as janelas, logo (uma vez só, mesmo pedido várias
/// vezes): para quem muda o estado do app fora dos eventos do kit (sinais de um widget do
/// sistema, como a página que terminou de carregar).
pub fn redraw_soon() {
    if ACORDAR_PEDIDO.with(|p| p.replace(true)) {
        return;
    }
    glib::idle_add_local_once(|| {
        ACORDAR_PEDIDO.with(|p| p.set(false));
        ACORDAR.with(|a| {
            if let Some(f) = a.borrow().as_ref() {
                f();
            }
        });
    });
}

/// Abre a janela e roda até a última ser fechada.
pub fn run<A: App>(opts: WindowOptions, app: A) -> Result<(), Box<dyn std::error::Error>> {
    glove_config::tune_memory();
    // O app_id das janelas (o Dock e a barra de menu acham o app por ele).
    glib::set_prgname(Some(opts.app_id.as_str()));
    gtk::init()?;
    if let Some(d) = gdk::Display::default() {
        let css = gtk::CssProvider::new();
        css.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(&d, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let laco = glib::MainLoop::new(None, false);
    let st = State {
        app,
        wins: Vec::new(),
        fonts: Fonts::load_system().ok_or("nenhuma fonte encontrada")?,
        doc_fonts: Fonts::load_document().ok_or("nenhuma fonte encontrada")?,
        theme: theme_from_config(),
        laco: laco.clone(),
        eu: Weak::new(),
        tick_active: false,
        menu_stream: None,
        menu_sent: String::new(),
        menu_shortcuts: HashMap::new(),
        alert_done: Vec::new(),
        config_fd: glove_config::watch_fd(),
    };
    let c = Rc::new(Comum { st: RefCell::new(st), fila: RefCell::new(Vec::new()) });
    {
        let fraco = Rc::downgrade(&c);
        ACORDAR.with(|a| *a.borrow_mut() = Some(Box::new(move || com_fraco(&fraco, |st| st.redraw_all()))));
    }
    {
        let mut st = c.st.borrow_mut();
        st.eu = Rc::downgrade(&c);
        st.conectar_menus();
        st.abrir(0, &opts, false, None);
        // O tema muda quando a configuração do Glove muda.
        if let Some(fd) = st.config_fd.as_ref().map(|f| f.as_raw_fd()) {
            let fraco = Rc::downgrade(&c);
            glib_unix::unix_fd_add_local(fd, glib::IOCondition::IN, move |_, _| {
                let Some(c) = fraco.upgrade() else { return glib::ControlFlow::Break };
                if let Ok(mut st) = c.st.try_borrow_mut() {
                    let mudou = st.config_fd.as_ref().is_some_and(glove_config::drain_watch);
                    if mudou {
                        st.theme = theme_from_config();
                        st.app.config_changed();
                        st.redraw_all();
                    }
                }
                glib::ControlFlow::Continue
            });
        }
        if let Some(fd) = st.app.watch_fd() {
            st.observar(fd);
        }
        st.ligar_tick();
    }
    esvaziar(&c);
    laco.run();
    Ok(())
}

impl<A: App> State<A> {
    fn idx(&self, id: u32) -> Option<usize> {
        self.wins.iter().position(|w| w.id == id)
    }

    /// Abre uma janela. `bare`: sem moldura de ninguém (alertas); `parent`: a dona.
    fn abrir(&mut self, id: u32, opts: &WindowOptions, bare: bool, parent: Option<&gtk::Window>) {
        let window = gtk::Window::new();
        window.set_title(Some(&opts.title));
        let overlay = gtk::Overlay::new();
        let host = NativeHost::new();
        host.set_hexpand(true);
        host.set_vexpand(true);
        overlay.set_child(Some(&host));
        let pintura = Pintura::new();
        pintura.set_focusable(true);
        pintura.set_can_focus(true);
        overlay.add_overlay(&pintura);
        overlay.set_measure_overlay(&pintura, true);
        window.set_child(Some(&overlay));
        let own = OWN_FRAME.load(std::sync::atomic::Ordering::Relaxed) || (id == 0 && OWN_FRAME_MAIN.load(std::sync::atomic::Ordering::Relaxed));
        if bare || own {
            // Uma barra de título vazia: o GTK desenha a moldura (nenhuma, pelo CSS) e diz
            // ao Glove que a janela não quer a dele.
            let tb = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            tb.set_visible(false);
            window.set_titlebar(Some(&tb));
            window.add_css_class("glove-bare");
        }
        if opts.fixed {
            window.set_resizable(false);
            pintura.set_size_request(opts.size.0 as i32, opts.size.1 as i32);
        } else {
            let min = opts.min_size.unwrap_or((opts.size.0 * 3 / 4, opts.size.1 * 3 / 4));
            pintura.set_size_request(min.0.min(opts.size.0) as i32, min.1.min(opts.size.1) as i32);
            window.set_default_size(opts.size.0 as i32, opts.size.1 as i32);
        }
        if let Some(p) = parent {
            window.set_transient_for(Some(p));
            window.set_modal(true);
        }
        let mut mem = Memory::with_app_icon(app_icon(&opts.app_id));
        mem.window_id = id;
        mem.native_popups = false;
        mem.alert_native = crate::ui::native_menus() && std::env::var_os("GLOVE_ALERT_SHEET").is_none();
        mem.native_host = Some(host);
        self.conectar(id, &window, &pintura);
        self.wins.push(Win {
            id,
            window: window.clone(),
            pintura,
            width: opts.size.0,
            height: opts.size.1,
            scale: 1.0,
            fixed: opts.fixed,
            dirty: true,
            agendado: false,
            input: Input::default(),
            mem,
            canvas: None,
            ops: Vec::new(),
            ops_valid: false,
            forced: Vec::new(),
            prev: Vec::new(),
            prev_key: (0, 0, 0),
            wake_pending: false,
            min: None,
            last_click: (0, (0.0, 0.0), 0),
            app_id: opts.app_id.clone(),
            alert_of: None,
            aperto: None,
            cursor_on: None,
        });
        window.present();
        let i = self.wins.len() - 1;
        self.pedir_quadro(i);
    }

    /// Os eventos da janela e do desenho.
    fn conectar(&mut self, id: u32, window: &gtk::Window, pintura: &Pintura) {
        let eu = self.eu.clone();
        // O tamanho do desenho (a janela mudou de tamanho).
        {
            let eu = eu.clone();
            *pintura.imp().ao_mudar.borrow_mut() = Some(Box::new(move |w, h| {
                com_fraco(&eu, move |st| st.tamanho(id, w, h));
            }));
        }
        // A escala da tela (fracionária: 1,25, 1,5…).
        {
            let eu = eu.clone();
            window.connect_realize(move |w| {
                if let Some(s) = w.surface() {
                    let eu = eu.clone();
                    s.connect_notify_local(Some("scale"), move |_, _| com_fraco(&eu, move |st| st.mudou_escala(id)));
                }
                com_fraco(&eu, move |st| st.mudou_escala(id));
            });
        }
        {
            let eu = eu.clone();
            window.connect_close_request(move |_| {
                com_fraco(&eu, move |st| st.pedido_fechar(id));
                glib::Propagation::Stop
            });
        }
        {
            let eu = eu.clone();
            window.connect_is_active_notify(move |w| {
                let ativa = w.is_active();
                com_fraco(&eu, move |st| st.com_janela(id, |w| w.input.active = ativa));
            });
        }
        {
            let eu = eu.clone();
            window.connect_maximized_notify(move |w| {
                let m = w.is_maximized();
                com_fraco(&eu, move |st| st.com_janela(id, |w| w.input.maximized = m));
            });
        }
        // O teclado foi para um widget do sistema (ou voltou para o kit).
        {
            let eu = eu.clone();
            let p = pintura.clone();
            window.connect_focus_widget_notify(move |w| {
                let nativo = gtk::prelude::GtkWindowExt::focus(w).is_some_and(|f| f != *p.upcast_ref::<gtk::Widget>());
                com_fraco(&eu, move |st| {
                    st.com_janela(id, |w| {
                        if nativo {
                            w.mem.clear_focus();
                        }
                        w.input.native_focus = nativo;
                    })
                });
            });
        }
        // Teclas com o teclado num widget do sistema: os atalhos do app vêm antes.
        {
            let k = gtk::EventControllerKey::new();
            k.set_propagation_phase(gtk::PropagationPhase::Capture);
            let eu = eu.clone();
            let p = pintura.clone();
            let win = window.clone();
            k.connect_key_pressed(move |_, keyval, _, state| {
                let nativo = gtk::prelude::GtkWindowExt::focus(&win).is_some_and(|f| f != *p.upcast_ref::<gtk::Widget>());
                if !nativo {
                    return glib::Propagation::Proceed;
                }
                let (_, combo, _) = traduzir(keyval, state);
                let Some(c) = eu.upgrade() else { return glib::Propagation::Proceed };
                if combo.is_empty() {
                    return glib::Propagation::Proceed;
                }
                let tratou = match c.st.try_borrow_mut() {
                    Ok(mut st) => {
                        let t = st.app.native_key(&combo);
                        if t {
                            st.com_janela(id, |_| {});
                        }
                        t
                    }
                    Err(_) => false,
                };
                esvaziar(&c);
                if tratou { glib::Propagation::Stop } else { glib::Propagation::Proceed }
            });
            window.add_controller(k);
        }
        // Teclas no kit (com o método de entrada: acentos, ç).
        {
            let k = gtk::EventControllerKey::new();
            let im = gtk::IMMulticontext::new();
            im.set_client_widget(Some(pintura));
            k.set_im_context(Some(&im));
            {
                let eu = eu.clone();
                im.connect_commit(move |_, t| {
                    let t = t.to_string();
                    // O método de entrada só manda texto de verdade (Ctrl+letra não chega aqui).
                    com_fraco(&eu, move |st| st.com_janela(id, |w| w.input.text.push_str(&t)));
                });
            }
            {
                let eu = eu.clone();
                k.connect_key_pressed(move |_, keyval, _, state| {
                    let (key, combo, mods) = traduzir(keyval, state);
                    com_fraco(&eu, move |st| {
                        st.com_janela(id, |w| {
                            w.input.mods = mods;
                            w.input.keys.push(KeyPress { key, combo });
                        })
                    });
                    glib::Propagation::Stop
                });
            }
            {
                let eu = eu.clone();
                k.connect_modifiers(move |_, state| {
                    let mods = modificadores(state);
                    com_fraco(&eu, move |st| {
                        if let Some(i) = st.idx(id) {
                            st.wins[i].input.mods = mods;
                        }
                    });
                    glib::Propagation::Proceed
                });
            }
            let f = gtk::EventControllerFocus::new();
            {
                let im = im.clone();
                f.connect_enter(move |_| im.focus_in());
            }
            f.connect_leave(move |_| im.focus_out());
            pintura.add_controller(f);
            pintura.add_controller(k);
        }
        // O mouse: entrar, mexer, sair.
        {
            let m = gtk::EventControllerMotion::new();
            {
                let eu = eu.clone();
                m.connect_enter(move |_, x, y| {
                    com_fraco(&eu, move |st| {
                        st.com_janela(id, |w| {
                            w.input.pos = (x as f32, y as f32);
                            w.input.inside = true;
                            w.input.moved = true;
                            w.cursor_on = None;
                        })
                    })
                });
            }
            {
                let eu = eu.clone();
                m.connect_motion(move |_, x, y| com_fraco(&eu, move |st| st.mexeu(id, x as f32, y as f32)));
            }
            {
                let eu = eu.clone();
                m.connect_leave(move |_| com_fraco(&eu, move |st| st.com_janela(id, |w| w.input.inside = false)));
            }
            pintura.add_controller(m);
        }
        // Os botões (do jeito cru: arrastar não cancela nada).
        {
            let l = gtk::EventControllerLegacy::new();
            let eu = eu.clone();
            let p = pintura.clone();
            l.connect_event(move |_, ev| {
                let t = ev.event_type();
                if t != gdk::EventType::ButtonPress && t != gdk::EventType::ButtonRelease {
                    return glib::Propagation::Proceed;
                }
                let Some(b) = ev.downcast_ref::<gdk::ButtonEvent>() else { return glib::Propagation::Proceed };
                let botao = b.button();
                let (sx, sy) = ev.position().unwrap_or((0.0, 0.0));
                // Da superfície para o widget.
                let (wx, wy) = p
                    .native()
                    .and_then(|n| {
                        let (ox, oy) = n.surface_transform();
                        n.upcast_ref::<gtk::Widget>().compute_point(&p, &graphene::Point::new((sx - ox) as f32, (sy - oy) as f32))
                    })
                    .map(|q| (q.x(), q.y()))
                    .unwrap_or((sx as f32, sy as f32));
                let hora = ev.time();
                let dev = ev.device();
                let desce = t == gdk::EventType::ButtonPress;
                if desce {
                    p.grab_focus();
                }
                let mods = modificadores(ev.modifier_state());
                com_fraco(&eu, move |st| st.botao(id, botao, desce, (wx, wy), (sx, sy), hora, dev, mods));
                glib::Propagation::Proceed
            });
            pintura.add_controller(l);
        }
        // A roda e o touchpad.
        {
            let s = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
            let eu = eu.clone();
            s.connect_scroll(move |c, dx, dy| {
                // A roda vem em "cliques" (o backend Wayland recebe pixels: 15 por clique).
                let k = if c.unit() == gdk::ScrollUnit::Wheel { 15.0 } else { 1.0 };
                com_fraco(&eu, move |st| {
                    st.com_janela(id, |w| {
                        w.input.scroll += (dy * k) as f32;
                        w.input.scroll_x += (dx * k) as f32;
                    })
                });
                glib::Propagation::Stop
            });
            pintura.add_controller(s);
        }
    }

    /// Mexe na janela `id` e pede um quadro.
    fn com_janela(&mut self, id: u32, f: impl FnOnce(&mut Win)) {
        let Some(i) = self.idx(id) else { return };
        f(&mut self.wins[i]);
        self.wins[i].dirty = true;
        self.pedir_quadro(i);
    }

    fn tamanho(&mut self, id: u32, w: i32, h: i32) {
        let Some(i) = self.idx(id) else { return };
        let (w, h) = (w.max(1) as u32, h.max(1) as u32);
        let win = &mut self.wins[i];
        if (win.width, win.height) != (w, h) {
            win.width = w;
            win.height = h;
            win.dirty = true;
            self.pedir_quadro(i);
        }
    }

    fn mudou_escala(&mut self, id: u32) {
        let Some(i) = self.idx(id) else { return };
        let k = self.wins[i].window.surface().map(|s| s.scale()).unwrap_or(1.0);
        if (k - self.wins[i].scale).abs() > 1e-6 {
            self.wins[i].scale = k;
            self.wins[i].dirty = true;
            self.pedir_quadro(i);
        }
    }

    /// O mouse mexeu: só redesenha se entrou ou saiu de algo que reage a ele.
    fn mexeu(&mut self, id: u32, x: f32, y: f32) {
        let Some(i) = self.idx(id) else { return };
        let w = &mut self.wins[i];
        let old = w.input.pos;
        w.input.pos = (x, y);
        w.input.moved = true;
        w.input.inside = true;
        let quieto = !w.input.down && !w.mem.pointer_sensitive && !w.dirty && !w.mem.hover_rects.iter().any(|r| r.contains(old) != r.contains((x, y)));
        if !quieto {
            w.dirty = true;
            self.pedir_quadro(i);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn botao(&mut self, id: u32, botao: u32, desce: bool, pos: (f32, f32), sup: (f64, f64), hora: u32, dev: Option<gdk::Device>, mods: crate::ui::Mods) {
        let Some(i) = self.idx(id) else { return };
        let w = &mut self.wins[i];
        w.input.pos = pos;
        w.input.mods = mods;
        match (botao, desce) {
            (3, true) => w.input.right_pressed = true,
            (3, false) => {}
            (2, true) => w.input.middle_pressed = true,
            (2, false) => w.input.middle_released = true,
            (_, true) => {
                w.input.down = true;
                w.input.pressed = true;
                let (t0, p0, n) = w.last_click;
                let perto = (p0.0 - pos.0).abs() < 4.0 && (p0.1 - pos.1).abs() < 4.0;
                let n = if hora.wrapping_sub(t0) < DOUBLE_CLICK_MS.load(std::sync::atomic::Ordering::Relaxed) && perto { (n % 3) + 1 } else { 1 };
                w.last_click = (hora, pos, n);
                w.input.clicks = n;
                if let Some(d) = dev {
                    w.aperto = Some((d, botao, sup.0, sup.1, hora));
                }
            }
            (_, false) => {
                w.input.down = false;
                w.input.released = true;
            }
        }
        w.dirty = true;
        self.pedir_quadro(i);
    }

    /// Pede um quadro ao relógio da tela (um por vez).
    fn pedir_quadro(&mut self, i: usize) {
        let w = &mut self.wins[i];
        if w.agendado {
            return;
        }
        w.agendado = true;
        let id = w.id;
        let eu = self.eu.clone();
        w.pintura.add_tick_callback(move |_, _| {
            com_fraco(&eu, move |st| st.quadro(id));
            glib::ControlFlow::Break
        });
    }

    fn quadro(&mut self, id: u32) {
        let Some(i) = self.idx(id) else { return };
        self.wins[i].agendado = false;
        if self.wins[i].dirty {
            self.desenhar(i);
        }
    }

    fn redraw_all(&mut self) {
        for i in 0..self.wins.len() {
            self.wins[i].dirty = true;
            self.pedir_quadro(i);
        }
    }

    /// O relógio do `App::tick` (o app pode começar a pedir depois de aberto).
    fn ligar_tick(&mut self) {
        if self.tick_active {
            return;
        }
        if let Some(d) = self.app.tick() {
            self.tick_active = true;
            self.armar_tick(d);
        }
    }

    fn armar_tick(&mut self, d: Duration) {
        let eu = self.eu.clone();
        glib::timeout_add_local_once(d, move || {
            com_fraco(&eu, |st| {
                st.app.on_tick();
                if st.app.tick_redraw() {
                    st.redraw_all();
                }
                match st.app.tick() {
                    Some(d) => st.armar_tick(d),
                    None => st.tick_active = false,
                }
            })
        });
    }

    /// Observa um descritor (o do app ou um pedido por `ui.watch_fd`).
    fn observar(&mut self, fd: std::os::fd::RawFd) {
        let eu = self.eu.clone();
        glib_unix::unix_fd_add_local(fd, glib::IOCondition::IN | glib::IOCondition::HUP, move |_, _| {
            let Some(c) = eu.upgrade() else { return glib::ControlFlow::Break };
            let Ok(mut st) = c.st.try_borrow_mut() else { return glib::ControlFlow::Continue };
            let keep = st.app.fd_ready_on(fd);
            st.redraw_all();
            drop(st);
            esvaziar(&c);
            if keep { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
        });
    }

    /// Um quadro: o app anota o que desenharia; só onde mudou ele pinta de novo; só as
    /// faixas que mudaram vão para a tela.
    fn desenhar(&mut self, i: usize) {
        self.ligar_tick();
        let mut ops = Vec::new();
        for pass in 0..3 {
            let State { app, wins, fonts, doc_fonts, theme, alert_done, .. } = self;
            let w = &mut wins[i];
            w.dirty = false;
            // A janela ativa (o GTK só avisa quando muda: a que já nasce ativa ficava com
            // os botões apagados).
            w.input.active = w.window.is_active();
            w.input.maximized = w.window.is_maximized();
            w.mem.anim_rects.clear();
            w.mem.anim_full = false;
            let scale = w.scale as f32;
            let reuse = w.canvas.take().filter(|c| c.w == w.width as f32 && c.h == w.height as f32 && c.scale == scale);
            if reuse.is_none() {
                w.ops_valid = false;
            }
            let Some(mut canvas) = reuse.or_else(|| Canvas::new(w.width as f32, w.height as f32, scale)) else { return };
            canvas.dark_remap = glove_paint::theme::dark();
            glove_paint::draw::set_grad_remap((canvas.dark_remap, glove_paint::theme::modern()));
            canvas.set_clip(None);
            canvas.set_region(None);
            canvas.start_record(true);
            fundo(app, theme, &mut canvas, w.width, w.height);
            let alert = w.alert_of.clone();
            let (redraw, close) = {
                let mut ui = Ui::new(&mut canvas, theme, fonts, doc_fonts, !w.fixed, &w.input, &mut w.mem);
                match &alert {
                    Some((parent, req)) => {
                        let b: Vec<&str> = req.buttons.iter().map(String::as_str).collect();
                        if let Some(k) = ui.alert_window(req.kind, &req.title, &req.text, &b) {
                            alert_done.push((*parent, req.key, k));
                            ui.close = true;
                        }
                    }
                    None => app.ui(&mut ui),
                }
                ui.finish()
            };
            ops = canvas.take_record();
            canvas.set_clip(None);
            w.canvas = Some(canvas);
            let resized = self.depois(i);
            let w = &mut self.wins[i];
            let had_events = w.input.has_events();
            w.input.end_frame();
            if close {
                self.fechar(i);
                return;
            }
            let settled = std::mem::take(&mut w.mem.settled);
            if (redraw || (had_events && !settled) || resized) && pass < 2 {
                w.dirty = true;
                continue;
            }
            break;
        }
        let State { app, wins, fonts, doc_fonts, theme, .. } = self;
        let w = &mut wins[i];
        let Some(mut canvas) = w.canvas.take() else { return };
        if canvas.w != w.width as f32 || canvas.h != w.height as f32 {
            w.ops_valid = false;
            w.dirty = true;
        }
        let (pw, ph) = (canvas.px_width() as i32, canvas.px_height() as i32);
        let full = [0, 0, pw, ph];
        let mut rects = if w.ops_valid { glove_paint::draw::diff_ops(&w.ops, &ops) } else { vec![full] };
        rects.append(&mut w.forced);
        let clamp = |r: [i32; 4]| [r[0].max(0), r[1].max(0), r[2].min(pw), r[3].min(ph)];
        let rects = glove_paint::draw::merge_rects(rects.into_iter().map(clamp).collect(), 3);
        w.ops = ops;
        w.ops_valid = true;
        if !rects.is_empty() {
            // Passadas só para pintar: o estado do quadro já veio da passada anotada.
            let keep = (std::mem::take(&mut w.mem.hover_rects), w.mem.pointer_sensitive, std::mem::take(&mut w.mem.anim_rects), w.mem.anim_full);
            let alert = w.alert_of.clone();
            for r in &rects {
                canvas.set_region(Some(*r));
                fundo(app, theme, &mut canvas, w.width, w.height);
                let _ = {
                    let mut ui = Ui::new(&mut canvas, theme, fonts, doc_fonts, !w.fixed, &w.input, &mut w.mem);
                    match &alert {
                        Some((_, req)) => {
                            let b: Vec<&str> = req.buttons.iter().map(String::as_str).collect();
                            let _ = ui.alert_window(req.kind, &req.title, &req.text, &b);
                        }
                        None => app.ui(&mut ui),
                    }
                    ui.finish()
                };
                canvas.set_clip(None);
                canvas.set_region(None);
            }
            w.mem.drop_requests();
            w.mem.settled = false;
            (w.mem.hover_rects, w.mem.pointer_sensitive, w.mem.anim_rects, w.mem.anim_full) = keep;
        }
        let key = (canvas.px_width(), canvas.px_height(), canvas.scale.to_bits());
        if key != w.prev_key {
            w.prev.clear();
            w.prev_key = key;
        }
        let damage = if rects.is_empty() {
            Vec::new()
        } else {
            let u = rects.iter().fold([pw, ph, 0, 0], |u, r| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]);
            frame_damage(&mut w.prev, canvas.data(), pw, ph, u)
        };
        let novo = w.pintura.imp().px.get() != (pw, ph);
        if std::env::var_os("GLOVE_PROF").is_some() {
            let px: i32 = damage.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
            eprintln!("PROF janela={} escala={} gdk={:?} px={}x{} pintou={} áreas mudou={}px", w.id, w.scale, w.window.surface().map(|s| (s.scale(), s.scale_factor())), pw, ph, rects.len(), px);
        }
        // GLOVE_GTK_DUMP=pasta: guarda a imagem de cada janela (para comparar com a tela).
        if let Some(d) = std::env::var_os("GLOVE_GTK_DUMP") {
            let _ = canvas.pm.save_png(std::path::Path::new(&d).join(format!("window-{}.png", w.id)));
        }
        if !damage.is_empty() || novo {
            w.pintura.mostrar(&canvas, &damage, w.scale);
        }
        w.canvas = Some(canvas);
        if w.dirty {
            self.pedir_quadro(i);
        }
    }

    /// Os pedidos que os widgets deixaram durante o quadro. Devolve verdadeiro se o
    /// quadro precisa ser refeito na hora.
    fn depois(&mut self, i: usize) -> bool {
        self.sync_alert_window(i);
        let eu = self.eu.clone();
        let w = &mut self.wins[i];
        // O cursor pedido.
        let want = w.mem.mouse_cursor;
        if w.cursor_on != Some(want) {
            w.pintura.set_cursor_from_name(Some(match want {
                crate::ui::Cursor::Arrow => "default",
                crate::ui::Cursor::Hand => "pointer",
                crate::ui::Cursor::Text => "text",
            }));
            w.cursor_on = Some(want);
        }
        let fds = std::mem::take(&mut w.mem.watch_req);
        if std::mem::take(&mut w.mem.redraw_all) {
            let eu = eu.clone();
            glib::idle_add_local_once(move || com_fraco(&eu, |st| st.redraw_all()));
        }
        if let Some((mw, mh, _)) = w.mem.min_req.take() {
            if w.min != Some((mw, mh)) && !w.fixed {
                w.min = Some((mw, mh));
                w.pintura.set_size_request(mw as i32, mh as i32);
            }
        }
        if let Some((sw, sh, _)) = w.mem.size_req.take() {
            if (sw, sh) != (w.width, w.height) {
                if w.fixed {
                    w.pintura.set_size_request(sw as i32, sh as i32);
                } else {
                    w.window.set_default_size(sw as i32, sh as i32);
                }
            }
        }
        let cmds = std::mem::take(&mut w.mem.glove_cmds);
        if std::mem::take(&mut w.mem.resize_req) {
            if let (Some((d, b, x, y, t)), Some(top)) = (w.aperto.clone(), w.window.surface().and_then(|s| s.downcast::<gdk::Toplevel>().ok())) {
                top.begin_resize(gdk::SurfaceEdge::SouthEast, Some(&d), b as i32, x, y, t);
            }
        }
        let novas = std::mem::take(&mut w.mem.open_windows);
        if let Some(d) = w.mem.wake_after.take() {
            if !w.wake_pending {
                w.wake_pending = true;
                let id = w.id;
                let eu = eu.clone();
                glib::timeout_add_local_once(d, move || {
                    com_fraco(&eu, move |st| {
                        if let Some(i) = st.idx(id) {
                            st.wins[i].wake_pending = false;
                            st.wins[i].dirty = true;
                            st.pedir_quadro(i);
                        }
                    })
                });
            }
        }
        let fonte = w.mem.doc_font_req.take();
        if std::mem::take(&mut w.mem.move_req) {
            if let (Some((d, b, x, y, t)), Some(top)) = (w.aperto.clone(), w.window.surface().and_then(|s| s.downcast::<gdk::Toplevel>().ok())) {
                top.begin_move(&d, b as i32, x, y, t);
                w.input.down = false;
            }
        }
        if std::mem::take(&mut w.mem.minimize_req) {
            w.window.minimize();
        }
        if std::mem::take(&mut w.mem.zoom_req) {
            if w.window.is_maximized() {
                w.window.unmaximize();
            } else {
                w.window.maximize();
            }
        }
        if let Some(t) = w.mem.title.take() {
            w.window.set_title(Some(&t));
        }
        if let Some(text) = w.mem.clip_out.take() {
            w.pintura.clipboard().set_text(&text);
        }
        if let Some((paths, cut)) = w.mem.clip_files.take() {
            let uris = crate::clipfiles::payload(&paths, cut, crate::clipfiles::URI_LIST);
            let texto: String = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
            let p = gdk::ContentProvider::new_union(&[
                gdk::ContentProvider::for_bytes(crate::clipfiles::URI_LIST, &glib::Bytes::from_owned(uris)),
                gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &glib::Bytes::from_owned(texto.into_bytes())),
            ]);
            let _ = w.pintura.clipboard().set_content(Some(&p));
        }
        // Arrastar arquivos para fora não existe neste backend.
        w.mem.drag_out = None;
        if let Some(pid) = w.mem.paste_req.take() {
            w.mem.paste_files = false;
            let id = w.id;
            let eu = eu.clone();
            w.pintura.clipboard().read_text_async(None::<&gio::Cancellable>, move |r| {
                if let Ok(Some(t)) = r {
                    let t = t.to_string();
                    com_fraco(&eu, move |st| st.com_janela(id, |w| w.mem.paste_in = Some((pid, t))));
                }
            });
        }
        let ctx = w.mem.ctx_req.take();
        // Um campo do kit pegou o cursor com o teclado num widget do sistema: o teclado
        // volta para o kit.
        if w.mem.focused() && w.input.native_focus {
            w.pintura.grab_focus();
        }
        let id = w.id;
        for fd in fds {
            self.observar(fd);
        }
        if let Some(f) = fonte {
            if let Some(fonts) = Fonts::load_family(&f) {
                self.doc_fonts = fonts;
                self.wins[i].dirty = true;
            }
        }
        self.send_menus();
        for cmd in cmds {
            use std::io::Write;
            if let Some(s) = &mut self.menu_stream {
                let _ = s.write_all(format!("{cmd}\n").as_bytes());
            }
        }
        if let Some((at, items)) = ctx {
            self.menu_gtk(i, at, &items);
        }
        for (nid, opts) in novas {
            if self.idx(nid).is_none() {
                self.abrir(nid, &opts, false, None);
            }
        }
        let _ = id;
        false
    }

    /// O menu de contexto fora do Glove: um menu do GTK no ponto pedido.
    fn menu_gtk(&mut self, i: usize, at: (f32, f32), items: &[crate::menu::MenuItem]) {
        let w = &self.wins[i];
        let grupo = gio::SimpleActionGroup::new();
        let menu = gio::Menu::new();
        let mut n = 0usize;
        fn montar<A: App>(items: &[crate::menu::MenuItem], menu: &gio::Menu, grupo: &gio::SimpleActionGroup, n: &mut usize, eu: &Weak<Comum<A>>, win: u32) {
            let mut secao = gio::Menu::new();
            for it in items {
                match it {
                    crate::menu::MenuItem::Item { id, label, shortcut, enabled, checked, .. } => {
                        let nome = format!("i{n}");
                        *n += 1;
                        let a = if *checked { gio::SimpleAction::new_stateful(&nome, None, &true.to_variant()) } else { gio::SimpleAction::new(&nome, None) };
                        a.set_enabled(*enabled);
                        let (id, sc, eu) = (id.clone(), shortcut.clone(), eu.clone());
                        a.connect_activate(move |_, _| {
                            let (id, sc) = (id.clone(), sc.clone());
                            com_fraco(&eu, move |st| st.menu_escolhido(Some(win), &id, &sc));
                        });
                        grupo.add_action(&a);
                        secao.append(Some(label), Some(&format!("ctx.{nome}")));
                    }
                    crate::menu::MenuItem::Separator => {
                        menu.append_section(None, &secao);
                        secao = gio::Menu::new();
                    }
                    crate::menu::MenuItem::Submenu { label, items, .. } => {
                        let sub = gio::Menu::new();
                        montar(items, &sub, grupo, n, eu, win);
                        secao.append_submenu(Some(label), &sub);
                    }
                }
            }
            menu.append_section(None, &secao);
        }
        montar(items, &menu, &grupo, &mut n, &self.eu, w.id);
        w.window.insert_action_group("ctx", Some(&grupo));
        let pop = gtk::PopoverMenu::from_model(Some(&menu));
        pop.set_parent(&w.pintura);
        pop.set_has_arrow(false);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(at.0.round() as i32, at.1.round() as i32, 1, 1)));
        pop.set_position(gtk::PositionType::Bottom);
        pop.set_halign(gtk::Align::Start);
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
    }

    /// Item escolhido num menu (da barra do Glove ou de contexto): com atalho, vira o
    /// atalho; sem, vai para o app.
    fn menu_escolhido(&mut self, win: Option<u32>, id: &str, atalho: &str) {
        let alvo = win.and_then(|w| self.idx(w)).or_else(|| self.wins.iter().position(|w| w.window.is_active())).or(if self.wins.is_empty() { None } else { Some(0) });
        let Some(i) = alvo else { return };
        let w = &mut self.wins[i];
        let sc = if atalho.is_empty() { self.menu_shortcuts.get(id).cloned().unwrap_or_default() } else { atalho.to_string() };
        if sc.is_empty() {
            w.input.menu_actions.push(id.to_string());
        } else {
            w.input.keys.push(KeyPress { key: Key::Other, combo: sc });
        }
        w.dirty = true;
        self.pedir_quadro(i);
    }

    /// Depois do quadro: abre a janela do alerta que o app pediu (ou a fecha).
    fn sync_alert_window(&mut self, i: usize) {
        if self.wins[i].alert_of.is_some() {
            return;
        }
        let pid = self.wins[i].id;
        let want = self.wins[i].mem.alert_req.clone();
        let open = self.wins.iter().position(|o| o.alert_of.as_ref().is_some_and(|(p, _)| *p == pid));
        match (want, open) {
            (Some(req), None) => {
                let opts = WindowOptions { title: req.title.clone(), app_id: self.wins[i].app_id.clone(), size: (440, 150), fixed: true, min_size: None };
                let id = 0x4000_0000 | pid;
                let dona = self.wins[i].window.clone();
                let icone = self.wins[i].mem.app_icon;
                self.wins[i].mem.alert_open = true;
                self.abrir(id, &opts, true, Some(&dona));
                if let Some(k) = self.idx(id) {
                    self.wins[k].mem.app_icon = icone;
                    self.wins[k].alert_of = Some((pid, req));
                }
            }
            (Some(req), Some(k)) => {
                if self.wins[k].alert_of.as_ref().is_some_and(|(_, r)| *r != req) {
                    self.wins[k].alert_of = Some((pid, req));
                    self.wins[k].dirty = true;
                    self.pedir_quadro(k);
                }
                self.wins[i].mem.alert_open = true;
            }
            (None, Some(k)) => {
                let w = self.wins.remove(k);
                w.window.destroy();
                if let Some(i) = self.idx(pid) {
                    self.wins[i].mem.alert_open = false;
                }
            }
            (None, None) => {}
        }
    }

    /// O botão de fechar da janela.
    fn pedido_fechar(&mut self, id: u32) {
        let Some(i) = self.idx(id) else { return };
        if let Some((parent, req)) = self.wins[i].alert_of.clone() {
            let k = req.buttons.iter().position(|b| b.eq_ignore_ascii_case("Cancelar") || b.eq_ignore_ascii_case(crate::tr!("Cancelar"))).unwrap_or(0);
            self.alert_done.push((parent, req.key, k));
            self.fechar(i);
            return;
        }
        if self.app.close_window(id) {
            self.fechar(i);
        } else {
            self.wins[i].dirty = true;
            self.pedir_quadro(i);
        }
    }

    /// Fecha a janela `i`; sem nenhuma, o app termina.
    fn fechar(&mut self, i: usize) {
        let w = self.wins.remove(i);
        let id = w.id;
        w.window.destroy();
        if let Some((parent, _)) = w.alert_of {
            // A resposta vai para a dona.
            let done: Vec<(u64, usize)> = self.alert_done.iter().filter(|d| d.0 == parent).map(|d| (d.1, d.2)).collect();
            self.alert_done.retain(|d| d.0 != parent);
            if let Some(k) = self.idx(parent) {
                self.wins[k].mem.alert_results.extend(done);
                self.wins[k].mem.alert_open = false;
                self.wins[k].dirty = true;
                self.pedir_quadro(k);
            }
            return;
        }
        // Os alertas desta vão junto.
        while let Some(k) = self.wins.iter().position(|o| o.alert_of.as_ref().is_some_and(|(p, _)| *p == id)) {
            self.wins.remove(k).window.destroy();
        }
        self.app.window_closed(id);
        if self.wins.is_empty() {
            self.laco.quit();
        }
    }

    /// Conecta ao Glove para mostrar os menus na barra global (se houver).
    fn conectar_menus(&mut self) {
        use std::io::Write;
        let Some(path) = std::env::var_os("GLOVE_MENU_SOCKET").filter(|p| !p.is_empty()) else { return };
        let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) else { return };
        // As janelas sem barra do Glove (a principal com moldura própria, os alertas) ganham
        // a sombra e os cantos dele: o GTK guarda em volta delas uma margem invisível para
        // redimensionar, e sem o pedido o Glove as trataria como as de apps com sombra própria.
        if stream.write_all(format!("HELLO\t{}\nMOLDURA\tglove\n", std::process::id()).as_bytes()).is_err() {
            return;
        }
        let Ok(mut reader) = stream.try_clone() else { return };
        let _ = reader.set_nonblocking(true);
        let fd = reader.as_raw_fd();
        let eu = self.eu.clone();
        let mut buf = Vec::new();
        glib_unix::unix_fd_add_local(fd, glib::IOCondition::IN | glib::IOCondition::HUP, move |_, _| {
            use std::io::Read;
            let mut tmp = [0u8; 1024];
            let mut fim = false;
            loop {
                match reader.read(&mut tmp) {
                    Ok(0) => {
                        fim = true;
                        break;
                    }
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        fim = true;
                        break;
                    }
                }
            }
            while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                if let Some(id) = line.strip_prefix("ACT\t") {
                    let id = id.to_string();
                    com_fraco(&eu, move |st| st.menu_escolhido(None, &id, ""));
                }
            }
            if fim { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
        });
        self.menu_stream = Some(stream);
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
}
