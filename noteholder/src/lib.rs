//! NoteHolder — multi-note sustain pad MIDI generator.

use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};

// ── Constants ─────────────────────────────────────────────────────────────────

const NOTE_MIN: u8 = 36; // C2
const NOTE_MAX: u8 = 84; // C6
const NOTE_COUNT: usize = (NOTE_MAX - NOTE_MIN + 1) as usize;

/// Key geometry ratios (standard piano proportions).
const BK_W_RATIO: f32 = 13.0 / 22.0; // black width  / white width
const BK_H_RATIO: f32 = 56.0 / 90.0; // black height / white height

// ── Note state params ─────────────────────────────────────────────────────────

/// One `BoolParam` per key.  Exposed to the host so held notes appear as
/// toggle switches in the rack/parameter view and are automatable.
#[derive(Params)]
pub struct NoteParams {
    #[id = "on"]
    pub on: BoolParam,
}

// ── Plugin struct ─────────────────────────────────────────────────────────────

pub struct NoteHolder {
    params: Arc<NoteHolderParams>,
    /// DSP-only: which notes have had NoteOn sent (not yet NoteOff).
    note_active: Vec<bool>,
    /// Set by reset() so the next process() flushes NoteOff for all active notes.
    pending_all_notes_off: bool,
    /// Cached octave offset so we can detect changes inside process().
    prev_octave_offset: i32,
}

// ── Parameters ────────────────────────────────────────────────────────────────

#[derive(Params)]
pub struct NoteHolderParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "velocity"]
    pub velocity: IntParam,

    #[id = "channel"]
    pub channel: IntParam,

    /// Shifts every sent note by N octaves (-4 … +4).
    #[id = "octave"]
    pub octave_offset: IntParam,

    /// Toggle state for each key (C2 = index 0 … C6 = index 48).
    /// Exposed as params → host rack view shows them as toggle switches.
    #[nested(array, group = "Keys")]
    pub notes: Vec<NoteParams>,
}

impl Default for NoteHolderParams {
    fn default() -> Self {
        let notes = (0..NOTE_COUNT)
            .map(|i| {
                let note = NOTE_MIN + i as u8;
                let name = note_name_display(note, 0);
                NoteParams {
                    on: BoolParam::new(name, false),
                }
            })
            .collect();

        Self {
            editor_state: EguiState::from_size(870, 220),
            velocity: IntParam::new("Velocity", 100, IntRange::Linear { min: 1, max: 127 }),
            channel: IntParam::new("Channel", 1, IntRange::Linear { min: 1, max: 16 }),
            octave_offset: IntParam::new("Octave", 0, IntRange::Linear { min: -4, max: 4 }),
            notes,
        }
    }
}

impl Default for NoteHolder {
    fn default() -> Self {
        Self {
            params: Arc::new(NoteHolderParams::default()),
            note_active: vec![false; NOTE_COUNT],
            pending_all_notes_off: false,
            prev_octave_offset: 0,
        }
    }
}

// ── Plugin trait ──────────────────────────────────────────────────────────────

impl Plugin for NoteHolder {
    const NAME: &'static str = "NoteHolder";
    const VENDOR: &'static str = "Aritz Beobide-Cardinal & Claude";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: None,
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            EditorState { params },
            |_ctx, _state| {},
            |ctx, setter, state| draw_ui(ctx, setter, state),
        )
    }

    fn reset(&mut self) {
        // Signal process() to flush NoteOff for all active notes on its next
        // call.  We cannot touch params here (no setter available), so key
        // toggle states are left as-is; the host preserves automation state.
        self.pending_all_notes_off = true;
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let channel = (self.params.channel.value() - 1) as u8;
        let velocity = self.params.velocity.value() as f32 / 127.0;
        let octave_offset = self.params.octave_offset.value();

        // ── Post-reset flush ─────────────────────────────────────────────────
        if self.pending_all_notes_off {
            self.pending_all_notes_off = false;
            for (i, active) in self.note_active.iter_mut().enumerate() {
                if *active {
                    context.send_event(NoteEvent::NoteOff {
                        timing: 0,
                        voice_id: None,
                        channel,
                        note: shifted_note(i, self.prev_octave_offset),
                        velocity: 0.0,
                    });
                    *active = false;
                }
            }
            self.prev_octave_offset = octave_offset;
            return ProcessStatus::Normal;
        }

        // ── Octave offset changed: retrigger all held notes ──────────────────
        if octave_offset != self.prev_octave_offset {
            for (i, active) in self.note_active.iter_mut().enumerate() {
                if *active {
                    context.send_event(NoteEvent::NoteOff {
                        timing: 0,
                        voice_id: None,
                        channel,
                        note: shifted_note(i, self.prev_octave_offset),
                        velocity: 0.0,
                    });
                    if self.params.notes[i].on.value() {
                        context.send_event(NoteEvent::NoteOn {
                            timing: 0,
                            voice_id: None,
                            channel,
                            note: shifted_note(i, octave_offset),
                            velocity,
                        });
                    } else {
                        *active = false;
                    }
                }
            }
            self.prev_octave_offset = octave_offset;
            return ProcessStatus::Normal;
        }

        // ── Normal note-on / note-off detection ──────────────────────────────
        for (i, (note_param, active)) in self
            .params
            .notes
            .iter()
            .zip(self.note_active.iter_mut())
            .enumerate()
        {
            let want = note_param.on.value();
            let note = shifted_note(i, octave_offset);

            match (want, *active) {
                (true, false) => {
                    context.send_event(NoteEvent::NoteOn {
                        timing: 0,
                        voice_id: None,
                        channel,
                        note,
                        velocity,
                    });
                    *active = true;
                }
                (false, true) => {
                    context.send_event(NoteEvent::NoteOff {
                        timing: 0,
                        voice_id: None,
                        channel,
                        note,
                        velocity: 0.0,
                    });
                    *active = false;
                }
                _ => {}
            }
        }

        ProcessStatus::Normal
    }
}

// ── CLAP / VST3 ───────────────────────────────────────────────────────────────

impl ClapPlugin for NoteHolder {
    const CLAP_ID: &'static str = "com.noteholder.noteholder";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Multi-note sustain pad MIDI generator");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::NoteEffect, ClapFeature::Instrument, ClapFeature::Utility];
}

impl Vst3Plugin for NoteHolder {
    const VST3_CLASS_ID: [u8; 16] = *b"NoteHolderPlugin";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(NoteHolder);
nih_export_vst3!(NoteHolder);

// ── Helpers ───────────────────────────────────────────────────────────────────

/// MIDI note number sent for key at `idx` given the current octave offset.
fn shifted_note(idx: usize, octave_offset: i32) -> u8 {
    (NOTE_MIN as i32 + idx as i32 + octave_offset * 12).clamp(0, 127) as u8
}

/// Note name string including octave, accounting for octave offset.
/// e.g. key C2 (MIDI 36) with offset +1 → "C3"
fn note_name_display(key_note: u8, octave_offset: i32) -> String {
    const NAMES: [&str; 12] =
        ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let midi = (key_note as i32 + octave_offset * 12).clamp(0, 127);
    let octave = midi / 12 - 1;
    format!("{}{}", NAMES[(midi % 12) as usize], octave)
}

fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}

/// X position of `note` in white-key units from MIDI 0.
/// White keys: left edge. Black keys: centre.
fn wk_x(note: u8) -> f32 {
    let octave = note / 12;
    let offset = match note % 12 {
        0 => 0.0,
        1 => 0.5,
        2 => 1.0,
        3 => 1.5,
        4 => 2.0,
        5 => 3.0,
        6 => 3.5,
        7 => 4.0,
        8 => 4.5,
        9 => 5.0,
        10 => 5.5,
        11 => 6.0,
        _ => 0.0,
    };
    octave as f32 * 7.0 + offset
}

fn key_rect(
    note: u8,
    origin: egui::Pos2,
    wkw: f32,
    wkh: f32,
    bkw: f32,
    bkh: f32,
) -> egui::Rect {
    let x = (wk_x(note) - wk_x(NOTE_MIN)) * wkw + origin.x;
    if is_black(note) {
        egui::Rect::from_min_size(
            egui::pos2(x - bkw / 2.0, origin.y),
            egui::vec2(bkw, bkh),
        )
    } else {
        egui::Rect::from_min_size(
            egui::pos2(x, origin.y),
            egui::vec2(wkw - 1.0, wkh),
        )
    }
}

// ── Editor ────────────────────────────────────────────────────────────────────

struct EditorState {
    params: Arc<NoteHolderParams>,
}

fn draw_ui(ctx: &egui::Context, setter: &ParamSetter, state: &mut EditorState) {
    // Controls strip at the top.
    egui::TopBottomPanel::top("controls")
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Velocity");
                let mut vel = state.params.velocity.value();
                if ui.add(egui::Slider::new(&mut vel, 1i32..=127).show_value(true)).changed() {
                    setter.begin_set_parameter(&state.params.velocity);
                    setter.set_parameter(&state.params.velocity, vel);
                    setter.end_set_parameter(&state.params.velocity);
                }

                ui.separator();

                ui.label("Channel");
                let mut ch = state.params.channel.value();
                if ui.add(egui::Slider::new(&mut ch, 1i32..=16).show_value(true)).changed() {
                    setter.begin_set_parameter(&state.params.channel);
                    setter.set_parameter(&state.params.channel, ch);
                    setter.end_set_parameter(&state.params.channel);
                }

                ui.separator();

                ui.label("Octave");
                let mut oct = state.params.octave_offset.value();
                if ui
                    .add(
                        egui::Slider::new(&mut oct, -4i32..=4)
                            .show_value(true)
                            .custom_formatter(|v, _| {
                                if v == 0.0 { "0".into() }
                                else if v > 0.0 { format!("+{}", v as i32) }
                                else { format!("{}", v as i32) }
                            }),
                    )
                    .changed()
                {
                    setter.begin_set_parameter(&state.params.octave_offset);
                    setter.set_parameter(&state.params.octave_offset, oct);
                    setter.end_set_parameter(&state.params.octave_offset);
                }

                ui.separator();

                if ui.button("All Notes Off").clicked() {
                    for note_param in state.params.notes.iter() {
                        setter.begin_set_parameter(&note_param.on);
                        setter.set_parameter(&note_param.on, false);
                        setter.end_set_parameter(&note_param.on);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(concat!("built ", env!("BUILD_DATETIME")))
                            .small()
                            .color(egui::Color32::from_gray(100)),
                    );
                });
            });
            ui.add_space(4.0);
        });


    // Piano fills the rest.
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            draw_piano(ui, setter, state);
        });
}

// ── Piano keyboard ────────────────────────────────────────────────────────────

fn draw_piano(ui: &mut egui::Ui, setter: &ParamSetter, state: &EditorState) {
    let white_count = (NOTE_MIN..=NOTE_MAX).filter(|&n| !is_black(n)).count() as f32;

    let wkw = 24.0_f32;
    let wkh = 100.0_f32;
    let bkw = wkw * BK_W_RATIO;
    let bkh = wkh * BK_H_RATIO;

    {

    let octave_offset = state.params.octave_offset.value();
    let piano_size = egui::vec2(white_count * wkw, wkh);

    let (rect, response) = ui.allocate_exact_size(piano_size, egui::Sense::click());
    let origin = rect.left_top();
    let painter = ui.painter_at(rect);

    // ── Click → toggle ────────────────────────────────────────────────────────
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let hit = (NOTE_MIN..=NOTE_MAX)
                .filter(|&n| is_black(n))
                .find(|&n| key_rect(n, origin, wkw, wkh, bkw, bkh).contains(pos))
                .or_else(|| {
                    (NOTE_MIN..=NOTE_MAX)
                        .filter(|&n| !is_black(n))
                        .find(|&n| key_rect(n, origin, wkw, wkh, bkw, bkh).contains(pos))
                });

            if let Some(note) = hit {
                let idx = (note - NOTE_MIN) as usize;
                let cur = state.params.notes[idx].on.value();
                setter.begin_set_parameter(&state.params.notes[idx].on);
                setter.set_parameter(&state.params.notes[idx].on, !cur);
                setter.end_set_parameter(&state.params.notes[idx].on);
            }
        }
    }

    // Font sizes scale with key dimensions, clamped for readability.
    let wk_font = (wkw * 0.50).clamp(7.0, 13.0);
    let bk_font = (bkw * 0.68).clamp(6.0, 11.0);

    // ── White keys ────────────────────────────────────────────────────────────
    for note in NOTE_MIN..=NOTE_MAX {
        if is_black(note) {
            continue;
        }
        let idx = (note - NOTE_MIN) as usize;
        let on = state.params.notes[idx].on.value();
        let kr = key_rect(note, origin, wkw, wkh, bkw, bkh);

        let fill = if on {
            egui::Color32::from_rgb(100, 180, 255)
        } else {
            egui::Color32::WHITE
        };
        painter.rect(
            kr,
            egui::CornerRadius::same(2),
            fill,
            egui::Stroke::new(1.0, egui::Color32::from_gray(110)),
            egui::StrokeKind::Inside,
        );

        // Note name near the bottom of the key.
        let label = note_name_display(note, octave_offset);
        painter.text(
            egui::pos2(kr.center().x, kr.bottom() - wk_font * 1.1),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(wk_font),
            if on {
                egui::Color32::from_gray(40)
            } else {
                egui::Color32::from_gray(130)
            },
        );
    }

    // ── Black keys (painted on top) ───────────────────────────────────────────
    for note in NOTE_MIN..=NOTE_MAX {
        if !is_black(note) {
            continue;
        }
        let idx = (note - NOTE_MIN) as usize;
        let on = state.params.notes[idx].on.value();
        let kr = key_rect(note, origin, wkw, wkh, bkw, bkh);

        let fill = if on {
            egui::Color32::from_rgb(50, 120, 220)
        } else {
            egui::Color32::from_gray(28)
        };
        painter.rect(
            kr,
            egui::CornerRadius::same(2),
            fill,
            egui::Stroke::new(1.0, egui::Color32::BLACK),
            egui::StrokeKind::Inside,
        );

        // Note name stacked character-by-character near the top of the key.
        let label = note_name_display(note, octave_offset);
        let char_h = bk_font * 1.25;
        let label_color = if on {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_gray(200)
        };
        for (j, ch) in label.chars().enumerate() {
            painter.text(
                egui::pos2(kr.center().x, kr.top() + 3.0 + j as f32 * char_h),
                egui::Align2::CENTER_TOP,
                ch.to_string(),
                egui::FontId::proportional(bk_font),
                label_color,
            );
        }
    }
    }
}
