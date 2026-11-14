//! Papel de parede procedural: fundo profundo com faixas de luz suaves.

use crate::draw::*;
use crate::theme::Theme;
use std::path::{Path, PathBuf};
use tiny_skia::PathBuilder;

/// `w` e `h` são lógicos; o resultado tem a resolução física da escala.
pub fn render(theme: &Theme, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let [deep, mid, light] = theme.wallpaper;

    // Base: gradiente diagonal.
    c.fill_rect(0.0, 0.0, w, h, &linear(0.0, 0.0, w * 0.3, h, &[(0.0, mid), (1.0, deep)]));

    // Brilho grande no alto, à esquerda.
    let r = w.max(h) * 0.75;
    c.fill_rect(0.0, 0.0, w, h, &radial(w * 0.28, h * 0.18, r, &[(0.0, with_alpha(light, 0.55)), (1.0, with_alpha(light, 0.0))]));

    // Faixas curvas de luz atravessando a tela.
    let bands: [(f32, f32, f32, f32); 4] = [(0.55, 0.30, 0.18, 0.20), (0.70, 0.12, 0.10, 0.14), (0.82, -0.05, 0.07, 0.10), (0.40, 0.45, 0.05, 0.08)];
    for (y, bend, thick, alpha) in bands {
        let y0 = h * y;
        let t = h * thick;
        let mut pb = PathBuilder::new();
        pb.move_to(-w * 0.1, y0);
        pb.cubic_to(w * 0.3, y0 - h * bend, w * 0.65, y0 + h * bend, w * 1.1, y0 - h * bend * 0.5);
        pb.line_to(w * 1.1, y0 - h * bend * 0.5 + t);
        pb.cubic_to(w * 0.65, y0 + h * bend + t * 1.6, w * 0.3, y0 - h * bend + t * 0.4, -w * 0.1, y0 + t);
        pb.close();
        c.fill(
            pb.finish(),
            // Degradê pela altura real da faixa (com curva negativa, os limites se
            // invertiam e o degradê virava uma emenda dura).
            &vgrad(
                y0 - h * bend.abs(),
                y0 + h * bend.abs() + t * 1.6,
                &[(0.0, with_alpha(light, 0.0)), (0.5, with_alpha(light, alpha)), (1.0, with_alpha(light, 0.0))],
            ),
        );
    }

    // Vinheta escura nas bordas.
    c.fill_rect(0.0, 0.0, w, h, &radial(w * 0.5, h * 0.45, w.max(h) * 0.8, &[(0.6, with_alpha(deep, 0.0)), (1.0, with_alpha(deep, 0.6))]));
    Some(c)
}

/// Como a imagem ocupa a tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Cobre a tela inteira, cortando o que sobra (sem distorcer).
    Fill,
    /// Cabe inteira, com faixas da cor de fundo.
    Fit,
    /// Ocupa a tela inteira, distorcendo se for preciso.
    Stretch,
    /// No tamanho dela, no meio.
    Center,
    /// Repetida, como azulejos.
    Tile,
}

impl Fit {
    /// (ajuste, id na configuração, nome).
    pub const ALL: [(Fit, &'static str, &'static str); 5] = [
        (Fit::Fill, "preencher", "Preencher a tela"),
        (Fit::Fit, "ajustar", "Ajustar à tela"),
        (Fit::Stretch, "esticar", "Esticar"),
        (Fit::Center, "centralizar", "Centralizar"),
        (Fit::Tile, "lado_a_lado", "Lado a lado"),
    ];

    pub fn from_id(id: &str) -> Fit {
        Self::ALL.iter().find(|(_, i, _)| *i == id).map_or(Fit::Fill, |x| x.0)
    }

    pub fn id(self) -> &'static str {
        Self::ALL.iter().find(|x| x.0 == self).map_or("preencher", |x| x.1)
    }
}

/// Um pedaço de uma área maior (panorama em vários monitores): a imagem ocupa a
/// área total (`total_w` × `total_h`, lógicos) e esta tela mostra a parte que
/// começa em (`x`, `y`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub total_w: f32,
    pub total_h: f32,
    pub x: f32,
    pub y: f32,
}

/// Papel de parede por estilo: "ondas", "aurora", "liso", "cor:R,G,B" ou o caminho
/// de uma imagem (PNG, JPEG, WebP) ou de um vídeo/GIF (aqui, o primeiro quadro).
pub fn render_style(theme: &Theme, style: &str, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    render_part(theme, style, w, h, scale, Fit::Fill, None)
}

/// Como `render_style`, com o ajuste da imagem e, no panorama, o pedaço desta tela.
/// (Os estilos desenhados por código ignoram o ajuste e o panorama.)
pub fn render_part(theme: &Theme, style: &str, w: f32, h: f32, scale: f32, fit: Fit, span: Option<Span>) -> Option<Canvas> {
    match style {
        "aurora" => aurora(theme, w, h, scale),
        "liso" => plain(theme, w, h, scale),
        "ondas" | "" => render(theme, w, h, scale),
        c if c.starts_with("cor:") => solid_color(c, w, h, scale).or_else(|| render(theme, w, h, scale)),
        path => {
            let file = if is_animated(path) { first_frame(path) } else { Some(PathBuf::from(path)) };
            file.and_then(|f| crate::image::decode(&f).ok()).and_then(|img| place(&img, w, h, scale, fit, span, theme.wallpaper[0])).or_else(|| render(theme, w, h, scale))
        }
    }
}

/// Põe uma imagem já aberta na tela (também os quadros dos papéis animados).
pub fn place(img: &tiny_skia::Pixmap, w: f32, h: f32, scale: f32, fit: Fit, span: Option<Span>, bg: tiny_skia::Color) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let (pw, ph) = (c.px_width() as f32, c.px_height() as f32);
    c.pm.fill(bg);
    // A área total em pixels desta tela e onde ela começa na área.
    let (tw, th, ox, oy) = match span {
        Some(s) => (s.total_w * scale, s.total_h * scale, s.x * scale, s.y * scale),
        None => (pw, ph, 0.0, 0.0),
    };
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    if iw < 1.0 || ih < 1.0 {
        return Some(c);
    }
    let quality = tiny_skia::FilterQuality::Bilinear;
    if fit == Fit::Tile {
        let pattern = tiny_skia::Pattern::new(img.as_ref(), tiny_skia::SpreadMode::Repeat, quality, 1.0, tiny_skia::Transform::from_translate(-ox, -oy));
        let paint = tiny_skia::Paint { shader: pattern, ..Default::default() };
        if let Some(r) = tiny_skia::Rect::from_xywh(0.0, 0.0, pw, ph) {
            c.pm.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
        }
        return Some(c);
    }
    let (kx, ky) = match fit {
        Fit::Fill => {
            let k = (tw / iw).max(th / ih);
            (k, k)
        }
        Fit::Fit => {
            let k = (tw / iw).min(th / ih);
            (k, k)
        }
        Fit::Stretch => (tw / iw, th / ih),
        _ => (1.0, 1.0),
    };
    let (dx, dy) = ((tw - iw * kx) / 2.0 - ox, (th - ih * ky) / 2.0 - oy);
    let paint = tiny_skia::PixmapPaint { quality, ..Default::default() };
    c.pm.draw_pixmap(0, 0, img.as_ref(), &paint, tiny_skia::Transform::from_row(kx, 0.0, 0.0, ky, dx, dy), None);
    Some(c)
}

/// Extensões de papel de parede animado (vídeos e GIF, tocados pelo ffmpeg).
pub const ANIMATED: [&str; 9] = ["mp4", "webm", "mkv", "mov", "m4v", "avi", "ogv", "gif", "mpg"];

/// O estilo é um papel animado (pela extensão do arquivo)?
pub fn is_animated(style: &str) -> bool {
    Path::new(style).extension().and_then(|e| e.to_str()).is_some_and(|e| ANIMATED.contains(&e.to_ascii_lowercase().as_str()))
}

/// O ffmpeg está instalado? (Ele toca os papéis animados.)
pub fn has_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg").arg("-version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

/// O primeiro quadro de um vídeo/GIF como PNG (para a miniatura e para mostrar
/// enquanto o animado não começa), guardado em `~/.cache/glove/papel`.
pub fn first_frame(path: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(path).ok()?;
    let mut k = std::collections::hash_map::DefaultHasher::new();
    (path, meta.len(), meta.modified().ok()).hash(&mut k);
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let dir = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".cache")).join("glove/papel");
    let out = dir.join(format!("{:016x}.png", k.finish()));
    if out.exists() {
        return Some(out);
    }
    std::fs::create_dir_all(&dir).ok()?;
    let ok = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i", path, "-frames:v", "1", "-vf", "scale='min(1920,iw)':-2"])
        .arg(&out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    (ok && out.exists()).then_some(out)
}

/// Cor sólida: "cor:R,G,B".
pub fn parse_color(style: &str) -> Option<(u8, u8, u8)> {
    let mut it = style.strip_prefix("cor:")?.split(',').map(|v| v.trim().parse::<u8>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

fn solid_color(style: &str, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let (r, g, b) = parse_color(style)?;
    let mut c = Canvas::new(w, h, scale)?;
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(r, g, b, 255)));
    Some(c)
}

fn plain(theme: &Theme, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let [deep, mid, _] = theme.wallpaper;
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, mid), (1.0, deep)]));
    Some(c)
}

fn aurora(theme: &Theme, w: f32, h: f32, scale: f32) -> Option<Canvas> {
    let mut c = Canvas::new(w, h, scale)?;
    let [deep, mid, light] = theme.wallpaper;
    c.fill_rect(0.0, 0.0, w, h, &vgrad(0.0, h, &[(0.0, deep), (1.0, mid)]));
    let m = w.max(h);
    for (x, y, r, a) in [(0.2, 0.75, 0.55, 0.45), (0.75, 0.3, 0.45, 0.35), (0.5, 1.05, 0.6, 0.5), (0.9, 0.85, 0.3, 0.25)] {
        c.fill_rect(0.0, 0.0, w, h, &radial(w * x, h * y, m * r, &[(0.0, with_alpha(light, a)), (1.0, with_alpha(light, 0.0))]));
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Imagem 200×100: metade esquerda vermelha, direita azul (ou de cima/de baixo).
    fn halves(vertical: bool) -> tiny_skia::Pixmap {
        let (w, h) = if vertical { (100, 200) } else { (200, 100) };
        let mut pm = tiny_skia::Pixmap::new(w, h).unwrap();
        for (i, px) in pm.data_mut().chunks_exact_mut(4).enumerate() {
            let (x, y) = (i as u32 % w, i as u32 / w);
            let first = if vertical { y < h / 2 } else { x < w / 2 };
            px.copy_from_slice(if first { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
        }
        pm
    }

    fn rgb(c: &Canvas, x: u32, y: u32) -> (u8, u8, u8) {
        let p = c.pm.pixel(x, y).unwrap();
        (p.red(), p.green(), p.blue())
    }

    #[test]
    fn panorama_lado_a_lado_e_empilhado() {
        let bg = tiny_skia::Color::BLACK;
        // Dois monitores 100×100 lado a lado: a imagem 200×100 se divide entre eles.
        let img = halves(false);
        let l = place(&img, 100.0, 100.0, 1.0, Fit::Fill, Some(Span { total_w: 200.0, total_h: 100.0, x: 0.0, y: 0.0 }), bg).unwrap();
        let r = place(&img, 100.0, 100.0, 1.0, Fit::Fill, Some(Span { total_w: 200.0, total_h: 100.0, x: 100.0, y: 0.0 }), bg).unwrap();
        assert_eq!((rgb(&l, 10, 50), rgb(&l, 90, 50)), ((255, 0, 0), (255, 0, 0)));
        assert_eq!((rgb(&r, 10, 50), rgb(&r, 90, 50)), ((0, 0, 255), (0, 0, 255)));
        // Um em cima do outro.
        let img = halves(true);
        let t = place(&img, 100.0, 100.0, 1.0, Fit::Fill, Some(Span { total_w: 100.0, total_h: 200.0, x: 0.0, y: 0.0 }), bg).unwrap();
        let b = place(&img, 100.0, 100.0, 1.0, Fit::Fill, Some(Span { total_w: 100.0, total_h: 200.0, x: 0.0, y: 100.0 }), bg).unwrap();
        assert_eq!((rgb(&t, 50, 90), rgb(&b, 50, 10)), ((255, 0, 0), (0, 0, 255)));
    }

    #[test]
    fn ajustes() {
        let bg = tiny_skia::Color::BLACK;
        let img = halves(false); // 200×100 numa tela 100×100
        // Ajustar: cabe inteira, com faixas pretas em cima e embaixo.
        let c = place(&img, 100.0, 100.0, 1.0, Fit::Fit, None, bg).unwrap();
        assert_eq!((rgb(&c, 50, 5), rgb(&c, 20, 50), rgb(&c, 80, 50)), ((0, 0, 0), (255, 0, 0), (0, 0, 255)));
        // Preencher: cobre a tela; aparece o meio da imagem (a divisa no meio).
        let c = place(&img, 100.0, 100.0, 1.0, Fit::Fill, None, bg).unwrap();
        assert_eq!((rgb(&c, 5, 5), rgb(&c, 95, 95)), ((255, 0, 0), (0, 0, 255)));
        // Esticar: a imagem inteira, achatada.
        let c = place(&img, 100.0, 100.0, 1.0, Fit::Stretch, None, bg).unwrap();
        assert_eq!((rgb(&c, 10, 10), rgb(&c, 90, 90)), ((255, 0, 0), (0, 0, 255)));
        assert_eq!(Fit::from_id("lado_a_lado"), Fit::Tile);
        assert!(is_animated("/x/fundo.MP4") && is_animated("a.gif") && !is_animated("a.png"));
    }
}
