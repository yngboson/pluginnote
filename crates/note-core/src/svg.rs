//! 렌더 결과 → SVG. 미리보기 이미지, 레포 썸네일, 헤드리스 검증에 쓴다.

use crate::draw::{DrawCmd, Pt};
use crate::doc::RenderedObject;

struct Bbox {
    min: Pt,
    max: Pt,
}
impl Bbox {
    fn add(&mut self, p: Pt) {
        for i in 0..2 {
            self.min[i] = self.min[i].min(p[i]);
            self.max[i] = self.max[i].max(p[i]);
        }
    }
}

fn each_cmd<'a>(objs: &'a [RenderedObject]) -> impl Iterator<Item = &'a DrawCmd> {
    objs.iter().flat_map(|o| o.output.draws.iter().chain(o.output.attachments.iter().flat_map(|a| a.draws.iter())))
}

fn extend(b: &mut Bbox, d: &DrawCmd) {
    match d {
        DrawCmd::Line { from, to, .. } | DrawCmd::Arrow { from, to, .. } => {
            b.add(*from);
            b.add(*to)
        }
        DrawCmd::Circle { c, r, .. } | DrawCmd::Arc { c, r, .. } => {
            b.add([c[0] - r, c[1] - r]);
            b.add([c[0] + r, c[1] + r])
        }
        DrawCmd::Polyline { points, .. } => points.iter().for_each(|p| b.add(*p)),
        DrawCmd::Text { at, .. } => b.add(*at),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn attrs(s: &crate::draw::Style, fill_default: &str) -> String {
    let mut a = format!("stroke=\"{}\" stroke-width=\"{}\" fill=\"{}\"", esc(&s.stroke), s.width, esc(s.fill.as_deref().unwrap_or(fill_default)));
    if let Some(d) = &s.dash {
        a += &format!(" stroke-dasharray=\"{}\"", d.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" "));
    }
    a
}

pub fn to_svg(objs: &[RenderedObject]) -> String {
    let mut b = Bbox { min: [f64::MAX; 2], max: [f64::MIN; 2] };
    each_cmd(objs).for_each(|d| extend(&mut b, d));
    if b.min[0] > b.max[0] {
        b = Bbox { min: [-1.0, -1.0], max: [1.0, 1.0] };
    }
    let pad = ((b.max[0] - b.min[0]).max(b.max[1] - b.min[1]) * 0.08).max(1e-6);
    let (x0, x1, y0, y1) = (b.min[0] - pad, b.max[0] + pad, b.min[1] - pad, b.max[1] + pad);
    let (w, h) = (x1 - x0, y1 - y0);
    // y 뒤집기: 수학 좌표 → SVG
    let fy = |y: f64| y1 - y + y0;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x0} {y0} {w} {h}\" width=\"640\" height=\"{}\">\n<rect x=\"{x0}\" y=\"{y0}\" width=\"{w}\" height=\"{h}\" fill=\"white\"/>\n",
        (640.0 * h / w).round()
    );
    let unit = w / 640.0; // 1 화면 px 에 해당하는 월드 길이
    // 선 굵기는 화면 px 기준
    let mut draw = |d: &DrawCmd| match d {
        DrawCmd::Line { from, to, style } => {
            s += &format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {} vector-effect=\"non-scaling-stroke\"/>\n", from[0], fy(from[1]), to[0], fy(to[1]), attrs(style, "none"))
        }
        DrawCmd::Circle { c, r, style } => {
            s += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {} vector-effect=\"non-scaling-stroke\"/>\n", c[0], fy(c[1]), r, attrs(style, "none"))
        }
        DrawCmd::Arc { c, r, start, end, style } => {
            let p = |a: f64| [c[0] + r * a.cos(), fy(c[1] + r * a.sin())];
            let (a, e) = (p(*start), p(*end));
            let sweep = (end - start).rem_euclid(std::f64::consts::TAU);
            let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
            // 반시계(수학) = SVG 에서 sweep-flag 0
            s += &format!("<path d=\"M {} {} A {r} {r} 0 {large} 0 {} {}\" {} vector-effect=\"non-scaling-stroke\"/>\n", a[0], a[1], e[0], e[1], attrs(style, "none"))
        }
        DrawCmd::Polyline { points, style } => {
            let pts: Vec<String> = points.iter().map(|p| format!("{},{}", p[0], fy(p[1]))).collect();
            s += &format!("<polyline points=\"{}\" {} vector-effect=\"non-scaling-stroke\"/>\n", pts.join(" "), attrs(style, "none"))
        }
        DrawCmd::Arrow { from, to, style } => {
            let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
            let len = dx.hypot(dy);
            s += &format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {} vector-effect=\"non-scaling-stroke\"/>\n", from[0], fy(from[1]), to[0], fy(to[1]), attrs(style, "none"));
            if len > 0.0 {
                let (ux, uy) = (dx / len, dy / len);
                let hl = (unit * 10.0).min(len * 0.5);
                let (bx, by) = (to[0] - ux * hl, to[1] - uy * hl);
                let (nx, ny) = (-uy * hl * 0.4, ux * hl * 0.4);
                s += &format!("<polygon points=\"{},{} {},{} {},{}\" fill=\"{}\" stroke=\"none\"/>\n", to[0], fy(to[1]), bx + nx, fy(by + ny), bx - nx, fy(by - ny), esc(&style.stroke));
            }
        }
        DrawCmd::Text { at, text, size, style } => {
            s += &format!("<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" font-family=\"sans-serif\">{}</text>\n", at[0], fy(at[1]), size * unit, esc(&style.stroke), esc(text))
        }
    };
    for o in objs {
        // 부속 요소를 먼저(뒤에) 그리고 본체를 위에
        o.output.attachments.iter().flat_map(|a| a.draws.iter()).for_each(&mut draw);
        o.output.draws.iter().for_each(&mut draw);
    }
    s += "</svg>\n";
    s
}
