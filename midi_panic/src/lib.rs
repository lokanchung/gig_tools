use nih_plug::prelude::*;
use std::sync::Arc;

// Sends midi note off when trigger is on
struct MidiPanic {
    params: Arc<MidiPanicParams>,
    sent_panic: bool,
}

#[derive(Params)]
struct MidiPanicParams {
    #[id = "trigger"]
    trigger: BoolParam,
}

impl Default for MidiPanicParams {
    fn default() -> Self {
        Self {
            trigger: BoolParam::new("trigger", false),
        }
    }
}

impl Default for MidiPanic {
    fn default() -> Self {
        Self {
            params: Arc::new(MidiPanicParams::default()),
            sent_panic: false,
        }
    }
}

impl Plugin for MidiPanic {
    const NAME: &'static str = "MIDI Panic";
    const VENDOR: &'static str = "Lokan Chung";
    const URL: &'static str = "lokanchung.me";
    const EMAIL: &'static str = "lokanchung@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // This plugin doesn't have any audio IO
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

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
            context.send_event(event);
        }

        // Detect trigger edge and early return
        if self.params.trigger.value() {
            if !self.sent_panic {
                self.sent_panic = true;
            } else {
                return ProcessStatus::Normal;
            }
        } else {
            self.sent_panic = false;
            return ProcessStatus::Normal;
        }

        // Send note off to all channel, all keys
        let num_samples = buffer.samples() as u32;
        let last_timing = if num_samples == 0 { 0 } else { num_samples - 1 };
        for channel in 0..16 {
            for note in 0..128 {
                context.send_event(NoteEvent::NoteOff {
                    timing: last_timing,
                    voice_id: None,
                    channel,
                    note,
                    velocity: 64.0,
                });
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for MidiPanic {
    const CLAP_ID: &'static str = "me.lokanchung.midi_panic";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("send panic to all midi channels");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect, ClapFeature::Utility];
}

impl Vst3Plugin for MidiPanic {
    const VST3_CLASS_ID: [u8; 16] = *b"lkGS___midipanic";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(MidiPanic);
nih_export_vst3!(MidiPanic);
