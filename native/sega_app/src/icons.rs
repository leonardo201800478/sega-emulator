//! Shared vector-painted toolbar icons for the native egui frontend.

use eframe::egui;

/// Supported machine and debugger toolbar glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconKind {
    Play,
    Pause,
    Stop,
    Reset,
    Step,
    Trace,
    Add,
    Remove,
    Breakpoint,
    Close,
}

/// Render a compact button whose icon is painted with egui vector primitives.
///
/// The button uses no font glyphs, so its appearance is independent of the
/// selected UI font and Windows text fallback.
pub(crate) fn icon_button(
    ui: &mut egui::Ui,
    icon: IconKind,
    tooltip: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(38.0, 30.0)),
    );
    let icon_rect = response.rect.shrink(8.0);
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    paint_icon(ui.painter(), icon_rect, icon, color);
    response.on_hover_text(tooltip)
}

/// Render a compact icon-only button for popup and inline controls.
pub(crate) fn small_icon_button(
    ui: &mut egui::Ui,
    icon: IconKind,
    tooltip: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(24.0, 24.0)),
    );
    let icon_rect = response.rect.shrink(5.0);
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    paint_icon(ui.painter(), icon_rect, icon, color);
    response.on_hover_text(tooltip)
}

fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: IconKind, color: egui::Color32) {
    let center = rect.center();
    let stroke = egui::Stroke::new(2.0, color);
    let left = rect.left() + 2.0;
    let right = rect.right() - 2.0;
    let top = rect.top() + 1.0;
    let bottom = rect.bottom() - 1.0;

    match icon {
        IconKind::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(left, top),
                    egui::pos2(left, bottom),
                    egui::pos2(right, center.y),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
        IconKind::Pause => {
            let bar_width = (rect.width() * 0.22).max(3.0);
            let bar_height = rect.height() * 0.78;
            for offset in [-1.0, 1.0] {
                let bar = egui::Rect::from_center_size(
                    egui::pos2(center.x + offset * bar_width, center.y),
                    egui::vec2(bar_width, bar_height),
                );
                painter.rect_filled(bar, egui::CornerRadius::same(1), color);
            }
        }
        IconKind::Stop => {
            let side = rect.height() * 0.72;
            painter.rect_filled(
                egui::Rect::from_center_size(center, egui::vec2(side, side)),
                egui::CornerRadius::same(1),
                color,
            );
        }
        IconKind::Reset => {
            painter.circle_stroke(center, rect.height() * 0.36, stroke);
            painter.line_segment(
                [
                    egui::pos2(center.x + 1.0, top + 1.0),
                    egui::pos2(right, top + 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(right, top + 1.0),
                    egui::pos2(right - 1.0, top + 6.0),
                ],
                stroke,
            );
        }
        IconKind::Step => {
            let triangle_right = right - 4.0;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(left, top),
                    egui::pos2(left, bottom),
                    egui::pos2(triangle_right, center.y),
                ],
                color,
                egui::Stroke::NONE,
            ));
            painter.line_segment([egui::pos2(right, top), egui::pos2(right, bottom)], stroke);
        }
        IconKind::Trace => {
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(left, bottom),
                    egui::pos2(left + 4.0, bottom - 4.0),
                    egui::pos2(center.x - 2.0, center.y + 2.0),
                    egui::pos2(center.x + 2.0, top + 2.0),
                    egui::pos2(right, top + 2.0),
                ],
                stroke,
            ));
        }
        IconKind::Add => {
            painter.line_segment(
                [
                    egui::pos2(center.x, top),
                    egui::pos2(center.x, bottom),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(left, center.y),
                    egui::pos2(right, center.y),
                ],
                stroke,
            );
        }
        IconKind::Remove => {
            painter.line_segment([egui::pos2(left, center.y), egui::pos2(right, center.y)], stroke);
        }
        IconKind::Breakpoint => {
            painter.circle_stroke(center, rect.height() * 0.36, stroke);
            painter.circle_filled(center, rect.height() * 0.14, color);
        }
        IconKind::Close => {
            painter.line_segment([egui::pos2(left, top), egui::pos2(right, bottom)], stroke);
            painter.line_segment([egui::pos2(right, top), egui::pos2(left, bottom)], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IconKind, paint_icon};
    use eframe::egui;

    #[test]
    fn every_toolbar_icon_paints_without_text_glyphs() {
        let context = egui::Context::default();
        let icons = [
            IconKind::Play,
            IconKind::Pause,
            IconKind::Stop,
            IconKind::Reset,
            IconKind::Step,
            IconKind::Trace,
            IconKind::Add,
            IconKind::Remove,
            IconKind::Breakpoint,
            IconKind::Close,
        ];

        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                let rect = egui::Rect::from_min_size(
                    ui.available_rect_before_wrap().min,
                    egui::vec2(24.0, 18.0),
                );
                for icon in icons {
                    paint_icon(ui.painter(), rect, icon, egui::Color32::WHITE);
                }
            });
        });

        assert!(output.shapes.len() >= 10);
    }
}
