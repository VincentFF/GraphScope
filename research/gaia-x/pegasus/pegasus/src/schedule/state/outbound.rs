use std::collections::VecDeque;

use ahash::AHashMap;
use nohash_hasher::{IntMap, IntSet};

use crate::graph::Port;
use crate::tag::tools::map::TidyTagMap;
use crate::Tag;
use crate::communication::cancel::CancelListener;


struct CancelSingle {
    ch: u32,
    listener: Box<dyn CancelListener>,
}

impl CancelSingle {
    fn cancel(&mut self, ch: u32, to: u32, tag: &Tag) -> Option<Tag> {
        assert_eq!(self.ch, ch);
        self.listener.cancel(tag, to)
    }
}

struct CancelTee {
    // the number of channels mounted to this port;
    channels: usize,
    // scope level of data which will enter this port;
    scope_level: u32,
    // channel's index -> channel's skip listener;
    tee: IntMap<u32, Box<dyn CancelListener>>,
    // trace canceled scope in each mounted channel;
    cancel_trace: Vec<TidyTagMap<IntSet<u32>>>,
}

impl CancelTee {
    fn cancel(&mut self, ch: u32, to: u32, tag: &Tag) -> Option<Tag> {
        let res = {
            let listener = self.tee.get_mut(&ch)?;
            listener.cancel(tag, to)
        }?;
        let level = res.len() as u32;
        if level <= self.scope_level {
            let guard = self.channels;
            let set = self.cancel_trace[level as usize].get_mut_or_insert(&res);
            set.insert(ch);
            if set.len() == guard {
                return Some(res);
            }
        }

        None
    }
}

enum HandleKind {
    Single(CancelSingle),
    Tee(CancelTee),
}

impl HandleKind {
    fn cancel(&mut self, ch: u32, to: u32, tag: &Tag) -> Option<Tag> {
        match self {
            HandleKind::Single(x) => x.cancel(ch, to, tag),
            HandleKind::Tee(x) => x.cancel(ch, to, tag),
        }
    }
}

pub struct OutputCancelState {
    port: Port,
    handle: HandleKind,
}

impl OutputCancelState {

    pub fn single(port: Port, ch: u32, listener: Box<dyn CancelListener>) -> Self {
        OutputCancelState { port, handle: HandleKind::Single(CancelSingle { ch, listener }) }
    }

    pub fn tee(port: Port, scope_level: u32, tee: Vec<(u32, Box<dyn CancelListener>)>) -> Self {
        let channels = tee.len();
        let mut map = IntMap::default();
        for (ch, lis) in tee {
            map.insert(ch, lis);
        }
        let mut cancel_trace = Vec::with_capacity(scope_level as usize);
        for i in 0..scope_level {
            cancel_trace.push(TidyTagMap::new(i));
        }
        let handle = CancelTee { channels, scope_level, tee: map,  cancel_trace };
        OutputCancelState {
            port,
            handle: HandleKind::Tee(handle)
        }
    }

    pub fn on_cancel(&mut self, ch: u32, to: u32, tag: &Tag) -> Option<Tag> {
        self.handle.cancel(ch, to, tag)
    }
}
