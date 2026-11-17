use crate::model::year_start;

/// Presets de tamanho usados na interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SizePreset {
    #[default]
    Any,
    Over100MiB,
    Over10MiB,
    Over1MiB,
    Under1MiB,
}

impl SizePreset {
    pub const ALL: [SizePreset; 5] = [
        SizePreset::Any,
        SizePreset::Over100MiB,
        SizePreset::Over10MiB,
        SizePreset::Over1MiB,
        SizePreset::Under1MiB,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "qualquer",
            Self::Over100MiB => "> 100 MiB",
            Self::Over10MiB => "> 10 MiB",
            Self::Over1MiB => "> 1 MiB",
            Self::Under1MiB => "< 1 MiB",
        }
    }

    pub fn bounds(self) -> (Option<u64>, Option<u64>) {
        const MIB: u64 = 1024 * 1024;
        match self {
            Self::Any => (None, None),
            Self::Over100MiB => (Some(100 * MIB), None),
            Self::Over10MiB => (Some(10 * MIB), None),
            Self::Over1MiB => (Some(MIB), None),
            Self::Under1MiB => (None, Some(MIB - 1)),
        }
    }
}

/// Presets de data de instalação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DatePreset {
    #[default]
    Any,
    Last7Days,
    Last30Days,
    Last90Days,
    ThisYear,
}

impl DatePreset {
    pub const ALL: [DatePreset; 5] = [
        DatePreset::Any,
        DatePreset::Last7Days,
        DatePreset::Last30Days,
        DatePreset::Last90Days,
        DatePreset::ThisYear,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "qualquer",
            Self::Last7Days => "últimos 7 dias",
            Self::Last30Days => "últimos 30 dias",
            Self::Last90Days => "últimos 90 dias",
            Self::ThisYear => "este ano",
        }
    }

    /// Início do intervalo em timestamp unix, relativo a `now`.
    pub fn since(self, now: i64) -> Option<i64> {
        const DAY: i64 = 86_400;
        match self {
            Self::Any => None,
            Self::Last7Days => Some(now - 7 * DAY),
            Self::Last30Days => Some(now - 30 * DAY),
            Self::Last90Days => Some(now - 90 * DAY),
            Self::ThisYear => Some(year_start(now)),
        }
    }
}
