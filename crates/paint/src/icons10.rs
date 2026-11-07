//! Ícones dos apps da Fase 10: Cópias de Segurança, Central de Apps, Fotos,
//! Gravador e Mensagens. Arte própria no acabamento dos ícones realistas.

use tiny_skia::{Color, PathBuilder, Transform};

use crate::draw::*;
use crate::tiger::{soft_shadow, tf, U};

/// Seta em arco (gel) em volta de (cx, cy): de `a0` a `a1` graus, com a ponta no fim.
fn arc_arrow(pm: &mut Canvas, u: U, cx: f32, cy: f32, r: f32, w: f32, a0: f32, a1: f32, base: Color) {
    let mut pb = PathBuilder::new();
    let n = 40;
    for i in 0..=n {
        let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
        let (x, y) = (u.px(cx + r * a.cos()), u.py(cy + r * a.sin()));
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    let stroke = tiny_skia::Stroke { width: u.d(w), line_cap: tiny_skia::LineCap::Butt, ..Default::default() };
    let band = pb.finish().and_then(|p| p.stroke(&stroke, 1.0));
    pm.fill(band.clone().and_then(|p| p.transform(Transform::from_translate(0.0, u.d(0.012)))), &solid(rgba(0, 0, 0, 60)));
    pm.fill(band.clone(), &solid(darken(base, 0.25)));
    let inner = band.and_then(|p| p.transform(Transform::from_scale(1.0, 1.0)));
    pm.fill(inner, &linear(u.px(cx), u.py(cy - r), u.px(cx), u.py(cy + r), &[(0.0, lighten(base, 0.45)), (0.5, base), (1.0, darken(base, 0.15))]));
    // A ponta: triângulo na direção do giro.
    let a = a1.to_radians();
    let dir = if a1 > a0 { 1.0 } else { -1.0 };
    let (tx, ty) = (cx + r * a.cos(), cy + r * a.sin());
    let (nx, ny) = (a.cos(), a.sin());
    let (fx, fy) = (-a.sin() * dir, a.cos() * dir);
    let h = w * 1.25;
    let tip = u.poly(&[(tx + nx * h, ty + ny * h), (tx - nx * h, ty - ny * h), (tx + fx * h * 1.3, ty + fy * h * 1.3)]);
    pm.fill(tip.clone(), &solid(darken(base, 0.1)));
}

/// Cópias de Segurança: disco externo de alumínio com a seta circular verde na frente.
pub fn backup(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let body = u.rr(0.1, 0.3, 0.8, 0.5, 0.08);
    soft_shadow(pm, body.clone(), s, 0.9);
    pm.fill(body.clone(), &vgrad(u.py(0.3), u.py(0.8), &[(0.0, rgba(222, 224, 230, 255)), (0.5, rgba(188, 192, 200, 255)), (1.0, rgba(140, 144, 154, 255))]));
    pm.stroke(body, &solid(rgba(90, 94, 104, 255)), u.lw(0.012));
    // Tampo claro e a faixa da frente.
    pm.fill(u.rr(0.12, 0.31, 0.76, 0.12, 0.06), &vgrad(u.py(0.31), u.py(0.43), &[(0.0, rgba(250, 251, 253, 255)), (1.0, rgba(214, 216, 222, 255))]));
    pm.fill(u.rr(0.16, 0.66, 0.68, 0.07, 0.03), &solid(rgba(60, 64, 72, 255)));
    pm.fill(u.circ(0.78, 0.695, 0.018), &radial(u.px(0.78), u.py(0.69), u.d(0.03), &[(0.0, rgba(180, 255, 180, 255)), (1.0, rgba(30, 150, 50, 255))]));
    // Seta verde girando (para trás: voltar no tempo).
    arc_arrow(pm, u, 0.5, 0.4, 0.22, 0.08, 20.0, -230.0, rgba(60, 170, 70, 255));
}

/// Central de Apps: caixa de papelão aberta com a seta azul entrando.
pub fn app_center(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let front = u.poly(&[(0.16, 0.45), (0.5, 0.56), (0.5, 0.92), (0.16, 0.8)]);
    let side = u.poly(&[(0.5, 0.56), (0.86, 0.45), (0.86, 0.8), (0.5, 0.92)]);
    soft_shadow(pm, u.poly(&[(0.16, 0.45), (0.5, 0.56), (0.86, 0.45), (0.86, 0.8), (0.5, 0.92), (0.16, 0.8)]), s, 0.9);
    pm.fill(front.clone(), &vgrad(u.py(0.45), u.py(0.92), &[(0.0, rgba(214, 170, 110, 255)), (1.0, rgba(180, 132, 76, 255))]));
    pm.fill(side.clone(), &vgrad(u.py(0.45), u.py(0.92), &[(0.0, rgba(196, 150, 92, 255)), (1.0, rgba(156, 112, 62, 255))]));
    // Abas abertas.
    pm.fill(u.poly(&[(0.16, 0.45), (0.5, 0.56), (0.42, 0.66), (0.06, 0.55)]), &solid(rgba(226, 186, 128, 255)));
    pm.fill(u.poly(&[(0.86, 0.45), (0.5, 0.56), (0.58, 0.66), (0.95, 0.55)]), &solid(rgba(206, 162, 104, 255)));
    pm.fill(u.poly(&[(0.16, 0.45), (0.5, 0.34), (0.86, 0.45), (0.5, 0.56)]), &solid(rgba(96, 66, 34, 255)));
    for p in [front, side] {
        pm.stroke(p, &solid(rgba(120, 84, 44, 200)), u.lw(0.01));
    }
    // Seta azul em gel descendo para a caixa.
    let arrow = u.poly(&[(0.42, 0.06), (0.58, 0.06), (0.58, 0.3), (0.7, 0.3), (0.5, 0.52), (0.3, 0.3), (0.42, 0.3)]);
    pm.fill(arrow.clone().and_then(|p| p.transform(Transform::from_translate(0.0, u.d(0.015)))), &solid(rgba(0, 0, 0, 70)));
    pm.fill(arrow.clone(), &linear(u.px(0.3), 0.0, u.px(0.7), 0.0, &[(0.0, rgba(40, 110, 220, 255)), (0.5, rgba(110, 175, 255, 255)), (1.0, rgba(30, 90, 200, 255))]));
    pm.stroke(arrow, &solid(rgba(20, 60, 150, 255)), u.lw(0.012));
    pm.fill(u.rr(0.445, 0.08, 0.05, 0.2, 0.025), &solid(rgba(255, 255, 255, 110)));
}

/// Fotos: três fotos de papel empilhadas, a de cima com uma paisagem.
pub fn photos(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let card = |ang: f32| tf(u.rr(0.14, 0.16, 0.72, 0.62, 0.015), Transform::from_rotate_at(ang, u.px(0.5), u.py(0.48)));
    soft_shadow(pm, card(-10.0), s, 0.7);
    for (ang, shade) in [(-10.0f32, 225u8), (6.0, 238)] {
        pm.fill(card(ang), &solid(rgba(shade, shade, shade - 4, 255)));
        pm.stroke(card(ang), &solid(rgba(0, 0, 0, 50)), u.lw(0.008));
    }
    let t = Transform::from_rotate_at(-2.0, u.px(0.5), u.py(0.48));
    let top = card(-2.0);
    soft_shadow(pm, top.clone(), s, 0.5);
    pm.fill(top.clone(), &solid(rgba(252, 252, 250, 255)));
    pm.stroke(top, &solid(rgba(0, 0, 0, 60)), u.lw(0.008));
    let pic = tf(u.rr(0.19, 0.21, 0.62, 0.45, 0.004), t);
    pm.fill(pic.clone(), &vgrad(u.py(0.21), u.py(0.66), &[(0.0, rgba(70, 140, 225, 255)), (0.6, rgba(170, 210, 245, 255)), (1.0, rgba(220, 236, 250, 255))]));
    // Sol, montanhas e campo.
    pm.fill_clipped(tf(u.circ(0.66, 0.33, 0.06), t), &radial(u.px(0.66), u.py(0.33), u.d(0.07), &[(0.0, rgba(255, 252, 210, 255)), (1.0, rgba(255, 196, 60, 255))]), pic.clone());
    pm.fill_clipped(tf(u.poly(&[(0.19, 0.58), (0.36, 0.38), (0.5, 0.52), (0.6, 0.44), (0.81, 0.6), (0.81, 0.66), (0.19, 0.66)]), t), &vgrad(u.py(0.38), u.py(0.66), &[(0.0, rgba(120, 130, 150, 255)), (1.0, rgba(80, 90, 110, 255))]), pic.clone());
    pm.fill_clipped(tf(u.poly(&[(0.19, 0.6), (0.45, 0.55), (0.81, 0.6), (0.81, 0.66), (0.19, 0.66)]), t), &vgrad(u.py(0.55), u.py(0.66), &[(0.0, rgba(110, 190, 80, 255)), (1.0, rgba(50, 130, 40, 255))]), pic);
}

/// Gravador: microfone de estúdio cromado no pedestal.
pub fn recorder(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    // Base e haste.
    soft_shadow(pm, u.ov(0.28, 0.84, 0.44, 0.1), s, 0.8);
    pm.fill(u.ov(0.28, 0.84, 0.44, 0.1), &vgrad(u.py(0.84), u.py(0.94), &[(0.0, rgba(90, 92, 100, 255)), (1.0, rgba(30, 30, 34, 255))]));
    pm.fill(u.rr(0.475, 0.56, 0.05, 0.31, 0.02), &linear(u.px(0.475), 0.0, u.px(0.525), 0.0, &[(0.0, rgba(90, 94, 104, 255)), (0.4, rgba(240, 242, 246, 255)), (1.0, rgba(100, 104, 114, 255))]));
    // Garfo em U em volta da cápsula.
    let mut pb = PathBuilder::new();
    pb.move_to(u.px(0.27), u.py(0.36));
    pb.cubic_to(u.px(0.27), u.py(0.62), u.px(0.73), u.py(0.62), u.px(0.73), u.py(0.36));
    let fork = pb.finish();
    pm.stroke(fork.clone(), &solid(rgba(70, 72, 80, 255)), u.d(0.05));
    pm.stroke(fork, &solid(rgba(200, 204, 212, 255)), u.d(0.025));
    // Cápsula: cromo com a grade.
    let cap = u.rr(0.32, 0.06, 0.36, 0.5, 0.18);
    soft_shadow(pm, cap.clone(), s, 0.6);
    pm.fill(cap.clone(), &linear(u.px(0.32), 0.0, u.px(0.68), 0.0, &[(0.0, rgba(110, 114, 124, 255)), (0.3, rgba(245, 246, 250, 255)), (0.55, rgba(170, 174, 184, 255)), (1.0, rgba(90, 94, 104, 255))]));
    let grille = u.rr(0.345, 0.09, 0.31, 0.3, 0.15);
    let mut dots = PathBuilder::new();
    for r in 0..9 {
        for c in 0..7 {
            let (cx, cy) = (0.37 + c as f32 * 0.043 + if r % 2 == 1 { 0.021 } else { 0.0 }, 0.11 + r as f32 * 0.032);
            dots.push_circle(u.px(cx), u.py(cy), u.d(0.011));
        }
    }
    pm.fill_clipped(dots.finish(), &solid(rgba(40, 42, 48, 200)), grille);
    pm.fill(u.rr(0.33, 0.4, 0.34, 0.035, 0.015), &solid(rgba(60, 62, 70, 255)));
    pm.stroke(cap, &solid(rgba(60, 62, 70, 255)), u.lw(0.01));
    // Luz vermelha de gravando.
    pm.fill(u.circ(0.5, 0.49, 0.022), &radial(u.px(0.495), u.py(0.485), u.d(0.03), &[(0.0, rgba(255, 190, 180, 255)), (1.0, rgba(200, 20, 20, 255))]));
}

/// Mensagens: dois balões de conversa, um azul (gel) na frente de um claro.
pub fn messages(pm: &mut Canvas, x: f32, y: f32, s: f32) {
    let u = U { x, y, s };
    let bubble = |bx: f32, by: f32, w: f32, h: f32, tail_left: bool| {
        let mut pb = PathBuilder::new();
        let (x0, y0, x1, y1) = (u.px(bx), u.py(by), u.px(bx + w), u.py(by + h));
        let r = u.d(h * 0.45);
        pb.move_to(x0 + r, y0);
        pb.line_to(x1 - r, y0);
        pb.quad_to(x1, y0, x1, y0 + r);
        pb.line_to(x1, y1 - r);
        pb.quad_to(x1, y1, x1 - r, y1);
        if tail_left {
            pb.line_to(x0 + r * 1.2, y1);
            pb.line_to(x0 + r * 0.2, y1 + u.d(0.1));
            pb.line_to(x0 + r * 0.6, y1 - u.d(0.02));
        } else {
            pb.line_to(x1 - r * 0.6, y1 - u.d(0.02));
            pb.line_to(x1 - r * 0.2, y1 + u.d(0.1));
            pb.line_to(x1 - r * 1.2, y1);
        }
        pb.line_to(x0 + r, y1);
        pb.quad_to(x0, y1, x0, y1 - r);
        pb.line_to(x0, y0 + r);
        pb.quad_to(x0, y0, x0 + r, y0);
        pb.close();
        pb.finish()
    };
    let back = bubble(0.3, 0.12, 0.62, 0.42, false);
    soft_shadow(pm, back.clone(), s, 0.6);
    pm.fill(back.clone(), &vgrad(u.py(0.12), u.py(0.6), &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(206, 208, 214, 255))]));
    pm.stroke(back, &solid(rgba(120, 124, 134, 255)), u.lw(0.012));
    let front = bubble(0.06, 0.4, 0.62, 0.4, true);
    soft_shadow(pm, front.clone(), s, 0.8);
    pm.fill(front.clone(), &vgrad(u.py(0.4), u.py(0.84), &[(0.0, rgba(120, 180, 255, 255)), (0.5, rgba(50, 120, 230, 255)), (1.0, rgba(30, 90, 200, 255))]));
    pm.fill_clipped(u.rr(0.08, 0.41, 0.58, 0.17, 0.08), &vgrad(u.py(0.41), u.py(0.58), &[(0.0, rgba(255, 255, 255, 150)), (1.0, rgba(255, 255, 255, 20))]), front.clone());
    pm.stroke(front, &solid(rgba(20, 70, 170, 255)), u.lw(0.012));
    for k in 0..3 {
        pm.fill(u.circ(0.25 + k as f32 * 0.12, 0.61, 0.03), &solid(rgba(255, 255, 255, 235)));
    }
}
