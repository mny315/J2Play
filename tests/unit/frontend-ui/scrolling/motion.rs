use super::*;

#[derive(Default)]
struct Frame {
    offset: f32,
    max_offset: f32,
    edge: f32,
    clicked: bool,
}

struct List {
    ctx: egui::Context,
    time: f64,
    height: f32,
    initial_offset: Option<f32>,
    center_row: Option<f32>,
}

impl List {
    fn new(height: f32, initial_offset: f32) -> Self {
        let mut list = Self {
            ctx: egui::Context::default(),
            time: 0.0,
            height,
            initial_offset: None,
            center_row: None,
        };
        list.paint(vec![], 0.016);
        list.initial_offset = Some(initial_offset);
        list.paint(vec![], 0.016);
        list.paint(vec![], 0.016);
        list
    }

    fn paint(&mut self, events: Vec<Event>, dt: f64) -> Frame {
        self.time += dt;
        let mut frame = Frame::default();
        self.ctx
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 250.0))),
                    time: Some(self.time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let mut scroll = crate::scrolling::vertical();
                    if let Some(offset) = self.initial_offset.take() {
                        scroll = scroll.vertical_scroll_offset(offset);
                    }
                    let mut origin = 0.0;
                    let output = scroll.show(ui, |ui| {
                        origin = ui.max_rect().top();
                        ui.set_min_height(self.height);
                        if let Some(row) = self.center_row.take() {
                            ui.scroll_to_rect(
                                Rect::from_min_size(
                                    Pos2::new(ui.max_rect().left(), origin + row),
                                    Vec2::new(340.0, 48.0),
                                ),
                                Some(egui::Align::Center),
                            );
                        }
                        frame.clicked = ui
                            .add_sized([340.0, 48.0], egui::Button::new("Item"))
                            .clicked();
                    });
                    frame.offset = output.state.offset.y;
                    frame.max_offset =
                        (output.content_size.y - output.inner_rect.height()).max(0.0);
                    frame.edge = origin + frame.offset - output.inner_rect.top();
                },
            )
            .drop_without_applying_deltas();
        frame
    }

    fn drag(&mut self, start: Pos2, step: f32) -> Pos2 {
        self.paint(
            vec![Event::PointerMoved(start), pointer(start, true)],
            0.016,
        );
        let mut pos = start;
        for _ in 0..6 {
            pos.y += step;
            assert!(!self.paint(vec![Event::PointerMoved(pos)], 0.016).clicked);
        }
        pos
    }
}

#[test]
fn centering_an_edge_row_never_bounces_or_leaves_scroll_bounds() {
    for height in [100.0, 1000.0] {
        for bottom in [false, true] {
            let mut list = List::new(height, if bottom { 10_000.0 } else { 0.0 });
            let baseline = list.paint(vec![], 0.016).offset;
            list.center_row = Some(if bottom { height - 48.0 } else { 0.0 });
            for _ in 0..40 {
                let frame = list.paint(vec![], 0.016);
                assert!((frame.offset - baseline).abs() < 0.1);
                assert!(frame.offset >= 0.0 && frame.offset <= frame.max_offset);
                assert!(
                    frame.edge.abs() < 1.0,
                    "centering must not pull past an edge: height={height}, bottom={bottom}, edge={}",
                    frame.edge
                );
            }
        }
    }
}

#[test]
fn programmatic_scrolling_animates_within_the_content() {
    let mut list = List::new(4000.0, 0.0);
    let baseline = list.paint(vec![], 0.016);
    let target = 2000.0 + 24.0 - (list.height - baseline.max_offset) / 2.0;
    list.center_row = Some(2000.0);
    let mut previous = 0.0;
    let mut intermediate_frames = 0;
    for _ in 0..40 {
        let frame = list.paint(vec![], 0.016);
        assert!(frame.offset >= previous && frame.offset <= target + 1.0);
        assert!(frame.edge.abs() < 1.0);
        if frame.offset > 1.0 && frame.offset < target - 1.0 {
            intermediate_frames += 1;
        }
        previous = frame.offset;
    }
    assert!(
        intermediate_frames > 1,
        "focus scrolling must stay animated"
    );
    assert!((previous - target).abs() < 1.0);
}

#[test]
fn pulling_either_edge_moves_content_then_returns_with_bounded_scroll_offset() {
    for bottom in [false, true] {
        let mut list = List::new(1000.0, if bottom { 10_000.0 } else { 0.0 });
        let baseline = list.paint(vec![], 0.016).offset;
        let pos = list.drag(
            Pos2::new(120.0, if bottom { 210.0 } else { 50.0 }),
            if bottom { -20.0 } else { 20.0 },
        );
        let pulled = list.paint(vec![], 0.016);
        assert!((pulled.offset - baseline).abs() < 0.1);
        assert!(
            pulled.edge.abs() > 5.0 && pulled.edge.abs() <= 30.0,
            "edge={}, bottom={bottom}",
            pulled.edge
        );
        assert_eq!(pulled.edge < 0.0, bottom);
        assert!(!list.paint(vec![pointer(pos, false)], 0.016).clicked);
        let mut previous = pulled.edge.abs();
        for _ in 0..60 {
            let frame = list.paint(vec![], 0.016);
            assert!((frame.offset - baseline).abs() < 0.1);
            assert!(frame.edge.abs() <= previous + 1.0);
            previous = frame.edge.abs();
        }
        assert!(previous < 1.0);
    }
}

#[test]
fn a_fling_keeps_moving_and_a_new_press_catches_it_without_clicking() {
    let mut list = List::new(4000.0, 300.0);
    let pos = list.drag(Pos2::new(120.0, 220.0), -22.0);
    let released = list.paint(vec![pointer(pos, false)], 0.016).offset;
    let moving = list.paint(vec![], 0.032).offset;
    assert!(
        moving > released + 10.0,
        "released={released}, moving={moving}"
    );
    let catch = Pos2::new(120.0, 35.0);
    let stopped = list.paint(
        vec![Event::PointerMoved(catch), pointer(catch, true)],
        0.016,
    );
    assert!((stopped.offset - moving).abs() < 0.1);
    let released = list.paint(vec![pointer(catch, false)], 0.016);
    assert!(!released.clicked);
    assert!((released.offset - moving).abs() < 0.1);
    for _ in 0..10 {
        assert!((list.paint(vec![], 0.016).offset - moving).abs() < 0.1);
    }
}

#[test]
fn cancellation_and_hidden_pages_discard_edge_motion() {
    for cancel in [true, false] {
        let mut list = List::new(1000.0, 0.0);
        list.drag(Pos2::new(120.0, 50.0), 20.0);
        let frame = if cancel {
            list.paint(vec![Event::PointerCancelled], 0.016)
        } else {
            list.paint(vec![], 1.0)
        };
        assert!(frame.edge.abs() < 1.0 && frame.offset.abs() < 0.1);
    }
}

#[test]
fn short_list_pulls_both_ways_and_settles_without_scrolling_or_clicking() {
    for step in [-10.0, 10.0] {
        let mut list = List::new(100.0, 0.0);
        let pos = list.drag(Pos2::new(120.0, 70.0), step);
        let pulled = list.paint(vec![], 0.016);
        assert!(
            pulled.edge.abs() > 5.0 && pulled.edge.abs() <= 30.0,
            "step={step}, edge={}, offset={}, max={}",
            pulled.edge,
            pulled.offset,
            pulled.max_offset
        );
        assert_eq!(pulled.edge.is_sign_negative(), step.is_sign_negative());
        let frame = list.paint(vec![pointer(pos, false)], 0.016);
        assert!(!frame.clicked);
        for _ in 0..60 {
            let frame = list.paint(vec![], 0.016);
            assert!(frame.offset.abs() < 0.1 && frame.max_offset.abs() < 0.1);
        }
        assert!(list.paint(vec![], 0.016).edge.abs() < 1.0);
    }
}
