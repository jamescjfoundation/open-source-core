// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shell driven headlessly with a small app: the ribbon runs the app's commands, keys reach
//! them, panels open and close, the File page shows the app's credit line, every control drawn
//! runs a command someone has, and the preferences round trip.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jcj_shell::commands::{ids, shell_ids};
use jcj_shell::widgets::{self, Control};
use jcj_shell::{About, App, Cmd, CommandInfo, Commands, Page, PanelSpec, Shell, Side, TabSpec};
use serde_json::json;

#[derive(Default)]
struct Sample {
    ran: Vec<String>,
    revision: u64,
}

impl Commands for Sample {
    fn command(&self, id: &str) -> Option<CommandInfo> {
        self.all_commands().into_iter().find(|c| c.id == id)
    }

    fn why_disabled(&self, id: &str) -> Option<String> {
        (id == "sample.never").then(|| "never".to_string())
    }

    fn all_commands(&self) -> Vec<CommandInfo> {
        vec![
            CommandInfo { icon: "save".into(), ..CommandInfo::new(ids::SAVE, "Save", "Mod+S") },
            CommandInfo { icon: "undo".into(), ..CommandInfo::new(ids::UNDO, "Undo", "Mod+Z") },
            CommandInfo::new(ids::NEW, "New", "Mod+N"),
            CommandInfo::new(ids::OPEN, "Open…", "Mod+O"),
            CommandInfo { takes_params: true, ..CommandInfo::new(ids::OPEN_PATH, "Open Recent", "") },
            CommandInfo { icon: "bold".into(), ..CommandInfo::new("sample.bold", "Bold", "Mod+B") },
            CommandInfo::new("sample.never", "Never", ""),
        ]
    }
}

impl App for Sample {
    fn document_name(&self) -> String {
        "Untitled".into()
    }

    fn revision(&self) -> u64 {
        self.revision
    }

    fn run(&mut self, cmd: &Cmd, _shell: &mut Shell) -> Result<(), String> {
        self.ran.push(cmd.id.clone());
        self.revision += 1;
        Ok(())
    }

    fn document_ui(&mut self, ui: &mut egui::Ui, _shell: &mut Shell) {
        ui.label("The document");
    }

    fn ribbon_tabs(&self) -> Vec<TabSpec> {
        vec![TabSpec::new("home", "Home", "H"), TabSpec::new("view", "View", "V")]
    }

    fn ribbon_ui(&mut self, tab: &str, ui: &mut egui::Ui, shell: &mut Shell) {
        if tab != "home" {
            return;
        }
        let me: &Sample = self;
        widgets::group(ui, shell, "Text", |ui, shell| {
            widgets::big(ui, shell, me, &Control::new("sample.bold").label("Bold"));
            widgets::small(ui, shell, me, &Control::new("sample.never").icon("check").label("Never"));
        });
        widgets::group(ui, shell, "Window", |ui, shell| {
            widgets::big(ui, shell, me, &Control::new(shell_ids::PALETTE).label("Commands"));
        });
    }

    fn panels(&self) -> Vec<PanelSpec> {
        vec![
            PanelSpec::new("outline", "Outline", "outline", Side::Left),
            PanelSpec::new("inspector", "Inspector", "properties", Side::Right),
            PanelSpec::new("log", "Log", "draft", Side::Bottom),
        ]
    }

    fn panel_ui(&mut self, panel: &str, ui: &mut egui::Ui, _shell: &mut Shell) {
        ui.label(format!("Contents of {panel}"));
    }

    fn about(&self) -> About {
        About { summary: "A sample app.".into(), credit: "Made for the shell's tests.".into(), ..About::default() }
    }
}

type State = (Shell, Sample);

fn harness() -> Harness<'static, State> {
    let shell = Shell::new("Sample", "1.2.3", jcj_shell::theme::Tokens::default());
    Harness::builder().with_size(vec2(1200.0, 800.0)).with_step_dt(0.05).build_ui_state(
        |ui, s: &mut State| {
            let (shell, app) = s;
            shell.ui(ui, app);
        },
        (shell, Sample::default()),
    )
}

/// The center of the control drawn for command `id`.
fn control(h: &Harness<'_, State>, id: &str) -> egui::Pos2 {
    h.state().0.chrome.drawn.iter().find(|d| d.id.as_deref() == Some(id)).map(|d| d.rect.center()).unwrap_or_else(|| panic!("no control runs {id}"))
}

fn click(h: &mut Harness<'_, State>, at: egui::Pos2) {
    h.hover_at(at);
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
}

#[test]
fn every_control_runs_a_command_someone_has() {
    let mut h = harness();
    h.state_mut().0.chrome.unfold_menus = true;
    h.run();
    let (shell, app) = h.state();
    assert!(!shell.chrome.drawn.is_empty());
    for d in &shell.chrome.drawn {
        if let Some(id) = &d.id {
            assert!(jcj_shell::commands::info(app, id).is_some(), "{id} is drawn but nobody has it");
        }
    }
    h.get_by_label("The document");
}

#[test]
fn ribbon_buttons_and_keys_run_the_app_s_commands() {
    let mut h = harness();
    h.run();
    let bold = control(&h, "sample.bold");
    click(&mut h, bold);
    assert_eq!(h.state().1.ran, ["sample.bold"]);
    // A greyed-out command does nothing.
    let never = control(&h, "sample.never");
    click(&mut h, never);
    assert_eq!(h.state().1.ran, ["sample.bold"]);
    // Mod+S runs Save through the key map; a user key overrides the command's own.
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    assert_eq!(h.state().1.ran.last().map(String::as_str), Some(ids::SAVE));
    h.state_mut().0.keys.bind("Mod+S", "sample.bold");
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    assert_eq!(h.state().1.ran.last().map(String::as_str), Some("sample.bold"));
    // The shell's own command from the ribbon: the palette opens.
    let palette = control(&h, shell_ids::PALETTE);
    click(&mut h, palette);
    assert!(h.state().0.dialogs.palette.is_some());
    h.get_by_label("Find a command");
}

#[test]
fn panels_open_close_and_move() {
    let mut h = harness();
    h.run();
    assert!(h.query_by_label("Contents of outline").is_none());
    h.state_mut().0.run(Cmd::with(shell_ids::SIDE, json!({"side": "left"})));
    h.run();
    h.get_by_label("Contents of outline");
    h.state_mut().0.run(Cmd::with(shell_ids::PANEL, json!({"panel": "log"})));
    h.run();
    h.get_by_label("Contents of log");
    let panels = h.state().1.panels();
    h.state_mut().0.layout.move_to(&panels, "log", Side::Right);
    h.run();
    assert_eq!(h.state().0.layout.open_on(Side::Right), Some("log"));
    h.get_by_label("Contents of log");
    // The quick-access row's switch closes the left side again; the rail opens it.
    h.get_by_label("Left panel").click();
    h.run();
    assert_eq!(h.state().0.layout.open_on(Side::Left), None);
    h.get_by_label("Outline").click();
    h.run();
    h.get_by_label("Contents of outline");
    h.get_by_label("Left panel").click();
    h.run();
    assert_eq!(h.state().0.layout.open_on(Side::Left), None);
}

#[test]
fn the_file_page_shows_recent_files_and_the_credit_line() {
    let mut h = harness();
    h.state_mut().0.add_recent("/documents/draft.sample");
    h.state_mut().0.open_file_page(Page::Open);
    h.run();
    h.get_by_label("draft.sample");
    h.state_mut().0.open_file_page(Page::About);
    h.run();
    h.get_by_label("Sample 1.2.3");
    h.get_by_label("Made for the shell's tests.");
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().0.chrome.file_page, None);
    h.get_by_label("The document");
}

#[test]
fn every_control_on_the_file_pages_has_a_name() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    let mut seen = 0;
    for page in [Page::New, Page::Open, Page::Save, Page::Preferences, Page::Keys, Page::About] {
        h.state_mut().0.open_file_page(page.clone());
        h.run();
        for n in h.root().children_recursive() {
            let a = n.accesskit_node();
            let control = [Role::Button, Role::CheckBox, Role::TextInput, Role::SpinButton, Role::Slider, Role::Link, Role::Tab];
            if a.is_hidden() || !control.contains(&a.role()) {
                continue;
            }
            seen += 1;
            let name = a.label().unwrap_or_default();
            assert!(
                !name.trim().is_empty() || a.value().is_some_and(|v| !v.trim().is_empty() && a.role() == Role::Button),
                "{page:?}: a {:?} has no name",
                a.role()
            );
        }
    }
    assert!(seen > 20, "{seen} controls");
}

#[test]
fn key_tips_open_a_tab_and_run_a_control() {
    let mut h = harness();
    h.run();
    h.key_press(Key::F10);
    h.run();
    assert!(h.state().0.chrome.keytips);
    h.event(egui::Event::Text("H".into()));
    h.run();
    assert_eq!(h.state().0.chrome.keytip_stage, 1);
    // The tab's first control is Bold: letter A.
    let target = h.state().0.chrome.keytip_targets.iter().find(|(l, _)| l == "A").cloned();
    assert!(target.is_some(), "{:?}", h.state().0.chrome.keytip_targets);
    h.event(egui::Event::Text("A".into()));
    h.run();
    assert_eq!(h.state().1.ran, ["sample.bold"]);
    assert!(!h.state().0.chrome.keytips);
}

#[test]
fn preferences_round_trip_through_the_shell() {
    let mut h = harness();
    h.run();
    let panels = h.state().1.panels();
    let (shell, _) = h.state_mut();
    shell.mode = jcj_shell::theme::Mode::Dark;
    shell.layout.show(&panels, "inspector");
    shell.layout.set_pinned("inspector", false);
    shell.keys.bind("Mod+K", "sample.bold");
    shell.add_recent("/documents/a.sample");
    let prefs = shell.prefs();
    let text = serde_json::to_string(&prefs).unwrap();
    let mut other = Shell::new("Sample", "1.2.3", jcj_shell::theme::Tokens::default());
    other.apply_prefs(&serde_json::from_str(&text).unwrap(), &panels);
    assert_eq!(other.prefs(), prefs);
    // The theme switches with the shell's own command.
    h.state_mut().0.run(Cmd::new(shell_ids::THEME));
    h.run();
    assert_eq!(h.state().0.mode, jcj_shell::theme::Mode::Light);
}

#[test]
fn the_shell_draws_offscreen() {
    let mut shell = Shell::new("Sample", "1.2.3", jcj_shell::theme::Tokens::default());
    shell.open_file_page(Page::New);
    let mut app = Sample::default();
    let ctx = egui::Context::default();
    let pic = jcj_shell::snapshot::render(&ctx, vec2(900.0, 600.0), 1.0, Vec::new(), 3, |ui| shell.ui(ui, &mut app));
    let brand = jcj_shell::theme::Tokens::default().light.brand;
    let want = [brand.r(), brand.g(), brand.b(), 255];
    let column = (0..pic.height).flat_map(|y| (0..pic.width).map(move |x| (x, y))).filter(|&(x, y)| pic.at(x, y) == want).count();
    assert!(column > 10_000, "the File page's column is drawn in the brand color: {column}");
}
