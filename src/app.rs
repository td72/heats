use iced::widget::{column, container};
use iced::window;
use iced::{event, keyboard, Color, Element, Fill, Padding, Point, Size, Subscription, Task, Theme};

use crate::config::{Config, WindowMode};
use crate::hotkey::{self, HotkeyMessage};
use crate::matcher::engine::Matcher;
use crate::source::applications::ApplicationsSource;
use crate::source::{SourceItem, SourceRegistry};
use crate::ui::{result_list, search_input, theme};

pub struct State {
    config: Config,
    sources: SourceRegistry,
    matcher: Matcher,
    all_items: Vec<SourceItem>,
    results: Vec<SourceItem>,
    query: String,
    selected: usize,

    /// Current window ID
    window_id: Option<window::Id>,
    /// Whether the launcher is currently shown
    visible: bool,
    /// Fixed display bounds (only used in Fixed mode)
    fixed_display: (f64, f64, f64, f64),

    _hotkey_manager: global_hotkey::GlobalHotKeyManager,
    hotkey_id: u32,
}

#[derive(Debug, Clone)]
pub enum Message {
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    QueryChanged(String),
    Execute,
    SelectAndExecute(usize),
    ItemsLoaded(Vec<SourceItem>),
    MatcherTick,
    KeyEvent(keyboard::Event),
    Hotkey(HotkeyMessage),
}

impl State {
    pub fn new(
        config: Config,
        manager: global_hotkey::GlobalHotKeyManager,
        hotkey_id: u32,
    ) -> (Self, Task<Message>) {
        let mut sources = SourceRegistry::new();
        sources.register(Box::new(ApplicationsSource::new()));

        let fixed_display = if config.window.display.is_empty() {
            crate::platform::macos::focused_display_bounds()
        } else {
            crate::platform::macos::display_bounds_by_name(&config.window.display)
        };

        tracing::info!(
            "Window mode: {:?}, display bounds: {:?}",
            config.window.mode,
            fixed_display
        );

        // Fixed mode: create window at boot (hidden via native orderOut)
        // Normal mode: no window at boot (created on demand)
        let (window_id, boot_task) = if config.window.mode == WindowMode::Fixed {
            let pos = Self::center_on_display(
                &fixed_display,
                config.window.width,
                config.window.height,
            );
            let (id, open_task) = window::open(window::Settings {
                size: Size::new(config.window.width, config.window.height),
                position: window::Position::Specific(pos),
                visible: false,
                decorations: false,
                transparent: true,
                level: window::Level::AlwaysOnTop,
                resizable: false,
                exit_on_close_request: false,
                ..window::Settings::default()
            });
            (Some(id), open_task.discard())
        } else {
            (None, Task::none())
        };

        let state = Self {
            config,
            sources,
            matcher: Matcher::new(),
            all_items: Vec::new(),
            results: Vec::new(),
            query: String::new(),
            selected: 0,
            window_id,
            visible: false,
            fixed_display,
            _hotkey_manager: manager,
            hotkey_id,
        };

        let load_task = Task::perform(async { load_items().await }, Message::ItemsLoaded);
        (state, Task::batch([boot_task, load_task]))
    }

    pub fn title(&self, _window: window::Id) -> String {
        String::from("Heats")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.window_id = Some(id);
                // Normal mode: window just opened, focus the input
                Task::batch([
                    window::gain_focus(id),
                    iced::widget::operation::focus(search_input::SEARCH_INPUT_ID),
                ])
            }
            Message::WindowClosed(id) => {
                if self.window_id == Some(id) {
                    self.window_id = None;
                }
                self.visible = false;
                self.reset_state();
                Task::none()
            }
            Message::QueryChanged(query) => {
                self.query = query.clone();
                self.selected = 0;
                self.matcher.update_query(&query);
                Task::none()
            }
            Message::Execute => {
                if let Some(item) = self.results.get(self.selected) {
                    if let Err(e) = self.sources.execute(item) {
                        tracing::error!("Failed to execute: {}", e);
                    }
                }
                self.hide()
            }
            Message::SelectAndExecute(index) => {
                self.selected = index;
                if let Some(item) = self.results.get(self.selected) {
                    if let Err(e) = self.sources.execute(item) {
                        tracing::error!("Failed to execute: {}", e);
                    }
                }
                self.hide()
            }
            Message::ItemsLoaded(items) => {
                self.all_items = items;
                self.matcher.set_items(self.all_items.clone());
                Task::none()
            }
            Message::MatcherTick => {
                let changed = self.matcher.tick();
                if changed {
                    self.results = if self.matcher.query_is_empty() {
                        self.all_items.clone()
                    } else {
                        self.matcher.results(50)
                    };
                }
                Task::none()
            }
            Message::KeyEvent(kb_event) => match kb_event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                } => self.hide(),
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowUp),
                    ..
                } => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                    Task::none()
                }
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowDown),
                    ..
                } => {
                    if self.selected + 1 < self.results.len() {
                        self.selected += 1;
                    }
                    Task::none()
                }
                _ => Task::none(),
            },
            Message::Hotkey(HotkeyMessage::TogglePressed) => {
                if self.visible {
                    self.hide()
                } else {
                    self.show()
                }
            }
        }
    }

    pub fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let input = search_input::view(&self.query);
        let results = result_list::view(&self.results, self.selected);

        let content = column![input, results].spacing(8).padding(Padding::new(12.0));

        let main = container(content)
            .width(Fill)
            .height(Fill)
            .style(theme::main_container);

        container(main).width(Fill).height(Fill).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            hotkey::subscription(self.hotkey_id).map(Message::Hotkey),
        ];

        // Normal mode needs close events to track window lifecycle
        if self.config.window.mode == WindowMode::Normal {
            subs.push(window::close_events().map(Message::WindowClosed));
        }

        if self.visible {
            subs.push(
                event::listen_with(|event, status, _window| match event {
                    iced::Event::Keyboard(kb_event) => {
                        if matches!(status, event::Status::Ignored) {
                            Some(Message::KeyEvent(kb_event))
                        } else {
                            match &kb_event {
                                keyboard::Event::KeyPressed {
                                    key:
                                        keyboard::Key::Named(keyboard::key::Named::Escape),
                                    ..
                                } => Some(Message::KeyEvent(kb_event)),
                                _ => None,
                            }
                        }
                    }
                    _ => None,
                }),
            );

            subs.push(
                iced::time::every(std::time::Duration::from_millis(16))
                    .map(|_| Message::MatcherTick),
            );
        }

        Subscription::batch(subs)
    }

    pub fn theme(&self, _window: window::Id) -> Theme {
        Theme::Dark
    }

    pub fn style(&self, theme: &Theme) -> iced::theme::Style {
        let _ = theme;
        iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        }
    }

    // ---- Show / Hide ----

    fn show(&mut self) -> Task<Message> {
        self.visible = true;
        let load_task = Task::perform(async { load_items().await }, Message::ItemsLoaded);

        match self.config.window.mode {
            WindowMode::Fixed => self.show_fixed(load_task),
            WindowMode::Normal => self.show_normal(load_task),
        }
    }

    fn hide(&mut self) -> Task<Message> {
        self.visible = false;
        self.reset_state();

        match self.config.window.mode {
            WindowMode::Fixed => self.hide_fixed(),
            WindowMode::Normal => self.hide_normal(),
        }
    }

    // -- Normal mode: open/close window each time --

    fn show_normal(&mut self, load_task: Task<Message>) -> Task<Message> {
        let display = crate::platform::macos::focused_display_bounds();
        let pos = Self::center_on_display(
            &display,
            self.config.window.width,
            self.config.window.height,
        );

        let (_id, open_task) = window::open(window::Settings {
            size: Size::new(self.config.window.width, self.config.window.height),
            position: window::Position::Specific(pos),
            decorations: false,
            transparent: true,
            level: window::Level::AlwaysOnTop,
            resizable: false,
            exit_on_close_request: false,
            ..window::Settings::default()
        });

        Task::batch([open_task.map(Message::WindowOpened), load_task])
    }

    fn hide_normal(&mut self) -> Task<Message> {
        if let Some(id) = self.window_id.take() {
            window::close(id)
        } else {
            Task::none()
        }
    }

    // -- Fixed mode: native NSWindow show/hide (Raycast-style) --

    fn show_fixed(&mut self, load_task: Task<Message>) -> Task<Message> {
        let id = match self.window_id {
            Some(id) => id,
            None => return Task::none(),
        };

        // Use native NSWindow API to position and show the window.
        // This bypasses winit's coordinate handling and avoids AeroSpace interference.
        crate::platform::macos::native_show_window(
            &self.fixed_display,
            self.config.window.width as f64,
            self.config.window.height as f64,
        );

        // Still use iced's focus APIs for input handling
        let focus = window::gain_focus::<Message>(id)
            .chain(iced::widget::operation::focus(search_input::SEARCH_INPUT_ID));

        Task::batch([focus, load_task])
    }

    fn hide_fixed(&self) -> Task<Message> {
        // Use native NSWindow.orderOut to truly hide the window.
        // Unlike moving off-screen, this is invisible to window managers.
        crate::platform::macos::native_hide_window();
        Task::none()
    }

    // ---- Helpers ----

    fn center_on_display(
        display: &(f64, f64, f64, f64),
        win_w: f32,
        win_h: f32,
    ) -> Point {
        let (disp_x, disp_y, disp_w, disp_h) = *display;
        let x = disp_x + (disp_w - win_w as f64) / 2.0;
        let y = disp_y + (disp_h - win_h as f64) / 3.0;
        Point::new(x as f32, y as f32)
    }

    fn reset_state(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.results.clear();
        self.matcher.update_query("");
    }
}

async fn load_items() -> Vec<SourceItem> {
    let source = ApplicationsSource::new();
    use crate::source::Source;
    source.load().await
}
