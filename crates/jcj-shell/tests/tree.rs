// SPDX-License-Identifier: MIT OR Apache-2.0
//! The tree widget: rows drawn and named, a click selects, a click on the arrow toggles, and a
//! long tree draws only what is in view, fast.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jcj_shell::egui;
use jcj_shell::tree::{TreeEvent, TreeRow, TreeState, Zone, tree, zone};

struct St {
    styled: bool,
    state: TreeState<u32>,
    rows: Vec<TreeRow<u32>>,
    events: Vec<TreeEvent<u32>>,
}

fn rows(n: u32) -> Vec<TreeRow<u32>> {
    (0..n)
        .map(|i| {
            let mut r = TreeRow::new(i, (i % 3) as usize, format!("Row {i}"));
            r.has_children = i % 3 == 0;
            r.open = true;
            r
        })
        .collect()
}

fn harness(n: u32) -> Harness<'static, St> {
    Harness::builder().with_size(egui::vec2(300.0, 400.0)).build_ui_state(
        |ui, st: &mut St| {
            if !st.styled {
                let t = jcj_shell::theme::Tokens::default();
                jcj_shell::theme::install_fonts(ui.ctx(), &t.fonts, &jcj_shell::theme::default_faces());
                jcj_shell::theme::apply(ui.ctx(), &t.theme(jcj_shell::theme::Mode::Light));
                st.styled = true;
            }
            let ev = tree(ui, "t", &mut st.state, &st.rows);
            st.events.extend(ev);
        },
        St { styled: false, state: TreeState::default(), rows: rows(n), events: Vec::new() },
    )
}

#[test]
fn rows_are_named_and_a_click_selects() {
    let mut h = harness(10);
    h.run();
    let row = h.get_by_label("Row 4");
    row.click();
    h.run();
    assert!(h.state().events.contains(&TreeEvent::Select(4)), "{:?}", h.state().events);
}

#[test]
fn zones_split_a_row_into_before_inside_and_after() {
    assert_eq!(zone(1.0, 0.0, 20.0, true), Zone::Before);
    assert_eq!(zone(10.0, 0.0, 20.0, true), Zone::Inside);
    assert_eq!(zone(19.0, 0.0, 20.0, true), Zone::After);
    assert_eq!(zone(9.0, 0.0, 20.0, false), Zone::Before);
}

#[test]
fn a_thousand_rows_draw_only_what_is_in_view() {
    let mut h = harness(1000);
    h.run();
    assert!(h.query_by_label("Row 0").is_some());
    assert!(h.query_by_label("Row 999").is_none(), "rows out of view are not drawn");
    let start = std::time::Instant::now();
    for _ in 0..10 {
        h.step();
    }
    let per_frame = start.elapsed() / 10;
    assert!(per_frame.as_millis() < 50, "{per_frame:?} a frame");
}
