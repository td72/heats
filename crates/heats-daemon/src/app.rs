use std::sync::{Arc, Mutex};

use iced::widget::{column, container};
use iced::window;
use iced::{event, keyboard, Color, Element, Fill, Padding, Point, Size, Subscription, Task, Theme};
use tokio::sync::oneshot;

use crate::command::{self, LoadedItem};
use crate::hotkey::{self, HotkeyMessage};
use crate::ipc_server;
use crate::matcher::engine::Matcher;
use crate::ui::{result_list, search_input, theme};
use heats_core::config::{Config, WindowMode};
use heats_core::source::SourceItem;

pub struct State {
    config: Config,
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
    hotkey_modes: Vec<(u32, String)>,

    /// Loaded items with action resolution metadata
    loaded_items: Vec<LoadedItem>,

    /// Active dmenu session response channel
    dmenu_tx: Option<oneshot::Sender<Option<String>>>,
    /// Whether current session is dmenu (external items) vs built-in
    is_dmenu_session: bool,
}

/// Wrapper to make oneshot::Sender cloneable for Message (taken once via take()).
#[derive(Clone)]
pub struct ResponseSender(pub Arc<Mutex<Option<oneshot::Sender<Option<String>>>>>);

impl std::fmt::Debug for ResponseSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResponseSender")
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    QueryChanged(String),
    Execute,
    SelectAndExecute(usize),
    ItemsLoaded(Vec<LoadedItem>),
    MatcherTick,
    KeyEvent(keyboard::Event),
    Hotkey(HotkeyMessage),
    ActivateWindow,
    DmenuSession {
        items: Vec<SourceItem>,
        response_tx: ResponseSender,
    },
}

impl State {
    pub fn new(
        config: Config,
        manager: global_hotkey::GlobalHotKeyManager,
        hotkey_modes: Vec<(u32, String)>,
    ) -> (Self, Task<Message>) {
        let fixed_display = if config.window.display.is_empty() {
            heats_core::platform::macos::focused_display_bounds()
        } else {
            heats_core::platform::macos::display_bounds_by_name(&config.window.display)
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
            matcher: Matcher::new(),
            all_items: Vec::new(),
            results: Vec::new(),
            query: String::new(),
            selected: 0,
            window_id,
            visible: false,
            fixed_display,
            _hotkey_manager: manager,
            hotkey_modes,
            loaded_items: Vec::new(),
            dmenu_tx: None,
            is_dmenu_session: false,
        };

        (state, boot_task)
    }

    pub fn title(&self, _window: window::Id) -> String {
        String::from("Heats")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                tracing::debug!("WindowOpened: id={:?}, is_dmenu={}", id, self.is_dmenu_session);
                self.window_id = Some(id);
                // Delay native focus to next run loop iteration so macOS has
                // time to fully realize the window before we activate it
                let activate = Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(50)).await },
                    |_| Message::ActivateWindow,
                );
                Task::batch([
                    window::gain_focus(id),
                    iced::widget::operation::focus(search_input::SEARCH_INPUT_ID),
                    activate,
                ])
            }
            Message::WindowClosed(id) => {
                tracing::debug!(
                    "WindowClosed: id={:?}, current_window_id={:?}, is_dmenu={}",
                    id,
                    self.window_id,
                    self.is_dmenu_session
                );
                if self.window_id == Some(id) {
                    self.window_id = None;
                    self.visible = false;
                    self.cancel_dmenu_session();
                    self.reset_state();
                }
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
                    if self.is_dmenu_session {
                        // Dmenu mode: send selected title back to client
                        self.send_dmenu_response(Some(item.title.clone()));
                    } else {
                        self.execute_selected(self.selected);
                    }
                }
                self.hide()
            }
            Message::SelectAndExecute(index) => {
                self.selected = index;
                if let Some(item) = self.results.get(self.selected) {
                    if self.is_dmenu_session {
                        self.send_dmenu_response(Some(item.title.clone()));
                    } else {
                        self.execute_selected(self.selected);
                    }
                }
                self.hide()
            }
            Message::ItemsLoaded(loaded_items) => {
                // Ignore items while a dmenu session is active
                if self.is_dmenu_session {
                    tracing::debug!("ItemsLoaded ignored (dmenu session active)");
                    return Task::none();
                }
                self.all_items = loaded_items.iter().map(|li| li.item.clone()).collect();
                self.loaded_items = loaded_items;
                self.matcher.set_items(self.all_items.clone());
                self.results = self.all_items.clone();
                // No focus call here — WindowOpened already handled focus
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
            Message::ActivateWindow => {
                heats_core::platform::macos::native_focus_heats_window();
                if let Some(id) = self.window_id {
                    Task::batch([
                        window::gain_focus(id),
                        iced::widget::operation::focus(search_input::SEARCH_INPUT_ID),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::Hotkey(hotkey_msg) => {
                let mode_name = hotkey_msg.mode_name;
                if self.visible {
                    self.hide()
                } else {
                    // If a dmenu session is active, cancel it before showing
                    if self.is_dmenu_session {
                        self.cancel_dmenu_session();
                        self.reset_state();
                    }
                    self.show_mode(&mode_name)
                }
            }
            Message::DmenuSession { items, response_tx } => {
                tracing::debug!(
                    "DmenuSession: {} items, visible={}, window_id={:?}",
                    items.len(),
                    self.visible,
                    self.window_id
                );

                // If already visible, hide first then show dmenu
                if self.visible {
                    let hide_task = self.hide();
                    // hide() already cancelled any active dmenu + reset state

                    let tx = response_tx.0.lock().unwrap().take();
                    self.start_dmenu_session(items, tx);
                    let show_task = self.show_dmenu();
                    Task::batch([hide_task, show_task])
                } else {
                    let tx = response_tx.0.lock().unwrap().take();
                    self.start_dmenu_session(items, tx);
                    self.show_dmenu()
                }
            }
        }
    }

    pub fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let input = search_input::view(&self.query);
        let results = result_list::view(&self.results, self.selected, self.config.window.height);

        let content = column![input, results]
            .spacing(8)
            .padding(Padding::new(12.0))
            .height(Fill);

        let main = container(content)
            .width(Fill)
            .height(Fill)
            .style(theme::main_container);

        container(main).width(Fill).height(Fill).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            hotkey::subscription(self.hotkey_modes.clone()).map(Message::Hotkey),
            ipc_server::dmenu_subscription(),
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

    // ---- Action execution ----

    fn execute_selected(&self, selected_index: usize) {
        let selected_item = match self.results.get(selected_index) {
            Some(item) => item,
            None => return,
        };

        // Find the LoadedItem matching this result
        let loaded = self.loaded_items.iter().find(|li| {
            li.item.title == selected_item.title
                && li.item.source_name == selected_item.source_name
                && li.item.exec_path == selected_item.exec_path
        });

        if let Some(loaded) = loaded {
            if let Some(provider) = self.config.provider.get(&loaded.provider_name) {
                command::execute_action(provider, &loaded.dmenu_item);
            } else {
                tracing::warn!("No provider configured for '{}'", loaded.provider_name);
            }
        } else {
            tracing::warn!("Could not find LoadedItem for selected result");
        }
    }

    // ---- Dmenu ----

    fn start_dmenu_session(
        &mut self,
        items: Vec<SourceItem>,
        tx: Option<oneshot::Sender<Option<String>>>,
    ) {
        self.dmenu_tx = tx;
        self.is_dmenu_session = true;

        self.all_items = items;
        self.results = self.all_items.clone();
        self.matcher.set_items(self.all_items.clone());
    }

    fn show_dmenu(&mut self) -> Task<Message> {
        self.visible = true;
        tracing::debug!(
            "show_dmenu: results={}, all_items={}",
            self.results.len(),
            self.all_items.len()
        );

        match self.config.window.mode {
            WindowMode::Fixed => self.show_fixed(Task::none()),
            WindowMode::Normal => self.show_normal(Task::none()),
        }
    }

    fn send_dmenu_response(&mut self, response: Option<String>) {
        if let Some(tx) = self.dmenu_tx.take() {
            let _ = tx.send(response);
        }
        self.is_dmenu_session = false;
    }

    fn cancel_dmenu_session(&mut self) {
        if self.is_dmenu_session {
            // Send None (cancelled) to the client
            self.send_dmenu_response(None);
        }
    }

    // ---- Show / Hide ----

    fn show_mode(&mut self, mode_name: &str) -> Task<Message> {
        // Look up mode config to get provider list
        let provider_names: Vec<String> = self
            .config
            .mode
            .iter()
            .find(|m| m.name == mode_name)
            .map(|m| m.providers.clone())
            .unwrap_or_default();

        if provider_names.is_empty() {
            tracing::warn!("No providers configured for mode '{}'", mode_name);
            return Task::none();
        }

        // Open window immediately (so macOS grants focus while user interaction is fresh),
        // then load items in parallel
        self.visible = true;
        let providers = self.config.provider.clone();
        let load_task = Task::perform(
            async move { command::load_from_providers(&provider_names, &providers).await },
            Message::ItemsLoaded,
        );

        match self.config.window.mode {
            WindowMode::Fixed => self.show_fixed(load_task),
            WindowMode::Normal => self.show_normal(load_task),
        }
    }

    fn hide(&mut self) -> Task<Message> {
        self.visible = false;
        // If this is a dmenu session, cancel it (send None to client)
        self.cancel_dmenu_session();
        self.reset_state();

        match self.config.window.mode {
            WindowMode::Fixed => self.hide_fixed(),
            WindowMode::Normal => self.hide_normal(),
        }
    }

    // -- Normal mode: open/close window each time --

    fn show_normal(&mut self, load_task: Task<Message>) -> Task<Message> {
        let disp_bounds = heats_core::platform::macos::focused_display_bounds();
        let pos = Self::center_on_display(
            &disp_bounds,
            self.config.window.width,
            self.config.window.height,
        );
        tracing::debug!("show_normal: disp_bounds={:?}, pos={:?}", disp_bounds, pos);

        let (_id, open_task) = window::open(window::Settings {
            size: Size::new(self.config.window.width, self.config.window.height),
            position: window::Position::Specific(pos),
            visible: true,
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
        heats_core::platform::macos::native_show_window(
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
        heats_core::platform::macos::native_hide_window();
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
        self.loaded_items.clear();
        self.matcher.update_query("");
    }
}
