use smallvec::SmallVec;
use std::sync::Arc;

use parking_lot::Mutex;

pub type IoMessage = str;
pub type Command = str;

pub type IoHandler = Box<dyn Send + FnMut(IoKind, Option<&Command>, &IoMessage)>;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum LogKind {
    Adapter,
    Rpc,
}

#[derive(Clone, Copy)]
pub enum IoKind {
    StdIn,
    StdOut,
    StdErr,
}

pub type LogHandlers = Arc<Mutex<SmallVec<[(LogKind, IoHandler); 2]>>>;

#[cfg(any(test, feature = "test-support"))]
pub enum RequestHandling<T> {
    Respond(T),
    Exit,
}
