use ratatui::widgets::ListState;
use crate::core::versions::VersionManifest;
use crate::api::modrinth::ProjectVersion;
use crate::core::instance::Instance;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentScreen {
    Dashboard,
    Instances,
    ModpackSearch,
    ModpackVersions,
    Settings,
    NewInstance,
}

#[derive(Debug, Clone)]
pub struct ModpackResult {
    pub title: String,
    pub slug: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub project_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewInstanceField {
    Name,
    Memory,
    JavaPath,
    Version,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsField {
    AuthType,
    Username,
    Password,
    LoginButton,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    LoggedOut,
    Authenticating,
    WaitingForCode(String, String), // UserCode, VerificationUri
    Success(String),                // Username
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub current_screen: CurrentScreen,

    // Data
    pub instances: Vec<Instance>,
    pub modpack_results: Vec<ModpackResult>,
    pub modpack_versions: Vec<ProjectVersion>,
    pub selected_modpack_name: String,
    pub available_versions: Vec<String>,
    pub manifest: Option<VersionManifest>,
    pub search_query: String,

    // UI State
    pub instance_list_state: ListState,
    pub search_results_state: ratatui::widgets::TableState,
    pub modpack_version_list_state: ListState,
    pub input_mode: InputMode,
    pub show_popup: bool,
    pub popup_message: String,
    pub is_loading: bool,       // true while async work is in progress
    pub confirm_delete: bool,   // delete confirmation dialog

    // New Instance / Edit Form
    pub new_instance_name: String,
    pub new_instance_memory: String,
    pub new_instance_java: String,
    pub new_instance_version_state: ListState,
    pub new_instance_focus: NewInstanceField,
    pub editing_instance_index: Option<usize>,

    // Config & Auth
    pub config: crate::core::config::Config,
    pub auth_state: AuthState,
    pub temp_password: String,
    pub settings_focus: SettingsField,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            current_screen: CurrentScreen::Dashboard,
            instances: Vec::new(),
            modpack_results: Vec::new(),
            modpack_versions: Vec::new(),
            selected_modpack_name: String::new(),
            available_versions: Vec::new(),
            manifest: None,
            search_query: String::new(),
            instance_list_state: ListState::default(),
            search_results_state: ratatui::widgets::TableState::default(),
            modpack_version_list_state: ListState::default(),
            input_mode: InputMode::Normal,
            show_popup: false,
            popup_message: String::new(),
            is_loading: false,
            confirm_delete: false,
            new_instance_name: String::new(),
            new_instance_memory: "4096".to_string(),
            new_instance_java: "java".to_string(),
            new_instance_version_state: ListState::default(),
            new_instance_focus: NewInstanceField::Name,
            editing_instance_index: None,
            config: crate::core::config::Config::default(),
            auth_state: AuthState::LoggedOut,
            temp_password: String::new(),
            settings_focus: SettingsField::AuthType,
        }
    }
}

impl App {
    pub fn new() -> Self {
        let mut app = Self::default();
        if let Ok(instances) = crate::core::fs::load_instances() {
            app.instances = instances;
        }
        app.config = crate::core::config::load_config();
        // Restore auth state from saved config
        if !app.config.auth.username.is_empty()
            && app.config.auth.access_token != "0"
        {
            app.auth_state = AuthState::Success(app.config.auth.username.clone());
        }
        if !app.instances.is_empty() {
            app.instance_list_state.select(Some(0));
        }
        app
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn close_popup(&mut self) {
        self.show_popup = false;
        self.is_loading = false;
    }

    pub fn open_new_instance(&mut self) {
        self.current_screen = CurrentScreen::NewInstance;
        self.editing_instance_index = None;
        self.new_instance_name.clear();
        self.new_instance_memory = "4096".to_string();
        self.new_instance_java = "java".to_string();
        self.new_instance_focus = NewInstanceField::Name;
        self.new_instance_version_state.select(Some(0));
    }

    pub fn open_edit_instance(&mut self) {
        if let Some(index) = self.instance_list_state.selected() {
            if let Some(instance) = self.instances.get(index) {
                self.current_screen = CurrentScreen::NewInstance;
                self.editing_instance_index = Some(index);
                self.new_instance_name = instance.name.clone();
                self.new_instance_memory = instance.max_memory.to_string();
                self.new_instance_java = instance.java_path.clone();
                if let Some(ver_idx) = self.available_versions.iter().position(|v| *v == instance.version) {
                    self.new_instance_version_state.select(Some(ver_idx));
                } else {
                    self.new_instance_version_state.select(Some(0));
                }
                self.new_instance_focus = NewInstanceField::Name;
            }
        }
    }

    pub fn add_instance(&mut self, name: String, id: String, version: String, memory: u32, java: String, loader: String) {
        // Remove existing if reinstalling same id
        self.instances.retain(|i| i.id != id);
        self.instances.push(Instance {
            name,
            id,
            version,
            loader,
            max_memory: memory,
            java_path: java,
            played_last: None,
        });
        let _ = crate::core::fs::save_instances(&self.instances);
        self.current_screen = CurrentScreen::Instances;
        self.instance_list_state.select(Some(self.instances.len().saturating_sub(1)));
    }

    pub fn update_instance(&mut self, index: usize, name: String, memory: u32, java: String) {
        if let Some(instance) = self.instances.get_mut(index) {
            instance.name = name;
            instance.max_memory = memory;
            instance.java_path = java;
        }
        let _ = crate::core::fs::save_instances(&self.instances);
        self.current_screen = CurrentScreen::Instances;
        self.editing_instance_index = None;
    }

    pub fn delete_selected_instance(&mut self) {
        if let Some(index) = self.instance_list_state.selected() {
            if index < self.instances.len() {
                self.instances.remove(index);
                let _ = crate::core::fs::save_instances(&self.instances);
                let new_sel = if self.instances.is_empty() {
                    None
                } else {
                    Some(index.min(self.instances.len() - 1))
                };
                self.instance_list_state.select(new_sel);
            }
        }
    }

    pub fn cycle_new_instance_focus(&mut self) {
        self.new_instance_focus = match self.new_instance_focus {
            NewInstanceField::Name => NewInstanceField::Memory,
            NewInstanceField::Memory => NewInstanceField::JavaPath,
            NewInstanceField::JavaPath => NewInstanceField::Version,
            NewInstanceField::Version => NewInstanceField::Name,
        };
    }

    pub fn next_tab(&mut self) {
        self.current_screen = match self.current_screen {
            CurrentScreen::Dashboard => CurrentScreen::Instances,
            CurrentScreen::Instances => CurrentScreen::ModpackSearch,
            CurrentScreen::ModpackSearch => {
                self.input_mode = InputMode::Normal;
                CurrentScreen::Settings
            }
            CurrentScreen::Settings => CurrentScreen::Dashboard,
            CurrentScreen::NewInstance => CurrentScreen::Instances,
            CurrentScreen::ModpackVersions => CurrentScreen::ModpackSearch,
        };
    }

    pub fn previous_tab(&mut self) {
        self.current_screen = match self.current_screen {
            CurrentScreen::Dashboard => CurrentScreen::Settings,
            CurrentScreen::Instances => CurrentScreen::Dashboard,
            CurrentScreen::ModpackSearch => CurrentScreen::Instances,
            CurrentScreen::Settings => {
                self.input_mode = InputMode::Normal;
                CurrentScreen::ModpackSearch
            }
            CurrentScreen::NewInstance => CurrentScreen::Instances,
            CurrentScreen::ModpackVersions => CurrentScreen::ModpackSearch,
        };
    }

    pub fn next_item(&mut self) {
        match self.current_screen {
            CurrentScreen::Instances => {
                let len = self.instances.len();
                if len == 0 { return; }
                let i = self.instance_list_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.instance_list_state.select(Some(i));
            }
            CurrentScreen::ModpackSearch => {
                let len = self.modpack_results.len();
                if len == 0 { return; }
                let i = self.search_results_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.search_results_state.select(Some(i));
            }
            CurrentScreen::ModpackVersions => {
                let len = self.modpack_versions.len();
                if len == 0 { return; }
                let i = self.modpack_version_list_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.modpack_version_list_state.select(Some(i));
            }
            CurrentScreen::NewInstance => {
                if self.new_instance_focus == NewInstanceField::Version {
                    let len = self.available_versions.len();
                    if len == 0 { return; }
                    let i = self.new_instance_version_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                    self.new_instance_version_state.select(Some(i));
                }
            }
            _ => {}
        }
    }

    pub fn previous_item(&mut self) {
        match self.current_screen {
            CurrentScreen::Instances => {
                let len = self.instances.len();
                if len == 0 { return; }
                let i = self.instance_list_state.selected()
                    .map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                self.instance_list_state.select(Some(i));
            }
            CurrentScreen::ModpackSearch => {
                let len = self.modpack_results.len();
                if len == 0 { return; }
                let i = self.search_results_state.selected()
                    .map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                self.search_results_state.select(Some(i));
            }
            CurrentScreen::ModpackVersions => {
                let len = self.modpack_versions.len();
                if len == 0 { return; }
                let i = self.modpack_version_list_state.selected()
                    .map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                self.modpack_version_list_state.select(Some(i));
            }
            CurrentScreen::NewInstance => {
                if self.new_instance_focus == NewInstanceField::Version {
                    let len = self.available_versions.len();
                    if len == 0 { return; }
                    let i = self.new_instance_version_state.selected()
                        .map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                    self.new_instance_version_state.select(Some(i));
                }
            }
            _ => {}
        }
    }
}
