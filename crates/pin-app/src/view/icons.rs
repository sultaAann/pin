use std::f32::consts::{FRAC_1_SQRT_2, PI};

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

/// Line icons drawn with the painter, so they tint with the theme and need no image assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Clear,
    Text,
    Rich,
    Link,
    Image,
    Folder,
    File,
    Lock,
    Key,
    Pin,
    Copy,
    Keyboard,
    Trash,
    Chevron,
    Clipboard,
}

/// Paints `icon` into a square `rect`; coordinates below are on a 24-unit grid.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let pen = Pen::new(painter, rect, color);
    match icon {
        Icon::Search => {
            pen.circle(11.0, 11.0, 7.0);
            pen.line(&[(16.5, 16.5), (21.0, 21.0)]);
        }
        Icon::Clear => {
            pen.circle(12.0, 12.0, 9.0);
            pen.line(&[(9.0, 9.0), (15.0, 15.0)]);
            pen.line(&[(15.0, 9.0), (9.0, 15.0)]);
        }
        Icon::Text => {
            pen.line(&[(5.0, 6.0), (19.0, 6.0)]);
            pen.line(&[(5.0, 12.0), (19.0, 12.0)]);
            pen.line(&[(5.0, 18.0), (14.0, 18.0)]);
        }
        Icon::Rich => {
            pen.line(&[(4.0, 7.0), (4.0, 5.0), (15.0, 5.0), (15.0, 7.0)]);
            pen.line(&[(9.5, 5.0), (9.5, 19.0)]);
            pen.line(&[(7.5, 19.0), (11.5, 19.0)]);
            pen.line(&[(15.0, 12.0), (21.0, 12.0)]);
            pen.line(&[(18.0, 12.0), (18.0, 19.0)]);
        }
        Icon::Link => {
            let along = vec2(FRAC_1_SQRT_2, -FRAC_1_SQRT_2);
            pen.stadium(pos2(12.0, 12.0) - along * 3.0, along, 2.6, 3.0);
            pen.stadium(pos2(12.0, 12.0) + along * 3.0, along, 2.6, 3.0);
        }
        Icon::Image => {
            pen.rect((3.0, 5.0), (18.0, 14.0), 2.0);
            pen.circle(8.5, 10.0, 1.5);
            pen.line(&[(21.0, 16.0), (16.0, 11.0), (6.0, 19.0)]);
        }
        Icon::Folder => pen.closed(&[
            (3.0, 6.0),
            (9.0, 6.0),
            (11.0, 8.0),
            (21.0, 8.0),
            (21.0, 19.0),
            (3.0, 19.0),
        ]),
        Icon::File => {
            pen.closed(&[
                (6.0, 3.0),
                (14.0, 3.0),
                (19.0, 8.0),
                (19.0, 21.0),
                (6.0, 21.0),
            ]);
            pen.line(&[(14.0, 3.0), (14.0, 8.0), (19.0, 8.0)]);
        }
        _ => paint_controls(&pen, icon),
    }
}

fn paint_controls(pen: &Pen<'_>, icon: Icon) {
    match icon {
        Icon::Search
        | Icon::Clear
        | Icon::Text
        | Icon::Rich
        | Icon::Link
        | Icon::Image
        | Icon::Folder
        | Icon::File => {}
        Icon::Lock => {
            pen.rect((5.0, 11.0), (14.0, 9.0), 2.0);
            let mut shackle = vec![(8.0, 11.0)];
            shackle.extend(arc(12.0, 8.0, 4.0, PI, 2.0 * PI));
            shackle.push((16.0, 11.0));
            pen.line(&shackle);
        }
        Icon::Key => {
            pen.circle(8.0, 15.0, 4.0);
            pen.line(&[(11.0, 12.0), (20.0, 3.0)]);
            pen.line(&[(17.0, 6.0), (20.0, 9.0)]);
            pen.line(&[(15.0, 8.0), (17.0, 10.0)]);
        }
        Icon::Pin => {
            pen.line(&[(9.0, 4.0), (15.0, 4.0)]);
            pen.closed(&[
                (10.0, 4.0),
                (10.0, 9.0),
                (7.0, 13.0),
                (17.0, 13.0),
                (14.0, 9.0),
                (14.0, 4.0),
            ]);
            pen.line(&[(12.0, 13.0), (12.0, 20.0)]);
        }
        Icon::Copy => {
            pen.rect((9.0, 9.0), (11.0, 11.0), 2.0);
            pen.line(&[
                (5.0, 15.0),
                (4.5, 15.0),
                (3.0, 13.5),
                (3.0, 4.5),
                (4.5, 3.0),
                (13.5, 3.0),
                (15.0, 4.5),
                (15.0, 5.0),
            ]);
        }
        Icon::Keyboard => {
            pen.rect((3.0, 6.0), (18.0, 12.0), 2.0);
            for x in [7.0, 11.0, 15.0] {
                pen.dot(x, 10.0);
            }
            pen.line(&[(7.0, 14.0), (17.0, 14.0)]);
        }
        Icon::Trash => {
            pen.line(&[(4.0, 7.0), (20.0, 7.0)]);
            pen.line(&[(6.0, 7.0), (7.0, 20.0), (17.0, 20.0), (18.0, 7.0)]);
            pen.line(&[(9.0, 7.0), (9.0, 4.0), (15.0, 4.0), (15.0, 7.0)]);
            pen.line(&[(10.0, 11.0), (10.0, 17.0)]);
            pen.line(&[(14.0, 11.0), (14.0, 17.0)]);
        }
        Icon::Chevron => pen.line(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)]),
        Icon::Clipboard => {
            pen.rect((5.0, 4.0), (14.0, 17.0), 2.0);
            pen.rect((9.0, 2.5), (6.0, 3.0), 1.0);
        }
    }
}

fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    const STEPS: u8 = 12;
    (0..=STEPS)
        .map(|i| {
            let a = from + (to - from) * f32::from(i) / f32::from(STEPS);
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
    stroke: Stroke,
    color: Color32,
}

impl<'a> Pen<'a> {
    fn new(painter: &'a Painter, rect: Rect, color: Color32) -> Self {
        let scale = rect.width() / 24.0;
        Self {
            painter,
            origin: rect.min,
            scale,
            stroke: Stroke::new((1.8 * scale).max(1.2), color),
            color,
        }
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    fn points(&self, points: &[(f32, f32)]) -> Vec<Pos2> {
        points.iter().map(|&(x, y)| self.at(x, y)).collect()
    }

    fn line(&self, points: &[(f32, f32)]) {
        self.painter
            .add(Shape::line(self.points(points), self.stroke));
    }

    fn closed(&self, points: &[(f32, f32)]) {
        self.painter
            .add(Shape::closed_line(self.points(points), self.stroke));
    }

    fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_stroke(self.at(x, y), r * self.scale, self.stroke);
    }

    fn dot(&self, x: f32, y: f32) {
        self.painter
            .circle_filled(self.at(x, y), self.stroke.width * 0.6, self.color);
    }

    fn rect(&self, min: (f32, f32), size: (f32, f32), radius: f32) {
        let rect = Rect::from_min_size(self.at(min.0, min.1), vec2(size.0, size.1) * self.scale);
        self.painter
            .rect_stroke(rect, radius * self.scale, self.stroke, StrokeKind::Middle);
    }

    /// A pill around the segment of length `2 * half` centred on `center` along unit `along`.
    fn stadium(&self, center: Pos2, along: Vec2, radius: f32, half: f32) {
        const STEPS: u8 = 10;
        let normal = vec2(-along.y, along.x);
        let mut points = Vec::with_capacity(2 * usize::from(STEPS + 1));
        for (end, sign) in [(center + along * half, 1.0), (center - along * half, -1.0)] {
            for i in 0..=STEPS {
                let t = PI * f32::from(i) / f32::from(STEPS);
                let p = end + (normal * t.cos() + along * t.sin()) * radius * sign;
                points.push(self.at(p.x, p.y));
            }
        }
        self.painter.add(Shape::closed_line(points, self.stroke));
    }
}
