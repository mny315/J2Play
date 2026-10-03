use super::{
    FrontendApp, LIBRARY_ROW_HEIGHT, MaterialTheme, RichText, egui, library_game_info,
    library_profile_presentation, material_card_frame, material_tonal_button,
};
use frontend_core::LibraryView;

const AUTO_TILES_WIDTH: f32 = 600.0;
const MIN_TILE_WIDTH: f32 = 188.0;
const TILE_HEIGHT: f32 = 250.0;
const TILE_BODY_HEIGHT: f32 = 168.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LibraryLayout {
    pub(super) tiles: bool,
    pub(super) columns: usize,
    pub(super) item_size: egui::Vec2,
}

impl LibraryLayout {
    // UI coordinates and the library capacity bound these layout-only casts.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub(super) fn new(view: LibraryView, width: f32, gap: f32) -> Self {
        let tiles = match view {
            LibraryView::Automatic => width >= AUTO_TILES_WIDTH,
            LibraryView::List => false,
            LibraryView::Tiles => true,
        };
        let columns = if tiles {
            (((width + gap) / (MIN_TILE_WIDTH + gap)).floor() as usize)
                .clamp(1, frontend_core::MAX_LIBRARY_ENTRIES)
        } else {
            1
        };
        Self {
            tiles,
            columns,
            item_size: egui::vec2(
                ((width - (columns - 1) as f32 * gap) / columns as f32).max(0.0),
                if tiles {
                    TILE_HEIGHT
                } else {
                    LIBRARY_ROW_HEIGHT
                },
            ),
        }
    }

    // Coordinates only select library rows; retain one neighbor on either side
    // so keyboard/controller focus can cross the viewport edge.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub(super) fn visible_entries(
        self,
        viewport: egui::Rect,
        gap: f32,
        entry_count: usize,
    ) -> std::ops::Range<usize> {
        let rows = entry_count.div_ceil(self.columns);
        let stride = self.item_size.y + gap;
        let first =
            ((viewport.min.y / stride).floor().clamp(0.0, rows as f32) as usize).saturating_sub(1);
        let end = ((viewport.max.y / stride).ceil().clamp(0.0, rows as f32) as usize)
            .saturating_add(1)
            .min(rows);
        first * self.columns..(end * self.columns).min(entry_count)
    }
}

impl FrontendApp {
    #[allow(clippy::too_many_lines)]
    pub(super) fn draw_library_tile(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        width: f32,
    ) -> [egui::Id; 2] {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let icon = self.load_library_assets(ui.ctx(), index);
        let strength = self
            .library_added_strength(ui, index)
            .max(self.library_selection_strength(ui, self.entries[index].id()));
        let entry = &self.entries[index];
        let id = entry.id();
        let title = entry.title();
        let game_info = library_game_info(entry);
        let profile = library_profile_presentation(
            entry,
            &self.profile_options,
            &self.catalog_fingerprint,
            tr,
        );
        let description = profile.launch_description(title, &game_info, tr);
        let theme = &self.material_theme;
        let mut ids = [egui::Id::NULL; 2];
        let mut launch_response = None;
        let mut settings = false;
        let selecting = self.library_selection.active;
        let selected = self.library_selection.ids.contains(id);
        super::library_search::highlight_frame(material_card_frame(theme), theme, strength)
            .inner_margin(12)
            .show(ui, |ui| {
                let body_width = (width - 26.0).max(0.0);
                ui.set_min_width(body_width);
                ui.spacing_mut().item_spacing.y = 8.0;
                let (body, _) = ui.allocate_exact_size(
                    egui::vec2(body_width, TILE_BODY_HEIGHT),
                    egui::Sense::hover(),
                );
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(body.center().x, body.top() + 40.0),
                    egui::vec2(
                        super::library_icons::TILE_ICON_POINTS.min(body_width),
                        super::library_icons::TILE_ICON_POINTS,
                    ),
                );
                super::layout::paint_library_icon(ui, icon.as_ref(), icon_rect, theme);
                let title_rect = egui::Rect::from_min_size(
                    body.min + egui::vec2(0.0, 88.0),
                    egui::vec2(body_width, 40.0),
                );
                let mut job = egui::text::LayoutJob::simple(
                    title.to_owned(),
                    egui::FontId::proportional(17.0),
                    theme.on_surface,
                    body_width,
                );
                job.halign = egui::Align::Center;
                job.wrap.max_rows = 2;
                let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                ui.painter()
                    .with_clip_rect(title_rect.intersect(ui.clip_rect()))
                    .galley(title_rect.center_top(), galley, theme.on_surface);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                        body.min + egui::vec2(0.0, 132.0),
                        body.max,
                    )),
                    |ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        ui.spacing_mut().interact_size.y = 18.0;
                        ui.add_sized(
                            [body_width, 18.0],
                            egui::Label::new(
                                RichText::new(&game_info)
                                    .size(13.0)
                                    .color(theme.on_surface_variant),
                            )
                            .selectable(false)
                            .truncate()
                            .halign(egui::Align::Min),
                        );
                        super::layout::draw_library_profile(ui, &profile, body_width, theme);
                    },
                );
                let launch = ui
                    .interact(body, launch_id(id), egui::Sense::click())
                    .on_hover_text(if selecting { title } else { &description });
                launch.widget_info(|| {
                    if selecting {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            ui.is_enabled(),
                            selected,
                            title,
                        )
                    } else {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            &description,
                        )
                    }
                });
                track_launch(ui, &launch, theme);
                ids[0] = launch.id;
                launch_response = Some(launch);
                let response = if selecting {
                    ui.horizontal_centered(|ui| {
                        super::library_selection::selection_button(ui, theme, selected, title, true)
                    })
                    .inner
                } else {
                    settings_button(
                        ui,
                        id,
                        title,
                        egui::vec2(body_width, 48.0),
                        &tr.text("Settings"),
                        theme,
                    )
                };
                ids[1] = response.id;
                settings = response.clicked();
            });
        let open = launch_response
            .is_some_and(|launch| self.library_selection.interact(&launch, id, true));
        if open || settings {
            let id = id.to_owned();
            if open {
                self.open_game(&id);
            }
            if settings {
                if selecting {
                    self.library_selection.toggle(&id);
                } else {
                    self.open_settings(&id, false);
                }
            }
        }
        ids
    }
}

pub(super) fn launch_id(entry_id: &str) -> egui::Id {
    egui::Id::new(("library-launch", entry_id))
}

pub(super) fn row_action(
    ui: &mut egui::Ui,
    id: &str,
    title: &str,
    theme: &MaterialTheme,
    selection: Option<bool>,
) -> egui::Response {
    if let Some(selected) = selection {
        return super::library_selection::selection_button(ui, theme, selected, title, true);
    }
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let (label, width) = if ui.available_width() < super::LIBRARY_COMPACT_ACTION_BREAKPOINT {
        ("⚙".to_owned(), 48.0)
    } else {
        let label = tr.text("Settings");
        let width = settings_button_width(ui, &label);
        (label, width)
    };
    settings_button(ui, id, title, egui::vec2(width, 48.0), &label, theme)
}

pub(super) fn settings_button(
    ui: &mut egui::Ui,
    entry_id: &str,
    title: &str,
    size: egui::Vec2,
    label: &str,
    theme: &MaterialTheme,
) -> egui::Response {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    // Absolute scopes keep both actions stable across reflow and list/tile changes.
    ui.scope_builder(
        egui::UiBuilder::new().id(egui::Id::new(("library-settings", entry_id))),
        |ui| {
            let description = tr.format("Game settings for {title}", &[("title", title)]);
            let response = ui
                .add_sized(size, material_tonal_button(theme, label).truncate())
                .on_hover_text(&description);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &description)
            });
            response
        },
    )
    .inner
}

pub(super) fn track_launch(ui: &egui::Ui, launch: &egui::Response, theme: &MaterialTheme) {
    super::focus_ring::track(ui, launch, 14.0);
    if pointer_hover_enabled(ui.ctx()) && launch.hovered() && !launch.has_focus() {
        ui.painter()
            .rect_filled(launch.rect, 14.0, theme.primary.gamma_multiply_u8(18));
        ui.painter().rect_stroke(
            launch.rect,
            14.0,
            egui::Stroke::new(1.5, theme.primary),
            egui::StrokeKind::Inside,
        );
    }
}

fn pointer_hover_enabled(ctx: &egui::Context) -> bool {
    let (suppress, moved) = ctx.input(|input| {
        (
            input.pointer.any_down()
                || input.pointer.any_released()
                || input.any_touches()
                || input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Touch { .. }
                            | egui::Event::PointerCancelled
                            | egui::Event::PointerGone
                            | egui::Event::MouseWheel { .. }
                    )
                }),
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerMoved(_))),
        )
    });
    ctx.data_mut(|data| {
        let suppressed =
            data.get_temp_mut_or_default::<bool>(egui::Id::new("library-hover-suppressed"));
        if suppress {
            *suppressed = true;
        } else if moved {
            // A real mouse can hover again. A released or cancelled finger
            // must not highlight rows moving beneath it during inertia.
            *suppressed = false;
        }
        !*suppressed
    })
}

/// Size full captions from their translated text, padding and border.
pub(super) fn settings_button_width(ui: &egui::Ui, label: &str) -> f32 {
    let style = ui.style().button_style(
        &egui::widget_style::Classes::default(),
        egui::widget_style::WidgetState::Inactive,
    );
    let text_width = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
            style.text_style.font_id,
            style.text_style.color,
        )
        .size()
        .x;
    (text_width + ui.spacing().button_padding.x * 2.0 + 2.0)
        .ceil()
        .max(80.0)
}
