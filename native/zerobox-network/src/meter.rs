use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct BandwidthMeter {
    window: Duration,
    read: Arc<Mutex<VecDeque<(Instant, u64)>>>,
    write: Arc<Mutex<VecDeque<(Instant, u64)>>>,
}

impl BandwidthMeter {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            read: Default::default(),
            write: Default::default(),
        }
    }

    pub fn add_read(&self, count: usize) {
        self.push(&self.read, count);
    }

    pub fn add_write(&self, count: usize) {
        self.push(&self.write, count);
    }

    pub fn read_speed(&self) -> f64 {
        self.speed(&self.read)
    }

    pub fn write_speed(&self) -> f64 {
        self.speed(&self.write)
    }

    fn push(&self, queue: &Mutex<VecDeque<(Instant, u64)>>, count: usize) {
        let now = Instant::now();
        let mut queue = queue.lock().expect("bandwidth meter poisoned");
        queue.push_back((now, count as u64));
        self.evict(&mut queue, now);
    }

    fn speed(&self, queue: &Mutex<VecDeque<(Instant, u64)>>) -> f64 {
        let now = Instant::now();
        let mut queue = queue.lock().expect("bandwidth meter poisoned");
        self.evict(&mut queue, now);
        let bytes: u64 = queue.iter().map(|(_, count)| count).sum();
        let elapsed = queue
            .front()
            .map(|(first, _)| now.duration_since(*first).as_secs_f64().max(0.001))
            .unwrap_or(1.0);
        bytes as f64 / elapsed
    }

    fn evict(&self, queue: &mut VecDeque<(Instant, u64)>, now: Instant) {
        while queue
            .front()
            .is_some_and(|(time, _)| now.duration_since(*time) > self.window)
        {
            queue.pop_front();
        }
    }
}
