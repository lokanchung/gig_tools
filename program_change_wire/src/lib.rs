use nice_plug::prelude::*;
use std::sync::{Arc, LazyLock};

mod bus;
use bus::{Bus, ProgramChange, QUEUE_CAPACITY, Subscription};

// Both CLAP descriptors must be exported from this same library so every
// instance in the host process sees the same bus.
static BUS: LazyLock<Bus> = LazyLock::new(Bus::new);

#[derive(Default, Params)]
struct WireParams {}

#[derive(Default)]
struct Sender {
    params: Arc<WireParams>,
}

impl Sender {
    fn forward(&mut self, context: &mut impl ProcessContext<Self>, bus: &Bus) {
        while let Some(event) = context.next_event() {
            if let NoteEvent::MidiProgramChange {
                channel, program, ..
            } = event
            {
                bus.broadcast(ProgramChange { channel, program });
            }
            let _ = context.try_send_event(event);
        }
    }
}

impl Plugin for Sender {
    const NAME: &'static str = "Program Change Wire Sender";
    const VENDOR: &'static str = "Lokan Chung";
    const URL: &'static str = "lokanchung.me";
    const EMAIL: &'static str = "lokanchung@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
    type SysExMessage = ();
    type BackgroundTask = ();
    type Editor = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn activate(
        &mut self,
        _: &AudioIOLayout,
        _: &BufferConfig,
        _: &mut impl ActivateContext<Self>,
    ) -> bool {
        LazyLock::force(&BUS);
        true
    }

    fn process(
        &mut self,
        _: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.forward(context, &BUS);
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Sender {
    const CLAP_ID: &'static str = "me.lokanchung.program_change_wire.sender";
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "Broadcast MIDI program changes to every Program Change Wire Receiver in this process",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect, ClapFeature::Utility];
}

#[derive(Default)]
struct Receiver {
    params: Arc<WireParams>,
    subscription: Option<Subscription<'static>>,
    pending: Option<ProgramChange>,
}

impl Receiver {
    fn forward(&mut self, context: &mut impl ProcessContext<Self>) {
        // A cross-track event belongs to the receiver's next callback, not the
        // sender's sample clock. Emit it at offset zero before local input.
        if let Some(subscription) = &self.subscription {
            for _ in 0..QUEUE_CAPACITY {
                let Some(change) = self.pending.take().or_else(|| subscription.next()) else {
                    break;
                };
                let event = NoteEvent::MidiProgramChange {
                    timing: 0,
                    channel: change.channel,
                    program: change.program,
                };
                if context.try_send_event(event).is_err() {
                    self.pending = Some(change);
                    break;
                }
            }
        }
        // Receiver input is passed through but never broadcast back to the bus.
        while let Some(event) = context.next_event() {
            let _ = context.try_send_event(event);
        }
    }
}

impl Plugin for Receiver {
    const NAME: &'static str = "Program Change Wire Receiver";
    const VENDOR: &'static str = "Lokan Chung";
    const URL: &'static str = "lokanchung.me";
    const EMAIL: &'static str = "lokanchung@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
    type SysExMessage = ();
    type BackgroundTask = ();
    type Editor = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn activate(
        &mut self,
        _: &AudioIOLayout,
        _: &BufferConfig,
        _: &mut impl ActivateContext<Self>,
    ) -> bool {
        if self.subscription.is_none() {
            self.subscription = BUS.subscribe();
        }
        self.subscription.is_some()
    }

    fn reset(&mut self) {
        self.pending = None;
        if let Some(subscription) = &self.subscription {
            subscription.discard_pending();
        }
    }

    fn deactivate(&mut self) {
        self.subscription = None;
        self.pending = None;
    }

    fn process(
        &mut self,
        _: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.forward(context);
        // Keep polling even with no local input: a sender may be on another track.
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Receiver {
    const CLAP_ID: &'static str = "me.lokanchung.program_change_wire.receiver";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Receive MIDI program changes from every Program Change Wire Sender in this process");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect, ClapFeature::Utility];
}

nice_export_clap!(Sender, Receiver);

#[cfg(test)]
mod tests;
