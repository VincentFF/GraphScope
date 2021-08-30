use std::collections::VecDeque;

use crate::communication::IOResult;
use crate::event::{Event, EventKind};
use crate::graph::Port;
use crate::schedule::state::inbound::{InboundStreamState, InputEndNotify};
use crate::schedule::state::outbound::OutputCancelState;
use crate::Tag;

pub struct OperatorScheduler {
    pub index: usize,
    inputs_notify: Vec<Option<InboundStreamState>>,
    outputs_cancel: Vec<Option<OutputCancelState>>,
    discards: VecDeque<(Port, Tag)>,
}

impl OperatorScheduler {
    pub fn new(
        index: usize, scope_level: u32, inputs_notify: Vec<Option<Box<dyn InputEndNotify>>>,
        outputs_cancel: Vec<Option<OutputCancelState>>,
    ) -> Self {
        let mut input_events = Vec::with_capacity(inputs_notify.len());
        for (i, notify) in inputs_notify.into_iter().enumerate() {
            if let Some(notify) = notify {
                let port = Port::new(index, i);
                let state = InboundStreamState::new(port, scope_level, notify);
                input_events.push(Some(state));
            } else {
                input_events.push(None);
            }
        }

        OperatorScheduler { index, inputs_notify: input_events, outputs_cancel, discards: VecDeque::new() }
    }

    pub fn accept(&mut self, event: Event) -> IOResult<()> {
        let port = event.target_port;
        let src = event.from_worker;
        let kind = event.take_kind();
        match kind {
            EventKind::End(end) => {
                if let Some(Some(state)) = self.inputs_notify.get_mut(port.port) {
                    trace_worker!("accept end of {:?} from {} on port {:?}", end.tag, src, port);
                    state.on_end(src, end)?;
                } else {
                    warn_worker!("unrecognized event;")
                }
            }
            EventKind::Cancel((ch, tag)) => {
                if let Some(Some(handle)) = self.outputs_cancel.get_mut(port.port) {
                    trace_worker!(
                        "output[{:?}] EARLY-STOP: accept cancel of {:?} to ch {} to worker {}",
                        port,
                        tag,
                        ch,
                        src
                    );
                    if let Some(t) = handle.on_cancel(ch, src, &tag) {
                        self.discards.push_back((port, t));
                    }
                } else {
                    warn_worker!("unrecognized event of port {:?}; form worker {}", port, src)
                }
            }
        }
        Ok(())
    }

    pub fn get_discards(&mut self) -> &mut VecDeque<(Port, Tag)> {
        &mut self.discards
    }
}
