use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced::widget::{column, container};
use iced::window;
use iced::{event, keyboard, Color, Element, Fill, Padding, Point, Size, Subscription, Task, Theme};
use tokio::sync::oneshot;

use crate::command::{self, LoadedItem};
use crate::evaluator;
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

    /// Active dmenu session response channel (returns selected item's ID)
    dmenu_tx: Option<oneshot::Sender<Option<usize>>>,
    /// Whether current session is dmenu (external items) vs built-in
    is_dmenu_session: bool,

    /// Background cache: provider name → cached items
    provider_cache: HashMap<String, Vec<LoadedItem>>,
    /// Last update time per cached provider
    cache_last_updated: HashMap<String, Instant>,

    /// Evaluator results (displayed at the top of the list)
    eval_items: Vec<LoadedItem>,
    /// Debounce generation counter for evaluator queries
    eval_generation: u64,
    /// Active evaluator names for the current mode
    active_evaluators: Vec<String>,
}

/// Wrapper to make oneshot::Sender cloneable for Message (taken once via take()).
#[derive(Clone)]
pub struct ResponseSender(pub Arc<Mutex<Option<oneshot::Sender<Option<usize>>>>>);

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
    /// Timer tick for background cache refresh
    CacheRefresh,
    /// Background cache updated for a provider
    CacheUpdated {
        provider_name: String,
        items: Vec<LoadedItem>,
    },
    /// Evaluator results (debounced)
    EvalResults {
        generation: u64,
        items: Vec<LoadedItem>,
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
            provider_cache: HashMap::new(),
            cache_last_updated: HashMap::new(),
            eval_items: Vec::new(),
            eval_generation: 0,
            active_evaluators: Vec::new(),
        };

        // Kick initial cache load for providers with cache_interval
        let initial_cache_task = state.initial_cache_load();

        (state, Task::batch([boot_task, initial_cache_task]))
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

                // Trigger evaluators with debounce
                tracing::debug!(
                    "QueryChanged: active_evaluators={:?}, query='{}'",
                    self.active_evaluators, query
                );
                if !self.active_evaluators.is_empty() && !query.is_empty() {
                    self.eval_generation += 1;
                    let gen = self.eval_generation;
                    let evaluator_names = self.active_evaluators.clone();
                    let configs = self.config.evaluator.clone();
                    Task::perform(
                        async move {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            let items = evaluator::run_evaluators(&query, &evaluator_names, &configs).await;
                            (gen, items)
                        },
                        |(generation, items)| Message::EvalResults { generation, items },
                    )
                } else {
                    self.eval_items.clear();
                    Task::none()
                }
            }
            Message::Execute => {
                let eval_count = self.eval_items.len();
                if self.selected < eval_count {
                    // Selected an evaluator result
                    let eval_action = self.pending_eval_action(self.selected);
                    let hide_task = self.hide();
                    if let Some((config, dmenu_item)) = eval_action {
                        command::run_action(&config, &dmenu_item);
                    }
                    return hide_task;
                }

                let adjusted = self.selected - eval_count;
                if let Some(item) = self.results.get(adjusted) {
                    if self.is_dmenu_session {
                        self.send_dmenu_response(item.id);
                    }
                }
                // Capture action info before hide() clears state
                let action = self.pending_action(adjusted);
                // Hide first so macOS deactivates Heats before the action
                // activates the target app — avoids focus bounce-back
                let hide_task = self.hide();
                if let Some((provider, dmenu_item)) = action {
                    command::execute_action(&provider, &dmenu_item);
                }
                hide_task
            }
            Message::SelectAndExecute(index) => {
                self.selected = index;
                let eval_count = self.eval_items.len();
                if index < eval_count {
                    let eval_action = self.pending_eval_action(index);
                    let hide_task = self.hide();
                    if let Some((config, dmenu_item)) = eval_action {
                        command::run_action(&config, &dmenu_item);
                    }
                    return hide_task;
                }

                let adjusted = index - eval_count;
                if let Some(item) = self.results.get(adjusted) {
                    if self.is_dmenu_session {
                        self.send_dmenu_response(item.id);
                    }
                }
                let action = self.pending_action(adjusted);
                let hide_task = self.hide();
                if let Some((provider, dmenu_item)) = action {
                    command::execute_action(&provider, &dmenu_item);
                }
                hide_task
            }
            Message::ItemsLoaded(loaded_items) => {
                // Ignore items while a dmenu session is active
                if self.is_dmenu_session {
                    tracing::debug!("ItemsLoaded ignored (dmenu session active)");
                    return Task::none();
                }
                // Merge with existing items (cache may have pre-populated some)
                if self.loaded_items.is_empty() {
                    self.loaded_items = loaded_items;
                } else {
                    self.loaded_items.extend(loaded_items);
                }
                self.all_items = self.loaded_items.iter().map(|li| li.item.clone()).collect();
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
                    let total = self.eval_items.len() + self.results.len();
                    if self.selected + 1 < total {
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
            Message::EvalResults { generation, items } => {
                tracing::debug!(
                    "EvalResults: gen={}, current_gen={}, items={}",
                    generation, self.eval_generation, items.len()
                );
                if generation == self.eval_generation {
                    self.eval_items = items;
                }
                Task::none()
            }
            Message::CacheRefresh => {
                self.refresh_stale_caches()
            }
            Message::CacheUpdated { provider_name, items } => {
                tracing::debug!(
                    "CacheUpdated: provider='{}', {} items",
                    provider_name,
                    items.len()
                );
                self.provider_cache.insert(provider_name.clone(), items);
                self.cache_last_updated.insert(provider_name, Instant::now());
                Task::none()
            }
        }
    }

    pub fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let input = search_input::view(&self.query);

        // Merge evaluator results (at top) with provider results
        let display_items: Vec<&SourceItem> = self
            .eval_items
            .iter()
            .map(|li| &li.item)
            .chain(self.results.iter())
            .collect();
        let results = result_list::view(&display_items, self.selected, self.config.window.height);

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

        // Background cache refresh timer (runs regardless of visibility)
        if let Some(min_interval) = self.min_cache_interval() {
            subs.push(
                iced::time::every(min_interval).map(|_| Message::CacheRefresh),
            );
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

    /// Extract action info (provider config + dmenu item) for the selected index,
    /// returning owned copies so they survive hide()/reset_state().
    fn pending_action(
        &self,
        selected_index: usize,
    ) -> Option<(heats_core::config::ProviderConfig, heats_core::source::DmenuItem)> {
        if self.is_dmenu_session {
            return None;
        }
        let selected_item = self.results.get(selected_index)?;
        let loaded = self.loaded_items.iter().find(|li| {
            li.item.title == selected_item.title
                && li.item.source_name == selected_item.source_name
                && li.item.exec_path == selected_item.exec_path
        })?;
        let provider = self.config.provider.get(&loaded.provider_name)?;
        Some((provider.clone(), loaded.dmenu_item.clone()))
    }

    /// Extract evaluator action info for the selected eval index.
    fn pending_eval_action(
        &self,
        eval_index: usize,
    ) -> Option<(heats_core::config::EvaluatorConfig, heats_core::source::DmenuItem)> {
        let loaded = self.eval_items.get(eval_index)?;
        let config = self.config.evaluator.get(&loaded.provider_name)?;
        Some((config.clone(), loaded.dmenu_item.clone()))
    }

    // ---- Dmenu ----

    fn start_dmenu_session(
        &mut self,
        items: Vec<SourceItem>,
        tx: Option<oneshot::Sender<Option<usize>>>,
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

    fn send_dmenu_response(&mut self, response: Option<usize>) {
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
        // Look up mode config to get provider list and evaluator list
        let mode = self.config.mode.iter().find(|m| m.name == mode_name);
        let provider_names: Vec<String> = mode
            .map(|m| m.providers.clone())
            .unwrap_or_default();
        self.active_evaluators = mode
            .map(|m| m.evaluators.clone())
            .unwrap_or_default();

        if provider_names.is_empty() {
            tracing::warn!("No providers configured for mode '{}'", mode_name);
            return Task::none();
        }

        self.visible = true;

        // Split providers into cached (instant) and uncached (need async load)
        let mut cached_items: Vec<LoadedItem> = Vec::new();
        let mut uncached_names: Vec<String> = Vec::new();

        for name in &provider_names {
            if let Some(items) = self.provider_cache.get(name) {
                cached_items.extend(items.clone());
            } else {
                uncached_names.push(name.clone());
            }
        }

        // Pre-populate with cached items immediately
        if !cached_items.is_empty() {
            self.loaded_items = cached_items;
            self.all_items = self.loaded_items.iter().map(|li| li.item.clone()).collect();
            self.matcher.set_items(self.all_items.clone());
            self.results = self.all_items.clone();
        }

        // Load uncached providers asynchronously (if any)
        let load_task = if uncached_names.is_empty() {
            Task::none()
        } else {
            let providers = self.config.provider.clone();
            Task::perform(
                async move { command::load_from_providers(&uncached_names, &providers).await },
                Message::ItemsLoaded,
            )
        };

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
        self.eval_items.clear();
        self.eval_generation = 0;
        self.active_evaluators.clear();
        // provider_cache is intentionally NOT cleared — persists across show/hide
    }

    // ---- Background cache ----

    /// Returns the minimum cache_interval across all providers (for the subscription timer).
    fn min_cache_interval(&self) -> Option<Duration> {
        self.config
            .provider
            .values()
            .filter_map(|p| p.cache_interval)
            .min()
            .map(Duration::from_secs)
    }

    /// Kick initial cache load for all providers with cache_interval set.
    fn initial_cache_load(&self) -> Task<Message> {
        let tasks: Vec<Task<Message>> = self
            .config
            .provider
            .iter()
            .filter(|(_, p)| p.cache_interval.is_some())
            .map(|(name, p)| {
                let name = name.clone();
                let name_for_msg = name.clone();
                let providers = HashMap::from([(name.clone(), p.clone())]);
                Task::perform(
                    async move {
                        command::load_from_providers(&[name], &providers).await
                    },
                    move |items| Message::CacheUpdated {
                        provider_name: name_for_msg,
                        items,
                    },
                )
            })
            .collect();

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    /// Check each cached provider and refresh if stale.
    fn refresh_stale_caches(&self) -> Task<Message> {
        let now = Instant::now();
        let tasks: Vec<Task<Message>> = self
            .config
            .provider
            .iter()
            .filter_map(|(name, p)| {
                let interval = Duration::from_secs(p.cache_interval?);
                let is_stale = self
                    .cache_last_updated
                    .get(name)
                    .map(|t| now.duration_since(*t) >= interval)
                    .unwrap_or(true);
                if is_stale {
                    Some((name.clone(), p.clone()))
                } else {
                    None
                }
            })
            .map(|(name, p)| {
                let name_for_msg = name.clone();
                let providers = HashMap::from([(name.clone(), p)]);
                Task::perform(
                    async move {
                        command::load_from_providers(&[name], &providers).await
                    },
                    move |items| Message::CacheUpdated {
                        provider_name: name_for_msg,
                        items,
                    },
                )
            })
            .collect();

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }
}
