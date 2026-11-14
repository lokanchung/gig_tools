use crossbeam_queue::ArrayQueue;
use std::sync::atomic::{AtomicU64, Ordering};

// Slots and their queues live for the entire library lifetime. Audio callbacks never
// allocate, free memory, or wait for a receiver being created/destroyed.
pub(crate) const MAX_RECEIVERS: usize = 64;
pub(crate) const QUEUE_CAPACITY: usize = 1024;
const RESERVED: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProgramChange {
    pub channel: u8,
    pub program: u8,
}

#[derive(Clone, Copy)]
struct QueuedChange {
    generation: u64,
    change: ProgramChange,
}

struct Slot {
    generation: AtomicU64,
    queue: ArrayQueue<QueuedChange>,
}

pub(crate) struct Bus {
    slots: [Slot; MAX_RECEIVERS],
    next_generation: AtomicU64,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| Slot {
                generation: AtomicU64::new(0),
                queue: ArrayQueue::new(QUEUE_CAPACITY),
            }),
            next_generation: AtomicU64::new(1),
        }
    }

    pub fn subscribe(&self) -> Option<Subscription<'_>> {
        for slot in &self.slots {
            if slot
                .generation
                .compare_exchange(0, RESERVED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Reserve before draining so another subscriber cannot consume this
                // slot. Generation tags reject publishers still using the old slot.
                while slot.queue.pop().is_some() {}
                let generation = loop {
                    let next = self.next_generation.fetch_add(1, Ordering::Relaxed);
                    if next != 0 && next != RESERVED {
                        break next;
                    }
                };
                slot.generation.store(generation, Ordering::Release);
                return Some(Subscription { slot, generation });
            }
        }
        None
    }

    pub fn broadcast(&self, change: ProgramChange) {
        for slot in &self.slots {
            let generation = slot.generation.load(Ordering::Acquire);
            if generation != 0 && generation != RESERVED {
                // A full receiver drops the new event without blocking other receivers.
                let _ = slot.queue.push(QueuedChange { generation, change });
            }
        }
    }
}

pub(crate) struct Subscription<'a> {
    slot: &'a Slot,
    generation: u64,
}

impl Subscription<'_> {
    pub fn next(&self) -> Option<ProgramChange> {
        // Bound callback work even if publishers refill the queue concurrently.
        for _ in 0..QUEUE_CAPACITY {
            let entry = self.slot.queue.pop()?;
            if entry.generation == self.generation {
                return Some(entry.change);
            }
        }
        None
    }

    pub fn discard_pending(&self) {
        for _ in 0..QUEUE_CAPACITY {
            if self.slot.queue.pop().is_none() {
                break;
            }
        }
    }
}

impl Drop for Subscription<'_> {
    fn drop(&mut self) {
        self.slot.generation.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcasts_each_event_to_every_receiver_without_replaying_history() {
        let bus = Bus::new();
        let change = ProgramChange {
            channel: 15,
            program: 127,
        };
        bus.broadcast(change);
        let first = bus.subscribe().unwrap();
        let second = bus.subscribe().unwrap();
        assert_eq!(first.next(), None);
        bus.broadcast(change);
        bus.broadcast(change);
        for receiver in [&first, &second] {
            assert_eq!(receiver.next(), Some(change));
            assert_eq!(receiver.next(), Some(change));
            assert_eq!(receiver.next(), None);
        }
        drop(first);
        let replacement = bus.subscribe().unwrap();
        assert_eq!(replacement.next(), None);
    }

    #[test]
    fn slow_receiver_does_not_block_other_receivers() {
        let bus = Bus::new();
        let slow = bus.subscribe().unwrap();
        let fast = bus.subscribe().unwrap();
        let change = ProgramChange {
            channel: 0,
            program: 0,
        };
        for _ in 0..QUEUE_CAPACITY + 1 {
            bus.broadcast(change);
            assert_eq!(fast.next(), Some(change));
        }
        for _ in 0..QUEUE_CAPACITY {
            assert_eq!(slow.next(), Some(change));
        }
        assert_eq!(slow.next(), None);
    }

    #[test]
    fn slots_are_reusable_and_capacity_is_explicit() {
        let bus = Bus::new();
        let mut receivers: Vec<_> = (0..MAX_RECEIVERS)
            .map(|_| bus.subscribe().unwrap())
            .collect();
        assert!(bus.subscribe().is_none());
        drop(receivers.pop());
        assert!(bus.subscribe().is_some());
    }

    #[test]
    fn retired_publishers_cannot_leak_events_to_a_reused_slot() {
        let bus = Bus::new();
        let old = bus.subscribe().unwrap();
        let generation = old.generation;
        drop(old);
        let new = bus.subscribe().unwrap();
        new.slot
            .queue
            .push(QueuedChange {
                generation,
                change: ProgramChange {
                    channel: 0,
                    program: 1,
                },
            })
            .ok()
            .unwrap();
        assert_eq!(new.next(), None);
        let current = ProgramChange {
            channel: 1,
            program: 2,
        };
        bus.broadcast(current);
        assert_eq!(new.next(), Some(current));
    }

    #[test]
    fn concurrent_senders_reach_all_receivers() {
        let bus = Bus::new();
        let first = bus.subscribe().unwrap();
        let second = bus.subscribe().unwrap();
        std::thread::scope(|scope| {
            for channel in 0..4 {
                let bus = &bus;
                scope.spawn(move || {
                    for program in 0..128 {
                        bus.broadcast(ProgramChange { channel, program });
                    }
                });
            }
        });
        for receiver in [&first, &second] {
            let mut expected = [0; 4];
            for _ in 0..512 {
                let change = receiver.next().unwrap();
                assert_eq!(change.program as usize, expected[change.channel as usize]);
                expected[change.channel as usize] += 1;
            }
            assert_eq!(expected, [128; 4]);
            assert_eq!(receiver.next(), None);
        }
    }
}
