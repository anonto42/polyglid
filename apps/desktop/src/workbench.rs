use std::{
    collections::HashSet,
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread,
};

use gpui::{
    div, prelude::*, px, ClipboardItem, Context, CursorStyle, Div, FocusHandle, IntoElement,
    KeyDownEvent, MouseButton, Render, Window,
};
use polyglid_client::{
    client::{BootstrapSnapshot, Document, WorkspaceEntry},
    controllers::DesktopControllers,
    PluginStatus, ReportFormat,
};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::{
    actions::*,
    models::{WorkbenchState, WorkspaceView},
    theme,
};

pub struct Workbench {
    controllers: DesktopControllers,
    snapshot: BootstrapSnapshot,
    explorer_entries: Vec<WorkspaceEntry>,
    expanded_directories: HashSet<String>,
    state: WorkbenchState,
    focus_handle: FocusHandle,
    initial_focus_requested: bool,
    terminal_session: Option<TerminalSession>,
}

struct TerminalSession {
    _master: Box<dyn MasterPty + Send>,
    _child: Box<dyn Child + Send>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<String>>,
}

impl Workbench {
    fn workspace_label(&self) -> String {
        std::path::Path::new(&self.snapshot.active_workspace.root_path)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.snapshot.active_workspace.name)
            .to_owned()
    }

    pub fn new(
        controllers: DesktopControllers,
        snapshot: BootstrapSnapshot,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = WorkbenchState::new(&snapshot.shell);
        let explorer_entries = controllers
            .documents
            .list_directory(&snapshot.active_workspace.id, "")
            .unwrap_or_default();
        Self {
            controllers,
            snapshot,
            explorer_entries,
            expanded_directories: HashSet::new(),
            state,
            focus_handle: cx.focus_handle(),
            initial_focus_requested: false,
            terminal_session: None,
        }
    }

    fn activate(&mut self, view: WorkspaceView, cx: &mut Context<Self>) {
        self.state.activate(view);
        self.state.settings_visible = false;
        self.state.status = format!("{} selected", view.title());
        cx.notify();
    }

    fn reload(&mut self) {
        match self.controllers.application.bootstrap() {
            Ok(snapshot) => {
                match self
                    .controllers
                    .documents
                    .list_directory(&snapshot.active_workspace.id, "")
                {
                    Ok(entries) => {
                        self.explorer_entries = entries;
                        self.state.status = "Workspace refreshed".to_owned();
                    }
                    Err(error) => {
                        self.explorer_entries.clear();
                        self.state.status = format!("Explorer unavailable: {error}");
                    }
                }
                self.snapshot = snapshot;
            }
            Err(error) => self.state.status = format!("Refresh failed: {error}"),
        }
    }

    fn open_projects(&mut self, _: &OpenProjects, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(WorkspaceView::Projects, cx);
    }

    fn open_scanner(&mut self, _: &OpenScanner, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(WorkspaceView::Scanner, cx);
    }

    fn open_executions(&mut self, _: &OpenExecutions, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(WorkspaceView::Executions, cx);
    }

    fn open_reports(&mut self, _: &OpenReports, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(WorkspaceView::Reports, cx);
    }

    fn open_plugins(&mut self, _: &OpenPlugins, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(WorkspaceView::Plugins, cx);
    }

    fn open_palette(&mut self, _: &OpenCommandPalette, _: &mut Window, cx: &mut Context<Self>) {
        self.state.command_palette_visible = true;
        self.state.app_menu_visible = false;
        self.state.workspace_menu_visible = false;
        self.state.settings_visible = false;
        cx.notify();
    }

    fn toggle_app_menu(&mut self, cx: &mut Context<Self>) {
        self.state.app_menu_visible = !self.state.app_menu_visible;
        self.state.workspace_menu_visible = false;
        cx.notify();
    }

    fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.state.settings_visible = !self.state.settings_visible;
        self.state.active_document = None;
        self.state.app_menu_visible = false;
        self.state.workspace_menu_visible = false;
        cx.notify();
    }

    fn close_special_tab(&mut self, cx: &mut Context<Self>) {
        self.state.settings_visible = false;
        self.state.active_view = WorkspaceView::Projects;
        self.state.status = "Workspace tab closed".to_owned();
        cx.notify();
    }

    fn open_plugin_tab(&mut self, plugin_id: String, cx: &mut Context<Self>) {
        if let Some(plugin) = self
            .snapshot
            .plugins
            .iter()
            .find(|plugin| plugin.id == plugin_id)
        {
            let contents = format!(
                "[plugin]\nid = \"{}\"\nname = \"{}\"\nversion = \"{}\"\nauthor = \"{}\"\nstatus = \"{}\"\n\n[security]\ndescription = \"{}\"\ncapabilities = {:?}\n",
                plugin.id,
                plugin.name,
                plugin.version,
                plugin.author,
                plugin.status,
                plugin.description,
                plugin.capabilities,
            );
            self.state.open_document(Document {
                relative_path: format!("plugin://{}.toml", plugin.id),
                byte_size: contents.len() as u64,
                contents,
            });
        }
        self.state.active_view = WorkspaceView::Plugins;
        self.state.settings_visible = false;
        cx.notify();
    }

    fn toggle_workspace_menu(&mut self, cx: &mut Context<Self>) {
        self.state.workspace_menu_visible = !self.state.workspace_menu_visible;
        // The workspace chip is also the repository entry point: keep the
        // selected repository visible in the right-hand explorer.
        self.state.sidebar_visible = true;
        self.state.app_menu_visible = false;
        cx.notify();
    }

    fn switch_workspace(&mut self, workspace_id: &str, cx: &mut Context<Self>) {
        match self
            .controllers
            .application
            .activate_workspace(workspace_id)
        {
            Ok(()) => {
                self.state.documents.clear();
                self.state.active_document = None;
                self.state.explorer_directory.clear();
                self.expanded_directories.clear();
                self.state.sidebar_visible = true;
                self.state.workspace_menu_visible = false;
                self.reload();
                self.state.status = "Workspace activated".to_owned();
            }
            Err(error) => self.state.status = format!("Workspace switch failed: {error}"),
        }
        cx.notify();
    }

    fn open_repository(&mut self, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open PolyGlid Repository")
            .pick_folder()
        else {
            self.state.workspace_menu_visible = false;
            cx.notify();
            return;
        };

        let root_path = path.to_string_lossy().into_owned();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Repository");
        match self
            .controllers
            .application
            .register_workspace(name, &root_path)
        {
            Ok(workspace) => self.switch_workspace(&workspace.id, cx),
            Err(error) => {
                self.state.status = format!("Repository open failed: {error}");
                cx.notify();
            }
        }
    }

    fn close_overlay(&mut self, _: &CloseOverlay, _: &mut Window, cx: &mut Context<Self>) {
        self.state.command_palette_visible = false;
        self.state.app_menu_visible = false;
        self.state.workspace_menu_visible = false;
        cx.notify();
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.state.sidebar_visible = !self.state.sidebar_visible;
        cx.notify();
    }

    fn toggle_bottom_panel(
        &mut self,
        _: &ToggleBottomPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.bottom_panel_visible = !self.state.bottom_panel_visible;
        cx.notify();
    }

    fn toggle_terminal(&mut self, cx: &mut Context<Self>) {
        self.state.bottom_panel_visible = !self.state.bottom_panel_visible;
        self.state.terminal_focused = self.state.bottom_panel_visible;
        if self.state.bottom_panel_visible {
            self.ensure_terminal_session();
        }
        self.state.status = if self.state.bottom_panel_visible {
            "Terminal opened".to_owned()
        } else {
            "Terminal closed".to_owned()
        };
        cx.notify();
    }

    fn resize_panels(
        &mut self,
        event: &gpui::MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let bounds = window.bounds();
        if self.state.resizing_sidebar {
            self.state.sidebar_width = (f32::from(bounds.size.width) - f32::from(event.position.x))
                .clamp(180.0, 520.0) as f64;
            cx.notify();
        } else if self.state.resizing_terminal {
            self.state.terminal_height = (f32::from(bounds.size.height)
                - f32::from(event.position.y))
            .clamp(120.0, 520.0) as f64;
            cx.notify();
        }
    }

    fn stop_resizing(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.state.resizing_sidebar = false;
        self.state.resizing_terminal = false;
        cx.notify();
    }

    fn write_terminal_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.ensure_terminal_session();
        if let Some(session) = self.terminal_session.as_mut() {
            if let Err(error) = session.writer.write_all(bytes) {
                self.state.terminal_output.push(format!("error: {error}"));
            } else if let Err(error) = session.writer.flush() {
                self.state.terminal_output.push(format!("error: {error}"));
            }
        }
        cx.notify();
    }

    fn ensure_terminal_session(&mut self) {
        if self.terminal_session.is_some() {
            return;
        }
        let pty = native_pty_system();
        let pair = match pty.openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(pair) => pair,
            Err(error) => {
                self.state.terminal_output.push(format!("error: {error}"));
                return;
            }
        };
        let shell = if cfg!(target_os = "windows") {
            "cmd.exe".to_owned()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
        };
        let mut command = CommandBuilder::new(shell);
        command.cwd(&self.snapshot.active_workspace.root_path);
        if !cfg!(target_os = "windows") {
            // A PTY alone is not enough for every shell to enter interactive
            // mode (notably when the user's shell is invoked through a
            // wrapper). This makes prompts, line editing, and signals behave
            // like the terminal in an editor.
            command.arg("-i");
            command.env("TERM", "xterm-256color");
        }
        let child = match pair.slave.spawn_command(command) {
            Ok(child) => child,
            Err(error) => {
                self.state.terminal_output.push(format!("error: {error}"));
                return;
            }
        };
        let mut reader = match pair.master.try_clone_reader() {
            Ok(reader) => reader,
            Err(error) => {
                self.state.terminal_output.push(format!("error: {error}"));
                return;
            }
        };
        let output = Arc::new(Mutex::new(String::new()));
        let output_writer = Arc::clone(&output);
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(size) => {
                        if let Ok(mut output) = output_writer.lock() {
                            output.push_str(&String::from_utf8_lossy(&buffer[..size]));
                        }
                    }
                }
            }
        });
        let writer = match pair.master.take_writer() {
            Ok(writer) => writer,
            Err(error) => {
                self.state.terminal_output.push(format!("error: {error}"));
                return;
            }
        };
        self.terminal_session = Some(TerminalSession {
            _master: pair.master,
            _child: child,
            writer,
            output,
        });
    }

    fn open_system_terminal(&mut self, cx: &mut Context<Self>) {
        let root = self.snapshot.active_workspace.root_path.clone();
        let result = if cfg!(target_os = "windows") {
            std::process::Command::new("wt").args(["-d", &root]).spawn()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open")
                .args(["-a", "Terminal", &root])
                .spawn()
        } else {
            std::process::Command::new("x-terminal-emulator")
                .current_dir(&root)
                .spawn()
        };
        self.state.status = match result {
            Ok(_) => "System terminal opened".to_owned(),
            Err(error) => format!("System terminal unavailable: {error}"),
        };
        cx.notify();
    }

    fn handle_terminal_key(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let bytes: Option<&[u8]> = if key == "enter" || key == "return" {
            Some(b"\r")
        } else if key == "backspace" {
            Some(b"\x7f")
        } else if key == "tab" {
            Some(b"\t")
        } else if key == "up" {
            Some(b"\x1b[A")
        } else if key == "down" {
            Some(b"\x1b[B")
        } else if key == "right" {
            Some(b"\x1b[C")
        } else if key == "left" {
            Some(b"\x1b[D")
        } else if modifiers.control || modifiers.platform {
            match key {
                "c" => Some(b"\x03"),
                "d" => Some(b"\x04"),
                "l" => Some(b"\x0c"),
                "a" => Some(b"\x01"),
                "e" => Some(b"\x05"),
                "u" => Some(b"\x15"),
                "k" => Some(b"\x0b"),
                "w" => Some(b"\x17"),
                _ => None,
            }
        } else if !modifiers.alt {
            event.keystroke.key_char.as_deref().map(str::as_bytes)
        } else {
            None
        };
        if let Some(bytes) = bytes {
            self.write_terminal_bytes(bytes, cx);
        }
        cx.stop_propagation();
    }

    fn terminal_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let live_output = self
            .terminal_session
            .as_ref()
            .and_then(|session| session.output.lock().ok().map(|output| output.clone()))
            .unwrap_or_default();
        div()
            .id("terminal-panel")
            .h(px(self.state.terminal_height as f32))
            .flex()
            .flex_col()
            .bg(theme::TOP_BAR)
            .border_t_1()
            .border_color(theme::DARK_BORDER)
            .on_click(cx.listener(|this, _, window, _| {
                this.state.terminal_focused = true;
                window.focus(&this.focus_handle);
            }))
            .on_key_down(cx.listener(Self::handle_terminal_key))
            .child(
                div()
                    .h(px(4.))
                    .w_full()
                    .cursor(CursorStyle::ResizeRow)
                    .bg(theme::DARK_BORDER)
                    .hover(|style| style.bg(theme::BLUE))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.resizing_terminal = true;
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .px_3()
                    .text_xs()
                    .text_color(theme::MUTED)
                    .child("TERMINAL")
                    .child(
                        div()
                            .id("system-terminal")
                            .ml_auto()
                            .px_2()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_color(theme::BLUE)
                            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                            .child("Open system terminal")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_system_terminal(cx);
                            })),
                    )
                    .child(div().ml_2().child("⌃` to toggle")),
            )
            .child(
                div()
                    .id("terminal-output")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_3()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(theme::TEXT)
                    .children(
                        self.state
                            .terminal_output
                            .iter()
                            .enumerate()
                            .map(|(index, line)| {
                                div().id(("terminal-line", index)).child(line.clone())
                            }),
                    )
                    .child(div().id("terminal-live-output").child(live_output)),
            )
            .child(
                div()
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .px_3()
                    .text_xs()
                    .text_color(theme::GREEN)
                    .child("❯ ")
                    .child("PTY connected · type directly into the shell"),
            )
    }

    fn close_active_tab(&mut self, _: &CloseActiveTab, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(message) = self.state.close_active() {
            self.state.status = message.to_owned();
        }
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.state.close_at(index) {
            Ok(()) => self.state.status = "Tab closed".to_owned(),
            Err(message) => self.state.status = message.to_owned(),
        }
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        self.state.next_tab();
        cx.notify();
    }

    fn refresh(&mut self, _: &RefreshWorkspace, _: &mut Window, cx: &mut Context<Self>) {
        self.reload();
        cx.notify();
    }

    fn save_document(&mut self, _: &SaveDocument, _: &mut Window, cx: &mut Context<Self>) {
        let workspace_id = self.snapshot.active_workspace.id.clone();
        let Some(document) = self.state.active_document() else {
            self.state.status = "No document is open".to_owned();
            cx.notify();
            return;
        };
        let relative_path = document.relative_path.clone();
        let contents = document.contents.clone();
        if relative_path.starts_with("plugin://") || relative_path.starts_with("settings://") {
            if let Some(document) = self.state.active_document_mut() {
                document.mark_saved();
            }
            self.state.status = format!("Saved editable draft {relative_path}");
            cx.notify();
            return;
        }
        match self
            .controllers
            .documents
            .write(&workspace_id, &relative_path, &contents)
        {
            Ok(()) => {
                if let Some(document) = self.state.active_document_mut() {
                    document.mark_saved();
                }
                self.state.status = format!("Saved {relative_path}");
            }
            Err(error) => self.state.status = format!("Save failed: {error}"),
        }
        cx.notify();
    }

    fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool, cx: &mut Context<Self>) {
        match self.controllers.plugins.set_enabled(plugin_id, enabled) {
            Ok(()) => {
                self.reload();
                self.state.status = if enabled {
                    "Plugin enabled".to_owned()
                } else {
                    "Plugin disabled".to_owned()
                };
            }
            Err(error) => self.state.status = format!("Plugin update failed: {error}"),
        }
        cx.notify();
    }

    fn cancel_execution(&mut self, job_id: polyglid_client::JobId, cx: &mut Context<Self>) {
        match self.controllers.executions.cancel(job_id) {
            Ok(()) => {
                self.reload();
                self.state.status = format!("Cancelled execution {job_id}");
            }
            Err(error) => self.state.status = format!("Cancellation failed: {error}"),
        }
        cx.notify();
    }

    fn copy_report_json(&mut self, report_id: &str, cx: &mut Context<Self>) {
        match self
            .controllers
            .reports
            .export(report_id, ReportFormat::Json)
        {
            Ok(report) => {
                cx.write_to_clipboard(ClipboardItem::new_string(report));
                self.state.status = "Report JSON copied to clipboard".to_owned();
            }
            Err(error) => self.state.status = format!("Report export failed: {error}"),
        }
        cx.notify();
    }

    fn open_entry(&mut self, entry: WorkspaceEntry, cx: &mut Context<Self>) {
        if entry.is_symlink {
            self.state.status = "Symlink entries are not opened".to_owned();
            cx.notify();
            return;
        }
        let workspace_id = self.snapshot.active_workspace.id.clone();
        if entry.is_directory {
            if self.expanded_directories.remove(&entry.relative_path) {
                self.state.status = "Folder collapsed".to_owned();
            } else {
                match self
                    .controllers
                    .documents
                    .list_directory(&workspace_id, &entry.relative_path)
                {
                    Ok(_) => {
                        self.expanded_directories.insert(entry.relative_path);
                        self.state.status = "Folder expanded".to_owned();
                    }
                    Err(error) => self.state.status = format!("Folder unavailable: {error}"),
                }
            }
        } else {
            match self
                .controllers
                .documents
                .read(&workspace_id, &entry.relative_path)
            {
                Ok(document) => {
                    let path = document.relative_path.clone();
                    self.state.open_document(document);
                    self.state.status = format!("Opened {path}");
                }
                Err(error) => self.state.status = format!("Open failed: {error}"),
            }
        }
        cx.notify();
    }

    fn open_parent_directory(&mut self, cx: &mut Context<Self>) {
        let parent = self
            .state
            .explorer_directory
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent)
            .to_owned();
        match self
            .controllers
            .documents
            .list_directory(&self.snapshot.active_workspace.id, &parent)
        {
            Ok(entries) => {
                self.explorer_entries = entries;
                self.state.explorer_directory = parent;
                self.state.status = "Opened parent directory".to_owned();
            }
            Err(error) => self.state.status = format!("Directory unavailable: {error}"),
        }
        cx.notify();
    }

    fn explorer_rows(&self) -> Vec<(WorkspaceEntry, usize)> {
        fn append(
            workbench: &Workbench,
            entries: &[WorkspaceEntry],
            depth: usize,
            rows: &mut Vec<(WorkspaceEntry, usize)>,
        ) {
            for entry in entries {
                rows.push((entry.clone(), depth));
                if entry.is_directory
                    && workbench
                        .expanded_directories
                        .contains(&entry.relative_path)
                {
                    if let Ok(children) = workbench.controllers.documents.list_directory(
                        &workbench.snapshot.active_workspace.id,
                        &entry.relative_path,
                    ) {
                        append(workbench, &children, depth + 1, rows);
                    }
                }
            }
        }

        let mut rows = Vec::new();
        append(self, &self.explorer_entries, 0, &mut rows);
        rows
    }

    fn handle_editor_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.command_palette_visible {
            return;
        }
        if self.state.bottom_panel_visible && self.state.terminal_focused {
            self.handle_terminal_key(event, window, cx);
            return;
        }
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform)
            && event.keystroke.key == "`"
        {
            self.toggle_terminal(cx);
            cx.stop_propagation();
            return;
        }
        let Some(document) = self.state.active_document_mut() else {
            return;
        };
        let key = event.keystroke.key.as_str();
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                document.insert(&text);
                let (line, column) = document.line_and_column();
                self.state.status = format!("Ln {line}, Col {column} · modified");
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        let changed = match key {
            "backspace" => {
                document.backspace();
                true
            }
            "delete" => {
                document.delete_forward();
                true
            }
            "left" => {
                document.move_left();
                false
            }
            "right" => {
                document.move_right();
                false
            }
            "up" => {
                document.move_up();
                false
            }
            "down" => {
                document.move_down();
                false
            }
            "home" => {
                document.move_home();
                false
            }
            "end" => {
                document.move_end();
                false
            }
            "enter" => {
                document.insert("\n");
                true
            }
            "tab" => {
                document.insert("    ");
                true
            }
            _ if !event.keystroke.modifiers.control
                && !event.keystroke.modifiers.platform
                && !event.keystroke.modifiers.alt =>
            {
                if let Some(text) = event.keystroke.key_char.as_deref() {
                    document.insert(text);
                    true
                } else {
                    return;
                }
            }
            _ => return,
        };
        let (line, column) = document.line_and_column();
        self.state.status = if changed {
            format!("Ln {line}, Col {column} · modified")
        } else {
            format!("Ln {line}, Col {column}")
        };
        cx.stop_propagation();
        cx.notify();
    }

    fn top_chip(label: impl Into<String>) -> Div {
        div()
            .h(px(24.))
            .flex()
            .items_center()
            .px_2()
            .rounded_sm()
            .bg(theme::CHIP)
            .border_1()
            .border_color(theme::CHIP_BORDER)
            .text_xs()
            .text_color(theme::MUTED)
            .child(label.into())
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> Div {
        let workspace_name = self.workspace_label();
        let root_path = self.snapshot.active_workspace.root_path.clone();
        let file_label = self.state.active_document().map_or_else(
            || "No file open".to_owned(),
            |document| document.relative_path.clone(),
        );
        div()
            .h(px(31.))
            .w_full()
            .flex()
            .items_center()
            .bg(theme::TOP_BAR)
            .border_b_1()
            .border_color(theme::DARK_BORDER)
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .child(
                        div()
                            .id("menu")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(theme::MUTED)
                            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                            .child("☰")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_app_menu(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("workspace-selector")
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .rounded_sm()
                            .bg(theme::CHIP)
                            .border_1()
                            .border_color(theme::CHIP_BORDER)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(theme::MUTED)
                            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                            .child(workspace_name)
                            .child("⌄")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_workspace_menu(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("explorer-toggle")
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .rounded_sm()
                            .bg(if self.state.sidebar_visible {
                                theme::CHIP
                            } else {
                                theme::TOP_BAR
                            })
                            .border_1()
                            .border_color(theme::CHIP_BORDER)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if self.state.sidebar_visible {
                                theme::TEXT
                            } else {
                                theme::MUTED
                            })
                            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                            .child("Explorer")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.sidebar_visible = !this.state.sidebar_visible;
                                cx.notify();
                            })),
                    )
                    .child(Self::top_chip(file_label))
                    .child(Self::top_chip(root_path)),
            )
            .child(
                div()
                    .id("window-drag-region")
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_3()
                    .px_3()
                    .cursor(CursorStyle::Arrow)
                    .text_xs()
                    .text_color(theme::MUTED)
                    .child(
                        div()
                            .size(px(19.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(theme::CHIP)
                            .child("P"),
                    )
                    .child("Security Workspace")
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    }),
            )
    }

    fn menu_item(
        id: &'static str,
        label: &'static str,
        shortcut: &'static str,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_sm()
            .cursor_pointer()
            .text_sm()
            .text_color(theme::TEXT)
            .hover(|style| style.bg(theme::CHIP))
            .child(label)
            .child(div().text_xs().text_color(theme::MUTED).child(shortcut))
            .on_click(cx.listener(move |this, _, window, cx| {
                on_click(this, window, cx);
                this.state.app_menu_visible = false;
                cx.notify();
            }))
    }

    fn app_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("app-menu")
            .absolute()
            .top(px(35.))
            .left(px(8.))
            .w(px(250.))
            .p_2()
            .rounded_md()
            .bg(theme::EXPLORER)
            .border_1()
            .border_color(theme::CHIP_BORDER)
            .shadow_lg()
            .child(Self::menu_item(
                "menu-save",
                "Save",
                "Ctrl S",
                cx,
                |this, window, cx| {
                    this.save_document(&SaveDocument, window, cx);
                },
            ))
            .child(Self::menu_item(
                "menu-refresh",
                "Refresh workspace",
                "Ctrl R",
                cx,
                |this, _, cx| {
                    this.reload();
                    cx.notify();
                },
            ))
    }

    fn workspace_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("workspace-menu")
            .absolute()
            .top(px(35.))
            .left(px(38.))
            .w(px(300.))
            .p_2()
            .rounded_md()
            .bg(theme::EXPLORER)
            .border_1()
            .border_color(theme::CHIP_BORDER)
            .shadow_lg()
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .text_xs()
                    .text_color(theme::MUTED)
                    .child("WORKSPACES"),
            )
            .child(
                div()
                    .id("workspace-open-repository")
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(theme::TEXT)
                    .hover(|style| style.bg(theme::CHIP))
                    .child("Open Repository…")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_repository(cx);
                        this.state.workspace_menu_visible = false;
                    })),
            )
            .children(self.snapshot.workspaces.iter().cloned().enumerate().map(
                |(index, workspace)| {
                    let active = workspace.id == self.snapshot.active_workspace.id;
                    let workspace_id = workspace.id.clone();
                    div()
                        .id(("workspace-option", index))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .py_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_sm()
                        .text_color(if active { theme::BLUE } else { theme::TEXT })
                        .hover(|style| style.bg(theme::CHIP))
                        .child(workspace.name)
                        .child(if active { "Active" } else { "Open" })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_workspace(&workspace_id, cx);
                        }))
                },
            ))
    }

    fn entry_color(entry: &WorkspaceEntry) -> gpui::Rgba {
        if entry.is_directory {
            return if entry.name == "crates" {
                theme::GREEN
            } else if entry.name == "docs" {
                theme::ORANGE
            } else if entry.name == "apps" {
                theme::PINK
            } else {
                theme::EXPLORER_TEXT
            };
        }
        match entry.name.rsplit('.').next().unwrap_or_default() {
            "rs" => theme::ORANGE,
            "md" => theme::BLUE,
            "toml" | "lock" => theme::ORANGE,
            "json" => theme::YELLOW,
            _ => theme::MUTED,
        }
    }

    fn explorer(&self, cx: &mut Context<Self>) -> Div {
        let workspace_name = self.workspace_label();
        let rows = self.explorer_rows();
        let is_empty = rows.is_empty();
        div()
            .w(px(self.state.sidebar_width as f32))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme::EXPLORER)
            .border_l_1()
            .border_color(theme::DARK_BORDER)
            .child(
                div()
                    .h(px(56.))
                    .flex_col()
                    .justify_center()
                    .border_b_1()
                    .border_color(theme::DARK_BORDER)
                    .child(
                        div()
                            .h(px(27.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme::PINK)
                            .child("EXPLORER")
                            .child(
                                div()
                                    .text_color(theme::MUTED)
                                    .child(format!("{} items", self.explorer_entries.len())),
                            )
                            .child(
                                div()
                                    .id("explorer-refresh")
                                    .ml_auto()
                                    .px_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_color(theme::MUTED)
                                    .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                                    .child("↻")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reload();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("explorer-close")
                                    .px_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_color(theme::MUTED)
                                    .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                                    .child("×")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.sidebar_visible = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .px_3()
                            .text_xs()
                            .text_color(theme::EXPLORER_TEXT)
                            .child(format!("{}  /", workspace_name)),
                    ),
            )
            .child(
                div()
                    .id("explorer-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_2()
                    .children(rows.into_iter().enumerate().map(|(index, (entry, depth))| {
                        let color = Self::entry_color(&entry);
                        let label = entry.name.clone();
                        let entry_to_open = entry.clone();
                        let active = self
                            .state
                            .active_document()
                            .is_some_and(|document| document.relative_path == entry.relative_path);
                        div()
                            .id(("explorer-entry", index))
                            .h(px(26.))
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .pl(px(8. + depth as f32 * 14.))
                            .pr_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if active { theme::CHIP } else { theme::EXPLORER })
                            .text_color(color)
                            .hover(|style| style.bg(theme::CHIP))
                            .child(if entry.is_directory { "▱" } else { "◇" })
                            .child(label)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_entry(entry_to_open.clone(), cx);
                                window.focus(&this.focus_handle);
                            }))
                    }))
                    .when(is_empty, |explorer| {
                        explorer.child(
                            div()
                                .p_3()
                                .text_xs()
                                .text_color(theme::MUTED)
                                .child("No entries in this workspace")
                                .child(
                                    div()
                                        .id("refresh-empty-explorer")
                                        .mt_2()
                                        .cursor_pointer()
                                        .text_color(theme::BLUE)
                                        .child("Refresh")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.reload();
                                            cx.notify();
                                        })),
                                ),
                        )
                    }),
            )
    }

    fn action_button(label: impl Into<String>) -> Div {
        div()
            .h(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .px_3()
            .rounded_sm()
            .bg(theme::CHIP)
            .border_1()
            .border_color(theme::CHIP_BORDER)
            .cursor_pointer()
            .text_xs()
            .text_color(theme::TEXT)
            .hover(|style| style.bg(theme::BLUE))
            .child(label.into())
    }

    fn dashboard(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.state.active_view.title();
        let content = div().mt_4().flex().flex_col().gap_2();
        let (description, content) = match self.state.active_view {
            WorkspaceView::Projects => {
                let empty = self.snapshot.projects.is_empty();
                let content = content.children(self.snapshot.projects.iter().enumerate().map(
                    |(index, project)| {
                        div()
                            .id(("project-card", index))
                            .p_3()
                            .rounded_sm()
                            .bg(theme::EXPLORER)
                            .border_1()
                            .border_color(theme::CHIP_BORDER)
                            .text_sm()
                            .child(project.name.clone())
                            .child(
                                div()
                                    .mt_1()
                                    .text_xs()
                                    .text_color(theme::MUTED)
                                    .child(format!("{} · {}", project.kind, project.path)),
                            )
                    },
                ));
                (
                    "Projects discovered in the active workspace. Project creation and removal are disabled until their confirmation form is implemented.",
                    content.when(empty, |content| content.child(Self::empty_state("No projects found"))),
                )
            }
            WorkspaceView::Scanner => {
                let empty = self.snapshot.targets.is_empty();
                let content =
                    content.children(self.snapshot.targets.iter().enumerate().map(
                        |(index, target)| {
                            div()
                                .id(("target-card", index))
                                .p_3()
                                .rounded_sm()
                                .bg(theme::EXPLORER)
                                .border_1()
                                .border_color(theme::CHIP_BORDER)
                                .text_sm()
                                .child(target.name.clone())
                                .child(div().mt_1().text_xs().text_color(theme::MUTED).child(
                                    target.group.clone().unwrap_or_else(|| "Ungrouped".into()),
                                ))
                        },
                    ));
                (
                    "Saved scan targets. Starting a scan is disabled until target, plugin, and permission selection are available.",
                    content.when(empty, |content| content.child(Self::empty_state("No saved targets"))),
                )
            }
            WorkspaceView::Executions => {
                let empty = self.snapshot.executions.is_empty();
                let content =
                    content.children(self.snapshot.executions.iter().cloned().enumerate().map(
                        |(index, execution)| {
                            let is_active = !execution.state.is_terminal();
                            let job_id = execution.id;
                            div()
                                .id(("execution-card", index))
                                .p_3()
                                .flex()
                                .items_center()
                                .justify_between()
                                .rounded_sm()
                                .bg(theme::EXPLORER)
                                .border_1()
                                .border_color(theme::CHIP_BORDER)
                                .child(
                                    div().text_sm().child(execution.target).child(
                                        div().mt_1().text_xs().text_color(theme::MUTED).child(
                                            format!(
                                                "{} · {}",
                                                execution.plugin_id, execution.state
                                            ),
                                        ),
                                    ),
                                )
                                .when(is_active, |card| {
                                    card.child(
                                        Self::action_button("Cancel")
                                            .id(("cancel-execution", index))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.cancel_execution(job_id, cx)
                                            })),
                                    )
                                })
                        },
                    ));
                (
                    "Execution history. Running and queued jobs can be cancelled here.",
                    content.when(empty, |content| {
                        content.child(Self::empty_state("No executions yet"))
                    }),
                )
            }
            WorkspaceView::Reports => {
                let empty = self.snapshot.reports.is_empty();
                let content =
                    content.children(self.snapshot.reports.iter().cloned().enumerate().map(
                        |(index, report)| {
                            let report_id = report.id.clone();
                            div()
                                .id(("report-card", index))
                                .p_3()
                                .flex()
                                .items_center()
                                .justify_between()
                                .rounded_sm()
                                .bg(theme::EXPLORER)
                                .border_1()
                                .border_color(theme::CHIP_BORDER)
                                .child(
                                    div().text_sm().child(report.target).child(
                                        div().mt_1().text_xs().text_color(theme::MUTED).child(
                                            format!(
                                                "{} issues · {}",
                                                report.issues.len(),
                                                report.summary
                                            ),
                                        ),
                                    ),
                                )
                                .child(
                                    Self::action_button("Copy JSON")
                                        .id(("copy-report", index))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.copy_report_json(&report_id, cx)
                                        })),
                                )
                        },
                    ));
                (
                    "Generated security reports. Copy a complete JSON export to the clipboard.",
                    content.when(empty, |content| {
                        content.child(Self::empty_state("No reports yet"))
                    }),
                )
            }
            WorkspaceView::Plugins => {
                let empty = self.snapshot.plugins.is_empty();
                let content =
                    content.children(self.snapshot.plugins.iter().cloned().enumerate().map(
                        |(index, plugin)| {
                            let plugin_id = plugin.id.clone();
                            let plugin_tab_id = plugin_id.clone();
                            let enabled = plugin.status == PluginStatus::Enabled;
                            let can_toggle = matches!(
                                plugin.status,
                                PluginStatus::Enabled | PluginStatus::Disabled
                            );
                            div()
                                .id(("plugin-card", index))
                                .p_3()
                                .flex()
                                .items_center()
                                .justify_between()
                                .rounded_sm()
                                .bg(theme::EXPLORER)
                                .border_1()
                                .border_color(theme::CHIP_BORDER)
                                .child(
                                    div()
                                        .text_sm()
                                        .child(format!("{} · v{}", plugin.name, plugin.version))
                                        .child(
                                            div().mt_1().text_xs().text_color(theme::MUTED).child(
                                                format!("{} · {}", plugin.author, plugin.status),
                                            ),
                                        ),
                                )
                                .when(can_toggle, |card| {
                                    card.child(
                                        Self::action_button(if enabled {
                                            "Disable"
                                        } else {
                                            "Enable"
                                        })
                                        .id(("toggle-plugin", index))
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.set_plugin_enabled(&plugin_id, !enabled, cx)
                                            }),
                                        ),
                                    )
                                })
                                .when(!can_toggle, |card| {
                                    card.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::MUTED)
                                            .child("Unavailable"),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_plugin_tab(plugin_tab_id.clone(), cx);
                                }))
                        },
                    ));
                (
                    "Installed WASM security plugins. Enable or disable valid plugins here; installation is disabled until permission review is implemented.",
                    content.when(empty, |content| content.child(Self::empty_state("No plugins installed"))),
                )
            }
        };

        div()
            .id("dashboard-scroll")
            .flex_1()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(title),
            )
            .child(
                div()
                    .mt_2()
                    .text_sm()
                    .text_color(theme::MUTED)
                    .child(description),
            )
            .child(content)
    }

    fn empty_state(message: &'static str) -> Div {
        div()
            .p_3()
            .rounded_sm()
            .border_1()
            .border_color(theme::CHIP_BORDER)
            .text_sm()
            .text_color(theme::MUTED)
            .child(message)
    }

    fn editor_canvas(&self, cx: &mut Context<Self>) -> Div {
        let tabs = self.state.documents.clone();
        let active_index = self.state.active_document;
        let active_document = self.state.active_document().cloned();
        let mut canvas = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme::EDITOR)
            .overflow_hidden()
            .child(
                div()
                    .h(px(34.))
                    .w_full()
                    .flex()
                    .items_center()
                    .bg(theme::TOP_BAR)
                    .border_b_1()
                    .border_color(theme::DARK_BORDER)
                    .when(self.state.settings_visible, |tabs| {
                        let label = if self.state.settings_visible {
                            "Settings"
                        } else {
                            self.state.active_view.title()
                        };
                        tabs.child(
                            div()
                                .id("special-editor-tab")
                                .h_full()
                                .flex()
                                .items_center()
                                .px_3()
                                .cursor_pointer()
                                .text_xs()
                                .text_color(theme::TEXT)
                                .bg(theme::EDITOR)
                                .border_r_1()
                                .border_color(theme::DARK_BORDER)
                                .child(label)
                                .child(
                                    div()
                                        .id("special-editor-tab-close")
                                        .ml_1()
                                        .px_1()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_color(theme::MUTED)
                                        .hover(|style| {
                                            style.bg(theme::CHIP).text_color(theme::TEXT)
                                        })
                                        .child("×")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.close_special_tab(cx);
                                        })),
                                ),
                        )
                    })
                    .children(tabs.into_iter().enumerate().map(|(index, document)| {
                        let active = active_index == Some(index);
                        let label = if document.is_dirty() {
                            format!("● {}", document.title())
                        } else {
                            document.title().to_owned()
                        };
                        div()
                            .id(("document-tab", index))
                            .h_full()
                            .flex()
                            .items_center()
                            .px_3()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if active { theme::TEXT } else { theme::MUTED })
                            .bg(if active {
                                theme::EDITOR
                            } else {
                                theme::TOP_BAR
                            })
                            .border_r_1()
                            .border_color(theme::DARK_BORDER)
                            .child(label)
                            .child(
                                div()
                                    .id(("document-tab-close", index))
                                    .ml_1()
                                    .px_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_color(theme::MUTED)
                                    .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                                    .child("×")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.close_tab(index, cx);
                                    })),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.state.active_document = Some(index);
                                this.state.status = document.relative_path.clone();
                                window.focus(&this.focus_handle);
                                cx.notify();
                            }))
                    })),
            );

        canvas = if let Some(document) = active_document {
            let (cursor_line, cursor_column) = document.line_and_column();
            let lines: Vec<String> = document
                .contents
                .split('\n')
                .enumerate()
                .map(|(index, line)| {
                    if index + 1 != cursor_line {
                        return line.to_owned();
                    }
                    let byte_offset = line
                        .char_indices()
                        .nth(cursor_column.saturating_sub(1))
                        .map_or(line.len(), |(offset, _)| offset);
                    format!("{}│{}", &line[..byte_offset], &line[byte_offset..])
                })
                .collect();
            canvas.child(
                div()
                    .id("code-editor")
                    .flex_1()
                    .w_full()
                    .overflow_scroll()
                    .p_3()
                    .font_family("monospace")
                    .text_sm()
                    .line_height(px(21.))
                    .cursor(CursorStyle::IBeam)
                    .on_click(cx.listener(|this, _, window, _| {
                        this.state.terminal_focused = false;
                        window.focus(&this.focus_handle);
                    }))
                    .children(lines.into_iter().enumerate().map(|(index, line)| {
                        div()
                            .id(("editor-line", index))
                            .min_w(px(800.))
                            .flex()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(document) = this.state.active_document_mut() {
                                    document.move_to_line(index);
                                }
                                window.focus(&this.focus_handle);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(48.))
                                    .text_color(theme::MUTED)
                                    .child(format!("{:>4}", index + 1)),
                            )
                            .child(div().text_color(theme::TEXT).child(line))
                    })),
            )
        } else {
            canvas.child(self.dashboard(cx))
        };

        canvas.when(self.state.bottom_panel_visible, |canvas| {
            canvas.child(self.terminal_panel(cx))
        })
    }

    fn sidebar_button(
        id: &'static str,
        glyph: &'static str,
        label: &'static str,
        active: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(34.))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .rounded_sm()
            .cursor_pointer()
            .text_sm()
            .text_color(if active { theme::BLUE } else { theme::MUTED })
            .bg(if active { theme::CHIP } else { theme::EXPLORER })
            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
            .child(div().w(px(18.)).text_center().child(glyph))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    fn activity_sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(172.))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .bg(theme::EXPLORER)
            .border_r_1()
            .border_color(theme::DARK_BORDER)
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .child(
                        div()
                            .px_2()
                            .pb_2()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme::MUTED)
                            .child("WORKBENCH"),
                    )
                    .child(Self::sidebar_button(
                        "toggle-explorer",
                        "▣",
                        "Explorer",
                        self.state.sidebar_visible,
                        cx,
                        |this, cx| {
                            this.state.sidebar_visible = !this.state.sidebar_visible;
                            cx.notify();
                        },
                    ))
                    .child(Self::sidebar_button(
                        "search",
                        "⌕",
                        "Commands",
                        self.state.command_palette_visible,
                        cx,
                        |this, cx| {
                            this.state.command_palette_visible = true;
                            cx.notify();
                        },
                    ))
                    .child(Self::sidebar_button(
                        "refresh",
                        "↻",
                        "Refresh",
                        false,
                        cx,
                        |this, cx| {
                            this.reload();
                            cx.notify();
                        },
                    ))
                    .child(div().h(px(1.)).w_full().my_2().bg(theme::DARK_BORDER))
                    .children(
                        WorkspaceView::ALL
                            .into_iter()
                            .enumerate()
                            .map(|(index, view)| {
                                let active = self.state.active_view == view;
                                div()
                                    .id(("product-view", index))
                                    .h(px(34.))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_sm()
                                    .text_color(if active { theme::BLUE } else { theme::MUTED })
                                    .bg(if active { theme::CHIP } else { theme::EXPLORER })
                                    .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                                    .child(div().w(px(18.)).text_center().child(view.glyph()))
                                    .child(view.title())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.activate(view, cx);
                                    }))
                            }),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p_2()
                    .child(
                        div()
                            .mb_2()
                            .px_2()
                            .text_xs()
                            .text_color(theme::MUTED)
                            .child(self.state.status.clone()),
                    )
                    .child(Self::sidebar_button(
                        "toggle-panel",
                        "▤",
                        "Output",
                        self.state.bottom_panel_visible,
                        cx,
                        |this, cx| {
                            this.state.bottom_panel_visible = !this.state.bottom_panel_visible;
                            cx.notify();
                        },
                    )),
            )
    }

    fn activity_rail(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(31.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_between()
            .py_2()
            .bg(theme::TOP_BAR)
            .border_r_1()
            .border_color(theme::DARK_BORDER)
            .child(
                div().flex().flex_col().items_center().gap_1().children(
                    WorkspaceView::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, view)| {
                            div()
                                .id(("activity-rail", index))
                                .w(px(26.))
                                .h(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .cursor_pointer()
                                .text_base()
                                .text_color(if self.state.active_view == view {
                                    theme::BLUE
                                } else {
                                    theme::MUTED
                                })
                                .bg(if self.state.active_view == view {
                                    theme::CHIP
                                } else {
                                    theme::TOP_BAR
                                })
                                .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                                .child(view.glyph())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.activate(view, cx);
                                }))
                        }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("activity-terminal")
                            .w(px(26.))
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if self.state.bottom_panel_visible {
                                theme::BLUE
                            } else {
                                theme::MUTED
                            })
                            .bg(if self.state.bottom_panel_visible {
                                theme::CHIP
                            } else {
                                theme::TOP_BAR
                            })
                            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                            .child(">_")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_terminal(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("activity-settings")
                            .w(px(26.))
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_base()
                            .text_color(if self.state.settings_visible {
                                theme::BLUE
                            } else {
                                theme::MUTED
                            })
                            .bg(if self.state.settings_visible {
                                theme::CHIP
                            } else {
                                theme::TOP_BAR
                            })
                            .hover(|style| style.bg(theme::CHIP).text_color(theme::TEXT))
                            .child("⚙")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_settings(cx);
                            })),
                    ),
            )
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let preferences = polyglid_client::ShellPreferences {
            sidebar_visible: self.state.sidebar_visible,
            bottom_panel_visible: self.state.bottom_panel_visible,
            sidebar_width: self.state.sidebar_width,
            bottom_panel_height: self.state.terminal_height,
        };
        match self.controllers.settings.save_shell(&preferences) {
            Ok(()) => self.state.status = "Settings saved".to_owned(),
            Err(error) => self.state.status = format!("Settings save failed: {error}"),
        }
        cx.notify();
    }

    fn settings_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-editor")
            .flex_1()
            .h_full()
            .p_8()
            .bg(theme::EXPLORER)
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_2xl().child("Settings"))
                    .child(
                        div()
                            .id("settings-close-editor")
                            .px_3()
                            .py_2()
                            .rounded_sm()
                            .rounded_sm()
                            .cursor_pointer()
                            .bg(theme::CHIP)
                            .text_color(theme::TEXT)
                            .hover(|style| style.bg(theme::BLUE))
                            .child("Back to editor")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.settings_visible = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .mt_4()
                    .flex_col()
                    .gap_2()
                    .text_sm()
                    .child(Self::empty_state("Workspace and editor preferences"))
                    .child(
                        div()
                            .p_3()
                            .rounded_sm()
                            .bg(theme::TOP_BAR)
                            .border_1()
                            .border_color(theme::CHIP_BORDER)
                            .child(format!(
                                "[workspace]\nroot = \"{}\"\n\n[editor]\nexplorer = {}\n\n[terminal]\nenabled = {}\nshell = \"{}\"",
                                self.snapshot.active_workspace.root_path,
                                self.state.sidebar_visible,
                                self.state.bottom_panel_visible,
                                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("settings-explorer-toggle")
                                    .px_3()
                                    .py_2()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .bg(theme::CHIP)
                                    .text_color(theme::TEXT)
                                    .child(format!("Explorer: {}", self.state.sidebar_visible))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.sidebar_visible = !this.state.sidebar_visible;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("settings-terminal-toggle")
                                    .px_3()
                                    .py_2()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .bg(theme::CHIP)
                                    .text_color(theme::TEXT)
                                    .child(format!(
                                        "Terminal panel: {}",
                                        self.state.bottom_panel_visible
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.bottom_panel_visible =
                                            !this.state.bottom_panel_visible;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-save")
                            .mt_2()
                            .px_3()
                            .py_2()
                            .w(px(130.))
                            .rounded_sm()
                            .cursor_pointer()
                            .bg(theme::BLUE)
                            .text_color(theme::EDITOR)
                            .child("Save settings")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_settings(cx);
                            })),
                    ),
            )
    }

    fn command_palette(&self, cx: &mut Context<Self>) -> Div {
        let commands = [
            (WorkspaceView::Projects, "Open Projects", "Ctrl 1"),
            (WorkspaceView::Scanner, "Open Scanner", "Ctrl 2"),
            (WorkspaceView::Executions, "Open Executions", "Ctrl 3"),
            (WorkspaceView::Reports, "Open Reports", "Ctrl 4"),
            (WorkspaceView::Plugins, "Open Plugins", "Ctrl 5"),
        ];
        div()
            .absolute()
            .top(px(64.))
            .left(px(320.))
            .w(px(560.))
            .p_2()
            .rounded_md()
            .bg(theme::EXPLORER)
            .border_1()
            .border_color(theme::CHIP_BORDER)
            .shadow_lg()
            .child(
                div()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .px_3()
                    .rounded_sm()
                    .bg(theme::TOP_BAR)
                    .text_sm()
                    .text_color(theme::MUTED)
                    .child("Search commands…"),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .flex_col()
                    .children(commands.into_iter().enumerate().map(
                        |(index, (view, label, shortcut))| {
                            div()
                                .id(("palette-command", index))
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_3()
                                .py_2()
                                .rounded_sm()
                                .cursor_pointer()
                                .text_sm()
                                .text_color(theme::TEXT)
                                .hover(|style| style.bg(theme::CHIP))
                                .child(label)
                                .child(div().text_xs().text_color(theme::MUTED).child(shortcut))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.activate(view, cx);
                                }))
                        },
                    )),
            )
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.initial_focus_requested {
            window.focus(&self.focus_handle);
            self.initial_focus_requested = true;
        }
        if self.state.bottom_panel_visible && self.terminal_session.is_some() {
            window.request_animation_frame();
        }
        div()
            .id("polyglid-workbench")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_palette))
            .on_action(cx.listener(Self::close_overlay))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_bottom_panel))
            .on_action(cx.listener(Self::open_projects))
            .on_action(cx.listener(Self::open_scanner))
            .on_action(cx.listener(Self::open_executions))
            .on_action(cx.listener(Self::open_reports))
            .on_action(cx.listener(Self::open_plugins))
            .on_action(cx.listener(Self::close_active_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::save_document))
            .on_action(cx.listener(Self::refresh))
            .on_key_down(cx.listener(Self::handle_editor_key))
            .on_mouse_move(cx.listener(Self::resize_panels))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::stop_resizing))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::EDITOR)
            .text_color(theme::TEXT)
            .child(self.top_bar(cx))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .overflow_hidden()
                    .child(self.activity_rail(cx))
                    .when(!self.state.settings_visible, |body| {
                        body.child(self.editor_canvas(cx))
                    })
                    .when(self.state.settings_visible, |body| {
                        body.child(self.settings_editor(cx))
                    })
                    .when(self.state.sidebar_visible, |body| {
                        body.child(
                            div()
                                .w(px(4.))
                                .h_full()
                                .cursor(CursorStyle::ResizeColumn)
                                .bg(theme::DARK_BORDER)
                                .hover(|style| style.bg(theme::BLUE))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.state.resizing_sidebar = true;
                                        cx.notify();
                                    }),
                                ),
                        )
                        .child(self.explorer(cx))
                    }),
            )
            .when(self.state.app_menu_visible, |root| {
                root.child(self.app_menu(cx))
            })
            .when(self.state.workspace_menu_visible, |root| {
                root.child(self.workspace_menu(cx))
            })
            .when(self.state.command_palette_visible, |root| {
                root.child(self.command_palette(cx))
            })
    }
}
