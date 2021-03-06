/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 *
 * Copyright 2020 Joyent, Inc.
 */

use rand::Rng;

use std::error;
use std::fmt;
use std::str::FromStr;

const DEF_QUEUE_CAP: usize = 1_000_000;

/*
 * Operating modes that the queue supports. See the block comment above the
 * Queue impl for an explanation.
 */
pub enum QueueMode {
    Lru,
    Mru,
    Rand,
}

#[derive(Debug)]
pub struct QueueModeError;
impl error::Error for QueueModeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}
impl fmt::Display for QueueModeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid queue mode")
    }
}

/*
 * To make calling code cleaner, let users create the QueueMode from a
 * lowercase str.
 */
impl FromStr for QueueMode {
    type Err = QueueModeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mode = match s {
            "lru" => Some(QueueMode::Lru),
            "mru" => Some(QueueMode::Mru),
            "rand" => Some(QueueMode::Rand),
            _ => None,
        };

        if mode.is_none() {
            return Err(QueueModeError);
        }
        Ok(mode.unwrap())
    }
}

impl fmt::Display for QueueMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let strmode = match self {
            QueueMode::Lru => "lru",
            QueueMode::Mru => "mru",
            QueueMode::Rand => "rand",
        };
        write!(f, "{}", strmode)
    }
}

pub struct Queue<T> {
    items: Vec<T>,
    cap: usize,
    mode: QueueMode,
    cursor: usize,
}

/*
 * This is a simple queue data structure. It supports a few different modes of
 * operation.
 *
 * Modes:
 * - Lru (least recently used). Operates like a FIFO queue. When the queue fills
 *   up new items replace the oldest items.
 * - Mru (most recently used). Operates like a LIFO queue (AKA a stack). When
 *   the queue is at capacity the 'bottom' item in the stack is removed and the
 *   new item is added to the top of the stack.
 * - Rand (random). Operates like an array. Random items are returned when using
 *   the accessor function. New items replace a random item.
 */
impl<T> Queue<T> {
    pub fn new(mode: QueueMode) -> Queue<T> {
        Queue {
            items: Vec::with_capacity(DEF_QUEUE_CAP),
            cap: DEF_QUEUE_CAP,
            mode,
            cursor: 0,
        }
    }

    /*
     * Inserts an item into the queue.
     * Removes an item if the queue has hit its capacity.
     */
    pub fn insert(&mut self, qi: T) {
        if self.items.len() < self.cap {
            self.items.push(qi);
            return;
        }

        self.replace(qi);
    }

    /*
     * Return an item from the queue.
     * Returns None if nothing is in the queue.
     */
    pub fn get(&mut self) -> Option<&T> {
        if self.items.is_empty() {
            return None;
        }

        match self.mode {
            QueueMode::Lru => self.items.get(0),
            QueueMode::Mru => self.items.get(self.items.len()),
            QueueMode::Rand => self
                .items
                .get(rand::thread_rng().gen_range(0, self.items.len())),
        }
    }

    pub fn remove(&mut self) -> Option<T> {
        if self.items.is_empty() {
            return None;
        }

        match self.mode {
            QueueMode::Lru => Some(self.items.remove(0)),
            QueueMode::Mru => Some(self.items.remove(0)),
            QueueMode::Rand => {
                let ret = Some(self.items.swap_remove(self.cursor));
                if !self.items.is_empty() {
                    self.cursor = (self.cursor + 1) % self.items.len();
                } else {
                    self.cursor = 0;
                }
                ret
            }
        }
    }

    pub fn replace(&mut self, qi: T) {
        if self.items.is_empty() {
            return;
        }

        let len = self.items.len();

        match self.mode {
            QueueMode::Lru => self.items[len] = qi,
            QueueMode::Mru => self.items[len] = qi,
            QueueMode::Rand => {
                self.items[self.cursor] = qi;
                self.cursor = (self.cursor + 1) % len;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /*
     * Historically we've had problems with the naiive queue implementation.
     * Specifically the naiive implementation isn't efficient when new items
     * are added or removed from a full queue.
     *
     * This test is like a benchmark, except it doesn't fit the rust
     * benchmarking model because the routine here can be fairly expensive
     * depending on the length of the queue.
     *
     * If you're interested in seeing the results that are printed here
     * (timings for filling a queue and then overwriting all items within), I
     * recommend running `cargo test -- --nocapture` to see the timings.
     */

    #[test]
    fn test_queue_overwrite() {
        let mut q = Queue::new(QueueMode::Rand);
        let start = Instant::now();
        for _ in 0..DEF_QUEUE_CAP {
            q.insert("testobj".to_string());
        }
        let end = start.elapsed().as_millis();
        println!("adding {} items took {}ms", DEF_QUEUE_CAP, end);

        let noverflow = DEF_QUEUE_CAP;
        let start = Instant::now();
        for _ in 0..noverflow {
            q.insert("testobj".to_string());
        }
        let end = start.elapsed().as_millis();
        println!("adding {} overflow items took {}ms", noverflow, end);
    }

    #[test]
    fn test_queue_clear() {
        let mut q = Queue::new(QueueMode::Rand);
        let start = Instant::now();
        for _ in 0..DEF_QUEUE_CAP {
            q.insert("testobj".to_string());
        }
        let end = start.elapsed().as_millis();
        println!("adding {} items took {}ms", DEF_QUEUE_CAP, end);

        let start = Instant::now();
        for _ in 0..DEF_QUEUE_CAP {
            q.remove();
        }
        let end = start.elapsed().as_millis();
        println!("removing {} items took {}ms", DEF_QUEUE_CAP, end);
    }
}
