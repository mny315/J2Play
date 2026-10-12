use super::*;

fn layout() -> LibraryLayout {
    LibraryLayout::new(frontend_core::LibraryView::List, 360.0, 8.0)
}

fn row(y: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(0.0, y), layout().item_size)
}

#[test]
fn added_rows_fade_once_and_scrolling_does_not_restart_them() {
    let mut motion = LibraryMotion::default();
    let ids = [egui::Id::new("a"), egui::Id::new("b")];
    motion.begin(ids.into_iter(), layout());
    let (rect, opacity, animating) = motion.item(ids[0], row(0.0), 0, 0.0);
    assert_eq!(rect, row(0.0));
    assert!(opacity < 0.1 && animating);
    motion.finish();
    motion.begin(ids.into_iter(), layout());
    assert_eq!(
        motion.item(ids[0], row(0.0), 0, 1.0),
        (row(0.0), 1.0, false)
    );
    motion.finish();
    // An already known entry first encountered by scrolling is immediately visible.
    motion.begin(ids.into_iter(), layout());
    assert_eq!(
        motion.item(ids[1], row(120.0), 0, 1.1),
        (row(120.0), 1.0, false)
    );
    motion.finish();
    assert_eq!(motion.items.len(), 1);
    let added = egui::Id::new("c");
    motion.invalidate_entries();
    motion.begin([ids[0], ids[1], added].into_iter(), layout());
    assert!(motion.item(added, row(240.0), 0, 2.0).1 < 0.1);
}

#[test]
fn removal_closes_the_gap_without_retaining_the_deleted_entry() {
    let mut motion = LibraryMotion::default();
    let ids = [egui::Id::new("a"), egui::Id::new("b")];
    motion.begin(ids.into_iter(), layout());
    motion.item(ids[0], row(0.0), 0, 0.0);
    motion.item(ids[1], row(120.0), 1, 0.0);
    motion.finish();
    motion.invalidate_entries();
    motion.begin([ids[1]].into_iter(), layout());
    assert!(!motion.known.contains(&ids[0]));
    assert!(!motion.items.contains_key(&ids[0]));
    let (before, opacity, animating) = motion.item(ids[1], row(0.0), 0, 1.0);
    assert!((before.top() - 120.0).abs() < f32::EPSILON);
    assert!((opacity - 1.0).abs() < f32::EPSILON);
    assert!(animating);
    let during = motion.item(ids[1], row(0.0), 0, 1.1).0.top();
    assert!(during > 0.0 && during < 120.0);
    assert_eq!(
        motion.item(ids[1], row(0.0), 0, 1.3),
        (row(0.0), 1.0, false)
    );
}

#[test]
fn suspend_and_layout_changes_settle_pending_motion() {
    let mut motion = LibraryMotion::default();
    let id = egui::Id::new("a");
    motion.begin([id].into_iter(), layout());
    motion.item(id, row(0.0), 0, 0.0);
    motion.settle();
    motion.begin([id].into_iter(), layout());
    assert_eq!(motion.item(id, row(0.0), 0, 0.1), (row(0.0), 1.0, false));
    motion.finish();
    let tiles = LibraryLayout::new(frontend_core::LibraryView::Tiles, 720.0, 8.0);
    motion.begin([id].into_iter(), tiles);
    let rect = egui::Rect::from_min_size(egui::pos2(240.0, 0.0), tiles.item_size);
    assert_eq!(motion.item(id, rect, 0, 0.2), (rect, 1.0, false));
}

#[test]
fn unchanged_library_is_not_scanned_during_paints_scrolls_or_layout_changes() {
    let mut motion = LibraryMotion::default();
    let visited = std::cell::Cell::new(0);
    let ids = || {
        (0..=frontend_core::MAX_LIBRARY_ENTRIES).map(|index| {
            visited.set(visited.get() + 1);
            egui::Id::new(index)
        })
    };
    motion.begin(ids(), layout());
    motion.finish();
    assert_eq!(visited.get(), frontend_core::MAX_LIBRARY_ENTRIES);
    visited.set(0);
    for layout in [
        layout(),
        LibraryLayout::new(frontend_core::LibraryView::Tiles, 720.0, 8.0),
    ] {
        motion.begin(ids(), layout);
        motion.item(egui::Id::new(100_usize), row(120.0), 0, 1.0);
        motion.finish();
        motion.settle();
    }
    assert_eq!(visited.get(), 0);
    motion.invalidate_entries();
    motion.begin(ids(), layout());
    assert_eq!(visited.get(), frontend_core::MAX_LIBRARY_ENTRIES);
}
