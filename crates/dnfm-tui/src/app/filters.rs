//! Painel de filtros, seletor de categoria/família e atribuição de categorias.

use std::collections::BTreeMap;

use dnfm_core::Reason;

use super::{App, FilterFacet, FilterPanel, Picker, PickerTarget, RowRef};

impl App {
    // ---------- painel de filtros ----------

    pub fn open_filter_panel(&mut self) {
        self.filter_panel = Some(FilterPanel::default());
        self.search = None;
    }

    pub fn close_filter_panel(&mut self) {
        self.filter_panel = None;
    }

    pub fn filter_panel_move(&mut self, delta: isize) {
        let Some(panel) = self.filter_panel.as_mut() else {
            return;
        };
        let len = FilterFacet::ALL.len() as isize;
        if len == 0 {
            return;
        }
        panel.cursor = ((panel.cursor as isize + delta).rem_euclid(len)) as usize;
    }

    pub fn focused_facet(&self) -> Option<FilterFacet> {
        self.filter_panel
            .as_ref()
            .and_then(|panel| FilterFacet::ALL.get(panel.cursor).copied())
    }

    /// Valor atual de uma faceta, já formatado para exibição.
    pub fn facet_value(&self, facet: FilterFacet) -> String {
        match facet {
            FilterFacet::Text => {
                if self.filter.text.is_empty() {
                    "—".to_string()
                } else {
                    self.filter.text.clone()
                }
            }
            FilterFacet::Category => join_or(&self.filter.categories, "todas"),
            FilterFacet::Family => join_or(&self.filter.families, "todas"),
            FilterFacet::Repo => self
                .filter
                .repo
                .clone()
                .unwrap_or_else(|| "todos".to_string()),
            FilterFacet::Reason => self
                .filter
                .reason
                .map(|r| r.label().to_string())
                .unwrap_or_else(|| "todos".to_string()),
            FilterFacet::Arch => self
                .filter
                .arch
                .clone()
                .unwrap_or_else(|| "todas".to_string()),
            FilterFacet::Size => self.filter.size_preset.label().to_string(),
            FilterFacet::Date => self.filter.date_preset.label().to_string(),
            FilterFacet::Marked => yes_no(self.filter.marked_only),
            FilterFacet::Protected => match self.filter.protected {
                None => "todos".to_string(),
                Some(true) => "só protegidos".to_string(),
                Some(false) => "só não protegidos".to_string(),
            },
            FilterFacet::Updates => yes_no(self.filter.only_updates),
            FilterFacet::Clear => String::new(),
        }
    }

    /// `espaço`/`enter` na faceta em foco.
    pub fn activate_facet(&mut self) -> Option<super::Cmd> {
        let facet = self.focused_facet()?;
        match facet {
            FilterFacet::Text => {
                self.filter_panel = None;
                self.search = Some(self.filter.text.clone());
                None
            }
            FilterFacet::Category => {
                self.open_picker(PickerTarget::Category);
                None
            }
            FilterFacet::Family => {
                self.open_picker(PickerTarget::Family);
                None
            }
            FilterFacet::Clear => {
                self.filter.clear();
                self.refresh();
                self.status = "filtros limpos".to_string();
                None
            }
            other => {
                self.cycle_facet(other);
                None
            }
        }
    }

    /// Avança o valor de uma faceta que não abre seletor.
    pub fn cycle_facet(&mut self, facet: FilterFacet) {
        match facet {
            FilterFacet::Repo => {
                let mut options: Vec<Option<String>> = vec![None];
                let mut ids: Vec<String> = self.repos.iter().map(|r| r.id.clone()).collect();
                ids.sort();
                options.extend(ids.into_iter().map(Some));
                self.filter.repo = next_option(&options, self.filter.repo.clone());
            }
            FilterFacet::Reason => {
                let options = vec![
                    None,
                    Some(Reason::User),
                    Some(Reason::Dependency),
                    Some(Reason::WeakDependency),
                    Some(Reason::Clean),
                ];
                self.filter.reason = next_option(&options, self.filter.reason);
            }
            FilterFacet::Arch => {
                let mut arches: Vec<String> = self
                    .installed
                    .iter()
                    .map(|p| p.arch.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                arches.sort();
                let mut options: Vec<Option<String>> = vec![None];
                options.extend(arches.into_iter().map(Some));
                self.filter.arch = next_option(&options, self.filter.arch.clone());
            }
            FilterFacet::Size => {
                let next = self.filter.size_preset.next();
                self.filter.apply_size_preset(next);
            }
            FilterFacet::Date => {
                let next = self.filter.date_preset.next();
                let now = dnfm_core::marks::now_unix();
                self.filter.apply_date_preset(next, now);
            }
            FilterFacet::Marked => self.filter.marked_only = !self.filter.marked_only,
            FilterFacet::Updates => self.filter.only_updates = !self.filter.only_updates,
            FilterFacet::Protected => {
                self.filter.protected = match self.filter.protected {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
            }
            FilterFacet::Text
            | FilterFacet::Category
            | FilterFacet::Family
            | FilterFacet::Clear => {}
        }
        self.refresh();
    }

    // ---------- seletor de categoria/família ----------

    pub fn open_picker(&mut self, target: PickerTarget) {
        let items = match target {
            PickerTarget::Category => self
                .taxonomy
                .categories()
                .iter()
                .map(|c| (c.label.clone(), c.count))
                .collect(),
            PickerTarget::Family => self.family_counts(),
        };
        let selected = match target {
            PickerTarget::Category => self.filter.categories.clone(),
            PickerTarget::Family => self.filter.families.clone(),
        };
        self.picker = Some(Picker::new(target, items, selected));
        self.filter_panel = None;
    }

    fn family_counts(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for pkg in &self.installed {
            *counts
                .entry(self.taxonomy.family_of(pkg).to_string())
                .or_insert(0) += 1;
        }
        let mut items: Vec<(String, usize)> = counts.into_iter().collect();
        items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        items
    }

    pub fn picker_move(&mut self, delta: isize) {
        if let Some(picker) = self.picker.as_mut() {
            picker.move_cursor(delta);
        }
    }

    pub fn picker_toggle(&mut self) {
        if let Some(picker) = self.picker.as_mut() {
            picker.toggle_current();
        }
    }

    pub fn picker_input(&mut self, ch: char) {
        if let Some(picker) = self.picker.as_mut() {
            picker.push_char(ch);
        }
    }

    pub fn picker_backspace(&mut self) {
        if let Some(picker) = self.picker.as_mut() {
            picker.pop_char();
        }
    }

    /// Aplica a seleção do seletor ao filtro e agrupa pela mesma dimensão.
    pub fn picker_apply(&mut self) {
        let Some(picker) = self.picker.take() else {
            return;
        };
        let Some(target) = picker.target else {
            return;
        };
        match target {
            PickerTarget::Category => self.filter.categories = picker.selected.clone(),
            PickerTarget::Family => self.filter.families = picker.selected.clone(),
        }
        let count = picker.selected.len();
        self.group = target.group_mode();
        self.collapsed.clear();
        self.refresh();
        self.status = if count == 0 {
            format!("filtro de {} limpo", target.title())
        } else {
            format!("{count} selecionado(s) em {}", target.title())
        };
    }

    // ---------- atribuição de categoria ----------

    /// `t`: começa a digitar o nome da categoria para marcados (ou o atual).
    pub fn assign_input_start(&mut self) {
        let prefill = self
            .current_package()
            .and_then(|p| self.custom_categories.get(&p.name, &p.arch))
            .unwrap_or("")
            .to_string();
        self.assign_input = Some(prefill);
        self.filter_panel = None;
        self.picker = None;
    }

    pub fn assign_input_push(&mut self, ch: char) {
        if let Some(buffer) = self.assign_input.as_mut() {
            buffer.push(ch);
        }
    }

    pub fn assign_input_pop(&mut self) {
        if let Some(buffer) = self.assign_input.as_mut() {
            buffer.pop();
        }
    }

    /// Enter no campo de categoria: aplica a marcados ou ao pacote atual.
    pub fn assign_input_commit(&mut self) {
        let Some(label) = self.assign_input.take() else {
            return;
        };
        let label = label.trim().to_string();
        if label.is_empty() {
            self.status = "categoria vazia: nada foi alterado".to_string();
            return;
        }
        let targets: Vec<dnfm_core::Package> = if !self.marks.is_empty() {
            self.installed
                .iter()
                .filter(|p| self.marks.contains(&p.name, &p.arch))
                .cloned()
                .collect()
        } else {
            self.current_package().cloned().into_iter().collect()
        };
        if targets.is_empty() {
            self.status = "nada selecionado para categorizar".to_string();
            return;
        }
        let count = targets.len();
        self.custom_categories.assign(&targets, &label);
        if let Err(err) = self.custom_categories.save() {
            self.status = format!("erro ao gravar categorias: {err}");
        } else {
            self.status = format!("{count} pacote(s) em «{label}»");
        }
        self.rebuild_taxonomy();
    }

    /// Remove a categoria própria dos pacotes selecionados.
    pub fn unassign_current(&mut self) {
        let targets: Vec<(String, String)> = if !self.marks.is_empty() {
            self.installed
                .iter()
                .filter(|p| self.marks.contains(&p.name, &p.arch))
                .map(|p| (p.name.clone(), p.arch.clone()))
                .collect()
        } else {
            self.current_package()
                .map(|p| vec![(p.name.clone(), p.arch.clone())])
                .unwrap_or_default()
        };
        if targets.is_empty() {
            self.status = "nada selecionado".to_string();
            return;
        }
        let mut removed = 0;
        for (name, arch) in targets {
            if self.custom_categories.unassign(&name, &arch) {
                removed += 1;
            }
        }
        let _ = self.custom_categories.save();
        self.status = format!("{removed} categoria(s) própria(s) removida(s)");
        self.rebuild_taxonomy();
    }

    pub fn rebuild_taxonomy(&mut self) {
        self.taxonomy = if self.catalog.is_empty() {
            dnfm_core::categories::Taxonomy::build(&self.installed, &self.custom_categories)
        } else {
            dnfm_core::categories::Taxonomy::build_with(
                &self.installed,
                &self.custom_categories,
                Some(&self.catalog),
            )
        };
        self.refresh();
    }

    /// Rótulo do grupo sob o cursor, quando há agrupamento ativo.
    pub fn current_group_label(&self) -> Option<String> {
        match self.current_row()? {
            RowRef::Header { label, .. } => Some(label.clone()),
            RowRef::Package { index } => self
                .installed
                .get(*index)
                .map(|p| self.taxonomy.label_for(p, self.group)),
        }
    }
}

fn join_or(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_string()
    } else if values.len() <= 2 {
        values.join(", ")
    } else {
        format!("{} e mais {}", values[0], values.len() - 1)
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "sim".to_string()
    } else {
        "não".to_string()
    }
}

fn next_option<T: Clone + PartialEq>(options: &[Option<T>], current: Option<T>) -> Option<T> {
    let idx = options
        .iter()
        .position(|option| *option == current)
        .unwrap_or(0);
    options[(idx + 1) % options.len()].clone()
}
