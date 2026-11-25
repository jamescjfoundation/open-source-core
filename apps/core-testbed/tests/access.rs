// SPDX-License-Identifier: MIT OR Apache-2.0
//! Screen readers: every control on the testbed's window and its Preferences page has a name in
//! the accessibility tree (core issue #26).

use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;
use jcj_shell::egui::accesskit::Role;
use jcj_shell::{Page, Shell, egui, theme::Tokens};

type State = (Shell, core_testbed::window::Testbed);

#[test]
fn every_control_has_a_name() {
    let dir = std::env::temp_dir().join(format!("core-testbed-access-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let app = core_testbed::window::Testbed::new(dir.join("untitled.jcjtest"));
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, s: &mut State| {
            let (shell, app) = s;
            shell.ui(ui, app);
        },
        (Shell::new("Core testbed", "0", Tokens::default()), app),
    );
    for page in [None, Some(Page::Preferences)] {
        if let Some(p) = page.clone() {
            h.state_mut().0.open_file_page(p);
        }
        h.run();
        for n in h.root().children_recursive() {
            let a = n.accesskit_node();
            if a.is_hidden() || ![Role::Button, Role::CheckBox, Role::TextInput, Role::SpinButton, Role::Slider, Role::Link].contains(&a.role()) {
                continue;
            }
            let named =
                a.label().is_some_and(|l| !l.trim().is_empty()) || a.value().is_some_and(|v| !v.trim().is_empty() && a.role() == Role::Button);
            assert!(named, "{page:?}: a {:?} has no name", a.role());
        }
    }
}
