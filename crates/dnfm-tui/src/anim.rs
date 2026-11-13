//! Motor de animação: valores suavizados por tempo, sem dependência de
//! biblioteca gráfica.
//!
//! Tudo é derivado de `tick(dt)`, então o mesmo estado produz a mesma animação
//! em qualquer taxa de quadros — e os testes podem avançar o tempo à mão.

use std::time::{Duration, Instant};

/// Duração das transições de modal.
const MODAL_SECS: f32 = 0.16;
/// Duração da animação de entrada das barras do sidebar.
const BARS_SECS: f32 = 0.35;
/// Duração do pulso de destaque.
const PULSE_SECS: f32 = 0.45;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Curva de saída cúbica: rápida no começo, suave no fim.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Curva de entrada e saída, para pulsos.
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[derive(Debug)]
pub struct Animator {
    pub modal: f32,
    pub closing: bool,
    pub bars: f32,
    pub pulse: f32,
    pub spinner_phase: f32,
    last: Instant,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            modal: 0.0,
            closing: false,
            bars: 0.0,
            pulse: 0.0,
            spinner_phase: 0.0,
            last: Instant::now(),
        }
    }
}

impl Animator {
    /// Abre o modal do zero (mesmo que já esteja aberto).
    pub fn open_modal(&mut self) {
        self.modal = if self.closing { 1.0 - self.modal } else { 0.0 };
        self.closing = false;
    }

    /// Inicia o fechamento; `modal` cai até 0.
    pub fn close_modal(&mut self) {
        self.closing = true;
    }

    pub fn start_bars(&mut self) {
        self.bars = 0.0;
    }

    pub fn pulse(&mut self) {
        self.pulse = 1.0;
    }

    /// Avança o tempo e devolve `true` se algo ainda está animando.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last)
            .as_secs_f32()
            .min(0.25);
        self.last = now;
        self.advance(dt);
        self.is_animating()
    }

    /// Versão determinística, usada pelos testes.
    pub fn advance(&mut self, dt: f32) {
        self.spinner_phase = (self.spinner_phase + dt * 8.0) % SPINNER.len() as f32;

        if self.closing {
            self.modal = (self.modal - dt / MODAL_SECS).max(0.0);
        } else {
            self.modal = (self.modal + dt / MODAL_SECS).min(1.0);
        }

        if self.pulse > 0.0 {
            self.pulse = (self.pulse - dt / PULSE_SECS).max(0.0);
        }
        if self.bars < 1.0 {
            self.bars = (self.bars + dt / BARS_SECS).min(1.0);
        }
    }

    pub fn is_animating(&self) -> bool {
        let modal_moving = if self.closing {
            self.modal > 0.0
        } else {
            self.modal > 0.0 && self.modal < 1.0
        };
        modal_moving || self.pulse > 0.0 || self.bars < 1.0
    }

    /// Fração suavizada do modal (0 = fechado, 1 = aberto).
    pub fn modal_eased(&self) -> f32 {
        ease_out(self.modal)
    }

    /// Intensidade do pulso, com curva de sino.
    pub fn pulse_level(&self) -> f32 {
        ease_in_out((1.0 - self.pulse).clamp(0.0, 1.0)) * self.pulse.min(1.0)
    }

    pub fn spinner(&self) -> &'static str {
        SPINNER[(self.spinner_phase as usize) % SPINNER.len()]
    }

    /// Intervalo de redesenho: rápido enquanto anima, lento quando parado.
    pub fn frame_time(&self) -> Duration {
        if self.is_animating() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(200)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_abre_e_fecha_no_tempo() {
        let mut anim = Animator::default();
        assert_eq!(anim.modal, 0.0);
        anim.open_modal();
        anim.advance(0.08);
        assert!(
            anim.modal > 0.3 && anim.modal < 0.8,
            "meio aberto: {}",
            anim.modal
        );
        anim.advance(0.1);
        assert_eq!(anim.modal, 1.0);

        anim.close_modal();
        anim.advance(0.08);
        assert!(anim.modal < 1.0 && anim.modal > 0.0);
        anim.advance(0.2);
        assert_eq!(anim.modal, 0.0);
        assert!(!anim.is_animating());
    }

    #[test]
    fn easing_tem_extremos_corretos() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!(ease_out(0.5) > 0.5, "deve adiantar no meio");
        assert_eq!(ease_in_out(0.0), 0.0);
        assert_eq!(ease_in_out(1.0), 1.0);
    }

    #[test]
    fn barras_animam_ate_o_fim() {
        let mut anim = Animator::default();
        anim.start_bars();
        assert_eq!(anim.bars, 0.0);
        anim.advance(0.2);
        assert!(anim.bars > 0.0 && anim.bars < 1.0);
        anim.advance(0.5);
        assert_eq!(anim.bars, 1.0);
    }

    #[test]
    fn pulso_decai_ate_sumir() {
        let mut anim = Animator::default();
        anim.pulse();
        assert_eq!(anim.pulse, 1.0);
        anim.advance(0.3);
        assert!(anim.pulse > 0.0 && anim.pulse < 1.0);
        anim.advance(0.3);
        assert_eq!(anim.pulse, 0.0);
    }

    #[test]
    fn spinner_percorre_os_quadros() {
        let mut anim = Animator::default();
        let primeiro = anim.spinner();
        anim.advance(0.2);
        assert_ne!(primeiro, anim.spinner());
        // Uma volta completa devolve o mesmo quadro.
        let atual = anim.spinner();
        anim.advance(SPINNER.len() as f32 / 8.0);
        assert_eq!(atual, anim.spinner());
    }

    #[test]
    fn taxa_de_quadros_cai_quando_parado() {
        let mut anim = Animator {
            bars: 1.0,
            ..Default::default()
        };
        assert_eq!(anim.frame_time(), Duration::from_millis(200));
        anim.pulse();
        assert_eq!(anim.frame_time(), Duration::from_millis(16));
    }
}
