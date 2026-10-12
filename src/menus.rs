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
    if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::F8) {
        app.library_open = false;
        return;
    }
    if is_key_pressed(KeyCode::F9) {
        app.open_keys();
        return;
    }
    if is_key_pressed(KeyCode::F1) {
        app.speed = app.speed.previous();
    }
    if is_key_pressed(KeyCode::F2) {
        app.speed = app.speed.next();
    }
    if is_key_pressed(KeyCode::F5) {
        app.scan_music();
    }
    let old = app.library_query.clone();
    if is_key_pressed(KeyCode::Backspace) {
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
            app.library_query.clear();
        } else {
            app.library_query.pop();
        }
    }
    if (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
        && is_key_pressed(KeyCode::V)
    {
        if let Some(text) = miniquad::window::clipboard_get() {
            app.library_query.push_str(text.trim());
        }
    } else {
        while let Some(c) = get_char_pressed() {
            if !c.is_control() {
                app.library_query.push(c);
            }
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

fn load_folder(app: &mut App) {
    let path = std::path::PathBuf::from(
        app.folder_browser
            .as_ref()
            .unwrap()
            .input
            .trim()
            .trim_matches('"'),
    );
    if let Err(error) = app.set_music_folder(path) {
        app.folder_browser.as_mut().unwrap().error = Some(error);
    }
}

pub fn folder_input(app: &mut App) {
    if is_key_pressed(KeyCode::Escape) {
        app.folder_browser = None;
        return;
    }
    let control = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
    let characters: Vec<_> = std::iter::from_fn(get_char_pressed).collect();
    let browser = app.folder_browser.as_mut().unwrap();
    if (control && is_key_pressed(KeyCode::A)) || characters.contains(&'\u{1}') {
        browser.replace_input = true;
    } else if (control && is_key_pressed(KeyCode::V)) || characters.contains(&'\u{16}') {
        if let Some(text) = miniquad::window::clipboard_get() {
            if browser.replace_input {
                browser.input.clear();
                browser.replace_input = false;
            }
            browser.input.push_str(text.trim());
        }
    } else {
        if is_key_pressed(KeyCode::Backspace) {
            if browser.replace_input {
                browser.input.clear();
                browser.replace_input = false;
            } else {
                browser.input.pop();
            }
        }
        for c in characters {
            if !control && !c.is_control() {
                if browser.replace_input {
                    browser.input.clear();
                    browser.replace_input = false;
                }
                browser.input.push(c);
            }
        }
    }
    let max = browser.folders.len().saturating_sub(1);
    if is_key_pressed(KeyCode::Up) {
        browser.selected = browser.selected.saturating_sub(1);
    }
    if is_key_pressed(KeyCode::Down) {
        browser.selected = (browser.selected + 1).min(max);
    }
    if is_key_pressed(KeyCode::PageUp) || mouse_wheel().1 > 0.0 {
        browser.selected = browser.selected.saturating_sub(6);
    }
    if is_key_pressed(KeyCode::PageDown) || mouse_wheel().1 < 0.0 {
        browser.selected = (browser.selected + 6).min(max);
    }
    if is_key_pressed(KeyCode::Enter) {
        if control {
            load_folder(app);
        } else if browser.input != browser.directory.to_string_lossy() {
            browser.open_input();
        } else if let Some(path) = browser.folders.get(browser.selected).cloned()
            && let Err(error) = browser.navigate(&path)
        {
            browser.error = Some(error);
        }
    }
}

pub fn draw(app: &mut App, ui: &Ui, meta: &Ui) {
    if app.library_open {
        draw_library(app, ui, meta);
    }
    if app.key_editor.is_some() {
        draw_keys(app, ui);
    }
    if app.speed_menu {
        draw_speeds(app, ui);
    }
    if app.folder_browser.is_some() {
        draw_folders(app, ui, meta);
    }
}

fn draw_folders(app: &mut App, ui: &Ui, meta: &Ui) {
    ui.rect(0.0, 0.0, 1280.0, 800.0, Color::new(0.0, 0.0, 0.0, 0.82));
    ui.rect(270.0, 158.0, 740.0, 488.0, PANEL);
    ui.text("CHOOSE MUSIC FOLDER", 300.0, 203.0, 26.0, INK);
    ui.text(
        "Select the folder containing your charts and keysound banks.",
        300.0,
        233.0,
        14.0,
        MUTED,
    );
    let browser = app.folder_browser.as_mut().unwrap();
    ui.rect(
        300.0,
        250.0,
        680.0,
        43.0,
        if browser.replace_input {
            Color::new(0.12, 0.17, 0.10, 1.0)
        } else {
            BG
        },
    );
    meta.fit(&browser.input, 311.0, 278.0, 14.0, 653.0, INK);
    if ui.button("PARENT", 300.0, 303.0, 112.0, 32.0, false) {
        browser.parent();
    }
    if ui.button("HOME", 424.0, 303.0, 112.0, 32.0, false) {
        browser.home();
    }
    if ui.button("GO TO PATH", 548.0, 303.0, 152.0, 32.0, false) {
        browser.open_input();
    }
    ui.text(
        "CTRL+A / CTRL+V to replace the path",
        713.0,
        324.0,
        11.0,
        MUTED,
    );
    let page = browser.selected / 6;
    let mut navigate = None;
    for row in 0..6 {
        let index = page * 6 + row;
        let Some(path) = browser.folders.get(index) else {
            break;
        };
        let y = 345.0 + row as f32 * 30.0;
        ui.rect(
            300.0,
            y,
            680.0,
            27.0,
            if index == browser.selected {
                Color::new(0.09, 0.12, 0.14, 1.0)
            } else {
                BG
            },
        );
        meta.fit(
            &path.file_name().unwrap_or_default().to_string_lossy(),
            313.0,
            y + 19.0,
            14.0,
            623.0,
            INK,
        );
        ui.text(">", 959.0, y + 19.0, 15.0, MUTED);
        if ui.hover(300.0, y, 680.0, 27.0) && is_mouse_button_pressed(MouseButton::Left) {
            navigate = Some(path.clone());
        }
    }
    if let Some(path) = navigate
        && let Err(error) = browser.navigate(&path)
    {
        browser.error = Some(error);
    }
    if browser.folders.is_empty() {
        ui.center(
            "No subfolders. You can load this folder.",
            640.0,
            428.0,
            16.0,
            MUTED,
        );
    }
    if let Some(error) = &browser.error {
        ui.fit(
            error,
            300.0,
            550.0,
            12.0,
            680.0,
            Color::new(1.0, 0.55, 0.4, 1.0),
        );
    } else {
        ui.text(
            &format!(
                "{} folders / page {} of {}",
                browser.folders.len(),
                browser.selected / 6 + 1,
                browser.folders.len().div_ceil(6).max(1)
            ),
            300.0,
            550.0,
            12.0,
            MUTED,
        );
    }
    let mut cancel = false;
    let mut load = false;
    if ui.button("<", 300.0, 574.0, 44.0, 37.0, false) {
        browser.selected = browser.selected.saturating_sub(6);
    }
    if ui.button(">", 356.0, 574.0, 44.0, 37.0, false) {
        browser.selected = (browser.selected + 6).min(browser.folders.len().saturating_sub(1));
    }
    if ui.button("CANCEL", 607.0, 574.0, 145.0, 37.0, false) {
        cancel = true;
    }
    if ui.button("LOAD FOLDER", 770.0, 574.0, 210.0, 37.0, true) {
        load = true;
    }
    ui.text(
        "ENTER opens a folder / CTRL+ENTER loads it / ESC cancels",
        300.0,
        629.0,
        12.0,
        MUTED,
    );
    if cancel {
        app.folder_browser = None;
    } else if load {
        load_folder(app);
    }
}

fn draw_speeds(app: &mut App, ui: &Ui) {
    use hyperjam::speed::{PRESETS, ScrollSpeed};
    ui.rect(0.0, 0.0, 1280.0, 800.0, Color::new(0.0, 0.0, 0.0, 0.82));
    ui.rect(330.0, 233.0, 620.0, 334.0, PANEL);
    ui.text("NOTE SPEED", 360.0, 278.0, 27.0, INK);
    ui.text(
        "Speed presets / F1 decrease / F2 increase",
        360.0,
        310.0,
        15.0,
        MUTED,
    );
    for index in 0..PRESETS.len() {
        let speed = ScrollSpeed::from_index(index);
        if ui.button(
            &speed.label(),
            360.0 + (index % 4) as f32 * 145.0,
            334.0 + (index / 4) as f32 * 45.0,
            125.0,
            35.0,
            speed == app.speed,
        ) {
            app.speed = speed;
            app.speed_menu = false;
        }
    }
    ui.text(
        "Changes note scrolling. The song and hit timing stay at their original rate.",
        360.0,
        540.0,
        12.0,
        MUTED,
    );
    if ui.button("CLOSE", 795.0, 495.0, 125.0, 29.0, false) {
        app.speed_menu = false;
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
        495.0,
        MUTED,
    );
    if ui.button(
        if app.native_folder_picker.is_some() {
            "OPENING..."
        } else {
            "FOLDER..."
        },
        852.0,
        123.0,
        122.0,
        34.0,
        false,
    ) && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
        if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
            app.open_folder_browser();
        } else {
            app.open_system_folder_picker();
        }
    }
    let mut close = false;
    if ui.button("CLOSE", 989.0, 123.0, 96.0, 34.0, false)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
        close = true;
    }
    ui.rect(195.0, 177.0, 890.0, 43.0, BG);
    ui.fit(
        if app.library_query.is_empty() {
            "Filter by name, artist, filename, or ID..."
        } else {
            &app.library_query
        },
        211.0,
        204.0,
        16.0,
        815.0,
        if app.library_query.is_empty() {
            MUTED
        } else {
            INK
        },
    );
    if !app.library_query.is_empty()
        && ui.button("X", 1045.0, 182.0, 35.0, 31.0, false)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
        app.library_query.clear();
        app.library_selected = 0;
    }
    use hyperjam::library::SortColumn;
    for (column, label, x, width) in [
        (SortColumn::Title, "NAME", 195.0, 145.0),
        (SortColumn::Artist, "ARTIST", 340.0, 145.0),
        (SortColumn::Id, "ID", 762.0, 81.0),
        (SortColumn::Bpm, "BPM", 843.0, 75.0),
        (SortColumn::Easy, "EASY", 918.0, 50.0),
        (SortColumn::Normal, "NORMAL", 968.0, 67.0),
        (SortColumn::Hard, "HARD", 1035.0, 50.0),
    ] {
        if ui.hover(x, 229.0, width, 28.0)
            && is_mouse_button_pressed(MouseButton::Left)
            && app.key_editor.is_none()
            && app.folder_browser.is_none()
            && app.native_folder_picker.is_none()
        {
            app.sort_library(column);
        }
        let active = app.library_sort.column == column;
        let text = if active {
            format!(
                "{label} {}",
                if app.library_sort.descending {
                    "v"
                } else {
                    "^"
                }
            )
        } else {
            label.into()
        };
        ui.center(
            &text,
            x + width / 2.0,
            247.0,
            11.0,
            if active { LIME } else { MUTED },
        );
        if active {
            ui.rect(x + 5.0, 255.0, width - 10.0, 1.0, LIME);
        }
    }
    let filtered = app.library.filter(&app.library_query);
    app.library_selected = app.library_selected.min(filtered.len().saturating_sub(1));
    let page = app.library_selected / 8;
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
            540.0,
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
            540.0,
            MUTED,
        );
        ui.fit(
            &entry.header.id.to_string(),
            772.0,
            y + 25.0,
            13.0,
            65.0,
            INK,
        );
        ui.text(
            &format!("{:.0}", entry.header.bpm),
            852.0,
            y + 25.0,
            15.0,
            INK,
        );
        for (level, x) in entry.header.levels.iter().zip([943.0, 1001.5, 1060.0]) {
            ui.center(&format!("{level:02}"), x, y + 25.0, 15.0, CYAN);
        }
        if ui.hover(195.0, y, 890.0, 40.0)
            && is_mouse_button_pressed(MouseButton::Left)
            && app.key_editor.is_none()
            && app.folder_browser.is_none()
            && app.native_folder_picker.is_none()
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
    if ui.button("<", 853.0, 612.0, 50.0, 30.0, false)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
        app.library_selected = app.library_selected.saturating_sub(8);
    }
    if ui.button(">", 914.0, 612.0, 50.0, 30.0, false)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
        app.library_selected = (app.library_selected + 8).min(filtered.len().saturating_sub(1));
    }
    if ui.button("RESCAN", 975.0, 612.0, 110.0, 30.0, false)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
        app.scan_music();
    }
    if ui.button("PRACTICE", 195.0, 662.0, 160.0, 37.0, false)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
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
        "UP/DOWN / WHEEL    CLICK HEADERS TO SORT    ENTER LOAD",
        380.0,
        686.0,
        12.0,
        MUTED,
    );
    if ui.button("LOAD CHART", 899.0, 662.0, 186.0, 37.0, true)
        && app.key_editor.is_none()
        && app.folder_browser.is_none()
        && app.native_folder_picker.is_none()
    {
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
