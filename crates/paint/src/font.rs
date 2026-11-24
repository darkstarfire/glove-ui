//! Texto com ab_glyph: a fonte é lida sob demanda (só os caracteres usados
//! viram bitmap), então carregar uma fonte custa só o tamanho do arquivo.

use std::cell::RefCell;
use std::collections::HashMap;

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont, point};

/// Fonte lida do arquivo mapeado na memória (ver `map_file`).
type FontVec = FontRef<'static>;
use tiny_skia::Color;

use crate::draw::Canvas;

// Onde as distribuições guardam a fonte: Debian/Ubuntu (truetype/dejavu), Arch e
// Slackware (TTF), Fedora (dejavu-sans-fonts). Nas outras, o fontconfig responde (`load`).
const REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
];
const BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
];

/// O arquivo da fonte mapeado na memória (só leitura), em vez de copiado:
/// as páginas vêm do cache de disco do sistema e são as mesmas para todos os
/// processos do Glove que usam a fonte. Cada arquivo é mapeado uma vez só.
fn map_file(path: &std::path::Path) -> Option<&'static [u8]> {
    use std::collections::HashMap;
    use std::os::fd::AsRawFd;
    static MAPS: std::sync::Mutex<Option<HashMap<std::path::PathBuf, (usize, usize)>>> = std::sync::Mutex::new(None);
    let mut maps = MAPS.lock().ok()?;
    let maps = maps.get_or_insert_with(HashMap::new);
    if let Some(&(ptr, len)) = maps.get(path) {
        return Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) });
    }
    let file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len() as usize;
    if len == 0 {
        return None;
    }
    let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_PRIVATE, file.as_raw_fd(), 0) };
    if ptr == libc::MAP_FAILED {
        // Sem mapeamento (sistema de arquivos estranho): lê e guarda para sempre.
        let data: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
        maps.insert(path.to_path_buf(), (data.as_ptr() as usize, data.len()));
        return Some(data);
    }
    maps.insert(path.to_path_buf(), (ptr as usize, len));
    Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) })
}

fn open_font(path: &std::path::Path, index: u32) -> Option<FontVec> {
    FontRef::try_from_slice_and_index(map_file(path)?, index).ok()
}

/// A fonte: primeiro nos lugares conhecidos (sem rodar nada). Numa distribuição que a
/// guarda noutra pasta, o fontconfig diz onde está a família (ou a mais parecida que
/// houver): sem isto, o Glove nem abria onde a pasta não estava na lista (o Fedora).
fn load(paths: &[&str], family: &str) -> Option<FontVec> {
    paths.iter().find_map(|p| open_font(std::path::Path::new(p), 0)).or_else(|| fc_match(family).and_then(|p| open_font(&p, 0)))
}

/// Bitmap de um caractere já rasterizado.
struct Glyph {
    left: i32,
    top: i32,
    w: usize,
    h: usize,
    cov: Vec<u8>,
}

type Key = (char, u32, bool);

pub struct Fonts {
    regular: FontVec,
    bold: Option<FontVec>,
    cache: RefCell<HashMap<Key, Glyph>>,
    /// Fontes reserva (ícones Nerd Font, símbolos), lidas só na primeira vez
    /// que aparece um caractere que a fonte principal não tem.
    fallbacks: std::cell::OnceCell<Vec<(String, std::cell::OnceCell<Option<FontVec>>)>>,
    /// Número único deste conjunto de fontes (o redesenho parcial compara textos por ele).
    id: u64,
}

impl Fonts {
    /// Letras desenhadas guardadas: quantas e quantos bytes (para medir a memória).
    pub fn glyph_bytes(&self) -> (usize, usize) {
        let c = self.cache.borrow();
        (c.len(), c.values().map(|g| g.cov.len() + std::mem::size_of::<Glyph>()).sum())
    }
}

fn new_fonts(regular: FontVec, bold: Option<FontVec>) -> Fonts {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Fonts { regular, bold, cache: RefCell::new(HashMap::new()), fallbacks: std::cell::OnceCell::new(), id }
}

/// Arquivo de uma família de fonte instalada (via fontconfig).
/// O arquivo da fonte que o fontconfig escolhe para o padrão (ex.: "Liberation
/// Sans:bold"); None sem o fontconfig. Para quem precisa do arquivo (o PDF do Editor).
pub fn font_file(pattern: &str) -> Option<std::path::PathBuf> {
    fc_match(pattern)
}

fn fc_match(pattern: &str) -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", pattern]).output().ok()?;
    let p = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    p.is_file().then_some(p)
}

/// Fontes reserva (só os caminhos; cada uma é lida quando for necessária),
/// da mais específica para a mais genérica.
fn fallback_fonts() -> Vec<(String, std::cell::OnceCell<Option<FontVec>>)> {
    let mut files: Vec<String> = Vec::new();
    if let Ok(out) = std::process::Command::new("fc-list").args([":", "file", "family"]).output() {
        let list = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut nerd: Vec<(u8, String)> = list
            .lines()
            .filter(|l| l.contains("Nerd Font"))
            .filter_map(|l| {
                let file = l.split(':').next()?.trim().to_string();
                let rank = if l.contains("Symbols Nerd Font") { 0 } else if l.contains("Mono") && file.contains("Regular") { 1 } else if file.contains("Regular") { 2 } else { 3 };
                Some((rank, file))
            })
            .collect();
        nerd.sort();
        files.extend(nerd.into_iter().map(|(_, f)| f).take(1));
    }
    for name in ["Noto Sans Symbols2", "Noto Sans Symbols", "DejaVu Sans", "Noto Sans CJK SC"] {
        if let Some(p) = fc_match(name) {
            let p = p.to_string_lossy().into_owned();
            if !files.contains(&p) {
                files.push(p);
            }
        }
    }
    files.into_iter().map(|f| (f, std::cell::OnceCell::new())).collect()
}

const DOC_REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf",
    "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
];
const DOC_BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
    "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Bold.ttf",
    "/usr/share/fonts/TTF/LiberationSans-Bold.ttf",
];

const MONO_REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
];
const MONO_BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-B.ttf",
];

impl Fonts {
    /// Fonte de uma família instalada (ex.: "JetBrainsMono Nerd Font"); vazio = monoespaçada padrão.
    pub fn load_family(family: &str) -> Option<Self> {
        if family.trim().is_empty() {
            return Self::load_mono();
        }
        let reg = fc_match(family).and_then(|p| open_font(&p, 0));
        match reg {
            Some(r) => {
                let bold = fc_match(&format!("{family}:bold")).and_then(|p| open_font(&p, 0));
                Some(new_fonts(r, bold))
            }
            None => Self::load_mono(),
        }
    }

    /// Fonte de um arquivo (o `index` escolhe a fonte dentro de um .ttc).
    pub fn load_file(path: &std::path::Path, index: u32) -> Option<Self> {
        Some(new_fonts(open_font(path, index)?, None))
    }

    /// Todos os caracteres que a fonte principal desenha, em ordem.
    pub fn chars(&self) -> Vec<char> {
        let mut v: Vec<char> = self.regular.codepoint_ids().filter(|(id, c)| id.0 != 0 && !c.is_control()).map(|(_, c)| c).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Medidas verticais da fonte principal em `px`: (subida acima da linha de
    /// base, descida abaixo dela, altura do "x", altura das maiúsculas).
    pub fn v_metrics(&self, px: f32) -> (f32, f32, f32, f32) {
        let f = &self.regular;
        let sf = f.as_scaled(self.scale(f, px));
        // No ab_glyph, `bounds.min.y` guarda o topo do contorno (y para cima).
        let h = |ch: char| f.outline(f.glyph_id(ch)).map(|o| o.bounds.min.y.max(o.bounds.max.y)).unwrap_or(0.0) * sf.scale().y / f.height_unscaled().max(1.0);
        (sf.ascent(), -sf.descent(), h('x'), h('H'))
    }

    /// Se a fonte principal tem o caractere (sem contar as reservas).
    pub fn has_char(&self, ch: char) -> bool {
        self.regular.glyph_id(ch).0 != 0
    }

    /// Fonte de largura fixa (terminal).
    pub fn load_mono() -> Option<Self> {
        Some(new_fonts(load(MONO_REGULAR, "DejaVu Sans Mono,monospace")?, load(MONO_BOLD, "DejaVu Sans Mono,monospace:bold")))
    }

    /// Métricas da fonte em `px`: (avanço de um caractere, subida acima da linha de base).
    pub fn mono_metrics(&self, px: f32) -> (f32, f32) {
        let f = &self.regular;
        let sf = f.as_scaled(self.scale(f, px));
        (sf.h_advance(f.glyph_id('M')), sf.ascent())
    }

    pub fn load_system() -> Option<Self> {
        Some(new_fonts(load(REGULAR, "DejaVu Sans,sans-serif")?, load(BOLD, "DejaVu Sans,sans-serif:bold")))
    }

    /// Fonte para o texto dos documentos (sem serifa, mais estreita que a da interface).
    pub fn load_document() -> Option<Self> {
        match load(DOC_REGULAR, "Liberation Sans,sans-serif") {
            Some(regular) => Some(new_fonts(regular, load(DOC_BOLD, "Liberation Sans,sans-serif:bold"))),
            None => Self::load_system(),
        }
    }

    /// Fonte que tem o caractere: a principal ou a primeira reserva que tiver.
    fn font_for(&self, ch: char, bold: bool) -> &FontVec {
        let f = self.font(bold);
        if ch <= ' ' || f.glyph_id(ch).0 != 0 {
            return f;
        }
        for (path, cell) in self.fallbacks.get_or_init(fallback_fonts) {
            let font = cell.get_or_init(|| open_font(std::path::Path::new(path), 0));
            if let Some(fb) = font {
                if fb.glyph_id(ch).0 != 0 {
                    return fb;
                }
            }
        }
        f
    }

    fn font(&self, bold: bool) -> &FontVec {
        if bold { self.bold.as_ref().unwrap_or(&self.regular) } else { &self.regular }
    }

    /// Escala em que 1 em = `px` pixels (o mesmo que "tamanho da fonte").
    fn scale(&self, f: &FontVec, px: f32) -> PxScale {
        // PxScale do ab_glyph é a altura da linha; convertemos a partir do "em".
        match f.units_per_em() {
            Some(upem) if upem > 0.0 => PxScale::from(px * f.height_unscaled() / upem),
            _ => PxScale::from(px),
        }
    }

    pub fn measure(&self, text: &str, px: f32, bold: bool) -> f32 {
        let f = self.font(bold);
        let sf = f.as_scaled(self.scale(f, px));
        let mut w = 0.0;
        let mut prev: Option<GlyphId> = None;
        for ch in text.chars() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                w += sf.kern(p, id);
            }
            w += if id.0 == 0 && ch > ' ' { self.fallback_advance(ch, px, bold) } else { sf.h_advance(id) };
            prev = Some(id);
        }
        w
    }

    /// Where in `text` (in bytes) a click at `x` puts the cursor: before the first
    /// character whose middle is past `x`, or at the end. One pass over the text.
    pub fn index_at(&self, text: &str, px: f32, bold: bool, x: f32) -> usize {
        let f = self.font(bold);
        let sf = f.as_scaled(self.scale(f, px));
        let mut w = 0.0;
        let mut before = 0.0;
        let mut prev: Option<GlyphId> = None;
        for (i, ch) in text.char_indices() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                w += sf.kern(p, id);
            }
            w += if id.0 == 0 && ch > ' ' { self.fallback_advance(ch, px, bold) } else { sf.h_advance(id) };
            prev = Some(id);
            if x < (before + w) / 2.0 {
                return i;
            }
            before = w;
        }
        text.len()
    }

    /// Cuts the text with "…" to fit in `max_w`: the longest beginning that, followed by
    /// "…", is not wider than `max_w` (the whole text when it fits).
    ///
    /// One pass over the text. It used to measure the whole text again for every character
    /// taken off, which made a long one (an address with thousands of characters in a list
    /// drawn every frame) cost a second or more.
    pub fn ellipsize(&self, text: &str, px: f32, bold: bool, max_w: f32) -> String {
        const ELLIPSIS: char = '…';
        let f = self.font(bold);
        let sf = f.as_scaled(self.scale(f, px));
        // The same arithmetic as `measure`, in the same order, so both agree to the last bit.
        let advance = |ch: char, id: GlyphId| if id.0 == 0 && ch > ' ' { self.fallback_advance(ch, px, bold) } else { sf.h_advance(id) };
        let ellipsis = f.glyph_id(ELLIPSIS);
        let ellipsis_w = advance(ELLIPSIS, ellipsis);
        let mut w = 0.0;
        let mut prev: Option<GlyphId> = None;
        // Where (in bytes) the longest beginning that fits with the "…" ends.
        let mut fits: Option<usize> = None;
        let mut whole = true;
        for (i, ch) in text.char_indices() {
            let mut cut = w;
            if let Some(p) = prev {
                cut += sf.kern(p, ellipsis);
            }
            cut += ellipsis_w;
            if cut <= max_w {
                fits = Some(i);
            }
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                w += sf.kern(p, id);
            }
            w += advance(ch, id);
            prev = Some(id);
            // Far past the limit: nothing longer fits (kerning never gives this much back).
            if w > max_w + px * 4.0 {
                whole = false;
                break;
            }
        }
        if whole && w <= max_w {
            return text.to_string();
        }
        match fits {
            Some(i) => format!("{}{ELLIPSIS}", &text[..i]),
            None => String::new(),
        }
    }

    fn rasterize(&self, ch: char, ppx: f32, bold: bool) -> Glyph {
        let f = self.font_for(ch, bold);
        let g = f.glyph_id(ch).with_scale_and_position(self.scale(f, ppx), point(0.0, 0.0));
        let Some(og) = f.outline_glyph(g) else { return Glyph { left: 0, top: 0, w: 0, h: 0, cov: Vec::new() } };
        let b = og.px_bounds();
        let (w, h) = (b.width() as usize, b.height() as usize);
        let mut cov = vec![0u8; w * h];
        og.draw(|x, y, c| {
            let i = y as usize * w + x as usize;
            if i < cov.len() {
                cov[i] = (c.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        Glyph { left: b.min.x as i32, top: b.min.y as i32, w, h, cov }
    }

    /// Desenha `text` (coordenadas lógicas) com a linha de base em `baseline`.
    pub fn draw(&self, c: &mut Canvas, text: &str, x: f32, baseline: f32, px: f32, bold: bool, color: Color) {
        if text.is_empty() {
            return;
        }
        let color = c.remap_ink(color);
        if c.recording() {
            let mut k = crate::draw::Fp::new(17);
            k.u(self.id);
            k.bytes(text.as_bytes());
            for v in [x, baseline, px] {
                k.f(v);
            }
            k.u(bold as u64);
            k.color(color);
            // Caixa com folga para as letras que passam da medida (itálico, acentos).
            let w = self.measure(text, px, bold);
            let r = c.px_box(x - px * 0.5, baseline - px * 1.4, w + px, px * 2.0, 1.0);
            if c.note(r, k.finish()) {
                return;
            }
        }
        let s = c.scale;
        let ppx = px * s;
        let f = self.font(bold);
        let sf = f.as_scaled(self.scale(f, ppx));
        let mut pen = x * s;
        let base = (baseline * s).round() as i32;
        let mut prev: Option<GlyphId> = None;
        let pw = c.pm.width() as i32;
        let [bx0, by0, bx1, by1] = c.bounds();
        let (ox, oy) = c.offset();
        let mut cache = self.cache.borrow_mut();
        let data = c.pm.data_mut();
        let (cr, cg, cb, ca) = (color.red(), color.green(), color.blue(), color.alpha());
        for ch in text.chars() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                pen += sf.kern(p, id);
            }
            prev = Some(id);
            let g = cache.entry((ch, ppx.to_bits(), bold)).or_insert_with(|| self.rasterize(ch, ppx, bold));
            let gx = pen.round() as i32 + g.left;
            let gy = base + g.top;
            for row in 0..g.h as i32 {
                let y = gy + row;
                if y < by0 || y >= by1 {
                    continue;
                }
                for col in 0..g.w as i32 {
                    let x = gx + col;
                    if x < bx0 || x >= bx1 {
                        continue;
                    }
                    let cov = g.cov[row as usize * g.w + col as usize];
                    if cov == 0 {
                        continue;
                    }
                    let a = cov as f32 / 255.0 * ca;
                    let i = (((y - oy) * pw + (x - ox)) * 4) as usize;
                    let inv = 1.0 - a;
                    data[i] = (cr * a * 255.0 + data[i] as f32 * inv) as u8;
                    data[i + 1] = (cg * a * 255.0 + data[i + 1] as f32 * inv) as u8;
                    data[i + 2] = (cb * a * 255.0 + data[i + 2] as f32 * inv) as u8;
                    data[i + 3] = (a * 255.0 + data[i + 3] as f32 * inv) as u8;
                }
            }
            pen += if id.0 == 0 && ch > ' ' { self.fallback_advance(ch, ppx, bold) } else { sf.h_advance(id) };
        }
    }

    fn fallback_advance(&self, ch: char, px: f32, bold: bool) -> f32 {
        let f = self.font_for(ch, bold);
        f.as_scaled(self.scale(f, px)).h_advance(f.glyph_id(ch))
    }

    /// Texto com um "relevo" claro 1px abaixo (efeito gravado).
    pub fn draw_embossed(&self, c: &mut Canvas, text: &str, x: f32, baseline: f32, px: f32, bold: bool, color: Color, light: Color) {
        self.draw(c, text, x, baseline + 1.0, px, bold, light);
        self.draw(c, text, x, baseline, px, bold, color);
    }
}

#[cfg(test)]
mod tests {
    /// How `ellipsize` worked before: take one character off at a time and measure again.
    fn ellipsize_by_popping(f: &super::Fonts, text: &str, px: f32, bold: bool, max_w: f32) -> String {
        if f.measure(text, px, bold) <= max_w {
            return text.to_string();
        }
        let mut s: String = text.to_string();
        while !s.is_empty() {
            s.pop();
            let t = format!("{s}…");
            if f.measure(&t, px, bold) <= max_w {
                return t;
            }
        }
        String::new()
    }

    /// The one-pass `ellipsize` gives exactly what the old one gave, and a very long text
    /// (an address of thousands of characters) no longer takes a noticeable time.
    #[test]
    fn ellipsize_in_one_pass() {
        let Some(f) = super::Fonts::load_system() else { return };
        let texts = [
            "",
            "a",
            "Glove",
            "Um título comum de página — com acentuação, 日本語 e símbolos ✓",
            "https://example.com/a/very/long/path?with=a&lot=of&parameters=that&never=ends#and-a-fragment",
            "AVAVAVAV To To To WAWAWA fi fl ffi",
            "   espaços   no   meio   e no fim   ",
        ];
        for text in texts {
            for bold in [false, true] {
                for px in [9.0, 12.0, 12.5, 15.0] {
                    for max_w in [0.0, 3.0, 8.0, 20.0, 47.5, 100.0, 160.0, 333.0, 1000.0] {
                        assert_eq!(f.ellipsize(text, px, bold, max_w), ellipsize_by_popping(&f, text, px, bold, max_w), "{text:?} px={px} bold={bold} max_w={max_w}");
                    }
                }
            }
        }
        // The click position: the same as measuring each beginning of the text.
        for text in texts {
            for x in [-5.0, 0.0, 2.0, 7.5, 31.0, 90.0, 250.0, 5000.0] {
                let mut expected = text.len();
                let mut before = 0.0;
                for (i, ch) in text.char_indices() {
                    let w = f.measure(&text[..i + ch.len_utf8()], 12.0, false);
                    if x < (before + w) / 2.0 {
                        expected = i;
                        break;
                    }
                    before = w;
                }
                assert_eq!(f.index_at(text, 12.0, false, x), expected, "{text:?} x={x}");
            }
        }
        let long: String = "https://example.com/redirect?to=".chars().chain("abcdefghij%2F0123456789&x=".chars().cycle().take(20_000)).collect();
        assert_eq!(f.ellipsize(&long[..600], 12.0, false, 420.0), ellipsize_by_popping(&f, &long[..600], 12.0, false, 420.0));
        let start = std::time::Instant::now();
        let cut = f.ellipsize(&long, 12.0, false, 420.0);
        assert!(cut.ends_with('…') && cut.len() < 200);
        assert!(start.elapsed() < std::time::Duration::from_millis(100), "{:?}", start.elapsed());
    }

    /// Numa distribuição que guarda a fonte noutra pasta (nenhum dos caminhos conhecidos
    /// existe), o fontconfig acha: era o que faltava no Fedora.
    #[test]
    fn fonte_pelo_fontconfig() {
        if std::process::Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }
        assert!(super::load(&["/nao/existe/Fonte.ttf"], "DejaVu Sans,sans-serif").is_some());
        assert!(super::load(&["/nao/existe/Fonte.ttf"], "DejaVu Sans Mono,monospace:bold").is_some());
    }
}
