use super::*;
use nice_plug::context::process::SendEventError;
use nice_plug::midi::{Channel, Key, VoiceID};
use std::collections::VecDeque;
use std::sync::Mutex;

static TEST_BUS_LOCK: Mutex<()> = Mutex::new(());

struct Context {
    input: VecDeque<NoteEvent<()>>,
    output: Vec<NoteEvent<()>>,
    transport: Transport,
    capacity: usize,
}

impl Context {
    fn new(input: impl IntoIterator<Item = NoteEvent<()>>) -> Self {
        Self {
            input: input.into_iter().collect(),
            output: Vec::with_capacity(QUEUE_CAPACITY + 8),
            transport: Transport::new(48_000.0),
            capacity: QUEUE_CAPACITY + 8,
        }
    }
}

impl<P: Plugin<SysExMessage = (), BackgroundTask = ()>> ProcessContext<P> for Context {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute_background(&self, _: ()) {
        panic!("unexpected background task");
    }
    fn execute_gui(&self, _: ()) {
        panic!("unexpected GUI task");
    }
    fn transport(&self) -> &Transport {
        &self.transport
    }
    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        self.input.pop_front()
    }
    fn try_send_event(
        &mut self,
        event: NoteEvent<()>,
    ) -> Result<(), (NoteEvent<()>, SendEventError)> {
        if self.output.len() >= self.capacity {
            Err((event, SendEventError::HostBufferFull))
        } else {
            self.output.push(event);
            Ok(())
        }
    }
    fn set_latency_samples(&self, _: u32) {}
    fn request_restart(&self) {}
    fn set_current_voice_capacity(&self, _: u32) {}
}

fn process<P: Plugin<SysExMessage = (), BackgroundTask = ()>>(
    plugin: &mut P,
    context: &mut Context,
) -> ProcessStatus {
    plugin.process(
        &mut Buffer::default(),
        &mut AuxiliaryBuffers {
            inputs: &mut [],
            outputs: &mut [],
        },
        context,
    )
}

fn receiver() -> Receiver {
    Receiver {
        subscription: BUS.subscribe(),
        ..Default::default()
    }
}

fn pc(timing: u32, channel: u8, program: u8) -> NoteEvent<()> {
    NoteEvent::MidiProgramChange {
        timing,
        channel,
        program,
    }
}

#[test]
fn multiple_senders_broadcast_only_pc_and_preserve_local_midi_without_allocations() {
    let _guard = TEST_BUS_LOCK.lock().unwrap();
    LazyLock::force(&BUS);
    let mut first = receiver();
    let mut second = receiver();
    let mut sender_a = Sender::default();
    let mut sender_b = Sender::default();
    let note = NoteEvent::NoteOn {
        timing: 3,
        voice_id: VoiceID::Wildcard,
        channel: Channel::Number(2),
        key: Key::Number(60),
        velocity: 0.5,
    };
    let cc = NoteEvent::MidiCC {
        timing: 4,
        channel: 3,
        cc: 0,
        value: 1.0,
    };
    let mut input_a = Context::new([note, cc, pc(8, 15, 127), pc(9, 15, 127)]);
    let mut input_b = Context::new([pc(0, 0, 0)]);
    let mut output_a = Context::new([note]);
    let mut output_b = Context::new([]);
    nice_assert_no_alloc::assert_no_alloc(|| {
        process(&mut sender_a, &mut input_a);
        process(&mut sender_b, &mut input_b);
        assert_eq!(process(&mut first, &mut output_a), ProcessStatus::KeepAlive);
        process(&mut second, &mut output_b);
    });
    assert_eq!(input_a.output, [note, cc, pc(8, 15, 127), pc(9, 15, 127)]);
    assert_eq!(input_b.output, [pc(0, 0, 0)]);
    assert_eq!(
        output_a.output,
        [pc(0, 15, 127), pc(0, 15, 127), pc(0, 0, 0), note]
    );
    assert_eq!(
        output_b.output,
        [pc(0, 15, 127), pc(0, 15, 127), pc(0, 0, 0)]
    );
    output_a.output.clear();
    process(&mut first, &mut output_a);
    assert!(output_a.output.is_empty());
}

#[test]
fn receiver_retries_full_host_output_in_order_and_never_rebroadcasts_input() {
    let _guard = TEST_BUS_LOCK.lock().unwrap();
    LazyLock::force(&BUS);
    let mut first = receiver();
    let mut observer = receiver();
    let mut sender = Sender::default();
    let mut input = Context::new([pc(2, 3, 7), pc(4, 4, 8)]);
    process(&mut sender, &mut input);
    let mut full = Context::new([]);
    full.capacity = 0;
    process(&mut first, &mut full);
    assert!(full.output.is_empty());
    full.capacity = 1;
    process(&mut first, &mut full);
    assert_eq!(full.output, [pc(0, 3, 7)]);
    full.output.clear();
    process(&mut first, &mut full);
    assert_eq!(full.output, [pc(0, 4, 8)]);
    let mut local = Context::new([pc(5, 6, 9)]);
    process(&mut first, &mut local);
    assert_eq!(local.output, [pc(5, 6, 9)]);
    let mut output = Context::new([]);
    process(&mut observer, &mut output);
    assert_eq!(output.output, [pc(0, 3, 7), pc(0, 4, 8)]);
}

#[test]
fn late_receiver_reset_and_deactivation_do_not_replay_old_program_changes() {
    let _guard = TEST_BUS_LOCK.lock().unwrap();
    LazyLock::force(&BUS);
    let mut sender = Sender::default();
    process(&mut sender, &mut Context::new([pc(0, 1, 2)]));
    let mut receiver = receiver();
    let mut output = Context::new([]);
    process(&mut receiver, &mut output);
    assert!(output.output.is_empty());
    process(&mut sender, &mut Context::new([pc(0, 2, 3)]));
    receiver.reset();
    process(&mut receiver, &mut output);
    assert!(output.output.is_empty());
    process(&mut sender, &mut Context::new([pc(0, 3, 4)]));
    receiver.deactivate();
    receiver.subscription = BUS.subscribe();
    process(&mut receiver, &mut output);
    assert!(output.output.is_empty());
}
