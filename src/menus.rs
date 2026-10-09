use crate::{App, BG, CYAN, INK, LIME, MUTED, PANEL, Ui};
use hyperjam::{
    audio::practice_bank,
    bindings::{Bindings, allowed_keys},
    chart::practice_song,
};
use macroquad::prelude::*;

pub struct KeyEditor {
    pub draft: Bindings,
    pub selected: usize,
    pub capture: Option<usize>,
    pub error: Option<String>,
}
impl KeyEditor {
    pub fn new(draft: Bindings) -> Self {
        Self {
            draft,
            selected: 0,
            capture: None,
            error: None,
        }
    }
}

fn save_keys(app: &mut App) {
    let Some(editor) = &mut app.key_editor else {
        return;
    };
    match editor.draft.save(&app.bindings_path) {
        Ok(()) => {
            app.bindings = editor.draft.clone();
            app.key_editor = None;
        }
        Err(error) => editor.error = Some(error),
    }
}

pub fn keys_input(app: &mut App) {
    while get_char_pressed().is_some() {}
    let editor = app.key_editor.as_mut().unwrap();
    if is_key_pressed(KeyCode::Escape) {
        if editor.capture.take().is_none() {
            app.key_editor = None;
        }
        return;
    }
    if is_key_pressed(KeyCode::Tab) {
        editor.selected = (editor.selected + 1) % 7;
        editor.capture = None;
    }
    if is_key_pressed(KeyCode::Enter) {
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
            save_keys(app);
        } else {
            editor.capture = Some(editor.selected);
        }
        return;
    }
    if let Some(lane) = editor.capture {
        if let Some(key) = allowed_keys().into_iter().find(|&key| is_key_pressed(key)) {
            match editor.draft.assign(lane, key) {
                Ok(()) => {
                    editor.capture = None;
                    editor.error = None;
                }
                Err(error) => editor.error = Some(error),
            }
        } else if is_any_key_down() && !get_keys_pressed().is_empty() {
            editor.error = Some("This key is reserved for game controls.".into());
        }
    }
}

pub fn library_input(app: &mut App) {
    if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::F1) {
        app.library_open = false;
        return;
    }
    if is_key_pressed(KeyCode::F2) {
        app.open_keys();
        return;
    }
    if is_key_pressed(KeyCode::F5) {
        app.scan_music();
    }
    let old = app.library_query.clone();
    if is_key_pressed(KeyCode::Backspace) {
        app.library_query.pop();
    }
    while let Some(c) = get_char_pressed() {
        if !c.is_control() {
            app.library_query.push(c);
        }
    }
    if old != app.library_query {
        app.library_selected = 0;
    }
    let count = app.library.filter(&app.library_query).len();
    let max = count.saturating_sub(1);
    if is_key_pressed(KeyCode::Up) {
        app.library_selected = app.library_selected.saturating_sub(1);
    }
    if is_key_pressed(KeyCode::Down) {
        app.library_selected = (app.library_selected + 1).min(max);
    }
    if is_key_pressed(KeyCode::PageUp) {
        app.library_selected = app.library_selected.saturating_sub(8);
    }
    if is_key_pressed(KeyCode::PageDown) {
        app.library_selected = (app.library_selected + 8).min(max);
    }
    let wheel = mouse_wheel().1;
    if wheel > 0.0 {
        app.library_selected = app.library_selected.saturating_sub(3);
    }
    if wheel < 0.0 {
        app.library_selected = (app.library_selected + 3).min(max);
    }
    app.library_selected = app.library_selected.min(max);
    if is_key_pressed(KeyCode::Enter) {
        app.select_library();
    }
}

pub fn draw(app: &mut App, ui: &Ui, meta: &Ui) {
    if app.library_open {
        draw_library(app, ui, meta);
    }
    if app.key_editor.is_some() {
        draw_keys(app, ui);
    }
}

fn draw_library(app: &mut App, ui: &Ui, meta: &Ui) {
    ui.rect(0.0, 0.0, 1280.0, 800.0, Color::new(0.0, 0.0, 0.0, 0.82));
    ui.rect(165.0, 106.0, 950.0, 612.0, PANEL);
    ui.text("MUSIC", 195.0, 151.0, 27.0, INK);
    ui.fit(
        &format!(
            "{} charts / {}",
            app.library.entries.len(),
            app.music.display()
        ),
        330.0,
        147.0,
        14.0,
        540.0,
        MUTED,
    );
    let mut close = false;
    if ui.button("CLOSE", 989.0, 123.0, 96.0, 34.0, false) && app.key_editor.is_none() {
        close = true;
    }
    ui.rect(195.0, 177.0, 890.0, 43.0, BG);
    ui.fit(
        if app.library_query.is_empty() {
            "Type to search title, artist, or filename..."
        } else {
            &app.library_query
        },
        211.0,
        204.0,
        16.0,
        835.0,
        if app.library_query.is_empty() {
            MUTED
        } else {
            INK
        },
    );
    let filtered = app.library.filter(&app.library_query);
    app.library_selected = app.library_selected.min(filtered.len().saturating_sub(1));
    let page = app.library_selected / 8;
    ui.text("TITLE / ARTIST", 211.0, 247.0, 11.0, MUTED);
    ui.text("BPM", 852.0, 247.0, 11.0, MUTED);
    ui.text("EASY / NORMAL / HARD", 924.0, 247.0, 11.0, MUTED);
    let mut load = false;
    for row in 0..8 {
        let selection = page * 8 + row;
        let Some(&index) = filtered.get(selection) else {
            break;
        };
        let entry = &app.library.entries[index];
        let y = 260.0 + row as f32 * 43.0;
        let selected = selection == app.library_selected;
        ui.rect(
            195.0,
            y,
            890.0,
            40.0,
            if selected {
                Color::new(0.12, 0.17, 0.10, 1.0)
            } else {
                BG
            },
        );
        if selected {
            ui.rect(195.0, y, 3.0, 40.0, LIME);
        }
        meta.fit(
            &entry.header.title,
            211.0,
            y + 18.0,
            16.0,
            620.0,
            if selected { LIME } else { INK },
        );
        meta.fit(
            &format!(
                "{}  /  {}",
                entry.header.artist,
                entry.path.file_name().unwrap_or_default().to_string_lossy()
            ),
            211.0,
            y + 33.0,
            10.0,
            620.0,
            MUTED,
        );
        ui.text(
            &format!("{:.0}", entry.header.bpm),
            852.0,
            y + 25.0,
            15.0,
            INK,
        );
        let [easy, normal, hard] = entry.header.levels;
        ui.text(
            &format!("{easy:02}   /   {normal:02}   /   {hard:02}"),
            936.0,
            y + 25.0,
            15.0,
            CYAN,
        );
        if ui.hover(195.0, y, 890.0, 40.0)
            && is_mouse_button_pressed(MouseButton::Left)
            && app.key_editor.is_none()
        {
            app.library_selected = selection;
        }
    }
    if app.library_loader.is_some() {
        ui.center("SCANNING MUSIC...", 640.0, 427.0, 22.0, LIME);
    } else if filtered.is_empty() {
        ui.center("NO MATCHING CHARTS", 640.0, 427.0, 22.0, MUTED);
    }
    ui.text(
        &format!(
            "{} matches / page {} of {}",
            filtered.len(),
            page + 1,
            filtered.len().div_ceil(8).max(1)
        ),
        195.0,
        632.0,
        13.0,
        MUTED,
    );
    if ui.button("<", 853.0, 612.0, 50.0, 30.0, false) && app.key_editor.is_none() {
        app.library_selected = app.library_selected.saturating_sub(8);
    }
    if ui.button(">", 914.0, 612.0, 50.0, 30.0, false) && app.key_editor.is_none() {
        app.library_selected = (app.library_selected + 8).min(filtered.len().saturating_sub(1));
    }
    if ui.button("RESCAN", 975.0, 612.0, 110.0, 30.0, false) && app.key_editor.is_none() {
        app.scan_music();
    }
    if ui.button("PRACTICE", 195.0, 662.0, 160.0, 37.0, false) && app.key_editor.is_none() {
        app.ready();
        app.song = practice_song();
        let rate = app.audio.as_ref().map(|a| a.rate).unwrap_or(44100);
        app.bank = practice_bank(rate);
        app.path = None;
        app.samples = None;
        app.warnings.clear();
        app.error = None;
        app.ready();
        close = true;
    }
    ui.text(
        "UP/DOWN / WHEEL    ENTER LOAD    F2 KEYS",
        380.0,
        686.0,
        12.0,
        MUTED,
    );
    if ui.button("LOAD CHART", 899.0, 662.0, 186.0, 37.0, true) && app.key_editor.is_none() {
        load = true;
    }
    if close {
        app.library_open = false;
    }
    if load {
        app.select_library();
    }
}

fn draw_keys(app: &mut App, ui: &Ui) {
    ui.rect(0.0, 0.0, 1280.0, 800.0, Color::new(0.0, 0.0, 0.0, 0.82));
    ui.rect(210.0, 237.0, 860.0, 332.0, PANEL);
    ui.text("LANE KEYS", 240.0, 282.0, 27.0, INK);
    ui.text(
        "Click a lane, then press its new key. Existing bindings swap lanes.",
        240.0,
        314.0,
        15.0,
        MUTED,
    );
    let editor = app.key_editor.as_mut().unwrap();
    for (lane, label) in editor.draft.labels().iter().enumerate() {
        let x = 240.0 + lane as f32 * 115.0;
        ui.center(&format!("LANE {}", lane + 1), x + 52.0, 349.0, 12.0, MUTED);
        if ui.button(
            if editor.capture == Some(lane) {
                "PRESS..."
            } else {
                label
            },
            x,
            365.0,
            104.0,
            56.0,
            editor.selected == lane,
        ) {
            editor.selected = lane;
            editor.capture = Some(lane);
            editor.error = None;
        }
    }
    if let Some(error) = &editor.error {
        ui.fit(
            error,
            240.0,
            451.0,
            14.0,
            800.0,
            Color::new(1.0, 0.55, 0.4, 1.0),
        );
    } else {
        ui.text(
            "TAB select / ENTER remap / CTRL+ENTER save / ESC cancel",
            240.0,
            451.0,
            13.0,
            MUTED,
        );
    }
    ui.fit(
        &format!("Saved to {}", app.bindings_path.display()),
        240.0,
        482.0,
        12.0,
        790.0,
        MUTED,
    );
    let mut cancel = false;
    let mut save = false;
    if ui.button("DEFAULTS", 240.0, 507.0, 170.0, 37.0, false) {
        editor.draft = Bindings::default();
        editor.capture = None;
        editor.error = None;
    }
    if ui.button("CANCEL", 674.0, 507.0, 170.0, 37.0, false) {
        cancel = true;
    }
    if ui.button("SAVE KEYS", 870.0, 507.0, 170.0, 37.0, true) {
        save = true;
    }
    if cancel {
        app.key_editor = None;
    } else if save {
        save_keys(app);
    }
}
