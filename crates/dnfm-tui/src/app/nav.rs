//! Navegação e seleção de linhas/grupos.

use super::*;

impl App {
    pub fn selected_index(&self) -> usize {
        let len = self.rows_len();
        if len == 0 {
            return 0;
        }
        self.table.selected().unwrap_or(0).min(len - 1)
    }

    pub fn current_row(&self) -> Option<&RowRef> {
        if self.tab != Tab::Installed {
            return None;
        }
        self.rows.get(self.selected_index())
    }

    pub fn current_package(&self) -> Option<&Package> {
        let idx = self.selected_index();
        match self.tab {
            Tab::Installed => match self.rows.get(idx)? {
                RowRef::Package { index } => self.installed.get(*index),
                RowRef::Header { .. } => None,
            },
            Tab::Orphans => self.orphans.get(idx),
            _ => None,
        }
    }

    pub fn current_upgrade(&self) -> Option<&Upgrade> {
        self.upgrades.get(self.selected_index())
    }

    pub fn is_marked(&self, pkg: &Package) -> bool {
        self.marks.contains(&pkg.name, &pkg.arch)
    }

    pub fn category_label(&self, pkg: &Package) -> &str {
        self.taxonomy.category_of(pkg)
    }

    /// Recalcula filtro, ordenação, grupos e a lista de linhas.
    pub fn ensure_package_selection(&mut self) {
        if self.tab != Tab::Installed {
            let len = self.rows_len();
            if len == 0 {
                self.table.select(None);
            } else if self.table.selected().unwrap_or(0) >= len {
                self.table.select(Some(len - 1));
            }
            return;
        }
        if self.rows.is_empty() {
            self.table.select(None);
            return;
        }
        let mut idx = self.table.selected().unwrap_or(0);
        if idx >= self.rows.len() {
            idx = self.rows.len() - 1;
        }
        if self.rows[idx].is_header() {
            match self.rows[idx..]
                .iter()
                .position(|row| !row.is_header())
                .map(|offset| idx + offset)
            {
                Some(next) => idx = next,
                None => {
                    // Grupo recolhido no fim da lista: volta para o primeiro pacote.
                    if let Some(first) = self.rows.iter().position(|row| !row.is_header()) {
                        idx = first;
                    }
                }
            }
        }
        self.table.select(Some(idx));
        self.follow_selection();
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.rows_len();
        if len == 0 {
            self.table.select(None);
            return;
        }
        if self.tab != Tab::Installed {
            let next = (self.selected_index() as isize + delta).clamp(0, len as isize - 1);
            self.table.select(Some(next as usize));
            self.follow_selection();
            return;
        }

        let step: isize = if delta >= 0 { 1 } else { -1 };
        let mut remaining = delta.abs();
        let mut idx = self.selected_index() as isize;
        while remaining > 0 {
            let mut next = idx + step;
            while next >= 0 && (next as usize) < len && self.rows[next as usize].is_header() {
                next += step;
            }
            if next < 0 || next as usize >= len {
                break;
            }
            idx = next;
            remaining -= 1;
        }
        self.table.select(Some(idx as usize));
        self.follow_selection();
    }

    pub fn select(&mut self, index: usize) {
        if self.rows_len() == 0 {
            return;
        }
        let index = index.min(self.rows_len() - 1);
        self.table.select(Some(index));
        self.ensure_package_selection();
    }

    /// Mantém a linha selecionada dentro da janela visível.
    pub fn follow_selection(&mut self) {
        let sel = self.selected_index();
        if sel < self.scroll {
            self.scroll = sel;
        } else if sel >= self.scroll + self.viewport.max(1) {
            self.scroll = sel + 1 - self.viewport.max(1);
        }
    }
}
