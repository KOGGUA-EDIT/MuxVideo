//! Vector icons drawn with egui shapes — no icon font, no extra dependency.
//!
//! Every icon is authored on a 24×24 grid and scaled into the requested rect,
//! which keeps them crisp at any DPI and lets them inherit any tint colour.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2, pos2};

/// 24×24 glyph canvas.
const GRID: f32 = 24.0;

struct Icon {
    rect: Rect,
    color: Color32,
    stroke: f32,
}

impl Icon {
    fn point(&self, x: f32, y: f32) -> Pos2 {
        let s = self.rect.width() / GRID;
        pos2(self.rect.min.x + x * s, self.rect.min.y + y * s)
    }

    fn polygon(&self, points: &[(f32, f32)], close: bool) -> Shape {
        let mut v: Vec<Pos2> = points.iter().map(|(x, y)| self.point(*x, *y)).collect();
        if close {
            v.push(v[0]);
        }
        Shape::line(v, Stroke::new(self.stroke, self.color))
    }

    fn filled(&self, points: &[(f32, f32)]) -> Shape {
        Shape::convex_polygon(
            points.iter().map(|(x, y)| self.point(*x, *y)).collect(),
            self.color,
            Stroke::NONE,
        )
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) -> Shape {
        Shape::line_segment([self.point(a.0, a.1), self.point(b.0, b.1)], Stroke::new(self.stroke, self.color))
    }

    fn round_rect(&self, x: f32, y: f32, w: f32, h: f32, radius: f32, fill: bool) -> Shape {
        let s = self.rect.width() / GRID;
        let r = Rect::from_min_size(self.point(x, y), Vec2::new(w * s, h * s));
        if fill {
            Shape::rect_filled(r, egui::CornerRadius::same((radius * s).round() as u8), self.color)
        } else {
            Shape::rect_stroke(
                r,
                egui::CornerRadius::same((radius * s).round() as u8),
                Stroke::new(self.stroke, self.color),
                egui::StrokeKind::Inside,
            )
        }
    }

    fn circle(&self, cx: f32, cy: f32, r: f32, fill: bool) -> Shape {
        let s = self.rect.width() / GRID;
        let c = self.point(cx, cy);
        if fill {
            Shape::circle_filled(c, r * s, self.color)
        } else {
            Shape::circle_stroke(c, r * s, Stroke::new(self.stroke, self.color))
        }
    }
}

fn icon(ui: &mut egui::Ui, size: f32, color: Color32, paint: impl FnOnce(&Icon) -> Vec<Shape>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let icon = Icon {
            rect,
            color,
            stroke: (size / 11.0).max(1.2),
        };
        for shape in paint(&icon) {
            ui.painter().add(shape);
        }
    }
    response
}

pub fn play(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| vec![i.filled(&[(7.0, 4.5), (19.0, 12.0), (7.0, 19.5)])])
}

pub fn pause(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.round_rect(6.5, 5.0, 3.6, 14.0, 1.2, true),
            i.round_rect(13.9, 5.0, 3.6, 14.0, 1.2, true),
        ]
    })
}

pub fn step_back(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.filled(&[(14.0, 5.0), (14.0, 19.0), (5.5, 12.0)]),
            i.line((18.0, 5.0), (18.0, 19.0)),
        ]
    })
}

pub fn step_forward(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.filled(&[(10.0, 5.0), (10.0, 19.0), (18.5, 12.0)]),
            i.line((6.0, 5.0), (6.0, 19.0)),
        ]
    })
}

pub fn volume(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.filled(&[(4.0, 9.5), (8.0, 9.5), (13.0, 5.0), (13.0, 19.0), (8.0, 14.5), (4.0, 14.5)]),
            i.polygon(&[(16.0, 8.5), (18.6, 12.0), (16.0, 15.5)], false),
        ]
    })
}

pub fn mute(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.filled(&[(4.0, 9.5), (8.0, 9.5), (13.0, 5.0), (13.0, 19.0), (8.0, 14.5), (4.0, 14.5)]),
            i.line((16.0, 9.0), (20.5, 15.0)),
            i.line((20.5, 9.0), (16.0, 15.0)),
        ]
    })
}

pub fn loop_icon(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.polygon(&[(6.0, 8.0), (18.0, 8.0), (18.0, 14.0), (14.0, 14.0)], false),
            i.line((18.0, 8.0), (18.0, 14.0)),
            i.line((18.0, 12.0), (7.0, 12.0)),
        ]
    })
}

pub fn folder_open(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.polygon(&[(3.5, 7.0), (10.0, 7.0), (12.0, 9.5), (20.5, 9.5), (20.5, 18.0), (3.5, 18.0)], true),
        ]
    })
}

pub fn compare(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.round_rect(3.5, 5.5, 7.5, 13.0, 1.5, false),
            i.round_rect(13.0, 5.5, 7.5, 13.0, 1.5, false),
        ]
    })
}

pub fn split_view(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.round_rect(3.5, 5.0, 17.0, 14.0, 2.0, false),
            i.line((12.0, 5.0), (12.0, 19.0)),
        ]
    })
}

pub fn playlist(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.line((4.5, 7.0), (19.5, 7.0)),
            i.line((4.5, 12.0), (19.5, 12.0)),
            i.line((4.5, 17.0), (13.5, 17.0)),
            i.filled(&[(16.5, 15.0), (20.5, 17.0), (16.5, 19.0)]),
        ]
    })
}

pub fn settings(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.circle(12.0, 12.0, 3.2, false),
            i.circle(12.0, 12.0, 7.4, false),
            i.line((12.0, 3.0), (12.0, 5.5)),
            i.line((12.0, 18.5), (12.0, 21.0)),
            i.line((3.0, 12.0), (5.5, 12.0)),
            i.line((18.5, 12.0), (21.0, 12.0)),
        ]
    })
}

pub fn palette(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.circle(12.0, 12.0, 8.0, false),
            i.circle(8.8, 9.0, 1.6, true),
            i.circle(15.2, 9.0, 1.6, true),
            i.circle(9.5, 15.4, 1.6, true),
            i.circle(14.5, 15.4, 1.6, true),
        ]
    })
}

pub fn swap(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.line((5.0, 9.5), (19.0, 9.5)),
            i.polygon(&[(16.0, 9.5), (19.0, 9.5), (19.0, 6.5)], false),
            i.line((19.0, 14.5), (5.0, 14.5)),
            i.polygon(&[(8.0, 14.5), (5.0, 14.5), (5.0, 17.5)], false),
        ]
    })
}

pub fn diff(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.round_rect(3.5, 5.0, 12.5, 12.5, 1.5, false),
            i.round_rect(8.0, 8.0, 12.5, 12.5, 1.5, false),
        ]
    })
}

pub fn grid(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.round_rect(4.0, 4.0, 7.0, 7.0, 1.5, false),
            i.round_rect(13.0, 4.0, 7.0, 7.0, 1.5, false),
            i.round_rect(4.0, 13.0, 7.0, 7.0, 1.5, false),
            i.round_rect(13.0, 13.0, 7.0, 7.0, 1.5, false),
        ]
    })
}

pub fn close(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![i.line((6.5, 6.5), (17.5, 17.5)), i.line((17.5, 6.5), (6.5, 17.5))]
    })
}

pub fn minimize(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| vec![i.line((5.5, 12.0), (18.5, 12.0))])
}

pub fn maximize(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| vec![i.round_rect(6.0, 6.0, 12.0, 12.0, 1.5, false)])
}

pub fn chevron_down(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![i.polygon(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)], false)]
    })
}

pub fn check(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![i.polygon(&[(5.5, 12.5), (10.0, 17.0), (18.5, 7.5)], false)]
    })
}

pub fn keyboard(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    icon(ui, size, color, |i| {
        vec![
            i.round_rect(2.5, 7.0, 19.0, 10.0, 2.0, false),
            i.line((6.0, 10.5), (6.0, 10.5)),
            i.line((9.5, 10.5), (9.5, 10.5)),
            i.line((13.0, 10.5), (13.0, 10.5)),
            i.line((16.5, 10.5), (16.5, 10.5)),
            i.line((7.5, 14.0), (16.5, 14.0)),
        ]
    })
}
