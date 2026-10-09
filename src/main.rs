use hyperjam::{
    audio::{Audio, Bank, LoadedSong, load_song, practice_bank},
    bindings::Bindings,
    chart::{Song, practice_song},
    gameplay::{Gameplay, Judgment, NoteState},
    library::{self, Library},
};
mod menus;
use macroquad::prelude::*;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

const BG: Color = Color::new(0.027, 0.035, 0.047, 1.0);
const PANEL: Color = Color::new(0.047, 0.059, 0.078, 1.0);
const BORDER: Color = Color::new(0.12, 0.15, 0.18, 1.0);
const MUTED: Color = Color::new(0.46, 0.51, 0.57, 1.0);
const INK: Color = Color::new(0.9, 0.93, 0.95, 1.0);
const LIME: Color = Color::new(0.77, 0.95, 0.35, 1.0);
const CYAN: Color = Color::new(0.40, 0.80, 0.89, 1.0);

fn window_conf() -> Conf {
    Conf {
        window_title: "Hyperjam - 7-key rhythm player".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ready,
    Playing,
    Paused,
    Results,
}

struct Options {
    music: PathBuf,
    bindings_path: PathBuf,
    keys: Option<Bindings>,
    list_music: bool,
    validate_music: bool,
    chart: Option<PathBuf>,
    samples: Option<PathBuf>,
    difficulty: usize,
    autoplay: bool,
    offset: f64,
    speed: f32,
    volume: f32,
    inspect: bool,
    font: Option<PathBuf>,
    start: bool,
    screenshot: Option<PathBuf>,
    screenshot_frame: u64,
    screenshot_seconds: Option<f64>,
    exit_after: Option<u64>,
    exit_seconds: Option<f64>,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        music: "music".into(),
        bindings_path: "hyperjam.keys".into(),
        keys: None,
        list_music: false,
        validate_music: false,
        chart: None,
        samples: None,
        difficulty: 1,
        autoplay: false,
        offset: 0.0,
        speed: 1.0,
        volume: 0.7,
        inspect: false,
        font: None,
        start: false,
        screenshot: None,
        screenshot_frame: 10,
        screenshot_seconds: None,
        exit_after: None,
        exit_seconds: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "Hyperjam - native 7-key OJN/OJM gameplay\n\nUsage: hyperjam [chart.ojn] [--samples bank.ojm] [--difficulty easy|normal|hard]\n                [--autoplay] [--start] [--offset milliseconds] [--speed 0.5..3.0]\n                [--volume 0..100] [--inspect] [--font font.ttf]\n                [--music folder] [--keys \"S D F SPACE J K L\"]\n                [--bindings hyperjam.keys] [--list-music] [--validate-music]\n\nWithout a chart, the music picker opens when charts exist in ./music.\nDefaults: S D F Space J K L / Enter start / Esc pause / R retry / O open\n          F1 music / F2 keys / Tab autoplay / Up,Down scroll / +,- offset / F11 fullscreen\nR/O shortcuts are disabled when those letters are assigned to a lane."
                );
                std::process::exit(0);
            }
            "--samples" => o.samples = Some(args.next().ok_or("--samples needs a path")?.into()),
            "--music" => o.music = args.next().ok_or("--music needs a folder")?.into(),
            "--bindings" => {
                o.bindings_path = args
                    .next()
                    .ok_or("--bindings needs a configuration path")?
                    .into()
            }
            "--keys" => {
                o.keys = Some(Bindings::parse(
                    &args
                        .next()
                        .ok_or("--keys needs seven key names in quotes")?,
                )?)
            }
            "--list-music" => o.list_music = true,
            "--validate-music" => o.validate_music = true,
            "--difficulty" => {
                o.difficulty = match args.next().as_deref() {
                    Some("easy") => 0,
                    Some("normal") => 1,
                    Some("hard") => 2,
                    _ => return Err("Difficulty must be easy, normal or hard".into()),
                }
            }
            "--autoplay" => o.autoplay = true,
            "--offset" => {
                o.offset = args
                    .next()
                    .ok_or("--offset needs milliseconds")?
                    .parse::<f64>()
                    .map_err(|_| "Invalid offset")?
                    / 1000.0;
                if !o.offset.is_finite() || o.offset.abs() > 0.5 {
                    return Err("Offset must be between -500 and 500 ms".into());
                }
            }
            "--speed" => {
                o.speed = args
                    .next()
                    .ok_or("--speed needs a value")?
                    .parse()
                    .map_err(|_| "Invalid speed")?;
                if !(0.5..=3.0).contains(&o.speed) {
                    return Err("Speed must be between 0.5 and 3.0".into());
                }
            }
            "--volume" => {
                o.volume = args
                    .next()
                    .ok_or("--volume needs a percentage")?
                    .parse::<f32>()
                    .map_err(|_| "Invalid volume")?
                    / 100.0;
                if !(0.0..=1.0).contains(&o.volume) {
                    return Err("Volume must be between 0 and 100".into());
                }
            }
            "--inspect" => o.inspect = true,
            "--start" => o.start = true,
            "--font" => o.font = Some(args.next().ok_or("--font needs a TTF/OTF/TTC path")?.into()),
            "--screenshot-frame" => {
                o.screenshot_frame = args
                    .next()
                    .ok_or("--screenshot-frame needs a frame count")?
                    .parse()
                    .map_err(|_| "Invalid screenshot frame")?
            }
            "--screenshot-after-seconds" | "--exit-after-seconds" => {
                let seconds = args
                    .next()
                    .ok_or("Timed capture/exit needs seconds")?
                    .parse::<f64>()
                    .map_err(|_| "Invalid duration")?;
                if !seconds.is_finite() || seconds < 0.0 {
                    return Err("Duration must be finite and non-negative".into());
                }
                if arg == "--screenshot-after-seconds" {
                    o.screenshot_seconds = Some(seconds);
                } else {
                    o.exit_seconds = Some(seconds);
                }
            }
            "--screenshot" => {
                o.screenshot = Some(args.next().ok_or("--screenshot needs a path")?.into())
            }
            "--exit-after" => {
                o.exit_after = Some(
                    args.next()
                        .ok_or("--exit-after needs a frame count")?
                        .parse()
                        .map_err(|_| "Invalid frame count")?,
                )
            }
            _ if !arg.starts_with('-') && o.chart.is_none() => o.chart = Some(arg.into()),
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    Ok(o)
}

struct App {
    bindings: Bindings,
    bindings_path: PathBuf,
    key_editor: Option<menus::KeyEditor>,
    library: Library,
    library_loader: Option<Receiver<Result<Library, String>>>,
    music: PathBuf,
    library_open: bool,
    library_query: String,
    library_selected: usize,
    song: Song,
    bank: Bank,
    audio: Option<Audio>,
    game: Gameplay,
    mode: Mode,
    difficulty: usize,
    autoplay: bool,
    offset: f64,
    speed: f32,
    volume: f32,
    warnings: Vec<String>,
    error: Option<String>,
    path: Option<PathBuf>,
    samples: Option<PathBuf>,
    loader: Option<Receiver<Result<LoadedSong, String>>>,
    loading_path: Option<PathBuf>,
    path_entry: Option<String>,
    start_wall: f64,
    pause_wall: f64,
    fullscreen: bool,
}
impl App {
    fn new(o: &Options) -> Self {
        let audio = Audio::open();
        let mut warnings = vec![];
        let audio = match audio {
            Ok(a) => {
                a.volume(o.volume);
                Some(a)
            }
            Err(e) => {
                warnings.push(format!("Audio unavailable: {e}. Playing silently."));
                None
            }
        };
        let rate = audio.as_ref().map(|a| a.rate).unwrap_or(44100);
        let song = practice_song();
        let game = Gameplay::new(song.charts[o.difficulty].clone(), o.autoplay);
        let bindings = match &o.keys {
            Some(bindings) => bindings.clone(),
            None => match Bindings::load(&o.bindings_path) {
                Ok(b) => b,
                Err(e) => {
                    warnings.push(e);
                    Bindings::default()
                }
            },
        };
        let mut app = Self {
            bindings,
            bindings_path: o.bindings_path.clone(),
            key_editor: None,
            library: Library::default(),
            library_loader: None,
            music: o.music.clone(),
            library_open: o.chart.is_none() && !o.start,
            library_query: String::new(),
            library_selected: 0,
            song,
            bank: practice_bank(rate),
            audio,
            game,
            mode: Mode::Ready,
            difficulty: o.difficulty,
            autoplay: o.autoplay,
            offset: o.offset,
            speed: o.speed,
            volume: o.volume,
            warnings,
            error: None,
            path: None,
            samples: o.samples.clone(),
            loader: None,
            loading_path: None,
            path_entry: None,
            start_wall: 0.0,
            pause_wall: 0.0,
            fullscreen: false,
        };
        app.scan_music();
        app
    }
    fn modal(&self) -> bool {
        self.path_entry.is_some() || self.key_editor.is_some() || self.library_open
    }
    fn scan_music(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.library_loader = Some(rx);
        let root = self.music.clone();
        std::thread::spawn(move || {
            let _ = tx.send(library::scan(&root));
        });
    }
    fn open_library(&mut self) {
        if self.mode == Mode::Playing {
            self.pause();
        }
        self.library_open = true;
        self.library_selected = 0;
    }
    fn open_keys(&mut self) {
        if self.mode == Mode::Playing {
            self.pause();
        }
        self.key_editor = Some(menus::KeyEditor::new(self.bindings.clone()));
    }
    fn select_library(&mut self) {
        let filtered = self.library.filter(&self.library_query);
        if let Some(&index) = filtered.get(self.library_selected) {
            let path = self.library.entries[index].path.clone();
            self.library_open = false;
            self.samples = None;
            self.load(path);
        }
    }
    fn load(&mut self, path: PathBuf) {
        if let Some(a) = &self.audio {
            a.stop();
        }
        self.mode = Mode::Ready;
        self.error = None;
        let samples = self.samples.clone();
        let rate = self.audio.as_ref().map(|a| a.rate).unwrap_or(44100);
        let (tx, rx) = mpsc::channel();
        self.loading_path = Some(path.clone());
        self.loader = Some(rx);
        std::thread::spawn(move || {
            let result = load_song(&path, samples.as_deref(), rate);
            let _ = tx.send(result);
        });
    }
    fn poll_load(&mut self) {
        if let Some(rx) = &self.loader {
            match rx.try_recv() {
                Ok(result) => {
                    self.loader = None;
                    match result {
                        Ok(loaded) => {
                            self.song = loaded.song;
                            self.bank = loaded.bank;
                            self.warnings = loaded.warnings;
                            if self.audio.is_none() {
                                self.warnings
                                    .push("Audio device unavailable; gameplay is silent.".into());
                            }
                            for warning in &self.warnings {
                                eprintln!("Warning: {warning}");
                            }
                            self.path = self.loading_path.take();
                            self.game = Gameplay::new(
                                self.song.charts[self.difficulty].clone(),
                                self.autoplay,
                            );
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.loading_path = None;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loader = None;
                    self.loading_path = None;
                    self.error = Some("Chart loading worker stopped unexpectedly".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
    fn start(&mut self) {
        if self.loader.is_some() || self.song.charts[self.difficulty].notes.is_empty() {
            return;
        }
        self.game = Gameplay::new(self.song.charts[self.difficulty].clone(), self.autoplay);
        let mut events = self.game.chart.background.clone();
        if self.autoplay {
            events.extend(self.game.chart.notes.iter().map(|n| n.sound.clone()));
        }
        if let Some(a) = &self.audio {
            a.start(self.bank.clone(), events);
        }
        self.start_wall = get_time() + 2.0;
        self.mode = Mode::Playing;
    }
    fn time(&self) -> f64 {
        if self.mode == Mode::Ready {
            return -2.0;
        }
        let raw = if let Some(a) = &self.audio {
            a.time()
        } else {
            (if self.mode == Mode::Paused {
                self.pause_wall
            } else {
                get_time()
            }) - self.start_wall
        };
        raw - self.offset
    }
    fn pause(&mut self) {
        match self.mode {
            Mode::Playing => {
                self.mode = Mode::Paused;
                self.pause_wall = get_time();
                if let Some(a) = &self.audio {
                    a.pause(true);
                }
            }
            Mode::Paused => {
                self.start_wall += get_time() - self.pause_wall;
                self.mode = Mode::Playing;
                if let Some(a) = &self.audio {
                    a.pause(false);
                }
            }
            _ => {}
        }
    }
    fn ready(&mut self) {
        if let Some(a) = &self.audio {
            a.stop();
        }
        self.mode = Mode::Ready;
        self.game = Gameplay::new(self.song.charts[self.difficulty].clone(), self.autoplay);
    }
    fn difficulty(&mut self, difficulty: usize) {
        if self.mode == Mode::Ready {
            self.difficulty = difficulty;
            self.game = Gameplay::new(self.song.charts[difficulty].clone(), self.autoplay);
        }
    }
    fn input(&mut self) {
        self.poll_load();
        if let Some(rx) = &self.library_loader {
            match rx.try_recv() {
                Ok(result) => {
                    self.library_loader = None;
                    match result {
                        Ok(library) => {
                            if library.entries.is_empty() {
                                self.library_open = false;
                            }
                            for warning in &library.warnings {
                                eprintln!("Library: {warning}");
                            }
                            self.library = library;
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.library_open = false;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.library_loader = None;
                    self.library_open = false;
                    self.error = Some("Music scanner stopped unexpectedly".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let files = get_dropped_files();
        let dropped: Vec<_> = files.into_iter().filter_map(|f| f.path).collect();
        if !dropped.is_empty() {
            self.samples = dropped
                .iter()
                .find(|p| {
                    matches!(
                        p.extension()
                            .and_then(|e| e.to_str())
                            .map(str::to_lowercase)
                            .as_deref(),
                        Some("ojm" | "omc" | "m30")
                    )
                })
                .cloned();
            if let Some(chart) = dropped
                .iter()
                .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ojn")))
            {
                self.library_open = false;
                self.load(chart.clone());
            } else if let Some(path) = self.path.clone() {
                self.load(path);
            } else {
                self.error =
                    Some("Drop an OJN chart together with its OJM/OMC/M30 sample bank.".into());
            }
        }
        if self.key_editor.is_some() {
            menus::keys_input(self);
            return;
        }
        if self.library_open {
            menus::library_input(self);
            return;
        }
        if let Some(entry) = &mut self.path_entry {
            if is_key_pressed(KeyCode::Escape) {
                self.path_entry = None;
                return;
            }
            if is_key_pressed(KeyCode::Backspace) {
                entry.pop();
            }
            if (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
                && is_key_pressed(KeyCode::V)
            {
                if let Some(clipboard) = miniquad::window::clipboard_get() {
                    entry.push_str(clipboard.trim());
                }
            } else {
                while let Some(c) = get_char_pressed() {
                    if !c.is_control() {
                        entry.push(c);
                    }
                }
            }
            if is_key_pressed(KeyCode::Enter) {
                let path = PathBuf::from(entry.trim().trim_matches('"'));
                self.path_entry = None;
                self.samples = None;
                self.load(path);
            }
            return;
        }
        while get_char_pressed().is_some() {}
        if is_key_pressed(KeyCode::F11) {
            self.fullscreen = !self.fullscreen;
            set_fullscreen(self.fullscreen);
        }
        if is_key_pressed(KeyCode::F1) {
            self.open_library();
            return;
        }
        if is_key_pressed(KeyCode::F2) {
            self.open_keys();
            return;
        }
        if is_key_pressed(KeyCode::F3)
            || (is_key_pressed(KeyCode::O) && !self.bindings.keys.contains(&KeyCode::O))
        {
            if self.mode == Mode::Playing {
                self.pause();
            }
            self.path_entry = Some(String::new());
            return;
        }
        if is_key_pressed(KeyCode::Escape) {
            self.pause();
        }
        if is_key_pressed(KeyCode::Enter) {
            match self.mode {
                Mode::Ready | Mode::Results => self.start(),
                Mode::Paused => self.pause(),
                _ => {}
            }
        }
        if is_key_pressed(KeyCode::F4)
            || (is_key_pressed(KeyCode::R) && !self.bindings.keys.contains(&KeyCode::R))
        {
            self.start();
        }
        if is_key_pressed(KeyCode::Tab) && self.mode == Mode::Ready {
            self.autoplay = !self.autoplay;
            self.game.autoplay = self.autoplay;
        }
        if is_key_pressed(KeyCode::Up) {
            self.speed = (self.speed + 0.1).min(3.0);
        }
        if is_key_pressed(KeyCode::Down) {
            self.speed = (self.speed - 0.1).max(0.5);
        }
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
            self.offset = (self.offset + 0.010).min(0.5);
        }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
            self.offset = (self.offset - 0.010).max(-0.5);
        }
        if self.mode == Mode::Playing {
            let time = self.time();
            self.game.update(time);
            if !self.autoplay {
                for (lane, key) in self.bindings.keys.iter().enumerate() {
                    if is_key_pressed(*key)
                        && let Some(sound) = self.game.press(lane, time)
                        && let Some(a) = &self.audio
                    {
                        a.hit(sound);
                    }
                    // Reconcile held keys after a pause, including releases made while paused.
                    if self.game.pressed[lane] && !is_key_down(*key) {
                        self.game.release(lane, time);
                    }
                }
            }
            if self.game.finished(time) {
                self.mode = Mode::Results;
                if let Some(a) = &self.audio {
                    a.pause(true);
                }
            }
        }
    }
}

struct Ui<'a> {
    font: &'a Font,
    scale: f32,
    origin: Vec2,
    raster: f32,
}
impl<'a> Ui<'a> {
    fn new(font: &'a Font) -> Self {
        let scale = (screen_width() / 1280.0).min(screen_height() / 800.0);
        Self {
            font,
            scale,
            origin: vec2(
                (screen_width() - 1280.0 * scale) / 2.0,
                (screen_height() - 800.0 * scale) / 2.0,
            ),
            raster: scale * screen_dpi_scale(),
        }
    }
    fn point(&self, x: f32, y: f32) -> Vec2 {
        self.origin + vec2(x, y) * self.scale
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let p = self.point(x, y);
        draw_rectangle(p.x, p.y, w * self.scale, h * self.scale, color);
    }
    fn line(&self, x: f32, y: f32, x2: f32, y2: f32, width: f32, color: Color) {
        let a = self.point(x, y);
        let b = self.point(x2, y2);
        draw_line(a.x, a.y, b.x, b.y, width * self.scale, color);
    }
    fn text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let p = self.point(x, y);
        let raster = self.raster.max(0.1);
        draw_text_ex(
            text,
            p.x,
            p.y,
            TextParams {
                font: Some(self.font),
                font_size: (size * raster).round().clamp(1.0, 512.0) as u16,
                font_scale: self.scale / raster,
                color,
                ..Default::default()
            },
        );
    }
    fn center(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let raster = self.raster.max(0.1);
        let width = measure_text(
            text,
            Some(self.font),
            (size * raster).round().clamp(1.0, 512.0) as u16,
            1.0,
        )
        .width
            / raster;
        self.text(text, x - width / 2.0, y, size, color);
    }
    fn fit(&self, text: &str, x: f32, y: f32, size: f32, width: f32, color: Color) {
        let mut display = text.to_owned();
        while measure_text(
            &display,
            Some(self.font),
            (size * self.raster.max(0.1)).round().max(1.0) as u16,
            1.0,
        )
        .width
            / self.raster.max(0.1)
            > width
        {
            if display.ends_with("...") {
                display.truncate(display.len() - 3);
            }
            if display.pop().is_none() {
                break;
            }
            display.push_str("...");
        }
        self.text(&display, x, y, size, color);
    }
    fn wrap(&self, text: &str, x: f32, mut y: f32, size: f32, width: f32, color: Color) {
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if measure_text(
                &candidate,
                Some(self.font),
                (size * self.raster.max(0.1)).round().max(1.0) as u16,
                1.0,
            )
            .width
                / self.raster.max(0.1)
                > width
                && !line.is_empty()
            {
                self.text(&line, x, y, size, color);
                y += size * 1.45;
                line = word.into();
            } else {
                line = candidate;
            }
        }
        self.fit(&line, x, y, size, width, color);
    }
    fn hover(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (mx, my) = mouse_position();
        let m = (vec2(mx, my) - self.origin) / self.scale;
        Rect::new(x, y, w, h).contains(m)
    }
    fn button(&self, label: &str, x: f32, y: f32, w: f32, h: f32, active: bool) -> bool {
        let hover = self.hover(x, y, w, h);
        self.rect(
            x,
            y,
            w,
            h,
            if active {
                LIME
            } else if hover {
                Color::new(0.13, 0.17, 0.2, 1.0)
            } else {
                PANEL
            },
        );
        if !active {
            self.line(x, y + h, x + w, y + h, 1.0, BORDER);
        }
        self.center(
            label,
            x + w / 2.0,
            y + h / 2.0 + 5.0,
            18.0,
            if active { BG } else { INK },
        );
        hover && is_mouse_button_pressed(MouseButton::Left)
    }
}

fn lane_color(lane: usize) -> Color {
    if lane == 3 {
        LIME
    } else if lane % 2 == 1 {
        CYAN
    } else {
        Color::new(0.86, 0.89, 0.96, 1.0)
    }
}
fn clock(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn draw(app: &mut App, font: &Font, metadata_font: &Font) {
    clear_background(BG);
    let ui = Ui::new(font);
    let meta_ui = Ui::new(metadata_font);
    ui.text("HYPER", 40.0, 52.0, 29.0, INK);
    ui.text("JAM", 133.0, 52.0, 29.0, LIME);
    ui.text("7-KEY RHYTHM PLAYER", 245.0, 49.0, 13.0, MUTED);
    if ui.button("MUSIC / F1", 790.0, 23.0, 151.0, 39.0, false) && !app.modal() {
        app.open_library();
    }
    if ui.button("KEYS / F2", 961.0, 23.0, 151.0, 39.0, false) && !app.modal() {
        app.open_keys();
    }
    ui.text(
        &format!("{:.0}x HiDPI", screen_dpi_scale()),
        1135.0,
        49.0,
        13.0,
        LIME,
    );
    ui.line(40.0, 82.0, 1240.0, 82.0, 1.0, BORDER);
    ui.text("NOW PLAYING", 40.0, 128.0, 13.0, LIME);
    meta_ui.fit(&app.song.title, 40.0, 174.0, 32.0, 285.0, INK);
    meta_ui.fit(&app.song.artist, 40.0, 203.0, 16.0, 280.0, MUTED);
    ui.text(&format!("{:.0} BPM", app.song.bpm), 40.0, 246.0, 17.0, INK);
    ui.text(
        &format!("{}   /   7 KEYS", clock(app.game.chart.duration)),
        153.0,
        246.0,
        14.0,
        MUTED,
    );
    ui.line(40.0, 268.0, 318.0, 268.0, 1.0, BORDER);
    ui.text("DIFFICULTY", 40.0, 302.0, 12.0, MUTED);
    for (i, label) in ["EASY", "NORMAL", "HARD"].iter().enumerate() {
        if ui.button(
            label,
            40.0 + i as f32 * 96.0,
            318.0,
            86.0,
            38.0,
            app.difficulty == i,
        ) && !app.modal()
        {
            app.difficulty(i);
        }
        ui.center(
            &format!("LV {}", app.song.charts[i].level),
            83.0 + i as f32 * 96.0,
            379.0,
            12.0,
            MUTED,
        );
    }
    ui.line(40.0, 404.0, 318.0, 404.0, 1.0, BORDER);
    ui.text("SCROLL SPEED", 40.0, 439.0, 12.0, MUTED);
    ui.text(&format!("{:.1}x", app.speed), 258.0, 439.0, 18.0, INK);
    ui.rect(40.0, 455.0, 278.0, 3.0, BORDER);
    ui.rect(40.0, 455.0, (app.speed - 0.5) / 2.5 * 278.0, 3.0, LIME);
    if ui.hover(40.0, 445.0, 278.0, 25.0) && is_mouse_button_down(MouseButton::Left) && !app.modal()
    {
        let x = (mouse_position().0 - ui.origin.x) / ui.scale;
        app.speed = (0.5 + (x - 40.0) / 278.0 * 2.5).clamp(0.5, 3.0);
    }
    ui.text("TIMING OFFSET", 40.0, 493.0, 12.0, MUTED);
    ui.text(
        &format!("{:+.0} ms", app.offset * 1000.0),
        245.0,
        493.0,
        17.0,
        INK,
    );
    ui.text("Adjust with + / -  (10 ms)", 40.0, 517.0, 13.0, MUTED);
    ui.text("AUTOPLAY", 40.0, 558.0, 12.0, MUTED);
    if ui.button(
        if app.autoplay { "ON" } else { "OFF" },
        254.0,
        537.0,
        64.0,
        32.0,
        app.autoplay,
    ) && app.mode == Mode::Ready
        && !app.modal()
    {
        app.autoplay = !app.autoplay;
        app.game.autoplay = app.autoplay;
    }
    let start_label = match app.mode {
        Mode::Playing => "PAUSE   /   ESC",
        Mode::Paused => "RESUME   /   ENTER",
        Mode::Results => "PLAY AGAIN   /   ENTER",
        Mode::Ready => {
            if app.loader.is_some() {
                "LOADING..."
            } else {
                "START   /   ENTER"
            }
        }
    };
    if ui.button(start_label, 40.0, 609.0, 278.0, 48.0, true) && !app.modal() {
        match app.mode {
            Mode::Playing | Mode::Paused => app.pause(),
            _ => app.start(),
        }
    }
    if ui.button("OPEN CHART   /   F3", 40.0, 670.0, 278.0, 42.0, false) && !app.modal() {
        if app.mode == Mode::Playing {
            app.pause();
        }
        app.path_entry = Some(String::new());
    }
    ui.text(
        "or drop an .ojn + .ojm onto this window",
        40.0,
        739.0,
        12.0,
        MUTED,
    );

    let stage_x = 374.0;
    let stage_w = 532.0;
    let lane_w = 76.0;
    let top = 137.0;
    let hit = 645.0;
    ui.rect(stage_x, 102.0, stage_w, 642.0, PANEL);
    ui.text("7K", 390.0, 125.0, 14.0, INK);
    ui.text(
        if app.autoplay {
            "AUTO SESSION"
        } else {
            "FREE PLAY"
        },
        434.0,
        125.0,
        12.0,
        MUTED,
    );
    ui.fit(
        &app.bindings.labels().join("  "),
        675.0,
        125.0,
        12.0,
        216.0,
        MUTED,
    );
    let time = app.time();
    for (lane, label) in app.bindings.labels().iter().enumerate() {
        let x = stage_x + lane as f32 * lane_w;
        ui.rect(
            x + 1.0,
            top,
            lane_w - 2.0,
            hit - top,
            if lane == 3 {
                Color::new(0.065, 0.085, 0.055, 1.0)
            } else if lane % 2 == 1 {
                Color::new(0.035, 0.047, 0.062, 1.0)
            } else {
                Color::new(0.045, 0.055, 0.068, 1.0)
            },
        );
        ui.line(x, top, x, 734.0, 1.0, BORDER);
        let flash = (1.0 - ((time - app.game.flashes[lane]) / 0.20) as f32).clamp(0.0, 1.0);
        if app.game.pressed[lane] || flash > 0.0 {
            let mut color = lane_color(lane);
            color.a = 0.09 + flash * 0.12;
            ui.rect(x + 1.0, hit - 150.0, lane_w - 2.0, 150.0, color);
        }
        let color = lane_color(lane);
        ui.rect(
            x + 5.0,
            678.0,
            lane_w - 10.0,
            42.0,
            if app.game.pressed[lane] {
                color
            } else {
                Color::new(0.08, 0.10, 0.12, 1.0)
            },
        );
        ui.line(x + 5.0, 721.0, x + lane_w - 5.0, 721.0, 2.0, color);
        ui.center(
            label,
            x + lane_w / 2.0,
            704.0,
            if label.len() > 2 { 11.0 } else { 18.0 },
            if app.game.pressed[lane] { BG } else { INK },
        );
    }
    let preview = app.mode == Mode::Ready;
    let view_time = if preview { 2.0 } else { time };
    let pixels = 300.0 * app.speed as f64;
    // Notes are drawn directly at framebuffer resolution; no low-resolution render target.
    for (i, note) in app.game.chart.notes.iter().enumerate() {
        if !preview && app.game.states[i] == NoteState::Done {
            continue;
        }
        let head = hit - (note.sound.time - view_time) as f32 * pixels as f32;
        let tail = note
            .end
            .map(|end| hit - (end - view_time) as f32 * pixels as f32);
        if head < top - 12.0 || tail.unwrap_or(head) > hit + 20.0 {
            continue;
        }
        let x = stage_x + note.lane as f32 * lane_w + 6.0;
        let mut color = lane_color(note.lane);
        if preview {
            color.a = 0.42;
        }
        if let Some(tail) = tail {
            let y = tail.max(top);
            let end = head.min(hit);
            let mut body = color;
            body.a = if app.game.states[i] == NoteState::Holding {
                0.55
            } else {
                0.23
            };
            if end > y {
                ui.rect(x + 8.0, y, lane_w - 28.0, end - y, body);
                ui.line(
                    x + lane_w / 2.0 - 6.0,
                    y,
                    x + lane_w / 2.0 - 6.0,
                    end,
                    2.0,
                    color,
                );
            }
            if tail >= top && tail <= hit {
                ui.rect(x, tail, lane_w - 12.0, 5.0, color);
            }
        }
        if head >= top && head <= hit {
            ui.rect(x, head, lane_w - 12.0, 10.0, color);
            ui.rect(
                x + 1.0,
                head + 1.0,
                lane_w - 14.0,
                2.0,
                Color::new(1.0, 1.0, 1.0, color.a * 0.7),
            );
        }
    }
    ui.rect(stage_x, hit, stage_w, 2.0, LIME);
    ui.line(
        stage_x,
        hit + 5.0,
        stage_x + stage_w,
        hit + 5.0,
        1.0,
        Color::new(0.3, 0.37, 0.12, 1.0),
    );
    ui.rect(stage_x, 746.0, stage_w, 3.0, BORDER);
    ui.rect(
        stage_x,
        746.0,
        stage_w * (time / app.game.chart.duration).clamp(0.0, 1.0) as f32,
        3.0,
        LIME,
    );
    if app.mode == Mode::Ready {
        ui.rect(
            stage_x + 54.0,
            309.0,
            stage_w - 108.0,
            118.0,
            Color::new(0.035, 0.043, 0.055, 0.94),
        );
        ui.center(
            if app.loader.is_some() {
                "READING CHART"
            } else {
                "READY WHEN YOU ARE"
            },
            640.0,
            355.0,
            24.0,
            INK,
        );
        ui.center("Seven keys. One rhythm.", 640.0, 388.0, 16.0, MUTED);
    } else if app.mode == Mode::Playing && time < 0.0 {
        ui.center(
            &format!("{}", (-time).ceil() as i32),
            640.0,
            365.0,
            76.0,
            LIME,
        );
        ui.center("GET READY", 640.0, 403.0, 14.0, MUTED);
    } else if let Some((judgment, when, error)) = app.game.last
        && time - when < 0.7
    {
        ui.center(
            judgment.label(),
            640.0,
            400.0,
            30.0,
            match judgment {
                Judgment::Miss => Color::new(1.0, 0.4, 0.43, 1.0),
                Judgment::Perfect => LIME,
                _ => CYAN,
            },
        );
        if judgment != Judgment::Miss {
            ui.center(
                &format!("{:+.0} ms", error * 1000.0),
                640.0,
                425.0,
                13.0,
                MUTED,
            );
        }
    }

    ui.text("SESSION", 962.0, 128.0, 13.0, MUTED);
    ui.text("SCORE", 962.0, 178.0, 12.0, MUTED);
    ui.text(&format!("{:07}", app.game.score), 958.0, 225.0, 43.0, INK);
    ui.line(962.0, 252.0, 1240.0, 252.0, 1.0, BORDER);
    ui.text("ACCURACY", 962.0, 286.0, 12.0, MUTED);
    ui.text(
        &format!("{:.2}%", app.game.accuracy()),
        960.0,
        326.0,
        33.0,
        LIME,
    );
    ui.text("COMBO", 962.0, 373.0, 12.0, MUTED);
    ui.text(&format!("{:03}", app.game.combo), 960.0, 425.0, 49.0, INK);
    ui.text(
        &format!("BEST  {:03}", app.game.max_combo),
        1076.0,
        420.0,
        13.0,
        MUTED,
    );
    ui.line(962.0, 452.0, 1240.0, 452.0, 1.0, BORDER);
    for (i, label) in ["PERFECT", "COOL", "GOOD", "MISS"].iter().enumerate() {
        let y = 484.0 + i as f32 * 29.0;
        ui.text(label, 962.0, y, 13.0, if i == 0 { LIME } else { MUTED });
        ui.text(&format!("{:04}", app.game.counts[i]), 1195.0, y, 14.0, INK);
    }
    ui.text("GAUGE", 962.0, 621.0, 12.0, MUTED);
    ui.rect(962.0, 636.0, 278.0, 5.0, BORDER);
    ui.rect(962.0, 636.0, 278.0 * app.game.health, 5.0, LIME);
    ui.text("VOLUME", 962.0, 683.0, 12.0, MUTED);
    ui.text(
        &format!("{:.0}%", app.volume * 100.0),
        1203.0,
        683.0,
        13.0,
        INK,
    );
    ui.rect(962.0, 699.0, 278.0, 3.0, BORDER);
    ui.rect(962.0, 699.0, 278.0 * app.volume, 3.0, CYAN);
    if ui.hover(962.0, 689.0, 278.0, 24.0)
        && is_mouse_button_down(MouseButton::Left)
        && !app.modal()
    {
        let x = (mouse_position().0 - ui.origin.x) / ui.scale;
        app.volume = ((x - 962.0) / 278.0).clamp(0.0, 1.0);
        if let Some(a) = &app.audio {
            a.volume(app.volume);
        }
    }
    ui.text(
        &format!("{} / {}", clock(time), clock(app.game.chart.duration)),
        962.0,
        738.0,
        13.0,
        MUTED,
    );
    ui.text(
        "ESC  PAUSE     F4  RETRY     F11  FULLSCREEN",
        40.0,
        783.0,
        11.0,
        MUTED,
    );
    ui.text(
        "UP / DOWN  SPEED     TAB  AUTOPLAY",
        746.0,
        783.0,
        11.0,
        MUTED,
    );

    if app.mode == Mode::Paused || app.mode == Mode::Results {
        ui.rect(
            stage_x,
            137.0,
            stage_w,
            532.0,
            Color::new(0.025, 0.033, 0.043, 0.94),
        );
        ui.center(
            if app.mode == Mode::Paused {
                "PAUSED"
            } else {
                "SESSION COMPLETE"
            },
            640.0,
            287.0,
            32.0,
            INK,
        );
        if app.mode == Mode::Results {
            ui.center(
                &format!("{:.2}%", app.game.accuracy()),
                640.0,
                354.0,
                48.0,
                LIME,
            );
            ui.center(
                if app.autoplay {
                    "AUTOPLAY / UNRANKED"
                } else {
                    "FREE PLAY / NO FAIL"
                },
                640.0,
                387.0,
                13.0,
                MUTED,
            );
        }
        if ui.button(
            if app.mode == Mode::Paused {
                "RESUME"
            } else {
                "PLAY AGAIN"
            },
            505.0,
            443.0,
            270.0,
            44.0,
            true,
        ) && !app.modal()
        {
            if app.mode == Mode::Paused {
                app.pause();
            } else {
                app.start();
            }
        }
        if ui.button("BACK TO CHART", 505.0, 501.0, 270.0, 40.0, false) && !app.modal() {
            app.ready();
        }
    }
    if let Some(warning) = app.warnings.first() {
        ui.rect(374.0, 87.0, 532.0, 15.0, BG);
        ui.fit(
            warning,
            378.0,
            99.0,
            10.0,
            524.0,
            Color::new(0.96, 0.69, 0.38, 1.0),
        );
    }
    if let Some(error) = &app.error {
        ui.rect(334.0, 244.0, 612.0, 230.0, BG);
        ui.rect(334.0, 244.0, 3.0, 230.0, Color::new(1.0, 0.4, 0.43, 1.0));
        ui.text("COULD NOT OPEN CHART", 360.0, 282.0, 22.0, INK);
        ui.wrap(error, 360.0, 319.0, 15.0, 554.0, MUTED);
        if ui.button("DISMISS", 360.0, 415.0, 554.0, 40.0, false) {
            app.error = None;
        }
    }
    if let Some(entry) = &app.path_entry {
        ui.rect(0.0, 0.0, 1280.0, 800.0, Color::new(0.0, 0.0, 0.0, 0.65));
        ui.rect(300.0, 250.0, 680.0, 280.0, PANEL);
        ui.text("OPEN AN OJN CHART", 330.0, 294.0, 25.0, INK);
        ui.text(
            "Type or paste its full path. Keysounds load from the same folder.",
            330.0,
            329.0,
            14.0,
            MUTED,
        );
        ui.rect(330.0, 353.0, 620.0, 53.0, BG);
        let mut visible = entry.clone();
        while measure_text(
            &visible,
            Some(font),
            (16.0 * ui.raster).round().max(1.0) as u16,
            1.0,
        )
        .width
            / ui.raster.max(0.1)
            > 590.0
        {
            if let Some((i, _)) = visible.char_indices().nth(1) {
                visible = visible[i..].into();
            } else {
                break;
            }
        }
        ui.text(&format!("{visible}_"), 343.0, 386.0, 16.0, INK);
        ui.text(
            "ENTER  OPEN       ESC  CANCEL       CTRL+V  PASTE",
            330.0,
            449.0,
            13.0,
            LIME,
        );
        ui.text(
            "You can also drag the OJN and OJM files into the window.",
            330.0,
            491.0,
            14.0,
            MUTED,
        );
    }
    menus::draw(app, &ui, &meta_ui);
}

fn main() {
    let o = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\nUse --help for usage.");
            std::process::exit(2);
        }
    };
    if o.list_music || o.validate_music {
        let library = match library::scan(&o.music) {
            Ok(library) => library,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        };
        let mut failures = library.warnings.len();
        for warning in library.warnings {
            eprintln!("{warning}");
        }
        for entry in &library.entries {
            if o.list_music {
                println!(
                    "{} | {} | {:.0} BPM | {:?} | {}",
                    entry.header.title,
                    entry.header.artist,
                    entry.header.bpm,
                    entry.header.levels,
                    entry.path.display()
                );
            }
            if o.validate_music {
                let result = std::fs::read(&entry.path)
                    .map_err(|e| e.to_string())
                    .and_then(|data| hyperjam::chart::parse_ojn(&data));
                if let Err(e) = result {
                    failures += 1;
                    eprintln!("{}: {e}", entry.path.display());
                }
            }
        }
        println!(
            "{} charts indexed; {failures} errors",
            library.entries.len()
        );
        if failures > 0 {
            std::process::exit(1);
        }
        return;
    }
    if o.inspect {
        let result = if let Some(path) = &o.chart {
            load_song(path, o.samples.as_deref(), 44100)
        } else {
            Ok(LoadedSong {
                song: practice_song(),
                bank: practice_bank(44100),
                warnings: vec![],
            })
        };
        match result {
            Ok(loaded) => {
                println!(
                    "{} / {} / {:.2} BPM",
                    loaded.song.title, loaded.song.artist, loaded.song.bpm
                );
                for (name, c) in ["Easy", "Normal", "Hard"].iter().zip(&loaded.song.charts) {
                    println!(
                        "{name}: level {}, {} notes, {} backing events, {:.2}s",
                        c.level,
                        c.notes.len(),
                        c.background.len(),
                        c.duration
                    );
                }
                println!("{} decoded samples", loaded.bank.len());
                for warning in loaded.warnings {
                    println!("Warning: {warning}");
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    macroquad::Window::from_config(window_conf(), run(o));
}

async fn run(o: Options) {
    // X11 reports scaled framebuffer dimensions but creates its initial window in pixels.
    // Request logical dimensions once so a 2x desktop gets a 2560x1600 framebuffer.
    if screen_dpi_scale() > 1.0 && screen_width() < 1000.0 {
        request_new_screen_size(1280.0, 800.0);
        next_frame().await;
    }
    let font = load_ttf_font_from_bytes(include_bytes!("../assets/NotoSans-Regular.ttf"))
        .expect("Bundled font is valid");
    let metadata_path = o.font.clone().or_else(|| {
        [
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "C:/Windows/Fonts/malgun.ttf",
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
    });
    let metadata_font = if let Some(path) = &metadata_path {
        match load_ttf_font(&path.to_string_lossy()).await {
            Ok(font) => font,
            Err(error) => {
                eprintln!("Could not load font {}: {error}", path.display());
                font.clone()
            }
        }
    } else {
        font.clone()
    };
    let mut app = App::new(&o);
    if let Some(path) = &o.chart {
        app.load(path.clone());
    }
    let mut frame = 0u64;
    let wall_start = get_time();
    let mut captured = false;
    let mut start_pending = o.start;
    loop {
        app.input();
        if start_pending && app.loader.is_none() {
            start_pending = false;
            if app.error.is_none() {
                app.start();
            }
        }
        draw(&mut app, &font, &metadata_font);
        if !captured
            && (if let Some(seconds) = o.screenshot_seconds {
                get_time() - wall_start >= seconds
            } else {
                frame == o.screenshot_frame
            })
            && let Some(path) = &o.screenshot
        {
            get_screen_data().export_png(&path.to_string_lossy());
            captured = true;
        }
        frame += 1;
        if o.exit_after.is_some_and(|end| frame >= end)
            || o.exit_seconds
                .is_some_and(|end| get_time() - wall_start >= end)
        {
            println!(
                "Session: {:.2}s chart time, {} judgments, {:.2}% accuracy, {} misses",
                app.time(),
                app.game.counts.iter().sum::<usize>(),
                app.game.accuracy(),
                app.game.counts[3]
            );
            break;
        }
        next_frame().await;
    }
}
