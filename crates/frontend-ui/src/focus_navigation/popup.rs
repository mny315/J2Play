use super::{Id, LayerId, egui};

const POPUP_ID: &str = "product-navigation-popup";

#[derive(Clone, Copy)]
struct Popup {
    owner: Id,
    layer: LayerId,
    pass: u64,
}

fn active(ctx: &egui::Context) -> Option<Popup> {
    ctx.data(|data| data.get_temp::<Popup>(Id::new(POPUP_ID)))
        .filter(|popup| {
            ctx.cumulative_pass_nr().saturating_sub(popup.pass) <= 1
                && egui::ComboBox::is_open(ctx, popup.owner)
        })
}

pub(crate) fn active_layer(ctx: &egui::Context) -> Option<LayerId> {
    active(ctx).map(|popup| popup.layer)
}

pub(crate) fn close_popup(ctx: &egui::Context) -> bool {
    let Some(popup) = active(ctx) else {
        return false;
    };
    egui::Popup::close_id(ctx, popup.owner.with("popup"));
    ctx.memory_mut(|memory| {
        memory.move_focus(egui::FocusDirection::None);
        memory.request_focus(popup.owner);
    });
    ctx.data_mut(|data| data.remove::<Popup>(Id::new(POPUP_ID)));
    true
}

/// Focus enters at the selected row and stays in this popup until selection or
/// dismissal. Every row is registered, including those beyond the viewport.
pub(crate) struct ComboMenu {
    initial: bool,
    first: Option<egui::Response>,
    selected: bool,
}

impl ComboMenu {
    pub(crate) fn selectable_value<T: PartialEq>(
        &mut self,
        ui: &mut egui::Ui,
        current: &mut T,
        alternative: T,
        label: &str,
    ) {
        let selected = *current == alternative;
        // A selectable button adds its border only when focused/hovered.
        // Without that border its content keeps the same position in all states;
        // the shared focus ring supplies the outline outside the row.
        let response = ui.add(egui::Button::selectable(selected, label).stroke(egui::Stroke::NONE));
        if response.clicked() {
            *current = alternative;
        }
        self.item(ui, &response, selected);
    }

    pub(crate) fn begin(ui: &egui::Ui, owner: Id) -> Self {
        let initial = active(ui.ctx()).is_none_or(|popup| popup.owner != owner);
        let popup = Popup {
            owner,
            layer: ui.layer_id(),
            pass: ui.ctx().cumulative_pass_nr(),
        };
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new(POPUP_ID), popup));
        Self {
            initial,
            first: None,
            selected: false,
        }
    }

    pub(crate) fn item(&mut self, ui: &egui::Ui, response: &egui::Response, selected: bool) {
        if self.first.is_none() {
            self.first = Some(response.clone());
        }
        if self.initial && selected {
            response.request_focus();
            self.selected = true;
            response.scroll_to_me(Some(egui::Align::Center));
        }
        crate::focus_ring::track(ui, response, 12.0);
        if response.clicked() {
            close_popup(ui.ctx());
        }
    }

    pub(crate) fn finish(self, ui: &egui::Ui) {
        if self.initial
            && !self.selected
            && let Some(first) = self.first
        {
            first.request_focus();
            crate::focus_ring::track(ui, &first, 12.0);
        }
    }
}
