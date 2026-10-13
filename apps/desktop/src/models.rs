use polyglid_client::{Document, ShellPreferences};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceView {
    Projects,
    Scanner,
    Executions,
    Reports,
    Plugins,
}

impl WorkspaceView {
    pub const ALL: [Self; 5] = [
        Self::Projects,
        Self::Scanner,
        Self::Executions,
        Self::Reports,
        Self::Plugins,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Projects => "Projects",
            Self::Scanner => "Scanner",
            Self::Executions => "Executions",
            Self::Reports => "Reports",
            Self::Plugins => "Plugins",
        }
    }

    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Projects => "P",
            Self::Scanner => "S",
            Self::Executions => "E",
            Self::Reports => "R",
            Self::Plugins => "X",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenDocument {
    pub relative_path: String,
    pub contents: String,
    saved_contents: String,
    pub cursor: usize,
}

impl OpenDocument {
    pub fn from_document(document: Document) -> Self {
        Self {
            relative_path: document.relative_path,
            saved_contents: document.contents.clone(),
            contents: document.contents,
            cursor: 0,
        }
    }

    pub fn title(&self) -> &str {
        self.relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.relative_path)
    }

    pub fn is_dirty(&self) -> bool {
        self.contents != self.saved_contents
    }

    pub fn mark_saved(&mut self) {
        self.saved_contents.clone_from(&self.contents);
    }

    pub fn insert(&mut self, text: &str) {
        self.contents.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    pub fn backspace(&mut self) {
        let Some(previous) = self.contents[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(offset, _)| offset)
        else {
            return;
        };
        self.contents.replace_range(previous..self.cursor, "");
        self.cursor = previous;
    }

    pub fn delete_forward(&mut self) {
        let Some(character) = self.contents[self.cursor..].chars().next() else {
            return;
        };
        self.contents
            .replace_range(self.cursor..self.cursor + character.len_utf8(), "");
    }

    pub fn move_left(&mut self) {
        if let Some(previous) = self.contents[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(offset, _)| offset)
        {
            self.cursor = previous;
        }
    }

    pub fn move_right(&mut self) {
        if let Some(character) = self.contents[self.cursor..].chars().next() {
            self.cursor += character.len_utf8();
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = self.contents[..self.cursor]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
    }

    pub fn move_end(&mut self) {
        self.cursor += self.contents[self.cursor..]
            .find('\n')
            .unwrap_or(self.contents.len() - self.cursor);
    }

    pub fn move_up(&mut self) {
        let current_start = self.contents[..self.cursor]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        if current_start == 0 {
            return;
        }
        let column = self.contents[current_start..self.cursor].chars().count();
        let previous_end = current_start - 1;
        let previous_start = self.contents[..previous_end]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        self.cursor = Self::byte_at_column(&self.contents, previous_start, previous_end, column);
    }

    pub fn move_down(&mut self) {
        let current_start = self.contents[..self.cursor]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        let column = self.contents[current_start..self.cursor].chars().count();
        let Some(current_end_delta) = self.contents[self.cursor..].find('\n') else {
            return;
        };
        let next_start = self.cursor + current_end_delta + 1;
        let next_end = self.contents[next_start..]
            .find('\n')
            .map_or(self.contents.len(), |offset| next_start + offset);
        self.cursor = Self::byte_at_column(&self.contents, next_start, next_end, column);
    }

    pub fn move_to_line(&mut self, line_index: usize) {
        self.cursor = self
            .contents
            .split_inclusive('\n')
            .take(line_index)
            .map(str::len)
            .sum::<usize>()
            .min(self.contents.len());
    }

    fn byte_at_column(contents: &str, start: usize, end: usize, column: usize) -> usize {
        contents[start..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(offset, _)| start + offset)
    }

    pub fn line_and_column(&self) -> (usize, usize) {
        let before = &self.contents[..self.cursor];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before
            .rsplit('\n')
            .next()
            .map_or(1, |value| value.chars().count() + 1);
        (line, column)
    }
}

#[derive(Clone, Debug)]
pub struct WorkbenchState {
    pub active_view: WorkspaceView,
    pub sidebar_visible: bool,
    pub bottom_panel_visible: bool,
    pub command_palette_visible: bool,
    pub app_menu_visible: bool,
    pub workspace_menu_visible: bool,
    pub settings_visible: bool,
    pub terminal_output: Vec<String>,
    pub terminal_focused: bool,
    pub sidebar_width: f64,
    pub terminal_height: f64,
    pub resizing_sidebar: bool,
    pub resizing_terminal: bool,
    pub status: String,
    pub explorer_directory: String,
    pub documents: Vec<OpenDocument>,
    pub active_document: Option<usize>,
}

impl WorkbenchState {
    pub fn new(shell: &ShellPreferences) -> Self {
        Self {
            active_view: WorkspaceView::Projects,
            sidebar_visible: shell.sidebar_visible,
            bottom_panel_visible: false,
            command_palette_visible: false,
            app_menu_visible: false,
            workspace_menu_visible: false,
            settings_visible: false,
            terminal_output: Vec::new(),
            terminal_focused: false,
            sidebar_width: shell.sidebar_width.clamp(180.0, 520.0),
            terminal_height: shell.bottom_panel_height.clamp(120.0, 520.0),
            resizing_sidebar: false,
            resizing_terminal: false,
            status: "Ready".to_owned(),
            explorer_directory: String::new(),
            documents: Vec::new(),
            active_document: None,
        }
    }

    pub fn activate(&mut self, view: WorkspaceView) {
        self.active_view = view;
        self.active_document = None;
        self.command_palette_visible = false;
    }

    pub fn open_document(&mut self, document: Document) {
        if let Some(index) = self
            .documents
            .iter()
            .position(|open| open.relative_path == document.relative_path)
        {
            self.active_document = Some(index);
            return;
        }
        self.documents.push(OpenDocument::from_document(document));
        self.active_document = Some(self.documents.len() - 1);
    }

    pub fn active_document(&self) -> Option<&OpenDocument> {
        self.active_document
            .and_then(|index| self.documents.get(index))
    }

    pub fn active_document_mut(&mut self) -> Option<&mut OpenDocument> {
        self.active_document
            .and_then(|index| self.documents.get_mut(index))
    }

    pub fn close_active(&mut self) -> Result<(), &'static str> {
        let Some(index) = self.active_document else {
            return Ok(());
        };
        self.close_at(index)
    }

    pub fn close_at(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.documents.len() {
            return Err("Tab no longer exists");
        }
        if self
            .documents
            .get(index)
            .is_some_and(OpenDocument::is_dirty)
        {
            return Err("Save the document before closing it");
        }
        self.documents.remove(index);
        self.active_document = if self.documents.is_empty() {
            None
        } else {
            Some(
                self.active_document
                    .unwrap_or(index)
                    .min(self.documents.len() - 1),
            )
        };
        Ok(())
    }

    pub fn next_tab(&mut self) {
        if self.documents.is_empty() {
            return;
        }
        self.active_document = Some(
            self.active_document
                .map_or(0, |index| (index + 1) % self.documents.len()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(contents: &str) -> Document {
        Document {
            relative_path: "src/main.rs".to_owned(),
            contents: contents.to_owned(),
            byte_size: contents.len() as u64,
        }
    }

    #[test]
    fn editing_tracks_dirty_and_saved_state() {
        let mut open = OpenDocument::from_document(document("fn main() {}"));
        open.move_end();
        open.insert("\n");
        assert!(open.is_dirty());
        open.mark_saved();
        assert!(!open.is_dirty());
    }

    #[test]
    fn unicode_backspace_uses_character_boundaries() {
        let mut open = OpenDocument::from_document(document("aλ"));
        open.move_end();
        open.backspace();
        assert_eq!(open.contents, "a");
        assert_eq!(open.cursor, 1);
    }

    #[test]
    fn vertical_movement_preserves_the_nearest_column() {
        let mut open = OpenDocument::from_document(document("alpha\nxy\nomega"));
        open.move_to_line(2);
        open.move_right();
        open.move_right();
        open.move_right();
        open.move_up();
        assert_eq!(open.line_and_column(), (2, 3));
        open.move_up();
        assert_eq!(open.line_and_column(), (1, 3));
        open.move_down();
        assert_eq!(open.line_and_column(), (2, 3));
    }

    #[test]
    fn dirty_documents_are_not_closed() {
        let mut state = WorkbenchState::new(&ShellPreferences::default());
        state.open_document(document("before"));
        state
            .active_document_mut()
            .expect("document")
            .insert("after");
        assert!(state.close_active().is_err());
        assert_eq!(state.documents.len(), 1);
    }
}
