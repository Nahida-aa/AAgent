use anyhow::{Result, bail};
use dap_types::messages::Response;
use futures::channel::oneshot;
use std::collections::HashMap;

pub(crate) struct PendingRequests {
    inner: Option<HashMap<u64, oneshot::Sender<Result<Response>>>>,
}

impl PendingRequests {
    pub(crate) fn new() -> Self {
        Self {
            inner: Some(HashMap::default()),
        }
    }

    pub(crate) fn flush(&mut self, e: anyhow::Error) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        for (_, sender) in inner.drain() {
            sender.send(Err(e.cloned())).ok();
        }
    }

    pub(crate) fn insert(
        &mut self,
        sequence_id: u64,
        callback_tx: oneshot::Sender<Result<Response>>,
    ) -> Result<()> {
        let Some(inner) = self.inner.as_mut() else {
            bail!("client is closed")
        };
        inner.insert(sequence_id, callback_tx);
        Ok(())
    }

    pub(crate) fn remove(
        &mut self,
        sequence_id: u64,
    ) -> Result<Option<oneshot::Sender<Result<Response>>>> {
        let Some(inner) = self.inner.as_mut() else {
            bail!("client is closed");
        };
        Ok(inner.remove(&sequence_id))
    }

    pub(crate) fn shutdown(&mut self) {
        self.flush(anyhow::anyhow!("transport shutdown"));
        self.inner = None;
    }
}
