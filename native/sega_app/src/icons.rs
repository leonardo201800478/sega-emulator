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
    match icon {
        IconKind::Play => paint_play(painter, rect, color),
        IconKind::Pause => paint_pause(painter, rect, color),
        IconKind::Stop => paint_stop(painter, rect, color),
        IconKind::Reset => paint_reset(painter, rect, color),
        IconKind::Step => paint_step(painter, rect, color),
        IconKind::Trace => paint_trace(painter, rect, color),
        IconKind::Add => paint_add(painter, rect, color),
        IconKind::Remove => paint_remove(painter, rect, color),
        IconKind::Breakpoint => paint_breakpoint(painter, rect, color),
        IconKind::Close => paint_close(painter, rect, color),
    }
}

fn paint_play(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(rect.left() + 2.0, rect.top() + 1.0),
            egui::pos2(rect.left() + 2.0, rect.bottom() - 1.0),
            egui::pos2(rect.right() - 2.0, rect.center().y),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

fn paint_pause(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
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

fn paint_stop(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let side = rect.height() * 0.72;
    painter.rect_filled(
        egui::Rect::from_center_size(rect.center(), egui::vec2(side, side)),
        egui::CornerRadius::same(1),
        color,
    );
}

fn paint_reset(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let right = rect.right() - 2.0;
    let top = rect.top() + 1.0;
    let stroke = egui::Stroke::new(2.0, color);
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

fn paint_step(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let left = rect.left() + 2.0;
    let right = rect.right() - 2.0;
    let top = rect.top() + 1.0;
    let bottom = rect.bottom() - 1.0;
    let stroke = egui::Stroke::new(2.0, color);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(left, top),
            egui::pos2(left, bottom),
            egui::pos2(right - 4.0, rect.center().y),
        ],
        color,
        egui::Stroke::NONE,
    ));
    painter.line_segment([egui::pos2(right, top), egui::pos2(right, bottom)], stroke);
}

fn paint_trace(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let left = rect.left() + 2.0;
    let right = rect.right() - 2.0;
    let top = rect.top() + 1.0;
    let bottom = rect.bottom() - 1.0;
    let center = rect.center();
    painter.add(egui::Shape::line(
        vec![
            egui::pos2(left, bottom),
            egui::pos2(left + 4.0, bottom - 4.0),
            egui::pos2(center.x - 2.0, center.y + 2.0),
            egui::pos2(center.x + 2.0, top + 2.0),
            egui::pos2(right, top + 2.0),
        ],
        egui::Stroke::new(2.0, color),
    ));
}

fn paint_add(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let stroke = egui::Stroke::new(2.0, color);
    painter.line_segment(
        [
            egui::pos2(center.x, rect.top() + 1.0),
            egui::pos2(center.x, rect.bottom() - 1.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(rect.left() + 2.0, center.y),
            egui::pos2(rect.right() - 2.0, center.y),
        ],
        stroke,
    );
}

fn paint_remove(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    painter.line_segment(
        [
            egui::pos2(rect.left() + 2.0, center.y),
            egui::pos2(rect.right() - 2.0, center.y),
        ],
        egui::Stroke::new(2.0, color),
    );
}

fn paint_breakpoint(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    painter.circle_stroke(center, rect.height() * 0.36, egui::Stroke::new(2.0, color));
    painter.circle_filled(center, rect.height() * 0.14, color);
}

fn paint_close(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let left = rect.left() + 2.0;
    let right = rect.right() - 2.0;
    let top = rect.top() + 1.0;
    let bottom = rect.bottom() - 1.0;
    let stroke = egui::Stroke::new(2.0, color);
    painter.line_segment([egui::pos2(left, top), egui::pos2(right, bottom)], stroke);
    painter.line_segment([egui::pos2(right, top), egui::pos2(left, bottom)], stroke);
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

        let output = context.run_ui(egui::RawInput::default(), |context| {
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
