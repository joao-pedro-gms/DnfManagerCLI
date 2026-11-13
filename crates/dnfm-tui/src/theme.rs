//! Paletas e preferências visuais.
//!
//! Três paletas com a mesma hierarquia (destaque, texto, dim, estados) para que
//! o layout não dependa de cor específica. A escolha fica em `ui.json`.

use std::fs;
use std::path::PathBuf;

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Palette {
    #[default]
    Midnight,
    Solar,
    Mono,
}

impl Palette {
    pub const ALL: [Palette; 3] = [Palette::Midnight, Palette::Solar, Palette::Mono];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Midnight => "midnight",
            Self::Solar => "solar",
            Self::Mono => "mono",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: Palette,
    pub name: &'static str,
    /// Destaque principal (foco, seleção, cabeçalho ativo).
    pub accent: Color,
    /// Destaque secundário (categorias próprias, badges).
    pub accent_alt: Color,
    pub text: Color,
    pub dim: Color,
    /// Fundo de linhas alternadas / barras.
    pub surface: Color,
    pub surface_alt: Color,
    pub ok: Color,
    pub warn: Color,
    pub danger: Color,
    pub mark: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from(Palette::default())
    }
}

impl Theme {
    pub fn from(palette: Palette) -> Self {
        match palette {
            Palette::Midnight => Self {
                palette,
                name: "midnight",
                accent: Color::Rgb(86, 182, 194),
                accent_alt: Color::Rgb(198, 120, 221),
                text: Color::Rgb(220, 223, 228),
                dim: Color::Rgb(110, 118, 129),
                surface: Color::Rgb(30, 34, 42),
                surface_alt: Color::Rgb(40, 45, 55),
                ok: Color::Rgb(152, 195, 121),
                warn: Color::Rgb(229, 192, 123),
                danger: Color::Rgb(224, 108, 117),
                mark: Color::Rgb(229, 192, 123),
            },
            Palette::Solar => Self {
                palette,
                name: "solar",
                accent: Color::Rgb(38, 139, 210),
                accent_alt: Color::Rgb(108, 113, 196),
                text: Color::Rgb(238, 232, 213),
                dim: Color::Rgb(147, 161, 161),
                surface: Color::Rgb(0, 43, 54),
                surface_alt: Color::Rgb(7, 54, 66),
                ok: Color::Rgb(133, 153, 0),
                warn: Color::Rgb(181, 137, 0),
                danger: Color::Rgb(220, 50, 47),
                mark: Color::Rgb(203, 75, 22),
            },
            Palette::Mono => Self {
                palette,
                name: "mono",
                accent: Color::White,
                accent_alt: Color::Gray,
                text: Color::Gray,
                dim: Color::DarkGray,
                surface: Color::Indexed(235),
                surface_alt: Color::Indexed(238),
                ok: Color::White,
                warn: Color::Gray,
                danger: Color::LightRed,
                mark: Color::White,
            },
        }
    }

    pub fn next(&self) -> Self {
        Self::from(self.palette.next())
    }

    pub fn title(&self) -> Style {
        Style::default().fg(self.text).add_modifier(Modifier::BOLD)
    }

    pub fn subtle(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn selected(&self) -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn hover(&self) -> Style {
        Style::default().bg(self.surface_alt)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn border_focus(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Cor de fundo da linha selecionada na tabela.
    pub fn row_highlight(&self) -> Style {
        Style::default()
            .bg(self.surface_alt)
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Intensidade de cor para "acabou de mudar" (pulso).
    pub fn pulsed(&self, level: f32) -> Color {
        if level > 0.5 {
            self.accent
        } else if level > 0.0 {
            self.accent_alt
        } else {
            self.text
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct UiFile {
    #[serde(default)]
    theme: Palette,
}

/// Preferências de aparência, persistidas em `ui.json`.
#[derive(Debug, Clone, Default)]
pub struct UiSettings {
    path: PathBuf,
    pub dirty: bool,
}

impl UiSettings {
    pub fn default_path() -> PathBuf {
        dnfm_core::paths::state_dir().join("ui.json")
    }

    pub fn load() -> (Self, Theme) {
        let path = Self::default_path();
        let palette = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<UiFile>(&raw).ok())
            .map(|file| file.theme)
            .unwrap_or_default();
        (Self { path, dirty: false }, Theme::from(palette))
    }

    pub fn save(&mut self, theme: &Theme) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(&UiFile {
            theme: theme.palette,
        })?;
        fs::write(&self.path, body)?;
        self.dirty = false;
        Ok(())
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paletas_ciclam_e_tem_nomes() {
        let mut palette = Palette::default();
        let mut vistos = Vec::new();
        for _ in 0..Palette::ALL.len() {
            vistos.push(palette.key());
            palette = palette.next();
        }
        assert_eq!(palette, Palette::default(), "o ciclo deve fechar");
        assert_eq!(vistos.len(), 3);
    }

    #[test]
    fn tema_mantem_hierarquia_em_todas_as_paletas() {
        for palette in Palette::ALL {
            let theme = Theme::from(palette);
            assert_ne!(theme.accent, theme.text, "{:?}", palette);
            assert_ne!(theme.dim, theme.text, "{:?}", palette);
            assert_ne!(theme.surface, theme.surface_alt, "{:?}", palette);
        }
    }

    #[test]
    fn persistencia_de_tema() {
        let dir = std::env::temp_dir().join(format!("dnfm-ui-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("ui.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, r#"{"theme":"solar"}"#).unwrap();

        let palette = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<UiFile>(&raw).ok())
            .map(|f| f.theme)
            .unwrap_or_default();
        assert_eq!(palette, Palette::Solar);
    }
}
