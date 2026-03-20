use std::io;
use std::sync::mpsc;
use crossterm::event::{KeyCode, KeyEvent};

mod app;
mod event;
mod tui;
mod ui;
mod api;
mod core;

use crate::app::{App, AuthState, CurrentScreen, InputMode, ModpackResult, NewInstanceField};
use crate::event::{Event, EventHandler};
use crate::api::modrinth;
use crate::core::versions::VersionManifest;

// ── Async result channel type ──────────────────────────────────────────────────
enum ApiResult {
    SearchSuccess(Vec<modrinth::SearchResult>),
    VersionsSuccess(Vec<modrinth::ProjectVersion>),
    VersionManifestLoaded(VersionManifest),
    InstallSuccess {
        name: String,
        id: String,
        version: String,
        memory: u32,
        java: String,
        loader: String,
    },
    Error(String),
    GameExited,
    AuthCode(String, String), // UserCode, Uri
    AuthSuccess(String, String, String), // Name, UUID, Token
    AuthError(String),
}

// ── Entry point ────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() -> io::Result<()> {
    let mut app = App::new();
    let events = EventHandler::new(100); // 100ms tick for responsive UI
    let (api_tx, api_rx) = mpsc::channel::<ApiResult>();

    // Fetch version manifest at startup
    {
        let tx = api_tx.clone();
        tokio::spawn(async move {
            match crate::core::versions::fetch_manifest().await {
                Ok(manifest) => { let _ = tx.send(ApiResult::VersionManifestLoaded(manifest)); }
                Err(e) => { let _ = tx.send(ApiResult::Error(format!("Failed to fetch versions: {}", e))); }
            }
        });
    }

    let mut tui = tui::init()?;

    while app.running {
        tui.draw(|frame| ui::render(&mut app, frame))?;

        match events.next() {
            Ok(Event::Tick) => {
                while let Ok(result) = api_rx.try_recv() {
                    handle_api_result(result, &mut app);
                }
            }
            Ok(Event::Key(key)) => handle_key(key, &mut app, &api_tx),
            Ok(Event::Mouse(_)) | Ok(Event::Resize(_, _)) => {}
            Err(_) => app.quit(),
        }
    }

    tui::restore()?;
    Ok(())
}

// ── API result handler ─────────────────────────────────────────────────────────
fn handle_api_result(result: ApiResult, app: &mut App) {
    match result {
        ApiResult::VersionManifestLoaded(manifest) => {
            app.available_versions = manifest.versions.iter()
                .filter(|v| v.version_type == "release")
                .map(|v| v.id.clone())
                .collect();
            app.manifest = Some(manifest);
        }
        ApiResult::SearchSuccess(hits) => {
            app.modpack_results = hits
                .into_iter()
                .map(|h| ModpackResult {
                    title: h.title,
                    slug: h.slug,
                    description: h.description,
                    author: h.author,
                    downloads: h.downloads,
                    project_id: h.project_id,
                })
                .collect();
            if !app.modpack_results.is_empty() {
                app.search_results_state.select(Some(0));
            }
        }
        ApiResult::VersionsSuccess(versions) => {
            app.modpack_versions = versions;
            app.current_screen = CurrentScreen::ModpackVersions;
            app.modpack_version_list_state.select(Some(0));
            app.show_popup = false;
            app.is_loading = false;
        }
        ApiResult::InstallSuccess { name, id, version, memory, java, loader } => {
            app.popup_message = format!("'{}' installed successfully!", name);
            app.is_loading = false;
            app.add_instance(name, id, version, memory, java, loader);
        }
        ApiResult::Error(err) => {
            app.popup_message = format!("Error: {}", err);
            app.show_popup = true;
            app.is_loading = false;
        }
        ApiResult::GameExited => {
            app.popup_message = "Game exited.".to_string();
            app.show_popup = true;
            app.is_loading = false;
        }
        ApiResult::AuthCode(code, uri) => {
            app.auth_state = AuthState::WaitingForCode(code, uri);
            app.show_popup = false;
            app.is_loading = false;
        }
        ApiResult::AuthSuccess(name, uuid, token) => {
            app.config.auth.username = name.clone();
            app.config.auth.uuid = uuid;
            app.config.auth.access_token = token;
            app.auth_state = AuthState::Success(name);
            let _ = crate::core::config::save_config(&app.config);
        }
        ApiResult::AuthError(err) => {
            app.auth_state = AuthState::Error(err);
        }
    }
}

// ── Main key handler ───────────────────────────────────────────────────────────
fn handle_key(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<ApiResult>) {
    // Delete confirmation dialog takes priority
    if app.confirm_delete {
        match key.code {
            KeyCode::Enter => {
                app.confirm_delete = false;
                app.delete_selected_instance();
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                app.confirm_delete = false;
            }
            _ => {}
        }
        return;
    }

    // Popup dismissal
    if app.show_popup && !app.is_loading {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => app.close_popup(),
            _ => {}
        }
        return;
    }
    if app.show_popup && app.is_loading {
        return; // Can't dismiss a loading popup
    }

    // Global quit & tab (only when not in a text field)
    let in_text_field = app.input_mode == InputMode::Editing
        || app.current_screen == CurrentScreen::NewInstance
        || app.current_screen == CurrentScreen::Settings;

    if !in_text_field {
        match key.code {
            KeyCode::Char('q') => { app.quit(); return; }
            KeyCode::Tab => { app.next_tab(); return; }
            KeyCode::BackTab => { app.previous_tab(); return; }
            _ => {}
        }
    }

    match app.current_screen {
        CurrentScreen::Dashboard => handle_dashboard(key, app),
        CurrentScreen::Instances => handle_instances(key, app, tx),
        CurrentScreen::NewInstance => handle_new_instance(key, app, tx),
        CurrentScreen::ModpackSearch => handle_modpack_search(key, app, tx),
        CurrentScreen::ModpackVersions => handle_modpack_versions(key, app, tx),
        CurrentScreen::Settings => handle_settings(key, app, tx),
    }
}

fn handle_dashboard(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.previous_tab(),
        _ => {}
    }
}

fn handle_instances(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<ApiResult>) {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => app.next_item(),
        KeyCode::Char('k') | KeyCode::Up => app.previous_item(),
        KeyCode::Char('n') => app.open_new_instance(),
        KeyCode::Char('e') => app.open_edit_instance(),
        KeyCode::Char('d') => {
            if !app.instances.is_empty() {
                app.confirm_delete = true;
            }
        }
        KeyCode::Enter => {
            if let Some(idx) = app.instance_list_state.selected() {
                if let Some(instance) = app.instances.get(idx) {
                    let id = instance.id.clone();
                    let version = instance.version.clone();
                    let memory = instance.max_memory;
                    let java = instance.java_path.clone();
                    let name = instance.name.clone();
                    let auth_data = app.config.auth.clone();

                    app.popup_message = format!("Launching {}…", name);
                    app.show_popup = true;
                    app.is_loading = true;

                    let tx = tx.clone();
                    tokio::spawn(async move {
                        match crate::core::launcher::launch_instance(&id, &version, memory, &java, &auth_data).await {
                            Ok(mut child) => {
                                let _ = child.wait().await;
                                let _ = tx.send(ApiResult::GameExited);
                            }
                            Err(e) => {
                                let _ = tx.send(ApiResult::Error(format!("Launch failed: {}", e)));
                            }
                        }
                    });
                }
            }
        }
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.previous_tab(),
        _ => {}
    }
}

fn handle_new_instance(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<ApiResult>) {
    match key.code {
        KeyCode::Esc => {
            app.current_screen = CurrentScreen::Instances;
            app.editing_instance_index = None;
        }
        KeyCode::Tab => app.cycle_new_instance_focus(),
        KeyCode::Enter => {
            if let Some(edit_idx) = app.editing_instance_index {
                // Save edits (no reinstall)
                let name = app.new_instance_name.clone();
                let memory: u32 = app.new_instance_memory.parse().unwrap_or(4096);
                let java = if app.new_instance_java.trim().is_empty() {
                    "java".to_string()
                } else {
                    app.new_instance_java.clone()
                };
                app.update_instance(edit_idx, name, memory, java);
            } else {
                spawn_install(app, tx, false);
            }
        }
        code => match app.new_instance_focus {
            NewInstanceField::Name => match code {
                KeyCode::Char(c) => app.new_instance_name.push(c),
                KeyCode::Backspace => { app.new_instance_name.pop(); }
                _ => {}
            },
            NewInstanceField::Memory => match code {
                KeyCode::Char(c) if c.is_ascii_digit() => app.new_instance_memory.push(c),
                KeyCode::Backspace => { app.new_instance_memory.pop(); }
                _ => {}
            },
            NewInstanceField::JavaPath => match code {
                KeyCode::Char(c) => app.new_instance_java.push(c),
                KeyCode::Backspace => { app.new_instance_java.pop(); }
                _ => {}
            },
            NewInstanceField::Version => match code {
                KeyCode::Char('j') | KeyCode::Down => app.next_item(),
                KeyCode::Char('k') | KeyCode::Up => app.previous_item(),
                _ => {}
            },
        },
    }
}

/// Spawn a vanilla install task from the NewInstance form.
fn spawn_install(app: &mut App, tx: &mpsc::Sender<ApiResult>, _is_fabric: bool) {
    let Some(manifest) = &app.manifest else { return; };

    let version = app
        .new_instance_version_state
        .selected()
        .and_then(|i| app.available_versions.get(i))
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());

    let name = if app.new_instance_name.trim().is_empty() {
        format!("Minecraft {}", version)
    } else {
        app.new_instance_name.clone()
    };

    let id: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .replace(' ', "_");

    let memory: u32 = app.new_instance_memory.parse().unwrap_or(4096);
    let java = if app.new_instance_java.trim().is_empty() {
        "java".to_string()
    } else {
        app.new_instance_java.clone()
    };

    app.popup_message = format!("Installing {}…\nThis may take a moment.", name);
    app.show_popup = true;
    app.is_loading = true;

    let manifest_clone = manifest.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        match crate::core::installer::install_version(version.clone(), &manifest_clone, id.clone()).await {
            Ok(_) => {
                let _ = tx.send(ApiResult::InstallSuccess {
                    name,
                    id,
                    version,
                    memory,
                    java,
                    loader: "Vanilla".to_string(),
                });
            }
            Err(e) => {
                let _ = tx.send(ApiResult::Error(format!("Install failed: {}", e)));
            }
        }
    });
}

fn handle_modpack_search(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<ApiResult>) {
    match app.input_mode {
        InputMode::Editing => {
            match key.code {
                KeyCode::Enter => {
                    app.input_mode = InputMode::Normal;
                    let query = app.search_query.clone();
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        match modrinth::search_modpacks(&query).await {
                            Ok(hits) => { let _ = tx.send(ApiResult::SearchSuccess(hits)); }
                            Err(e) => { let _ = tx.send(ApiResult::Error(e.to_string())); }
                        }
                    });
                }
                KeyCode::Esc => {
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => app.search_query.push(c),
                KeyCode::Backspace => { app.search_query.pop(); }
                _ => {}
            }
        }
        InputMode::Normal => {
            match key.code {
                KeyCode::Char('e') | KeyCode::Char('/') => {
                    app.input_mode = InputMode::Editing;
                }
                KeyCode::Char('j') | KeyCode::Down => app.next_item(),
                KeyCode::Char('k') | KeyCode::Up => app.previous_item(),
                KeyCode::Enter => {
                    if let Some(idx) = app.search_results_state.selected() {
                        if let Some(modpack) = app.modpack_results.get(idx) {
                            let id = modpack.project_id.clone();
                            let name = modpack.title.clone();
                            app.selected_modpack_name = name.clone();
                            app.popup_message = format!("Fetching versions for {}…", name);
                            app.show_popup = true;
                            app.is_loading = true;
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                match modrinth::fetch_project_versions(&id).await {
                                    Ok(versions) => { let _ = tx.send(ApiResult::VersionsSuccess(versions)); }
                                    Err(e) => { let _ = tx.send(ApiResult::Error(format!("Failed to fetch versions: {}", e))); }
                                }
                            });
                        }
                    }
                }
                KeyCode::Tab => app.next_tab(),
                KeyCode::BackTab => app.previous_tab(),
                KeyCode::Char('q') => app.quit(),
                _ => {}
            }
        }
    }
}

fn handle_modpack_versions(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<ApiResult>) {
    match key.code {
        KeyCode::Esc => app.current_screen = CurrentScreen::ModpackSearch,
        KeyCode::Char('j') | KeyCode::Down => app.next_item(),
        KeyCode::Char('k') | KeyCode::Up => app.previous_item(),
        KeyCode::Enter => {
            if let Some(idx) = app.modpack_version_list_state.selected() {
                if let Some(version) = app.modpack_versions.get(idx) {
                    if let Some(file) = version.files.iter().find(|f| f.filename.ends_with(".mrpack")) {
                        let url = file.url.clone();
                        let name = app.selected_modpack_name.clone();
                        let version_name = version.version_number.clone();

                        app.popup_message = format!("Installing {} v{}…\nDownloading mods, this may take a while.", name, version_name);
                        app.show_popup = true;
                        app.is_loading = true;

                        if let Some(manifest) = &app.manifest {
                            let manifest_clone = manifest.clone();
                            let tx = tx.clone();
                            let folder_name = name
                                .chars()
                                .filter(|c| c.is_alphanumeric() || *c == ' ')
                                .collect::<String>()
                                .replace(' ', "_");
                            let name_clone = name.clone();

                            tokio::spawn(async move {
                                match crate::core::installer::install_modpack(url, folder_name.clone()).await {
                                    Ok((game_version, loader_version)) => {
                                        match crate::core::installer::install_version(game_version.clone(), &manifest_clone, folder_name.clone()).await {
                                            Ok(_) => {
                                                let (loader_label, memory) = if !loader_version.is_empty() {
                                                    match crate::core::installer::install_fabric(game_version.clone(), loader_version, folder_name.clone()).await {
                                                        Ok(_) => ("Fabric".to_string(), 6144u32),
                                                        Err(e) => {
                                                            let _ = tx.send(ApiResult::Error(format!("Fabric install failed: {}", e)));
                                                            return;
                                                        }
                                                    }
                                                } else {
                                                    ("Vanilla".to_string(), 4096u32)
                                                };
                                                let _ = tx.send(ApiResult::InstallSuccess {
                                                    name: name_clone,
                                                    id: folder_name,
                                                    version: game_version,
                                                    memory,
                                                    java: "java".to_string(),
                                                    loader: loader_label,
                                                });
                                            }
                                            Err(e) => {
                                                let _ = tx.send(ApiResult::Error(format!("Base game install failed: {}", e)));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(ApiResult::Error(format!("Modpack download failed: {}", e)));
                                    }
                                }
                            });
                        }
                    } else {
                        app.popup_message = "No .mrpack file found for this version.".to_string();
                        app.show_popup = true;
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_settings(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<ApiResult>) {
    use crate::app::SettingsField;
    use crate::core::config::AuthType;

    // Esc goes back
    if key.code == KeyCode::Esc {
        app.current_screen = CurrentScreen::Dashboard;
        return;
    }

    // Tab/arrows cycle focus
    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            app.settings_focus = match app.settings_focus {
                SettingsField::AuthType => SettingsField::Username,
                SettingsField::Username => {
                    if app.config.auth.auth_type == AuthType::ElyBy {
                        SettingsField::Password
                    } else {
                        SettingsField::LoginButton
                    }
                }
                SettingsField::Password => SettingsField::LoginButton,
                SettingsField::LoginButton => SettingsField::AuthType,
            };
            return;
        }
        KeyCode::BackTab | KeyCode::Up => {
            app.settings_focus = match app.settings_focus {
                SettingsField::AuthType => SettingsField::LoginButton,
                SettingsField::Username => SettingsField::AuthType,
                SettingsField::Password => SettingsField::Username,
                SettingsField::LoginButton => {
                    if app.config.auth.auth_type == AuthType::ElyBy {
                        SettingsField::Password
                    } else {
                        SettingsField::Username
                    }
                }
            };
            return;
        }
        _ => {}
    }

    // Field-specific input
    match app.settings_focus {
        SettingsField::AuthType => {
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                    app.config.auth.auth_type = match app.config.auth.auth_type {
                        AuthType::Offline => AuthType::Microsoft,
                        AuthType::Microsoft => AuthType::ElyBy,
                        AuthType::ElyBy => AuthType::Offline,
                    };
                }
                _ => {}
            }
        }
        SettingsField::Username => match key.code {
            KeyCode::Char(c) => app.config.auth.username.push(c),
            KeyCode::Backspace => { app.config.auth.username.pop(); }
            _ => {}
        },
        SettingsField::Password => match key.code {
            KeyCode::Char(c) => app.temp_password.push(c),
            KeyCode::Backspace => { app.temp_password.pop(); }
            _ => {}
        },
        SettingsField::LoginButton => {}
    }

    // Enter triggers action
    if key.code == KeyCode::Enter {
        match app.config.auth.auth_type {
            AuthType::Offline => {
                app.config.auth.uuid = crate::core::auth::generate_offline_uuid(&app.config.auth.username);
                let _ = crate::core::config::save_config(&app.config);
                app.auth_state = AuthState::Success(app.config.auth.username.clone());
                app.current_screen = CurrentScreen::Dashboard;
            }
            AuthType::Microsoft => {
                app.auth_state = AuthState::Authenticating;
                app.current_screen = CurrentScreen::Dashboard;
                app.popup_message = "Starting Microsoft auth…\nOpen your browser when prompted.".to_string();
                app.show_popup = true;
                app.is_loading = true;

                let tx = tx.clone();
                tokio::spawn(async move {
                    let client = reqwest::Client::new();
                    match crate::core::auth::start_microsoft_auth_flow(&client).await {
                        Ok(device_code) => {
                            let _ = tx.send(ApiResult::AuthCode(
                                device_code.user_code.clone(),
                                device_code.verification_uri.clone(),
                            ));
                            let dc = device_code.device_code.clone();
                            tokio::spawn(async move {
                                match crate::core::auth::poll_microsoft_token(&client, &dc).await {
                                    Ok(token) if token.error.is_none() => {
                                        let client2 = reqwest::Client::new();
                                        match crate::core::auth::authenticate_minecraft_xbox(&client2, &token.access_token).await {
                                            Ok((mc_token, name, uuid)) => {
                                                let _ = tx.send(ApiResult::AuthSuccess(name, uuid, mc_token));
                                            }
                                            Err(e) => { let _ = tx.send(ApiResult::AuthError(e)); }
                                        }
                                    }
                                    Ok(token) => {
                                        let _ = tx.send(ApiResult::AuthError(
                                            token.error.unwrap_or_else(|| "Auth failed".to_string())
                                        ));
                                    }
                                    Err(e) => { let _ = tx.send(ApiResult::AuthError(e)); }
                                }
                            });
                        }
                        Err(e) => { let _ = tx.send(ApiResult::AuthError(e)); }
                    }
                });
            }
            AuthType::ElyBy => {
                app.auth_state = AuthState::Authenticating;
                app.current_screen = CurrentScreen::Dashboard;
                app.popup_message = "Logging in to Ely.by…".to_string();
                app.show_popup = true;
                app.is_loading = true;

                let username = app.config.auth.username.clone();
                let password = std::mem::take(&mut app.temp_password);
                let tx = tx.clone();
                tokio::spawn(async move {
                    let client = reqwest::Client::new();
                    match crate::core::auth::authenticate_ely_by(&client, &username, &password).await {
                        Ok((token, name, uuid)) => {
                            let _ = tx.send(ApiResult::AuthSuccess(name, uuid, token));
                        }
                        Err(e) => { let _ = tx.send(ApiResult::AuthError(e)); }
                    }
                });
            }
        }
    }
}
