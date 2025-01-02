use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct NoteOnValues {
    timing: u32,
    voice_id: Option<i32>,
    channel: u8,
    velocity: f32,
}

enum PlayingState {
    NotPlaying,
    ChordDetecting(u32, u8, NoteOnValues),
    Playing(u8, u8),
}

// Sends midi note off when trigger is on
struct TopLiner {
    params: Arc<TopLinerParam>,
    playing_state: PlayingState,
}

#[derive(Params)]
struct TopLinerParam {}

impl Default for TopLinerParam {
    fn default() -> Self {
        Self {}
    }
}

impl Default for TopLiner {
    fn default() -> Self {
        Self {
            params: Arc::new(TopLinerParam::default()),
            playing_state: PlayingState::NotPlaying,
        }
    }
}

impl Plugin for TopLiner {
    const NAME: &'static str = "Top Liner";
    const VENDOR: &'static str = "Lokan Chung";
    const URL: &'static str = "lokanchung.me";
    const EMAIL: &'static str = "lokanchung@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // This plugin doesn't have any audio IO
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // forward all events
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::Choke { channel, note, .. }
                | NoteEvent::NoteOff { channel, note, .. } => {
                    match self.playing_state {
                        PlayingState::ChordDetecting(_, _, _) => {
                            self.playing_state = PlayingState::NotPlaying;
                        }
                        PlayingState::NotPlaying => {}
                        PlayingState::Playing(current_channel, current_note) => {
                            if current_channel == channel && current_note == note {
                                self.playing_state = PlayingState::NotPlaying;
                                // release current top note
                                context.send_event(event);
                            }
                        }
                    }
                }
                NoteEvent::NoteOn {
                    timing,
                    voice_id,
                    channel,
                    note,
                    velocity,
                } => {
                    match self.playing_state {
                        PlayingState::ChordDetecting(target_timing, target_note, _) => {
                            if note > target_note {
                                self.playing_state = PlayingState::ChordDetecting(
                                    target_timing,
                                    note,
                                    NoteOnValues {
                                        timing,
                                        voice_id,
                                        channel,
                                        velocity,
                                    },
                                );
                            }
                        }
                        PlayingState::NotPlaying => {
                            self.playing_state =
                                PlayingState::ChordDetecting(timing + 1024, note, NoteOnValues {
                                    timing,
                                    voice_id,
                                    channel,
                                    velocity,
                                });
                        }
                        PlayingState::Playing(current_channel, current_note) => {
                            if note > current_note {
                                // This case, need to turn off existing note
                                context.send_event(NoteEvent::NoteOff {
                                    timing,
                                    voice_id,
                                    channel: current_channel,
                                    note: current_note,
                                    velocity,
                                });
                                // Then send the highest note
                                context.send_event(event);
                                self.playing_state = PlayingState::Playing(channel, note);
                            }
                        }
                    }
                }
                _ => context.send_event(event),
            }
        }

        if let PlayingState::ChordDetecting(target_timing, note, values) = self.playing_state {
            let num_samples = buffer.samples() as u32;
            if target_timing < num_samples {
                context.send_event(NoteEvent::NoteOn {
                    timing: target_timing,
                    voice_id: values.voice_id,
                    channel: values.channel,
                    note,
                    velocity: values.velocity,
                });
                self.playing_state = PlayingState::Playing(values.channel, note);
            } else {
                self.playing_state =
                    PlayingState::ChordDetecting(target_timing - num_samples, note, values);
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for TopLiner {
    const CLAP_ID: &'static str = "me.lokanchung.top_liner";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("send panic to all midi channels");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect, ClapFeature::Utility];
}

impl Vst3Plugin for TopLiner {
    const VST3_CLASS_ID: [u8; 16] = *b"lkGS___top_liner";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(TopLiner);
nih_export_vst3!(TopLiner);
