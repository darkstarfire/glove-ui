//! The system screens in the Modern look, as on the Mac of today: the lock screen (the
//! clock on top, the round photo, the name and the password field over the blurred
//! wallpaper) and the session starting (photo, name and a thin bar). Also the pieces they
//! share with the login screen: the blur and the round photo.

use crate::{
    chrome::{avatar, avatar_seed, user_picture_modern},
    draw::*,
    font::Fonts,
};
use glove_i18n::tr;
use tiny_skia::{FilterQuality, Paint, Pattern, Pixmap, PixmapPaint, SpreadMode, Transform};

/// Box blur (across and down), three times: almost a gaussian, over the (premultiplied)
/// pixels of a small picture.
pub fn blur_box(pm: &mut Pixmap, radius: usize) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    if w == 0 || h == 0 || radius == 0 {
        return;
    }
    let data = pm.data_mut();
    let mut tmp = vec![0u8; data.len()];
    // One pass along a direction: `n` lines of `len` pixels, `step` between neighbors and
    // `line` between lines (in pixels).
    let pass = |src: &[u8], dst: &mut [u8], n: usize, len: usize, step: usize, line: usize| {
        let window = (2 * radius + 1) as u32;
        for l in 0..n {
            let base = l * line;
            let px = |i: isize| base + (i.clamp(0, len as isize - 1) as usize) * step;
            let mut sum = [0u32; 4];
            for i in -(radius as isize)..=(radius as isize) {
                let o = px(i) * 4;
                for c in 0..4 {
                    sum[c] += src[o + c] as u32;
                }
            }
            for i in 0..len as isize {
                let o = px(i) * 4;
                for c in 0..4 {
                    dst[o + c] = (sum[c] / window) as u8;
                }
                let (out, inn) = (px(i - radius as isize) * 4, px(i + radius as isize + 1) * 4);
                for c in 0..4 {
                    sum[c] = sum[c] + src[inn + c] as u32 - src[out + c] as u32;
                }
            }
        }
    };
    for _ in 0..3 {
        pass(data, &mut tmp, h, w, 1, w);
        pass(&tmp, data, w, h, w, 1);
    }
}

/// The wallpaper `style` for a screen of `w` × `h` points, blurred: painted at a quarter
/// of the size (the GPU or the caller enlarges it) with the blur `radius` (in those small
/// pixels) and darkened by `dim` (0 to 1). None if it cannot be painted.
pub fn blurred_wallpaper(theme: &crate::theme::Theme, style: &str, w: f32, h: f32, radius: usize, dim: f32) -> Option<Pixmap> {
    let c = crate::wallpaper::render_style(theme, style, (w / 4.0).max(8.0), (h / 4.0).max(8.0), 1.0)?;
    let mut pm = c.pm;
    blur_box(&mut pm, radius);
    if dim > 0.0 {
        let k = (1.0 - dim.clamp(0.0, 1.0)) * 255.0;
        for p in pm.data_mut().chunks_exact_mut(4) {
            for v in &mut p[..3] {
                *v = ((*v as f32) * k / 255.0) as u8;
            }
        }
    }
    Some(pm)
}

/// The photo (or the drawn avatar of `login`; or the generic figure) in a circle of
/// diameter `d` at (x, y).
pub fn round_photo(c: &mut Canvas, photo: Option<&Pixmap>, login: Option<&str>, x: f32, y: f32, d: f32) {
    let sc = c.scale;
    let Some(mut tmp) = Canvas::new(d, d, sc) else { return };
    match (photo, login) {
        (Some(p), _) => {
            // Fills the circle: the shorter side of the photo is the diameter.
            let k = d * sc / p.width().min(p.height()) as f32;
            let (ox, oy) = ((d * sc - p.width() as f32 * k) / 2.0, (d * sc - p.height() as f32 * k) / 2.0);
            let t = Transform::from_scale(k, k).post_translate(ox, oy);
            tmp.draw_pixmap_t(p.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t);
        }
        (None, Some(l)) => avatar(&mut tmp, -d * 0.08, -d * 0.08, d * 1.16, avatar_seed(l)),
        (None, None) => user_picture_modern(&mut tmp, 0.0, 0.0, d, false),
    }
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = Pattern::new(tmp.pm.as_ref(), SpreadMode::Pad, FilterQuality::Bicubic, 1.0, Transform::from_scale(1.0 / sc, 1.0 / sc).post_translate(x, y));
    c.fill(circle(x + d / 2.0, y + d / 2.0, d / 2.0), &paint);
    c.stroke(circle(x + d / 2.0, y + d / 2.0, d / 2.0 - 0.5), &solid(rgba(255, 255, 255, 110)), 1.0);
}

/// Light text with a soft shadow, centered on `cx` (readable over any wallpaper).
pub fn light_text(c: &mut Canvas, fonts: &Fonts, t: &str, cx: f32, y: f32, px: f32, bold: bool, alpha: u8) {
    let tw = fonts.measure(t, px, bold);
    // A soft shadow around (also on a light wallpaper the text stands out).
    let sh = |k: u16| rgba(0, 0, 0, (alpha as u16 * k / 255) as u8);
    for (dx, dy, k) in [(0.0, 1.5, 120), (0.0, 0.5, 70), (-0.8, 0.8, 50), (0.8, 0.8, 50)] {
        fonts.draw(c, t, cx - tw / 2.0 + dx, y + dy, px, bold, sh(k));
    }
    fonts.draw(c, t, cx - tw / 2.0, y, px, bold, rgba(255, 255, 255, alpha));
}

/// The time ("15:49") and the date without the year ("quinta-feira, 26 de novembro"), in
/// the language of the system, for the clock of the lock and login screens.
pub fn clock_now() -> (String, String) {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        libc::localtime_r(&t, &mut tm);
    }
    let days = [tr!("domingo"), tr!("segunda-feira"), tr!("terça-feira"), tr!("quarta-feira"), tr!("quinta-feira"), tr!("sexta-feira"), tr!("sábado")];
    let months = [tr!("janeiro"), tr!("fevereiro"), tr!("março"), tr!("abril"), tr!("maio"), tr!("junho"), tr!("julho"), tr!("agosto"), tr!("setembro"), tr!("outubro"), tr!("novembro"), tr!("dezembro")];
    let time = format!("{:02}:{:02}", tm.tm_hour, tm.tm_min);
    // {0} day of the week, {1} day, {2} month (each language puts them in its order).
    let date = tr!("{0}, {1} de {2}", days[(tm.tm_wday.rem_euclid(7)) as usize], tm.tm_mday, months[(tm.tm_mon.rem_euclid(12)) as usize]);
    (time, date)
}

/// Size of the lock panel of the Modern look (photo, name, password, message).
pub const LOCK_MODERN_W: f32 = 320.0;
pub const LOCK_MODERN_H: f32 = 200.0;
const PHOTO: f32 = 76.0;
const FIELD_W: f32 = 210.0;
const FIELD_H: f32 = 30.0;

/// The lock panel of the Modern look, without a box (it floats over the wallpaper): the
/// round photo, the name, the password field (a translucent pill, the dots typed and the
/// caret, or "Enter Password") and the message under it. `spin`: the turning while the
/// password is checked.
#[allow(clippy::too_many_arguments)]
pub fn lock_modern(fonts: &Fonts, login: &str, name: &str, photo: Option<&Pixmap>, dots: usize, message: Option<&str>, busy: bool, caret: bool, spin: f32, scale: f32) -> Option<Canvas> {
    let (w, h) = (LOCK_MODERN_W, LOCK_MODERN_H);
    let mut c = Canvas::new(w, h, scale)?;
    let cx = w / 2.0;
    round_photo(&mut c, photo, Some(login), cx - PHOTO / 2.0, 0.0, PHOTO);
    light_text(&mut c, fonts, name, cx, PHOTO + 26.0, 15.0, true, 255);
    let (fx, fy) = (cx - FIELD_W / 2.0, PHOTO + 40.0);
    c.fill(rrect(fx, fy, FIELD_W, FIELD_H, FIELD_H / 2.0), &solid(rgba(255, 255, 255, 64)));
    c.stroke(rrect(fx + 0.5, fy + 0.5, FIELD_W - 1.0, FIELD_H - 1.0, FIELD_H / 2.0 - 0.5), &solid(rgba(255, 255, 255, 70)), 1.0);
    if dots == 0 && !busy {
        let t = tr!("Digite a senha");
        let tw = fonts.measure(t, 13.0, false);
        fonts.draw(&mut c, t, cx - tw / 2.0, fy + 19.5, 13.0, false, rgba(255, 255, 255, 150));
        if caret {
            c.fill_rect(cx - tw / 2.0 - 4.0, fy + 8.0, 1.0, FIELD_H - 16.0, &solid(rgba(255, 255, 255, 230)));
        }
    } else {
        let n = dots.min(24);
        let total = n as f32 * 9.0;
        let mut dx = cx - total / 2.0;
        for _ in 0..n {
            c.fill(circle(dx + 4.0, fy + FIELD_H / 2.0, 3.4), &solid(rgba(255, 255, 255, 240)));
            dx += 9.0;
        }
        if caret && !busy {
            c.fill_rect(dx + 2.0, fy + 8.0, 1.0, FIELD_H - 16.0, &solid(rgba(255, 255, 255, 230)));
        }
    }
    if busy {
        spinner(&mut c, fx + FIELD_W - 16.0, fy + FIELD_H / 2.0, 7.0, spin, rgba(255, 255, 255, 230));
    }
    if let Some(m) = message {
        light_text(&mut c, fonts, m, cx, fy + FIELD_H + 24.0, 12.0, true, 255);
    }
    Some(c)
}

/// The clock of the lock screen: the date and, under it, the time in big numbers.
pub fn lock_clock(fonts: &Fonts, time: &str, date: &str, scale: f32) -> Option<Canvas> {
    let (w, h) = (520.0, 130.0);
    let mut c = Canvas::new(w, h, scale)?;
    light_text(&mut c, fonts, date, w / 2.0, 22.0, 18.0, true, 235);
    light_text(&mut c, fonts, time, w / 2.0, 112.0, 84.0, true, 245);
    Some(c)
}

/// Size of the panel of the session starting in the Modern look.
pub const STARTUP_MODERN_W: f32 = 300.0;
pub const STARTUP_MODERN_H: f32 = 200.0;

/// The session starting in the Modern look: the round photo, the name and a thin bar of
/// progress (0 to 1) under them, over the wallpaper.
pub fn startup_modern(fonts: &Fonts, login: &str, name: &str, photo: Option<&Pixmap>, progress: f32, scale: f32) -> Option<Canvas> {
    let (w, h) = (STARTUP_MODERN_W, STARTUP_MODERN_H);
    let mut c = Canvas::new(w, h, scale)?;
    let cx = w / 2.0;
    let d = 96.0;
    round_photo(&mut c, photo, Some(login), cx - d / 2.0, 0.0, d);
    light_text(&mut c, fonts, name, cx, d + 30.0, 16.0, true, 255);
    let (bw, bh, by) = (170.0, 5.0, d + 76.0);
    // A dark track (seen on a light wallpaper too) and the white bar.
    c.fill(rrect(cx - bw / 2.0, by, bw, bh, bh / 2.0), &solid(rgba(0, 0, 0, 80)));
    c.stroke(rrect(cx - bw / 2.0 - 0.5, by - 0.5, bw + 1.0, bh + 1.0, bh / 2.0 + 0.5), &solid(rgba(255, 255, 255, 60)), 1.0);
    let p = progress.clamp(0.0, 1.0);
    if p > 0.0 {
        c.fill(rrect(cx - bw / 2.0, by, (bw * p).max(bh), bh, bh / 2.0), &solid(rgba(255, 255, 255, 235)));
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telas_modernas() {
        let Some(fonts) = Fonts::load_system() else { return };
        let mut bg = Canvas::new(800.0, 500.0, 1.0).unwrap();
        bg.fill_rect(0.0, 0.0, 800.0, 500.0, &vgrad(0.0, 500.0, &[(0.0, rgba(240, 140, 40, 255)), (1.0, rgba(40, 120, 220, 255))]));
        let lock = lock_modern(&fonts, "ana", "Ana Souza", None, 5, Some("Senha incorreta."), false, true, 0.0, 1.0).unwrap();
        bg.draw_canvas(&lock, 240.0, 280.0);
        let clock = lock_clock(&fonts, "15:49", "quinta-feira, 26 de novembro", 1.0).unwrap();
        bg.draw_canvas(&clock, 140.0, 40.0);
        let st = startup_modern(&fonts, "ana", "Ana Souza", None, 0.6, 1.0).unwrap();
        assert!(st.pm.width() > 0);
        let mut pm = bg.pm.clone();
        blur_box(&mut pm, 4);
        if let Ok(dir) = std::env::var("GLOVE_TEST_PNG") {
            bg.pm.save_png(std::path::Path::new(&dir).join("lock-modern.png")).unwrap();
            st.pm.save_png(std::path::Path::new(&dir).join("startup-modern.png")).unwrap();
        }
    }
}
