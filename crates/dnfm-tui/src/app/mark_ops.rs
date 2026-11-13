//! Marcações de remoção e seleção de atualizações.

use super::*;

impl App {
    pub fn toggle_mark_current(&mut self) {
        let Some(pkg) = self.current_package().cloned() else {
            return;
        };
        if let Some(reason) = dnfm_core::safety::protected_reason(&pkg.name, None) {
            self.status = format!("{} é protegido ({reason})", pkg.name);
            return;
        }
        let marked = self.marks.toggle(&pkg.name, &pkg.arch, &pkg.nevra);
        self.status = if marked {
            format!("{} marcado para remoção", pkg.name)
        } else {
            format!("{} desmarcado", pkg.name)
        };
    }

    /// Marca todos os pacotes do grupo atual (ou todos os visíveis sem grupo).
    pub fn mark_all_visible(&mut self) {
        let targets: Vec<usize> = match self.current_row().cloned() {
            Some(RowRef::Header { label, .. }) if self.group != GroupMode::None => self
                .visible
                .iter()
                .copied()
                .filter(|idx| {
                    self.installed
                        .get(*idx)
                        .map(|p| self.taxonomy.label_for(p, self.group) == label)
                        .unwrap_or(false)
                })
                .collect(),
            _ => self.visible.clone(),
        };
        let count = targets.len();
        for idx in targets {
            if let Some(pkg) = self.installed.get(idx).cloned() {
                if dnfm_core::safety::is_protected(&pkg.name).is_none()
                    && !self.marks.contains(&pkg.name, &pkg.arch)
                {
                    self.marks.mark(&pkg.name, &pkg.arch, &pkg.nevra, None);
                }
            }
        }
        self.status = format!("{count} pacotes marcados");
    }

    pub fn clear_marks(&mut self) {
        let count = self.marks.len();
        self.marks.clear();
        self.status = format!("{count} marcações removidas");
    }

    pub fn toggle_upgrade_selection(&mut self) {
        let Some(up) = self.current_upgrade().cloned() else {
            return;
        };
        if !self.selected_upgrades.remove(&up.installed.name) {
            self.selected_upgrades.insert(up.installed.name.clone());
        }
    }

    pub fn select_all_upgrades(&mut self, security_only: bool) {
        for up in &self.upgrades {
            if !security_only || up.severity.is_security_relevant() {
                self.selected_upgrades.insert(up.installed.name.clone());
            }
        }
        self.status = format!("{} atualizações selecionadas", self.selected_upgrades.len());
    }
}
