use super::egui;

// A compositor may report a touchscreen as a primary mouse pointer (Gamescope
// does this in its click modes). Accept content drags without requiring a prior
// Touch event. egui still gives sliders and other draggable children priority.
pub(super) fn vertical() -> egui::ScrollArea {
    egui::ScrollArea::vertical().scroll_source(egui::scroll_area::ScrollSource::ALL)
}
