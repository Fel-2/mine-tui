use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::app::{App, AuthState, CurrentScreen, InputMode, NewInstanceField, SettingsField};
use crate::core::config::AuthType;

// ── Theme ──────────────────────────────────────────────────────────────────────
const PRIMARY: Color = Color::Green;
const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const WARNING: Color = Color::Yellow;
const ERR: Color = Color::Red;

fn border_style(active: bool) -> Style {
    if active { Style::default().fg(PRIMARY) } else { Style::default().fg(MUTED) }
}

fn text_style(active: bool) -> Style {
    if active { Style::default().fg(Color::White) } else { Style::default().fg(MUTED) }
}

fn format_downloads(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

// ── Entry point ────────────────────────────────────────────────────────────────
pub fn render(app: &mut App, frame: &mut Frame) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    render_header(app, frame, root[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(0)])
        .split(root[1]);

    render_sidebar(app, frame, body[0]);
    render_content(app, frame, body[1]);
    render_footer(app, frame, root[2]);

    if app.confirm_delete {
        render_confirm_delete(app, frame);
    }
    if app.show_popup {
        render_popup(app, frame);
    }
}

// ── Header ─────────────────────────────────────────────────────────────────────
fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let auth_label = match &app.auth_state {
        AuthState::Success(name) => {
            let method = match app.config.auth.auth_type {
                AuthType::Offline => "Offline",
                AuthType::Microsoft => "Microsoft",
                AuthType::ElyBy => "Ely.by",
            };
            format!(" {} ({}) ", name, method)
        }
        AuthState::WaitingForCode(code, _) => format!(" Code: {} ", code),
        AuthState::Authenticating => " Authenticating… ".to_string(),
        AuthState::Error(_) => " Auth Error ".to_string(),
        AuthState::LoggedOut => format!(" {} (not logged in) ", app.config.auth.username),
    };

    let auth_color = match &app.auth_state {
        AuthState::Success(_) => PRIMARY,
        AuthState::Error(_) => ERR,
        AuthState::WaitingForCode(_, _) | AuthState::Authenticating => WARNING,
        AuthState::LoggedOut => MUTED,
    };

    let loading_label = if app.is_loading {
        " ⏳ Working… "
    } else {
        ""
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(11), // brand
            Constraint::Min(0),     // auth info
            Constraint::Length(16), // loading indicator
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(" ⛏  MineTUI", Style::default().fg(Color::Black).bg(PRIMARY).add_modifier(Modifier::BOLD))),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(auth_label, Style::default().fg(auth_color))),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(loading_label, Style::default().fg(WARNING)))
            .alignment(Alignment::Right),
        chunks[2],
    );
}

// ── Footer ─────────────────────────────────────────────────────────────────────
fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let hints = match app.current_screen {
        CurrentScreen::Dashboard => " Tab: Navigate   q: Quit ",
        CurrentScreen::Instances => " n: New   e: Edit   d: Delete   Enter: Launch   Tab: Switch   q: Quit ",
        CurrentScreen::ModpackSearch => {
            if app.input_mode == InputMode::Editing {
                " Enter: Search   Esc: Cancel "
            } else {
                " e: Search   Enter: Select   Tab: Switch   q: Quit "
            }
        }
        CurrentScreen::ModpackVersions => " Enter: Install   Esc: Back ",
        CurrentScreen::Settings => " Tab/↑↓: Navigate   ←→: Change Auth   Enter: Save/Login   Esc: Back ",
        CurrentScreen::NewInstance => " Tab: Next Field   ↑↓: Version   Enter: Install   Esc: Cancel ",
    };
    frame.render_widget(
        Paragraph::new(hints)
            .style(Style::default().fg(Color::Black).bg(MUTED))
            .alignment(Alignment::Center),
        area,
    );
}

// ── Sidebar ────────────────────────────────────────────────────────────────────
fn render_sidebar(app: &App, frame: &mut Frame, area: Rect) {
    let tabs = [
        ("Dashboard", CurrentScreen::Dashboard),
        ("Instances", CurrentScreen::Instances),
        ("Modpacks", CurrentScreen::ModpackSearch),
        ("Settings", CurrentScreen::Settings),
    ];

    let items: Vec<ListItem> = tabs
        .iter()
        .map(|(label, screen)| {
            let active = app.current_screen == *screen
                || (app.current_screen == CurrentScreen::NewInstance && *screen == CurrentScreen::Instances)
                || (app.current_screen == CurrentScreen::ModpackVersions && *screen == CurrentScreen::ModpackSearch);

            let style = if active {
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            };
            let prefix = if active { "> " } else { "  " };
            ListItem::new(Line::from(Span::styled(format!("{}{}", prefix, label), style)))
        })
        .collect();

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED))
                .title(Span::styled(" Menu ", Style::default().fg(ACCENT))),
        ),
        area,
    );
}

// ── Content router ─────────────────────────────────────────────────────────────
fn render_content(app: &mut App, frame: &mut Frame, area: Rect) {
    let title = match app.current_screen {
        CurrentScreen::Dashboard => " Dashboard ",
        CurrentScreen::Instances => " Instances ",
        CurrentScreen::ModpackSearch => " Browse Modpacks ",
        CurrentScreen::ModpackVersions => " Select Version ",
        CurrentScreen::Settings => " Settings ",
        CurrentScreen::NewInstance => {
            if app.editing_instance_index.is_some() {
                " Edit Instance "
            } else {
                " New Instance "
            }
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED))
        .title(Span::styled(title, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.current_screen {
        CurrentScreen::Dashboard => render_dashboard(app, frame, inner),
        CurrentScreen::Instances => render_instances(app, frame, inner),
        CurrentScreen::ModpackSearch => render_modpack_search(app, frame, inner),
        CurrentScreen::ModpackVersions => render_modpack_versions(app, frame, inner),
        CurrentScreen::Settings => render_settings(app, frame, inner),
        CurrentScreen::NewInstance => render_new_instance(app, frame, inner),
    }
}

// ── Dashboard ──────────────────────────────────────────────────────────────────
fn render_dashboard(app: &App, frame: &mut Frame, area: Rect) {
    let auth_info = match &app.auth_state {
        AuthState::Success(n) => n.clone(),
        _ => "Not logged in".to_string(),
    };
    let auth_color = match &app.auth_state {
        AuthState::Success(_) => PRIMARY,
        _ => MUTED,
    };
    let versions_label = if app.available_versions.is_empty() {
        "Loading…".to_string()
    } else {
        format!("{} releases available", app.available_versions.len())
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Welcome to MineTUI",
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "A terminal Minecraft launcher",
            Style::default().fg(MUTED),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Instances:  ", Style::default().fg(MUTED)),
            Span::styled(
                app.instances.len().to_string(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Account:    ", Style::default().fg(MUTED)),
            Span::styled(auth_info, Style::default().fg(auth_color)),
        ]),
        Line::from(vec![
            Span::styled("  Versions:   ", Style::default().fg(MUTED)),
            Span::styled(versions_label, Style::default().fg(ACCENT)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Tab to navigate between sections",
            Style::default().fg(MUTED),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        area,
    );
}

// ── Instances ──────────────────────────────────────────────────────────────────
fn render_instances(app: &mut App, frame: &mut Frame, area: Rect) {
    if app.instances.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No instances yet",
                    Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press 'n' to create your first instance",
                    Style::default().fg(MUTED),
                )),
            ])
            .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .instances
        .iter()
        .map(|inst| {
            let loader_color = match inst.loader.as_str() {
                "Fabric" => Color::LightBlue,
                "Forge" => Color::Magenta,
                _ => MUTED,
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        inst.name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", inst.loader),
                        Style::default().fg(loader_color),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("MC {}  |  {}MB RAM  |  {}", inst.version, inst.max_memory, inst.java_path),
                        Style::default().fg(MUTED),
                    ),
                ]),
                Line::from(""),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))
        .highlight_symbol("");

    frame.render_stateful_widget(list, area, &mut app.instance_list_state);
}

// ── Modpack Search ─────────────────────────────────────────────────────────────
fn render_modpack_search(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search bar
    let editing = app.input_mode == InputMode::Editing;
    let placeholder = if !editing && app.search_query.is_empty() {
        "Press 'e' to search Modrinth modpacks…"
    } else {
        ""
    };
    let display = if app.search_query.is_empty() { placeholder } else { &app.search_query };

    frame.render_widget(
        Paragraph::new(Span::styled(
            display,
            if editing { Style::default().fg(Color::White) } else { Style::default().fg(MUTED) },
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style(editing))
                .title(Span::styled(
                    if editing { " Search (typing) " } else { " Search " },
                    Style::default().fg(ACCENT),
                )),
        ),
        chunks[0],
    );

    if app.modpack_results.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    if editing { "Type and press Enter to search" } else { "No results — press 'e' to search" },
                    Style::default().fg(MUTED),
                )),
            ])
            .alignment(Alignment::Center),
            chunks[1],
        );
        return;
    }

    // Split results + description panel
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[1]);

    // Description panel
    if let Some(idx) = app.search_results_state.selected() {
        if let Some(pack) = app.modpack_results.get(idx) {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        pack.title.clone(),
                        Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        format!("by {}", pack.author),
                        Style::default().fg(MUTED),
                    )),
                    Line::from(""),
                    Line::from(pack.description.clone()),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("{} downloads", format_downloads(pack.downloads)),
                        Style::default().fg(MUTED),
                    )),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(MUTED))
                        .title(Span::styled(" Details ", Style::default().fg(ACCENT))),
                )
                .wrap(Wrap { trim: true }),
                split[1],
            );
        }
    }

    // Results table
    let header = Row::new(["Name", "Author", "Downloads"].iter().map(|h| {
        Cell::from(*h).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
    }));

    let rows = app.modpack_results.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.title.clone()),
            Cell::from(p.author.clone()),
            Cell::from(format_downloads(p.downloads)),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(55),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED))
            .title(Span::styled(
                format!(" {} Results ", app.modpack_results.len()),
                Style::default().fg(ACCENT),
            )),
    )
    .row_highlight_style(Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, split[0], &mut app.search_results_state);
}

// ── Modpack Versions ───────────────────────────────────────────────────────────
fn render_modpack_versions(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Versions for: ", Style::default().fg(MUTED)),
            Span::styled(
                app.selected_modpack_name.clone(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ])),
        chunks[0],
    );

    let items: Vec<ListItem> = app
        .modpack_versions
        .iter()
        .map(|v| {
            ListItem::new(vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(v.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(
                        format!("v{}", v.version_number),
                        Style::default().fg(MUTED),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("MC: {}  |  {}", v.game_versions.join(", "), v.loaders.join(", ")),
                        Style::default().fg(MUTED),
                    ),
                ]),
                Line::from(""),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED))
                .title(Span::styled(" Available Versions ", Style::default().fg(ACCENT))),
        )
        .highlight_style(Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))
        .highlight_symbol("");

    frame.render_stateful_widget(list, chunks[1], &mut app.modpack_version_list_state);
}

// ── New / Edit Instance ────────────────────────────────────────────────────────
fn render_new_instance(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // Memory
            Constraint::Length(3), // Java
            Constraint::Length(1), // version label
            Constraint::Min(0),    // version list
        ])
        .split(area);

    let name_focused = app.new_instance_focus == NewInstanceField::Name;
    let mem_focused = app.new_instance_focus == NewInstanceField::Memory;
    let java_focused = app.new_instance_focus == NewInstanceField::JavaPath;
    let ver_focused = app.new_instance_focus == NewInstanceField::Version;

    frame.render_widget(
        Paragraph::new(app.new_instance_name.as_str())
            .style(text_style(name_focused))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style(name_focused))
                    .title(Span::styled(" Name ", Style::default().fg(ACCENT))),
            ),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(app.new_instance_memory.as_str())
            .style(text_style(mem_focused))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style(mem_focused))
                    .title(Span::styled(" Max Memory (MB) ", Style::default().fg(ACCENT))),
            ),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(app.new_instance_java.as_str())
            .style(text_style(java_focused))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style(java_focused))
                    .title(Span::styled(" Java Path ", Style::default().fg(ACCENT))),
            ),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Minecraft Version:",
            if ver_focused {
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            },
        )),
        chunks[3],
    );

    if app.available_versions.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("Loading versions…", Style::default().fg(MUTED)))
                .alignment(Alignment::Center),
            chunks[4],
        );
    } else {
        let versions: Vec<ListItem> = app
            .available_versions
            .iter()
            .map(|v| ListItem::new(Line::from(Span::raw(v.clone()))))
            .collect();

        let list = List::new(versions)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style(ver_focused)),
            )
            .highlight_style(Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, chunks[4], &mut app.new_instance_version_state);
    }
}

// ── Settings ───────────────────────────────────────────────────────────────────
fn render_settings(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // auth type
            Constraint::Length(3), // username / info
            Constraint::Length(3), // password (ely.by only)
            Constraint::Min(0),    // status
            Constraint::Length(3), // button
        ])
        .split(area);

    let auth_idx = match app.config.auth.auth_type {
        AuthType::Offline => 0,
        AuthType::Microsoft => 1,
        AuthType::ElyBy => 2,
    };

    frame.render_widget(
        ratatui::widgets::Tabs::new(vec!["Offline", "Microsoft", "Ely.by"])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style(app.settings_focus == SettingsField::AuthType))
                    .title(Span::styled(" Auth Method ", Style::default().fg(ACCENT))),
            )
            .select(auth_idx)
            .highlight_style(Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))
            .divider("|"),
        chunks[0],
    );

    match app.config.auth.auth_type {
        AuthType::Offline | AuthType::ElyBy => {
            let focused = app.settings_focus == SettingsField::Username;
            let label = if app.config.auth.auth_type == AuthType::ElyBy {
                " Email "
            } else {
                " Username "
            };
            frame.render_widget(
                Paragraph::new(app.config.auth.username.as_str())
                    .style(text_style(focused))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(border_style(focused))
                            .title(Span::styled(label, Style::default().fg(ACCENT))),
                    ),
                chunks[1],
            );
        }
        AuthType::Microsoft => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "Device code flow — press Login to begin",
                    Style::default().fg(MUTED),
                ))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(MUTED)),
                ),
                chunks[1],
            );
        }
    }

    if app.config.auth.auth_type == AuthType::ElyBy {
        let focused = app.settings_focus == SettingsField::Password;
        frame.render_widget(
            Paragraph::new("*".repeat(app.temp_password.len()))
                .style(text_style(focused))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(border_style(focused))
                        .title(Span::styled(" Password ", Style::default().fg(ACCENT))),
                ),
            chunks[2],
        );
    }

    let (status_text, status_color) = match &app.auth_state {
        AuthState::LoggedOut => ("Not logged in".to_string(), MUTED),
        AuthState::Authenticating => ("Authenticating…".to_string(), WARNING),
        AuthState::WaitingForCode(code, uri) => {
            (format!("Visit: {}\nEnter code: {}", uri, code), WARNING)
        }
        AuthState::Success(name) => (format!("Logged in as: {}", name), PRIMARY),
        AuthState::Error(e) => (format!("Error: {}", e), ERR),
    };

    frame.render_widget(
        Paragraph::new(status_text)
            .style(Style::default().fg(status_color))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(MUTED))
                    .title(Span::styled(" Status ", Style::default().fg(ACCENT))),
            )
            .wrap(Wrap { trim: true }),
        chunks[3],
    );

    let btn_focused = app.settings_focus == SettingsField::LoginButton;
    let btn_label = match app.config.auth.auth_type {
        AuthType::Offline => " Save ",
        _ => " Login ",
    };

    frame.render_widget(
        Paragraph::new(btn_label)
            .alignment(Alignment::Center)
            .style(if btn_focused {
                Style::default().fg(Color::Black).bg(PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(PRIMARY)
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style(btn_focused)),
            ),
        chunks[4],
    );
}

// ── Popup ──────────────────────────────────────────────────────────────────────
fn render_popup(app: &App, frame: &mut Frame) {
    let area = centered_rect(55, 30, frame.area());
    frame.render_widget(Clear, area);

    let border_col = if app.is_loading { WARNING } else { PRIMARY };
    let title = if app.is_loading { " Working… " } else { " Info " };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_col))
        .title(Span::styled(title, Style::default().fg(ACCENT)))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let lines: Vec<Line> = app
        .popup_message
        .lines()
        .map(|l| Line::from(l.to_string()))
        .collect();

    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        inner_chunks[0],
    );

    if !app.is_loading {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(MUTED),
            ))
            .alignment(Alignment::Center),
            inner_chunks[1],
        );
    }
}

// ── Delete Confirm ─────────────────────────────────────────────────────────────
fn render_confirm_delete(app: &App, frame: &mut Frame) {
    let area = centered_rect(42, 25, frame.area());
    frame.render_widget(Clear, area);

    let name = app
        .instances
        .get(app.instance_list_state.selected().unwrap_or(0))
        .map(|i| i.name.clone())
        .unwrap_or_default();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ERR))
        .title(Span::styled(" Confirm Delete ", Style::default().fg(ERR)))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("Delete '{}'?", name),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Instance files on disk are kept.",
                Style::default().fg(MUTED),
            )),
        ])
        .alignment(Alignment::Center),
        inner_chunks[0],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter: Delete   Esc: Cancel",
            Style::default().fg(MUTED),
        ))
        .alignment(Alignment::Center),
        inner_chunks[1],
    );
}

// ── Helpers ────────────────────────────────────────────────────────────────────
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
