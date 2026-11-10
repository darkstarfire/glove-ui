//! Primitivas de desenho e o `Canvas`, que desenha em coordenadas lógicas
//! e gera pixels na escala da tela (1×, 1.5×, 2×…), sempre nítido.

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point,
    RadialGradient, Rect, Shader, SpreadMode, Stroke, Transform,
};

pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// Mistura duas cores (t = 0 → a, t = 1 → b).
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Color::from_rgba(l(a.red(), b.red()), l(a.green(), b.green()), l(a.blue(), b.blue()), l(a.alpha(), b.alpha()))
        .unwrap_or(a)
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    let mut c = c;
    c.set_alpha(a.clamp(0.0, 1.0));
    c
}

pub fn lighten(c: Color, t: f32) -> Color {
    mix(c, with_alpha(Color::WHITE, c.alpha()), t)
}

pub fn darken(c: Color, t: f32) -> Color {
    mix(c, with_alpha(Color::BLACK, c.alpha()), t)
}

pub fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn stops(s: &[(f32, Color)]) -> Vec<GradientStop> {
    s.iter().map(|(p, c)| GradientStop::new(*p, *c)).collect()
}

thread_local! {
    /// Os últimos degradês criados e a impressão digital dos parâmetros deles: o
    /// redesenho parcial reconhece a tinta por comparação, sem ter de resumi-la.
    static RECENT: std::cell::RefCell<(Vec<(u64, Shader<'static>)>, usize)> = const { std::cell::RefCell::new((Vec::new(), 0)) };
}

fn remember(tag: u64, p: [f32; 4], s: &[(f32, Color)], shader: &Shader<'static>) {
    if matches!(shader, Shader::SolidColor(_)) {
        return;
    }
    let mut k = Fp::new(tag);
    for v in p {
        k.f(v);
    }
    for (t, c) in s {
        k.f(*t);
        k.color(*c);
        // Cores fora de 8 bits (misturas) também contam.
        k.f(c.red() + c.green() * 3.0 + c.blue() * 7.0 + c.alpha() * 13.0);
    }
    RECENT.with(|r| {
        let mut r = r.borrow_mut();
        let (list, at) = &mut *r;
        if list.len() < 8 {
            list.push((k.finish(), shader.clone()));
        } else {
            list[*at] = (k.finish(), shader.clone());
            *at = (*at + 1) % 8;
        }
    });
}

fn recall(shader: &Shader) -> Option<u64> {
    RECENT.with(|r| r.borrow().0.iter().find(|(_, s)| s == shader).map(|(k, _)| *k))
}

/// Gradiente linear entre dois pontos.
pub fn linear(x0: f32, y0: f32, x1: f32, y1: f32, s: &[(f32, Color)]) -> Paint<'static> {
    let shader = LinearGradient::new(Point::from_xy(x0, y0), Point::from_xy(x1, y1), stops(s), SpreadMode::Pad, Transform::identity())
        .unwrap_or(Shader::SolidColor(s[0].1));
    remember(21, [x0, y0, x1, y1], s, &shader);
    Paint { shader, anti_alias: true, ..Default::default() }
}

/// Gradiente vertical (o mais usado).
pub fn vgrad(y0: f32, y1: f32, s: &[(f32, Color)]) -> Paint<'static> {
    linear(0.0, y0, 0.0, y1, s)
}

/// Gradiente radial simples (centro → raio).
pub fn radial(cx: f32, cy: f32, r: f32, s: &[(f32, Color)]) -> Paint<'static> {
    let c = Point::from_xy(cx, cy);
    let shader = RadialGradient::new(c, 0.0, c, r, stops(s), SpreadMode::Pad, Transform::identity())
        .unwrap_or(Shader::SolidColor(s[s.len() - 1].1));
    remember(22, [cx, cy, r, 0.0], s, &shader);
    Paint { shader, anti_alias: true, ..Default::default() }
}

/// Retângulo com cantos arredondados independentes (tl, tr, br, bl).
pub fn rrect4(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Option<Path> {
    // Área vazia (ex.: janela encolhendo numa animação): nada a desenhar.
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let max = w.min(h) / 2.0;
    let [tl, tr, br, bl] = r.map(|v| v.clamp(0.0, max));
    const K: f32 = 1.0 - 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + tl, y);
    pb.line_to(x + w - tr, y);
    if tr > 0.0 {
        pb.cubic_to(x + w - tr * K, y, x + w, y + tr * K, x + w, y + tr);
    }
    pb.line_to(x + w, y + h - br);
    if br > 0.0 {
        pb.cubic_to(x + w, y + h - br * K, x + w - br * K, y + h, x + w - br, y + h);
    }
    pb.line_to(x + bl, y + h);
    if bl > 0.0 {
        pb.cubic_to(x + bl * K, y + h, x, y + h - bl * K, x, y + h - bl);
    }
    pb.line_to(x, y + tl);
    if tl > 0.0 {
        pb.cubic_to(x, y + tl * K, x + tl * K, y, x + tl, y);
    }
    pb.close();
    pb.finish()
}

pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    rrect4(x, y, w, h, [r; 4])
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r)
}

pub fn oval(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    Rect::from_xywh(x, y, w, h).and_then(PathBuilder::from_oval)
}

/// Ruído determinístico barato.
#[inline]
pub fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
fn blend_px(data: &mut [u8], i: usize, r: f32, g: f32, b: f32, a: f32) {
    let inv = 1.0 - a;
    data[i] = (r * a * 255.0 + data[i] as f32 * inv) as u8;
    data[i + 1] = (g * a * 255.0 + data[i + 1] as f32 * inv) as u8;
    data[i + 2] = (b * a * 255.0 + data[i + 2] as f32 * inv) as u8;
    data[i + 3] = (a * 255.0 + data[i + 3] as f32 * inv) as u8;
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

/// Uma operação de desenho anotada (redesenho parcial): onde pinta, em pixels
/// da imagem inteira e já recortada, e uma impressão digital de tudo o que
/// decide os pixels dela. Duas iguais, na mesma ordem, pintam igual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Op {
    pub rect: [i32; 4],
    pub key: u64,
}

/// Impressão digital rápida (não criptográfica) dos parâmetros de um desenho.
#[derive(Clone, Copy)]
pub struct Fp(u64);

impl Fp {
    pub fn new(tag: u64) -> Self {
        let mut h = Fp(0x9E37_79B9_7F4A_7C15);
        h.u(tag);
        h
    }
    #[inline]
    pub fn u(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x517C_C1B7_2722_0A95);
    }
    #[inline]
    pub fn f(&mut self, x: f32) {
        self.u(x.to_bits() as u64);
    }
    pub fn color(&mut self, c: Color) {
        let c = c.to_color_u8();
        self.u(u32::from_le_bytes([c.red(), c.green(), c.blue(), c.alpha()]) as u64);
    }
    pub fn rect(&mut self, r: [i32; 4]) {
        for v in r {
            self.u(v as u32 as u64);
        }
    }
    /// Bytes (pixels de imagens, texto): quatro faixas de 64 bits por vez.
    pub fn bytes(&mut self, b: &[u8]) {
        const P: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut lanes = [1u64, 2, 3, 4];
        let mut it = b.chunks_exact(32);
        for c in &mut it {
            for (k, l) in lanes.iter_mut().enumerate() {
                let v = u64::from_le_bytes(c[k * 8..k * 8 + 8].try_into().unwrap());
                *l = (*l ^ v).wrapping_mul(P).rotate_left(29);
            }
        }
        for l in lanes {
            self.u(l);
        }
        for &x in it.remainder() {
            self.u(x as u64);
        }
        self.u(b.len() as u64);
    }
    pub fn path(&mut self, p: &Path) {
        for pt in p.points() {
            self.f(pt.x);
            self.f(pt.y);
        }
        for v in p.verbs() {
            self.u(*v as u64);
        }
    }
    /// A tinta; `false` quando não dá para resumir (padrão com imagem).
    pub fn paint(&mut self, p: &Paint) -> bool {
        self.u(p.blend_mode as u64);
        self.u(p.anti_alias as u64);
        match &p.shader {
            Shader::SolidColor(c) => {
                self.color(*c);
                true
            }
            Shader::Pattern(_) => false,
            // Degradê: o criado há pouco é reconhecido; senão, pelo texto dele (mais lento).
            g => match recall(g) {
                Some(k) => {
                    self.u(k);
                    true
                }
                None => self.debug(g),
            },
        }
    }
    fn debug(&mut self, v: &dyn std::fmt::Debug) -> bool {
        use std::fmt::Write;
        let _ = write!(FpWriter(self), "{v:?}");
        true
    }
    pub fn finish(self) -> u64 {
        self.0
    }
}

struct FpWriter<'a>(&'a mut Fp);

impl std::fmt::Write for FpWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        for c in s.as_bytes().chunks(8) {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            self.0.u(u64::from_le_bytes(b));
        }
        Ok(())
    }
}

/// Chave única: desenho que não dá para resumir é sempre tido como mudado.
fn unique_key() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed).wrapping_mul(0xA24B_AED4_963E_E407)
}

fn inter(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])]
}

fn is_empty(r: [i32; 4]) -> bool {
    r[0] >= r[2] || r[1] >= r[3]
}

fn area(r: [i32; 4]) -> i64 {
    if is_empty(r) { 0 } else { (r[2] - r[0]) as i64 * (r[3] - r[1]) as i64 }
}

fn union(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// Áreas onde dois quadros anotados diferem: as operações que não casam (de um
/// lado ou do outro). Uma operação que entrou ou saiu no meio não desalinha o
/// resto (procura o reencontro até 48 operações adiante).
pub fn diff_ops(old: &[Op], new: &[Op]) -> Vec<[i32; 4]> {
    const W: usize = 48;
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < old.len() && j < new.len() {
        if old[i] == new[j] {
            i += 1;
            j += 1;
            continue;
        }
        // Entrou algo em `new`? Saiu algo de `old`? Fica com o salto menor.
        let ins = (1..=W).take_while(|b| j + b < new.len()).find(|b| new[j + b] == old[i]);
        let del = (1..=W).take_while(|a| i + a < old.len()).find(|a| old[i + a] == new[j]);
        match (ins, del) {
            (Some(b), d) if d.is_none_or(|a| b <= a) => {
                out.extend(new[j..j + b].iter().map(|o| o.rect));
                j += b;
            }
            (_, Some(a)) => {
                out.extend(old[i..i + a].iter().map(|o| o.rect));
                i += a;
            }
            _ => {
                out.push(old[i].rect);
                out.push(new[j].rect);
                i += 1;
                j += 1;
            }
        }
    }
    out.extend(old[i..].iter().map(|o| o.rect));
    out.extend(new[j..].iter().map(|o| o.rect));
    out.retain(|r| !is_empty(*r));
    out
}

/// Junta retângulos em no máximo `max`: dois viram um quando a união não
/// custa muito mais que os dois separados (cada área a mais é uma passada).
pub fn merge_rects(mut rs: Vec<[i32; 4]>, max: usize) -> Vec<[i32; 4]> {
    // Quanto vale, em pixels, uma passada a mais do app.
    const PASS: i64 = 100_000;
    rs.retain(|r| !is_empty(*r));
    loop {
        let mut best: Option<(i64, usize, usize)> = None;
        for a in 0..rs.len() {
            for b in a + 1..rs.len() {
                let u = union(rs[a], rs[b]);
                let extra = area(u) - area(rs[a]) - area(rs[b]) + area(inter(rs[a], rs[b]));
                if best.is_none_or(|(e, _, _)| extra < e) {
                    best = Some((extra, a, b));
                }
            }
        }
        match best {
            Some((extra, a, b)) if extra <= PASS || rs.len() > max => {
                rs[a] = union(rs[a], rs[b]);
                rs.swap_remove(b);
            }
            _ => return rs,
        }
    }
}

/// Anotação em curso.
struct Rec {
    ops: Vec<Op>,
    /// Só anota, não pinta.
    dry: bool,
}

/// Superfície de desenho em coordenadas lógicas.
pub struct Canvas {
    /// Onde se desenha: a tela toda ou, com recorte, só a área recortada.
    pub pm: Pixmap,
    pub scale: f32,
    /// Tamanho lógico.
    pub w: f32,
    pub h: f32,
    /// Com recorte (redesenho parcial): a imagem inteira guardada e a posição,
    /// em pixels, da área recortada que está em `pm`.
    /// As imagens de fora, da mais externa para a mais interna (a inteira, depois
    /// a da região do redesenho parcial…), com a posição e o miolo de cada uma.
    saved: Vec<(Pixmap, (i32, i32), [i32; 4])>,
    off: (i32, i32),
    region_on: bool,
    clip_on: bool,
    empty_region: bool,
    /// Recorte pedido pelo app (áreas com rolagem) e o do redesenho parcial;
    /// vale a interseção dos dois.
    user_clip: Option<[i32; 4]>,
    region: Option<[i32; 4]>,
    /// A parte de `pm` que volta para a imagem (o recorte de verdade); `pm` tem
    /// uma folga em volta, porque traços finos na beirada de uma superfície
    /// saem diferentes do que sairiam no meio dela.
    inner: [i32; 4],
    /// O recorte não deixa nada à vista: nenhum desenho pinta.
    empty: bool,
    rec: Option<Rec>,
}

/// a·b/255 arredondado, só com inteiros.
#[inline]
fn mul255(a: u32, b: u32) -> u32 {
    let x = a * b + 128;
    (x + (x >> 8)) >> 8
}

/// Compõe uma linha de pixels pré-multiplicados (`src` sobre `dst`).
#[inline]
fn blend_row(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        match s[3] {
            0 => {}
            255 => d.copy_from_slice(s),
            a => {
                let inv = 255 - a as u32;
                for k in 0..4 {
                    d[k] = (s[k] as u32 + mul255(d[k] as u32, inv)) as u8;
                }
            }
        }
    }
}

/// Cor como RGBA pré-multiplicado em bytes.
fn premul(c: Color) -> [u8; 4] {
    let c = c.premultiply().to_color_u8();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

/// Copia um retângulo de pixels entre pixmaps (`w`×`h` a partir das origens dadas).
fn copy_px(src: &Pixmap, sx: i32, sy: i32, dst: &mut Pixmap, dx: i32, dy: i32, w: i32, h: i32) {
    let (sw, dw) = (src.width() as usize, dst.width() as usize);
    let n = w as usize * 4;
    let s = src.data();
    let d = dst.data_mut();
    for y in 0..h as usize {
        let si = ((sy as usize + y) * sw + sx as usize) * 4;
        let di = ((dy as usize + y) * dw + dx as usize) * 4;
        d[di..di + n].copy_from_slice(&s[si..si + n]);
    }
}

type MetalKey = (i32, i32, i32, i32, i32, u32, u32, [(u32, u32, u32, u32); 2]);

thread_local! {
    /// Faixas de metal escovado já calculadas (ver `brushed_metal`).
    static METAL: std::cell::RefCell<Vec<(MetalKey, Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Bytes das faixas de metal guardadas (para medir a memória).
pub fn metal_bytes() -> (usize, usize) {
    METAL.with(|m| (m.borrow().len(), m.borrow().iter().map(|(_, v)| v.len()).sum()))
}

impl Canvas {
    pub fn new(w: f32, h: f32, scale: f32) -> Option<Self> {
        let pw = (w * scale).ceil().max(1.0) as u32;
        let ph = (h * scale).ceil().max(1.0) as u32;
        Some(Self { pm: Pixmap::new(pw, ph)?, scale, w, h, saved: Vec::new(), off: (0, 0), region_on: false, clip_on: false, empty_region: false, user_clip: None, region: None, inner: [0, 0, pw as i32, ph as i32], empty: false, rec: None })
    }

    /// Limita o desenho a um retângulo em pixels físicos (`None` = a tela toda).
    /// Enquanto houver recorte, desenha-se numa superfície do tamanho dele (com
    /// os pixels atuais) que volta para a imagem ao tirar o recorte: o resultado
    /// é idêntico ao de redesenhar tudo, gastando só com a área recortada.
    /// Vale dentro da região do redesenho parcial (`set_region`), se houver.
    pub fn set_clip(&mut self, r: Option<[i32; 4]>) {
        if self.clip_on {
            self.pop();
            self.clip_on = false;
        }
        self.user_clip = r;
        self.empty = self.empty_region;
        if self.empty || self.rec.as_ref().is_some_and(|r| r.dry) {
            return;
        }
        if let Some(r) = r {
            if self.push(r) {
                self.clip_on = true;
            } else {
                self.empty = true;
            }
        }
    }

    /// Região do redesenho parcial (o kit): só ela é pintada.
    pub fn set_region(&mut self, r: Option<[i32; 4]>) {
        if self.clip_on {
            self.pop();
            self.clip_on = false;
        }
        if self.region_on {
            self.pop();
            self.region_on = false;
        }
        self.region = r;
        self.empty_region = false;
        if !self.rec.as_ref().is_some_and(|r| r.dry) {
            if let Some(r) = r {
                if self.push(r) {
                    self.region_on = true;
                } else {
                    self.empty_region = true;
                }
            }
        }
        self.set_clip(self.user_clip);
    }

    /// Entra numa superfície só com `r` (mais uma folga). `false`: nada à vista.
    fn push(&mut self, r: [i32; 4]) -> bool {
        let [x0, y0, x1, y1] = inter(r, self.inner);
        if x0 >= x1 || y0 >= y1 {
            return false;
        }
        const PAD: i32 = 3;
        let (ox, oy) = self.off;
        let (pw, ph) = (self.pm.width() as i32, self.pm.height() as i32);
        let [px0, py0, px1, py1] = [(x0 - PAD).max(ox), (y0 - PAD).max(oy), (x1 + PAD).min(ox + pw), (y1 + PAD).min(oy + ph)];
        let Some(mut sub) = Pixmap::new((px1 - px0) as u32, (py1 - py0) as u32) else { return false };
        copy_px(&self.pm, px0 - ox, py0 - oy, &mut sub, 0, 0, px1 - px0, py1 - py0);
        self.saved.push((std::mem::replace(&mut self.pm, sub), self.off, self.inner));
        self.off = (px0, py0);
        self.inner = [x0, y0, x1, y1];
        true
    }

    /// Sai da superfície recortada: o miolo dela volta para a de fora.
    fn pop(&mut self) {
        let Some((mut pm, off, inner)) = self.saved.pop() else { return };
        let [x0, y0, x1, y1] = self.inner;
        copy_px(&self.pm, x0 - self.off.0, y0 - self.off.1, &mut pm, x0 - off.0, y0 - off.1, x1 - x0, y1 - y0);
        self.pm = pm;
        self.off = off;
        self.inner = inner;
    }

    fn apply_clip(&mut self) {
        self.set_region(self.region);
    }

    fn base(&self) -> &Pixmap {
        self.saved.first().map_or(&self.pm, |s| &s.0)
    }

    /// Prepara um canvas de rascunho que vai ser copiado para este em (x, y):
    /// só anotando, ele também só anota; com recorte, pinta só a parte à vista.
    pub fn begin_sub(&self, sc: &mut Canvas, x: f32, y: f32) {
        if let Some(rec) = &self.rec {
            sc.start_record(rec.dry);
        }
        if self.rec.as_ref().is_some_and(|r| r.dry) {
        } else if !self.saved.is_empty() || self.empty {
            let (ox, oy) = ((x * self.scale).round() as i32, (y * self.scale).round() as i32);
            let b = self.bounds();
            sc.set_region(Some([b[0] - ox, b[1] - oy, b[2] - ox, b[3] - oy]));
        } else {
            sc.set_region(None);
        }
    }

    /// Copia o rascunho preparado com `begin_sub` para (x, y).
    pub fn end_sub(&mut self, sc: &mut Canvas, x: f32, y: f32) {
        if sc.recording() {
            // As operações do rascunho entram na lista deste, no lugar onde ele
            // é copiado: muda só uma linha dele, repinta só essa linha.
            let ops = sc.take_record();
            let s = self.scale;
            let (ox, oy) = ((x * s).round() as i32, (y * s).round() as i32);
            let sb = [ox, oy, ox + sc.px_width() as i32, oy + sc.px_height() as i32];
            let mut skip = self.empty;
            for o in ops {
                let r = inter([o.rect[0] + ox, o.rect[1] + oy, o.rect[2] + ox, o.rect[3] + oy], sb);
                let mut k = Fp(o.key);
                k.rect(sb);
                skip |= self.note(r, k.finish());
            }
            sc.set_region(None);
            if !skip {
                self.blit_canvas(sc, ox, oy);
            }
            return;
        }
        sc.set_region(None);
        self.draw_canvas(sc, x, y);
    }

    /// O recorte pedido pelo app (para guardar e devolver).
    pub fn clip(&self) -> Option<[i32; 4]> {
        self.user_clip
    }

    /// Começa a anotar as operações. `dry`: só anota, sem pintar nada.
    pub fn start_record(&mut self, dry: bool) {
        let cap = self.rec.as_ref().map_or(256, |r| r.ops.capacity());
        self.rec = Some(Rec { ops: Vec::with_capacity(cap), dry });
        self.apply_clip();
    }

    /// Está anotando as operações?
    pub fn recording(&self) -> bool {
        self.rec.is_some()
    }

    /// Para de anotar e devolve as operações do quadro.
    pub fn take_record(&mut self) -> Vec<Op> {
        let ops = self.rec.take().map(|r| r.ops).unwrap_or_default();
        self.apply_clip();
        ops
    }

    /// Retângulo lógico (com folga) em pixels da imagem inteira.
    pub fn px_box(&self, x: f32, y: f32, w: f32, h: f32, pad: f32) -> [i32; 4] {
        let s = self.scale;
        [((x - pad) * s).floor() as i32, ((y - pad) * s).floor() as i32, ((x + w + pad) * s).ceil() as i32, ((y + h + pad) * s).ceil() as i32]
    }

    /// Anota uma operação que pinta em `r` (pixels). Devolve `true` quando ela
    /// não deve ser pintada agora (só anotando, ou nada à vista).
    #[inline]
    pub fn note(&mut self, r: [i32; 4], key: u64) -> bool {
        if self.rec.is_some() {
            let full = [0, 0, self.px_width() as i32, self.px_height() as i32];
            let clip = self.user_clip.map_or(full, |c| inter(c, full));
            let b = inter(r, clip);
            let rec = self.rec.as_mut().unwrap();
            if !is_empty(b) {
                // O mesmo desenho com outro recorte pinta outros pixels.
                let mut h = Fp(key);
                h.rect(clip);
                rec.ops.push(Op { rect: b, key: h.finish() });
            }
            if rec.dry {
                return true;
            }
        }
        self.empty
    }

    fn note_path(&mut self, tag: u64, p: &Path, paint: &Paint, extra: f32, pad: f32) -> bool {
        if self.rec.is_none() {
            return self.empty;
        }
        let mut h = Fp::new(tag);
        h.path(p);
        h.f(extra);
        let key = if h.paint(paint) { h.finish() } else { unique_key() };
        let b = p.bounds();
        let r = self.px_box(b.x(), b.y(), b.width(), b.height(), pad);
        self.note(r, key)
    }

    /// Área em pixels (da imagem inteira) onde se pode desenhar.
    pub fn bounds(&self) -> [i32; 4] {
        if self.empty {
            return [0, 0, 0, 0];
        }
        let (ox, oy) = self.off;
        [ox, oy, ox + self.pm.width() as i32, oy + self.pm.height() as i32]
    }

    /// Posição, em pixels da imagem inteira, do pixel (0, 0) de `pm`.
    pub fn offset(&self) -> (i32, i32) {
        self.off
    }

    /// O retângulo lógico cai (ao menos em parte) dentro do recorte?
    pub fn visible(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        if self.empty {
            return false;
        }
        // Só anotando: vale o recorte do app (o que o quadro inteiro pintaria).
        let dry = self.rec.as_ref().is_some_and(|r| r.dry);
        let bounds = if dry {
            match self.user_clip {
                Some(c) => c,
                None => return true,
            }
        } else if self.saved.is_empty() {
            return true;
        } else {
            self.bounds()
        };
        let [x0, y0, x1, y1] = bounds;
        let s = self.scale;
        ((x * s).floor() as i32) < x1 && (((x + w) * s).ceil() as i32) > x0 && ((y * s).floor() as i32) < y1 && (((y + h) * s).ceil() as i32) > y0
    }

    fn path_visible(&self, p: &Path, pad: f32) -> bool {
        let b = p.bounds();
        self.visible(b.x() - pad, b.y() - pad, b.width() + pad * 2.0, b.height() + pad * 2.0)
    }

    /// Desenha um pixmap com uma transformação em pixels da imagem inteira.
    pub fn draw_pixmap_t(&mut self, img: tiny_skia::PixmapRef, paint: &PixmapPaint, t: Transform) {
        if self.rec.is_some() || self.empty {
            let mut h = Fp::new(16);
            for v in [t.sx, t.kx, t.ky, t.sy, t.tx, t.ty, paint.opacity] {
                h.f(v);
            }
            h.u(paint.blend_mode as u64);
            h.u(paint.quality as u64);
            h.u(img.width() as u64);
            h.u(img.height() as u64);
            h.bytes(img.data());
            let (iw, ih) = (img.width() as f32, img.height() as f32);
            let pts = [(0.0, 0.0), (iw, 0.0), (0.0, ih), (iw, ih)].map(|(x, y)| (t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty));
            let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
            let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
            let r = [x0.floor() as i32 - 1, y0.floor() as i32 - 1, x1.ceil() as i32 + 1, y1.ceil() as i32 + 1];
            if self.note(r, h.finish()) {
                return;
            }
        }
        let t = t.post_translate(-self.off.0 as f32, -self.off.1 as f32);
        self.pm.draw_pixmap(0, 0, img, paint, t, None);
    }

    pub fn px_width(&self) -> u32 {
        self.base().width()
    }
    pub fn px_height(&self) -> u32 {
        self.base().height()
    }
    pub fn data(&self) -> &[u8] {
        self.base().data()
    }

    fn t(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale).post_translate(-self.off.0 as f32, -self.off.1 as f32)
    }

    /// Índice em `pm` do pixel (x, y) da imagem inteira (que precisa estar em `bounds`).
    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        (((y - self.off.1) * self.pm.width() as i32 + (x - self.off.0)) * 4) as usize
    }

    pub fn clear(&mut self, c: Color) {
        let mut h = Fp::new(1);
        h.color(c);
        let full = [0, 0, self.px_width() as i32, self.px_height() as i32];
        if self.note(full, h.finish()) {
            return;
        }
        self.pm.fill(c);
    }

    pub fn fill(&mut self, path: Option<Path>, paint: &Paint) {
        self.fill_rule(path, paint, FillRule::Winding);
    }

    pub fn fill_rule(&mut self, path: Option<Path>, paint: &Paint, rule: FillRule) {
        if let Some(p) = path {
            if self.note_path(2 + rule as u64 * 100, &p, paint, 0.0, 1.0) || !self.path_visible(&p, 1.0) {
                return;
            }
            let t = self.t();
            self.pm.fill_path(&p, paint, rule, t, None);
        }
    }

    pub fn stroke(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            if self.note_path(3, &p, paint, width, width + 1.0) || !self.path_visible(&p, width + 1.0) {
                return;
            }
            let t = self.t();
            let s = Stroke { width, ..Default::default() };
            self.pm.stroke_path(&p, paint, &s, t, None);
        }
    }

    /// Retângulo de cor lisa alinhado aos pixels da tela, sem antisserrilhado: blocos
    /// vizinhos (células do terminal) se encostam sem fresta entre eles.
    pub fn fill_cell(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let s = self.scale;
        let (x0, y0) = ((x * s).round(), (y * s).round());
        let (x1, y1) = (((x + w) * s).round(), ((y + h) * s).round());
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let mut k = Fp::new(4);
        k.color(color);
        if self.note([x0 as i32, y0 as i32, x1 as i32, y1 as i32], { k.rect([x0 as i32, y0 as i32, x1 as i32, y1 as i32]); k.finish() }) {
            return;
        }
        let (x, y, w, h) = (x0 / s, y0 / s, (x1 - x0) / s, (y1 - y0) / s);
        if !self.visible(x, y, w, h) {
            return;
        }
        let mut paint = solid(color);
        paint.anti_alias = false;
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            let t = self.t();
            self.pm.fill_rect(r, &paint, t, None);
        }
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, paint: &Paint) {
        if self.rec.is_some() {
            let mut k = Fp::new(5);
            for v in [x, y, w, h] {
                k.f(v);
            }
            let key = if k.paint(paint) { k.finish() } else { unique_key() };
            if self.note(self.px_box(x, y, w, h, 1.0), key) {
                return;
            }
        }
        if !self.visible(x, y, w, h) {
            return;
        }
        // Na escala 1, dentro de um recorte: o tiny-skia desenha retângulos por
        // outro caminho quando há deslocamento, e as beiradas fracionárias saem
        // um tom diferente. Deslocando o retângulo e a tinta, o caminho é o mesmo
        // do quadro inteiro.
        if self.scale == 1.0 && self.off != (0, 0) {
            let (ox, oy) = (self.off.0 as f32, self.off.1 as f32);
            if let Some(r) = Rect::from_xywh(x - ox, y - oy, w, h) {
                if matches!(paint.shader, Shader::SolidColor(_)) {
                    self.pm.fill_rect(r, paint, Transform::identity(), None);
                } else {
                    let mut p = paint.clone();
                    p.shader.transform(Transform::from_translate(-ox, -oy));
                    self.pm.fill_rect(r, &p, Transform::identity(), None);
                }
            }
            return;
        }
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            let t = self.t();
            self.pm.fill_rect(r, paint, t, None);
        }
    }

    /// Linha de 1 pixel físico (para bordas sempre finas e nítidas).
    pub fn hline(&mut self, x: f32, y: f32, w: f32, c: Color) {
        let s = self.scale;
        let (px, py, pw) = ((x * s).round() as i32, (y * s).floor() as i32, (w * s).round() as i32);
        self.px_rect(px, py, pw, 1, c);
    }

    pub fn vline(&mut self, x: f32, y: f32, h: f32, c: Color) {
        let s = self.scale;
        let (px, py, ph) = ((x * s).floor() as i32, (y * s).round() as i32, (h * s).round() as i32);
        self.px_rect(px, py, 1, ph, c);
    }

    fn px_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        if self.rec.is_some() {
            let mut k = Fp::new(6);
            k.color(c);
            k.rect([x, y, w, h]);
            if self.note([x, y, x + w, y + h], k.finish()) {
                return;
            }
        }
        let [bx0, by0, bx1, by1] = self.bounds();
        for yy in y.max(by0)..(y + h).min(by1) {
            for xx in x.max(bx0)..(x + w).min(bx1) {
                let i = self.idx(xx, yy);
                blend_px(self.pm.data_mut(), i, c.red(), c.green(), c.blue(), c.alpha());
            }
        }
    }

    fn px_range(&self, x: f32, y: f32, w: f32, h: f32) -> (i32, i32, i32, i32) {
        let s = self.scale;
        let [bx0, by0, bx1, by1] = self.bounds();
        let x0 = ((x * s).round() as i32).clamp(bx0, bx1);
        let y0 = ((y * s).round() as i32).clamp(by0, by1);
        let x1 = (((x + w) * s).round() as i32).clamp(bx0, bx1);
        let y1 = (((y + h) * s).round() as i32).clamp(by0, by1);
        (x0, y0, x1.max(x0), y1.max(y0))
    }

    /// Listras finas horizontais; `a` e `b` alternam a cada 2 linhas lógicas.
    pub fn pinstripes(&mut self, x: f32, y: f32, w: f32, h: f32, a: Color, b: Color) {
        if self.rec.is_some() {
            let mut k = Fp::new(7);
            for v in [x, y, w, h] {
                k.f(v);
            }
            k.color(a);
            k.color(b);
            if self.note(self.px_box(x, y, w, h, 1.0), k.finish()) {
                return;
            }
        }
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        let s = self.scale;
        if x0 >= x1 {
            return;
        }
        // Linha pronta de cada cor: copiada (opaca) ou composta com inteiros.
        let n = ((x1 - x0) * 4) as usize;
        let rows = [a, b].map(|c| premul(c).repeat(n / 4));
        for yy in y0..y1 {
            let ly = ((yy as f32 + 0.5) / s - y).floor() as i32;
            let k = if (ly / 2) % 2 == 0 { 0 } else { 1 };
            let at = self.idx(x0, yy);
            let dst = &mut self.pm.data_mut()[at..at + n];
            match rows[k][3] {
                0 => {}
                255 => dst.copy_from_slice(&rows[k]),
                _ => blend_row(dst, &rows[k]),
            }
        }
    }

    /// Metal escovado: gradiente vertical + riscos horizontais aleatórios.
    /// O resultado de cada faixa fica guardado (é o mesmo a cada quadro): só a
    /// primeira vez faz a conta pixel a pixel.
    pub fn brushed_metal(&mut self, x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
        if self.rec.is_some() {
            let mut k = Fp::new(8);
            for v in [x, y, w, h] {
                k.f(v);
            }
            k.color(top);
            k.color(bottom);
            if self.note(self.px_box(x, y, w, h, 1.0), k.finish()) {
                return;
            }
        }
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let s = self.scale;
        // O gradiente usa a altura pedida, não só a parte visível.
        let ys = (y * s).round() as i32;
        let span = (((y + h) * s).round() as i32 - ys).max(1) as f32;
        let opaque = top.alpha() >= 1.0 && bottom.alpha() >= 1.0;
        let px = |xx: i32, yy: i32| -> [f32; 4] {
            let base = mix(top, bottom, (yy - ys) as f32 / span);
            let row = hash(yy as u32 * 7919) as f32 / u32::MAX as f32;
            // Riscos longos e fracos (o escovado do Tiger mal se vê).
            let seg = hash(((xx as f32 / (70.0 * s)) as u32) ^ (yy as u32).wrapping_mul(2_654_435_761));
            let n = (row - 0.5) * 0.028 + ((seg & 0xff) as f32 / 255.0 - 0.5) * 0.012;
            let c = |v: f32| (v + n).clamp(0.0, 1.0);
            [c(base.red()), c(base.green()), c(base.blue()), base.alpha()]
        };
        if !opaque {
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let [r, g, b, a] = px(xx, yy);
                    let i = self.idx(xx, yy);
                    blend_px(self.pm.data_mut(), i, r, g, b, a);
                }
            }
            return;
        }
        // Opaco: a faixa inteira (sem o recorte) fica guardada; com recorte, copia
        // só a parte à vista.
        let (pw, ph) = (self.px_width() as i32, self.px_height() as i32);
        let fx0 = ((x * s).round() as i32).clamp(0, pw);
        let fy0 = ((y * s).round() as i32).clamp(0, ph);
        let fx1 = (((x + w) * s).round() as i32).clamp(fx0, pw);
        let fy1 = (((y + h) * s).round() as i32).clamp(fy0, ph);
        let fw = (fx1 - fx0) as usize;
        let key = (fx0, fy0, fx1, fy1, ys, span.to_bits(), s.to_bits(), [top, bottom].map(|c| (c.red().to_bits(), c.green().to_bits(), c.blue().to_bits(), c.alpha().to_bits())));
        METAL.with(|m| {
            let mut m = m.borrow_mut();
            if !m.iter().any(|(k, _)| *k == key) {
                let mut strip = Vec::with_capacity(fw * 4 * (fy1 - fy0) as usize);
                for yy in fy0..fy1 {
                    for xx in fx0..fx1 {
                        let [r, g, b, _] = px(xx, yy);
                        strip.extend_from_slice(&[(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255]);
                    }
                }
                // Poucas faixas por app (barra de cima, de baixo, margens).
                if m.len() >= 12 {
                    m.remove(0);
                }
                m.push((key, strip));
            }
            let Some((_, strip)) = m.iter().find(|(k, _)| *k == key) else { return };
            let n = ((x1 - x0) * 4) as usize;
            for yy in y0..y1 {
                let from = ((yy - fy0) as usize * fw + (x0 - fx0) as usize) * 4;
                let at = self.idx(x0, yy);
                self.pm.data_mut()[at..at + n].copy_from_slice(&strip[from..from + n]);
            }
        });
    }

    /// Arredonda os cantos superiores apagando os pixels de fora.
    /// Brilho do metal escovado do Tiger: um pouco mais claro no meio da janela
    /// (`full_w` = largura dela, para as faixas separadas casarem).
    /// Brilho do metal: branco que vai de 0 nas pontas a 34 no meio de `full_w`.
    /// Só muda na horizontal: uma linha calculada e repetida (bem mais barato que o
    /// degradê genérico, e esta faixa é desenhada várias vezes por quadro).
    pub fn metal_sheen(&mut self, x: f32, y: f32, w: f32, h: f32, full_w: f32) {
        if self.rec.is_some() {
            let mut k = Fp::new(9);
            for v in [x, y, w, h, full_w] {
                k.f(v);
            }
            if self.note(self.px_box(x, y, w, h, 1.0), k.finish()) {
                return;
            }
        }
        let (x0, y0, x1, y1) = self.px_range(x, y, w, h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let s = self.scale;
        let fw = (full_w * s).max(1.0);
        let mut row = Vec::with_capacity(((x1 - x0) * 4) as usize);
        for xx in x0..x1 {
            let t = ((xx as f32 + 0.5) / fw).clamp(0.0, 1.0);
            let a = (34.0 * (1.0 - (2.0 * t - 1.0).abs())).round() as u8;
            // Branco pré-multiplicado: cor = alfa.
            row.extend_from_slice(&[a, a, a, a]);
        }
        let n = row.len();
        for yy in y0..y1 {
            let at = self.idx(x0, yy);
            blend_row(&mut self.pm.data_mut()[at..at + n], &row);
        }
    }

    /// Arredonda os cantos de baixo (janelas de metal do Tiger), apagando os pixels de fora.
    pub fn clip_bottom_corners(&mut self, r: f32) {
        let r = r * self.scale;
        let (w, h) = (self.px_width() as i32, self.px_height() as i32);
        let ri = r.ceil() as i32;
        if self.corner_notes(10, r, [[0, h - ri, ri, h], [w - ri, h - ri, w, h]]) {
            return;
        }
        let [bx0, by0, bx1, by1] = self.bounds();
        for dy in 0..ri {
            for dx in 0..ri {
                let (px, py) = (dx as f32 + 0.5, dy as f32 + 0.5);
                if px >= r || py >= r {
                    continue;
                }
                let d = ((r - px).powi(2) + (r - py).powi(2)).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0);
                if cov >= 1.0 {
                    continue;
                }
                let y = h - 1 - dy;
                for x in [dx, w - 1 - dx] {
                    if x < bx0 || x >= bx1 || y < by0 || y >= by1 {
                        continue;
                    }
                    let i = self.idx(x, y);
                    let data = self.pm.data_mut();
                    for k in 0..4 {
                        data[i + k] = (data[i + k] as f32 * cov) as u8;
                    }
                }
            }
        }
    }

    pub fn clip_top_corners(&mut self, r: f32) {
        let r = r * self.scale;
        let w = self.px_width() as i32;
        let ri = r.ceil() as i32;
        if self.corner_notes(11, r, [[0, 0, ri, ri], [w - ri, 0, w, ri]]) {
            return;
        }
        let [bx0, by0, bx1, by1] = self.bounds();
        for y in 0..ri {
            for dx in 0..ri {
                let (px, py) = (dx as f32 + 0.5, y as f32 + 0.5);
                if px >= r || py >= r {
                    continue;
                }
                let d = ((r - px).powi(2) + (r - py).powi(2)).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0);
                if cov >= 1.0 {
                    continue;
                }
                for x in [dx, w - 1 - dx] {
                    if x < bx0 || x >= bx1 || y < by0 || y >= by1 {
                        continue;
                    }
                    let i = self.idx(x, y);
                    let data = self.pm.data_mut();
                    for k in 0..4 {
                        data[i + k] = (data[i + k] as f32 * cov) as u8;
                    }
                }
            }
        }
    }

    /// Os dois cantos arredondados são duas operações (cada uma na sua ponta).
    fn corner_notes(&mut self, tag: u64, r: f32, corners: [[i32; 4]; 2]) -> bool {
        if self.rec.is_none() {
            return self.empty;
        }
        let mut skip = false;
        for c in corners {
            let mut k = Fp::new(tag);
            k.f(r);
            k.rect(c);
            skip |= self.note(c, k.finish());
        }
        skip
    }

    /// Preenche `path` só dentro de `clip` (recorte com borda suave).
    pub fn fill_clipped(&mut self, path: Option<Path>, paint: &Paint, clip: Option<Path>) {
        let (Some(p), Some(cl)) = (path, clip) else { return };
        let mut k = Fp::new(0);
        k.path(&cl);
        if self.note_path(12 ^ k.finish(), &p, paint, 0.0, 1.0) || !self.path_visible(&p, 1.0) {
            return;
        }
        let Some(mut mask) = tiny_skia::Mask::new(self.pm.width(), self.pm.height()) else { return };
        let t = self.t();
        mask.fill_path(&cl, FillRule::Winding, true, t);
        self.pm.fill_path(&p, paint, FillRule::Winding, t, Some(&mask));
    }

    /// Traço com pontas arredondadas.
    pub fn stroke_round(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            if self.note_path(13, &p, paint, width, width + 1.0) || !self.path_visible(&p, width + 1.0) {
                return;
            }
            let t = self.t();
            let s = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
            self.pm.stroke_path(&p, paint, &s, t, None);
        }
    }

    /// Copia outro canvas (mesma escala) na posição lógica (x, y).
    /// Copia um pixmap (pixels do aparelho, pré-multiplicado) com o canto em (x, y)
    /// lógicos, sem escala: bem mais rápido que `draw_pixmap_t` para imagens prontas.
    pub fn blit_pixmap(&mut self, img: &Pixmap, x: f32, y: f32) {
        let s = self.scale;
        let (ox, oy) = ((x * s).round() as i32, (y * s).round() as i32);
        let (sw, sh) = (img.width() as i32, img.height() as i32);
        if self.rec.is_some() {
            let mut k = Fp::new(14);
            k.rect([ox, oy, sw, sh]);
            k.bytes(img.data());
            if self.note([ox, oy, ox + sw, oy + sh], k.finish()) {
                return;
            }
        }
        let [bx0, by0, bx1, by1] = self.bounds();
        let (x0, x1) = (ox.max(bx0), (ox + sw).min(bx1));
        let (y0, y1) = (oy.max(by0), (oy + sh).min(by1));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let n = (x1 - x0) as usize * 4;
        let src = img.data();
        for yy in y0..y1 {
            let si = ((yy - oy) as usize * sw as usize + (x0 - ox) as usize) * 4;
            let di = self.idx(x0, yy);
            blend_row(&mut self.pm.data_mut()[di..di + n], &src[si..si + n]);
        }
    }

    pub fn draw_canvas(&mut self, other: &Canvas, x: f32, y: f32) {
        let s = self.scale;
        let (ox, oy) = ((x * s).round() as i32, (y * s).round() as i32);
        let (sw, sh) = (other.px_width() as i32, other.px_height() as i32);
        if self.rec.is_some() {
            let mut k = Fp::new(15);
            k.rect([ox, oy, sw, sh]);
            k.bytes(other.data());
            if self.note([ox, oy, ox + sw, oy + sh], k.finish()) {
                return;
            }
        }
        self.blit_canvas(other, ox, oy);
    }

    /// Compõe outro canvas com o canto em (ox, oy), em pixels (sem anotar).
    fn blit_canvas(&mut self, other: &Canvas, ox: i32, oy: i32) {
        let (sw, sh) = (other.px_width() as i32, other.px_height() as i32);
        let [bx0, by0, bx1, by1] = self.bounds();
        let (x0, x1) = (ox.max(bx0), (ox + sw).min(bx1));
        let (y0, y1) = (oy.max(by0), (oy + sh).min(by1));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let n = (x1 - x0) as usize * 4;
        let src = other.data();
        for yy in y0..y1 {
            let si = ((yy - oy) as usize * sw as usize + (x0 - ox) as usize) * 4;
            let di = self.idx(x0, yy);
            blend_row(&mut self.pm.data_mut()[di..di + n], &src[si..si + n]);
        }
    }

    /// Sombra suave de um retângulo arredondado (rw × rh) cobrindo a região
    /// lógica que começa em (ox, oy) relativa ao retângulo.
    pub fn shadow(ox: f32, oy: f32, w: f32, h: f32, rw: f32, rh: f32, spec: &ShadowSpec, scale: f32) -> Option<Canvas> {
        let mut c = Canvas::new(w, h, scale)?;
        let (pw, ph) = (c.pm.width(), c.pm.height());
        let data = c.pm.data_mut();
        let (x0, y0) = (2.0, spec.offset_y + 2.0);
        let (x1, y1) = (rw - 2.0, rh + spec.offset_y - 1.0);
        let r = spec.radius;
        for j in 0..ph {
            let py = oy + (j as f32 + 0.5) / scale;
            for i in 0..pw {
                let px = ox + (i as f32 + 0.5) / scale;
                let qx = (x0 + r - px).max(px - (x1 - r)).max(0.0);
                let qy = (y0 + r - py).max(py - (y1 - r)).max(0.0);
                let d = (qx * qx + qy * qy).sqrt() - r;
                let t = (d / spec.blur).clamp(0.0, 1.0);
                let a = spec.alpha * (1.0 - t).powi(3);
                if a > 0.002 {
                    data[((j * pw + i) * 4 + 3) as usize] = (a * 255.0) as u8;
                }
            }
        }
        Some(c)
    }
}

pub struct ShadowSpec {
    pub blur: f32,
    pub offset_y: f32,
    pub radius: f32,
    pub alpha: f32,
}

// ---------------------------------------------------------------------------
// Gel
// ---------------------------------------------------------------------------

/// Bolha de "gel" redonda: borda escura, corpo com brilho inferior
/// e reflexo branco na metade de cima.
pub fn gel_circle(c: &mut Canvas, cx: f32, cy: f32, r: f32, base: Color) {
    c.fill(circle(cx, cy, r), &solid(darken(base, 0.45)));
    let inner = r - 0.9;
    c.fill(
        circle(cx, cy, inner),
        &radial(cx, cy + inner * 0.55, inner * 1.25, &[(0.0, lighten(base, 0.55)), (0.55, base), (1.0, darken(base, 0.2))]),
    );
    let hw = inner * 1.3;
    let hh = inner * 0.95;
    c.fill(
        oval(cx - hw / 2.0, cy - inner + 0.6, hw, hh),
        &vgrad(cy - inner, cy - inner + hh, &[(0.0, rgba(255, 255, 255, 235)), (1.0, rgba(255, 255, 255, 20))]),
    );
}

/// Pílula/retângulo de gel (botões, destaque de menu, barras).
pub fn gel_rect(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32, base: Color) {
    // Vidro polido da era Aqua, em camadas: sombrinha embaixo, borda escura fina,
    // corpo com a cor mais funda logo abaixo do reflexo e acesa embaixo (a luz que
    // atravessa o vidro), um brilho difuso no fundo e o reflexo forte em cima, com
    // a borda de baixo bem definida.
    let colored = (base.red() - base.blue()).abs() + (base.green() - base.blue()).abs() + (base.red() - base.green()).abs() > 0.15;
    c.fill(rrect(x, y + 1.0, w, h, r), &solid(rgba(0, 0, 0, if colored { 45 } else { 28 })));
    c.fill(rrect(x, y, w, h, r), &solid(darken(base, if colored { 0.22 } else { 0.4 })));
    let (ix, iy, iw, ih) = (x + 1.0, y + 1.0, w - 2.0, h - 2.0);
    let ir = (r - 1.0).max(0.0);
    let body = || rrect(ix, iy, iw, ih, ir);
    if colored {
        c.fill(body(), &vgrad(iy, iy + ih, &[(0.0, lighten(base, 0.42)), (0.48, lighten(base, 0.28)), (0.56, lighten(base, 0.34)), (1.0, lighten(base, 0.88))]));
        // Brilho que vem de dentro, no fundo do botão.
        if iw > 8.0 && ih > 6.0 {
            let (cx, cy) = (ix + iw / 2.0, iy + ih * 1.05);
            let k = iw / ih.max(1.0);
            let glow = radial(0.0, 0.0, ih * 0.8, &[(0.0, with_alpha(lighten(base, 0.9), 1.0)), (1.0, with_alpha(lighten(base, 0.6), 0.0))]);
            let mut g = glow;
            g.shader.transform(tiny_skia::Transform::from_row(k * 0.6, 0.0, 0.0, 1.0, cx, cy));
            c.fill(body(), &g);
        }
    } else {
        c.fill(body(), &vgrad(iy, iy + ih, &[(0.0, darken(base, 0.04)), (0.5, darken(base, 0.1)), (0.75, darken(base, 0.04)), (1.0, lighten(base, 0.85))]));
    }
    let hh = ih * 0.48;
    let rr = (ir - 1.0).max(0.0);
    // Muito estreito ou baixo para o reflexo de cima.
    if iw <= 4.0 || hh <= 0.0 {
        return;
    }
    let inset = if colored { 1.5 } else { 2.0 };
    c.fill(
        rrect4(ix + inset, iy + 0.5, iw - 2.0 * inset, hh, [rr, rr, (rr * 0.35).min(3.0), (rr * 0.35).min(3.0)]),
        &vgrad(iy, iy + hh, &[(0.0, rgba(255, 255, 255, if colored { 250 } else { 230 })), (0.6, rgba(255, 255, 255, if colored { 150 } else { 70 })), (1.0, rgba(255, 255, 255, if colored { 95 } else { 40 }))]),
    );
}

// ---------------------------------------------------------------------------
// Progresso
// ---------------------------------------------------------------------------

/// Barra de progresso da era: trilho em gel cinza claro e preenchimento em gel azul.
/// `value` de 0 a 1; `None` = indeterminada, com listras diagonais que andam conforme
/// `phase` (0 a 1 = um período).
pub fn progress_bar(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, value: Option<f32>, phase: f32, accent: Color) {
    let r = (h / 4.0).min(3.0);
    c.fill(rrect(x, y, w, h, r), &solid(rgba(150, 150, 156, 255)));
    let (ix, iy, iw, ih) = (x + 1.0, y + 1.0, w - 2.0, h - 2.0);
    let inner = || rrect(ix, iy, iw, ih, (r - 1.0).max(0.0));
    c.fill(
        inner(),
        &vgrad(iy, iy + ih, &[(0.0, rgba(233, 233, 233, 255)), (0.3, rgba(226, 226, 226, 255)), (0.55, rgba(236, 236, 236, 255)), (0.75, rgba(248, 248, 248, 255)), (1.0, rgba(228, 228, 228, 255))]),
    );
    // Gel azul do 10.4: claro em cima, faixa mais forte no meio, bem claro embaixo.
    let blue = vgrad(iy, iy + ih, &[(0.0, lighten(accent, 0.45)), (0.4, lighten(accent, 0.3)), (0.5, lighten(accent, 0.12)), (1.0, lighten(accent, 0.65))]);
    match value {
        Some(v) => {
            let fw = (iw * v.clamp(0.0, 1.0)).round();
            if fw >= 1.0 {
                let fill = rrect(ix, iy, fw, ih, (r - 1.0).max(0.0));
                c.fill_clipped(fill, &blue, inner());
                // Textura leve de faixas verticais, como no original.
                let mut sx = ix + 3.0;
                while sx < ix + fw - 1.0 {
                    c.vline(sx, iy, ih, rgba(255, 255, 255, 22));
                    sx += 6.0;
                }
            }
        }
        None => {
            // "Poste de barbeiro": listras diagonais azuis sobre azul bem claro.
            c.fill_clipped(inner(), &vgrad(iy, iy + ih, &[(0.0, lighten(accent, 0.9)), (1.0, lighten(accent, 0.78))]), inner());
            let period = (ih * 2.0).max(8.0);
            let stripe = period / 2.0;
            let shift = phase.fract() * period;
            let mut sx = ix - ih - period + shift;
            let mut pb = tiny_skia::PathBuilder::new();
            while sx < ix + iw + ih {
                pb.move_to(sx, iy + ih);
                pb.line_to(sx + ih, iy);
                pb.line_to(sx + ih + stripe, iy);
                pb.line_to(sx + stripe, iy + ih);
                pb.close();
                sx += period;
            }
            c.fill_clipped(pb.finish(), &blue, inner());
        }
    }
    // Brilho em cima (vale para trilho e preenchimento).
    c.fill_clipped(
        rrect(ix, iy, iw, ih * 0.45, 0.0),
        &vgrad(iy, iy + ih * 0.45, &[(0.0, rgba(255, 255, 255, 110)), (1.0, rgba(255, 255, 255, 20))]),
        inner(),
    );
}

/// Giratório de 12 raios (espera). `phase` de 0 a 1 = uma volta; o raio mais forte
/// é o da vez e os seguintes vão sumindo.
pub fn spinner(c: &mut Canvas, cx: f32, cy: f32, r: f32, phase: f32, color: Color) {
    let head = (phase.fract() * 12.0).floor() as i32;
    let width = (r * 0.2).max(1.5);
    for i in 0..12 {
        let k = (head - i).rem_euclid(12) as f32;
        let alpha = 1.0 - k / 12.0 * 0.85;
        let a = i as f32 / 12.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (s, co) = a.sin_cos();
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(cx + co * r * 0.5, cy + s * r * 0.5);
        pb.line_to(cx + co * (r - width / 2.0), cy + s * (r - width / 2.0));
        c.stroke_round(pb.finish(), &solid(with_alpha(color, color.alpha() * alpha)), width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(c: &Canvas, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * c.px_width() + x) * 4) as usize;
        let d = c.data();
        (d[i], d[i + 1], d[i + 2])
    }

    #[test]
    fn progresso_pela_metade() {
        let mut c = Canvas::new(100.0, 12.0, 1.0).unwrap();
        c.clear(Color::WHITE);
        progress_bar(&mut c, 0.0, 0.0, 100.0, 12.0, Some(0.5), 0.0, rgba(45, 120, 230, 255));
        let (r, _, b) = px(&c, 25, 7);
        assert!(b > r + 40, "metade cheia deve ser azul: {r} {b}");
        let (r, g, b) = px(&c, 75, 7);
        assert!(r.abs_diff(b) < 10 && g.abs_diff(b) < 10, "metade vazia deve ser cinza: {r} {g} {b}");
    }

    #[test]
    fn listras_andam_com_a_fase() {
        let accent = rgba(45, 120, 230, 255);
        let mut a = Canvas::new(60.0, 12.0, 1.0).unwrap();
        let mut b = Canvas::new(60.0, 12.0, 1.0).unwrap();
        progress_bar(&mut a, 0.0, 0.0, 60.0, 12.0, None, 0.0, accent);
        progress_bar(&mut b, 0.0, 0.0, 60.0, 12.0, None, 0.5, accent);
        assert_ne!(a.data(), b.data());
    }

    /// Cena de teste: fundo listrado, área recortada copiada e um botão de gel
    /// cuja cor depende de `t` (como o pulso do botão padrão).
    fn scene(c: &mut Canvas, t: f32) {
        let (a, b) = (rgba(236, 236, 236, 255), rgba(242, 242, 242, 255));
        c.clear(a);
        c.pinstripes(0.0, 0.0, c.w, c.h, a, b);
        let mut sc = Canvas::new(60.0, 30.0, c.scale).unwrap();
        sc.fill(rrect(2.0, 2.0, 56.0, 26.0, 6.0), &solid(rgba(30, 90, 200, 160)));
        c.draw_canvas(&sc, 5.0, 5.0);
        gel_rect(c, 40.0, 50.0, 70.0, 22.0, 11.0, mix(rgba(40, 110, 220, 255), rgba(150, 200, 250, 255), t));
        c.stroke(rrect(1.0, 1.0, c.w - 2.0, c.h - 2.0, 4.0), &solid(rgba(0, 0, 0, 90)), 1.0);
    }

    #[test]
    fn redesenho_parcial_igual_ao_inteiro() {
        for scale in [1.0, 1.5, 2.0] {
            let mut full = Canvas::new(120.0, 90.0, scale).unwrap();
            scene(&mut full, 0.8);
            let mut part = Canvas::new(120.0, 90.0, scale).unwrap();
            scene(&mut part, 0.1);
            // Só a área do botão (com folga) é refeita.
            let r = [((38.0 * scale) as i32), ((48.0 * scale) as i32), ((112.0 * scale).ceil() as i32), ((74.0 * scale).ceil() as i32)];
            part.set_clip(Some(r));
            scene(&mut part, 0.8);
            part.set_clip(None);
            let pw = full.px_width() as usize;
            // Os degradês podem arredondar 1 nível diferente com a origem deslocada
            // (invisível); mais que isso é erro.
            let far = |a: &[u8], b: &[u8]| a.iter().zip(b).any(|(x, y)| x.abs_diff(*y) > 1);
            let bad: Vec<_> = full.data().chunks(4).zip(part.data().chunks(4)).enumerate().filter(|(_, (a, b))| far(a, b)).map(|(i, (a, b))| (i % pw, i / pw, a.to_vec(), b.to_vec())).take(5).collect();
            assert!(bad.is_empty(), "escala {scale}: parcial difere do inteiro: {bad:?}");
        }
    }

    #[test]
    fn recorte_nao_toca_fora() {
        let mut c = Canvas::new(40.0, 40.0, 1.0).unwrap();
        c.clear(rgba(255, 0, 0, 255));
        let before = c.data().to_vec();
        c.set_clip(Some([10, 10, 20, 20]));
        c.clear(rgba(0, 0, 255, 255));
        c.pinstripes(0.0, 0.0, 40.0, 40.0, rgba(0, 255, 0, 255), rgba(0, 0, 0, 255));
        c.fill(rrect(0.0, 0.0, 40.0, 40.0, 5.0), &solid(rgba(0, 0, 0, 128)));
        c.set_clip(None);
        for y in 0..40 {
            for x in 0..40 {
                let i = (y * 40 + x) * 4;
                let inside = (10..20).contains(&x) && (10..20).contains(&y);
                assert_eq!(c.data()[i..i + 4] == before[i..i + 4], !inside, "pixel ({x}, {y})");
            }
        }
    }

    /// Uma cena com o que as janelas usam: fundo listrado, metal, degradê,
    /// caminhos, linhas finas, imagem, área com recorte e cantos arredondados.
    fn cena(c: &mut Canvas, sel: usize, scroll: f32, extra: bool) {
        c.clear(Color::WHITE);
        c.pinstripes(0.0, 0.0, 200.0, 150.0, rgba(236, 236, 236, 255), rgba(244, 244, 244, 255));
        c.brushed_metal(0.0, 0.0, 200.0, 30.0, rgba(220, 220, 220, 255), rgba(180, 180, 180, 255));
        c.metal_sheen(0.0, 0.0, 200.0, 30.0, 200.0);
        c.fill_rect(10.0, 40.0, 80.0, 20.0, &vgrad(40.0, 60.0, &[(0.0, rgba(250, 250, 250, 255)), (1.0, rgba(200, 200, 210, 255))]));
        // Beiradas fracionárias (folha do Editor com zoom), lisas e em degradê.
        c.fill_rect(100.3, 34.6, 40.4, 110.2, &solid(rgba(255, 255, 255, 255)));
        c.fill_rect(99.3, 34.6, 1.0, 110.2, &solid(rgba(90, 90, 96, 120)));
        c.fill_rect(141.7, 36.2, 9.5, 100.0, &vgrad(36.2, 136.2, &[(0.0, rgba(250, 120, 120, 255)), (1.0, rgba(100, 200, 210, 255))]));
        // Lista com rolagem: só dentro da caixa.
        let s = c.scale;
        let old = c.clip();
        c.set_clip(Some([(20.0 * s) as i32, (70.0 * s) as i32, (180.0 * s) as i32, (140.0 * s) as i32]));
        for i in 0..8 {
            let y = 70.0 + i as f32 * 12.0 - scroll;
            if i == sel {
                c.fill_rect(20.0, y, 160.0, 12.0, &solid(rgba(56, 117, 215, 255)));
            }
            c.fill(rrect(24.0, y + 2.0, 8.0, 8.0, 2.0), &solid(rgba(90, 90, 200, 255)));
            c.hline(20.0, y + 11.5, 160.0, rgba(0, 0, 0, 30));
        }
        c.set_clip(old);
        c.stroke(rrect(20.5, 70.5, 159.0, 69.0, 3.0), &solid(rgba(0, 0, 0, 90)), 1.0);
        let mut img = Pixmap::new(6, 6).unwrap();
        img.fill(Color::from_rgba8(200, 30, 30, 255));
        c.blit_pixmap(&img, 150.0, 40.0);
        if extra {
            c.fill(circle(100.0, 50.0, 6.0), &solid(rgba(0, 160, 0, 200)));
        }
        c.clip_bottom_corners(6.0);
    }

    /// Redesenhar só onde a lista de operações mudou dá o mesmo que redesenhar tudo.
    #[test]
    fn anotado_parcial_igual_ao_inteiro() {
        let cases: [((usize, f32, bool), (usize, f32, bool)); 5] = [
            ((1, 0.0, false), (3, 0.0, false)),
            ((1, 0.0, false), (1, 0.0, true)),
            ((1, 0.0, true), (1, 0.0, false)),
            ((2, 0.0, false), (2, 7.0, false)),
            ((0, 0.0, false), (0, 0.0, false)),
        ];
        for scale in [1.0, 1.5] {
            for (a, b) in cases {
                let mut c = Canvas::new(200.0, 150.0, scale).unwrap();
                c.start_record(false);
                cena(&mut c, a.0, a.1, a.2);
                let old = c.take_record();
                // Anota B sem pintar.
                c.start_record(true);
                cena(&mut c, b.0, b.1, b.2);
                let new = c.take_record();
                let rects = merge_rects(diff_ops(&old, &new), 3);
                if a == b {
                    assert!(rects.is_empty(), "nada mudou: {rects:?}");
                }
                for r in &rects {
                    c.set_region(Some(*r));
                    cena(&mut c, b.0, b.1, b.2);
                    c.set_region(None);
                }
                let mut full = Canvas::new(200.0, 150.0, scale).unwrap();
                cena(&mut full, b.0, b.1, b.2);
                let (w, d1, d2) = (c.px_width() as usize, c.data(), full.data());
                let bad = (0..d1.len() / 4).find(|i| d1[i * 4..i * 4 + 4] != d2[i * 4..i * 4 + 4]);
                assert!(bad.is_none(), "escala {scale}, {a:?} → {b:?}: pixel ({}, {}) difere; áreas {rects:?}", bad.unwrap() % w, bad.unwrap() / w);
                // E a área redesenhada é bem menor que a janela.
                let total: i64 = rects.iter().map(|r| area(*r)).sum();
                assert!(total < (c.px_width() * c.px_height()) as i64 / 2, "{a:?} → {b:?}: {rects:?}");
            }
        }
    }

    #[test]
    fn diferencas_com_insercao() {
        let op = |k: u64, x: i32| Op { rect: [x, 0, x + 10, 10], key: k };
        let old = [op(1, 0), op(2, 10), op(3, 20), op(4, 30)];
        let new = [op(1, 0), op(9, 50), op(2, 10), op(3, 20), op(4, 30)];
        assert_eq!(diff_ops(&old, &new), vec![[50, 0, 60, 10]]);
        assert_eq!(diff_ops(&new, &old), vec![[50, 0, 60, 10]]);
        let changed = [op(1, 0), op(7, 10), op(3, 20), op(4, 30)];
        let mut d = diff_ops(&old, &changed);
        d.sort();
        d.dedup();
        assert_eq!(d, vec![[10, 0, 20, 10]]);
        // Dois retângulos longe um do outro ficam separados; perto, viram um.
        assert_eq!(merge_rects(vec![[0, 0, 10, 10], [1000, 1000, 1010, 1010]], 3).len(), 2);
        assert_eq!(merge_rects(vec![[0, 0, 10, 10], [12, 0, 20, 10]], 3), vec![[0, 0, 20, 10]]);
    }
}
