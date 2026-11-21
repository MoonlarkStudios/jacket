//! One admitted operation owns its cancellation token until its worker exits.
use crate::auth_session;
use crate::browser_sessions::AccessCheck;
use crate::downloader_updates::Status;
use audio_engine::{Cancellation, ExportResult, Inspection, InspectionUpdate, ProgressEvent};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Clone, Debug)]
pub enum Event {
    Track(Box<InspectionUpdate>),
    Progress(ProgressEvent),
    Inspected(Inspection),
    Exported(ExportResult),
    AccessChecked(AccessCheck),
    Downloader(Status),
    Session(auth_session::Status),
    Failed(String),
}

/// A bounded event queue plus its supervised worker. Poll from the UI tick.
/// Dropping cancels subprocesses, disconnects the queue, and joins the worker.
pub struct Job {
    id: String,
    cancel: Cancellation,
    events: Option<Receiver<Event>>,
    worker: Option<JoinHandle<Event>>,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Job")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

pub(crate) type Admission = Arc<Mutex<Option<Cancellation>>>;

pub(crate) struct Lease {
    admission: Admission,
    cancel: Cancellation,
}

impl Lease {
    pub(crate) fn acquire(admission: &Admission) -> Result<Self, String> {
        let mut active = admission
            .lock()
            .map_err(|_| "Desktop job state is unavailable.")?;
        if active.is_some() {
            return Err("Wait for the current operation to finish.".into());
        }
        let cancel = Cancellation::default();
        *active = Some(cancel.clone());
        Ok(Self {
            admission: Arc::clone(admission),
            cancel,
        })
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.admission.lock() {
            *active = None;
        }
    }
}

impl Job {
    pub(crate) fn spawn(
        id: String,
        admission: &Admission,
        run: impl FnOnce(&Cancellation, &SyncSender<Event>) -> Result<Event, String> + Send + 'static,
    ) -> Result<Self, String> {
        let lease = Lease::acquire(admission)?;
        let cancel = lease.cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(32);
        let worker_cancel = cancel.clone();
        let worker = thread::Builder::new()
            .name(format!("jacket-{id}"))
            .spawn(move || {
                let _lease = lease;
                match run(&worker_cancel, &sender) {
                    Ok(event) => event,
                    Err(error) => Event::Failed(error),
                }
            })
            .map_err(|error| format!("Could not start the background operation: {error}"))?;
        Ok(Self {
            id,
            cancel,
            events: Some(receiver),
            worker: Some(worker),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn is_finished(&self) -> bool {
        self.worker.is_none()
    }

    /// Streaming messages are delivered before the terminal join result.
    /// A worker panic is always reported as a terminal failure.
    pub fn poll(&mut self) -> Option<Event> {
        if let Some(receiver) = &self.events {
            match receiver.try_recv() {
                Ok(event) => return Some(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
        }
        if !self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        // The worker may have queued its final update between our first
        // receive and the finished check. Once finished, drain again before
        // taking its terminal result; no later sends can race this receive.
        if let Some(receiver) = &self.events {
            if let Ok(event) = receiver.try_recv() {
                return Some(event);
            }
        }
        let worker = self.worker.take()?;
        self.events = None;
        Some(match worker.join() {
            Ok(event) => event,
            Err(_) => Event::Failed("The background operation stopped unexpectedly. Retry the operation; downloaded media is retained for recovery.".into()),
        })
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
        // Unblock a streaming sender before joining it.
        self.events = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Admission, Event, Job};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn terminal(job: &mut Job) -> Event {
        let start = Instant::now();
        loop {
            if let Some(event) = job.poll() {
                return event;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "worker did not complete"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_failed_worker_is_supervised_and_releases_admission() {
        let admission: Admission = Arc::new(Mutex::new(None));
        let mut job = Job::spawn("panic".into(), &admission, |_, _| {
            panic!("fixture worker panic")
        })
        .unwrap();
        assert!(matches!(terminal(&mut job), Event::Failed(_)));
        assert!(job.is_finished());
        assert!(admission.lock().unwrap().is_none());
        let mut next =
            Job::spawn("next".into(), &admission, |_, _| Err("expected".into())).unwrap();
        assert!(matches!(terminal(&mut next), Event::Failed(_)));
    }

    #[test]
    fn busy_rejection_does_not_cancel_the_admitted_operation() {
        let admission: Admission = Arc::new(Mutex::new(None));
        let (release, wait) = std::sync::mpsc::channel();
        let mut job = Job::spawn("first".into(), &admission, move |cancel, _| {
            wait.recv().unwrap();
            cancel.check().map_err(|error| error.to_string())?;
            Err("first finished".into())
        })
        .unwrap();
        assert!(Job::spawn("rejected".into(), &admission, |_, _| Err("unused".into())).is_err());
        release.send(()).unwrap();
        match terminal(&mut job) {
            Event::Failed(error) => assert_eq!(error, "first finished"),
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn teardown_disconnects_a_full_queue_before_joining() {
        let admission: Admission = Arc::new(Mutex::new(None));
        let (ready, wait) = std::sync::mpsc::channel();
        let job = Job::spawn("full".into(), &admission, move |_, sender| {
            for _ in 0..32 {
                sender.send(Event::Failed("progress".into())).unwrap();
            }
            ready.send(()).unwrap();
            let _ = sender.send(Event::Failed("blocked".into()));
            Err("done".into())
        })
        .unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(job);
        assert!(admission.lock().unwrap().is_none());
    }

    #[test]
    fn cancellation_tokens_are_never_reused() {
        let admission: Admission = Arc::new(Mutex::new(None));
        let mut first = Job::spawn("first".into(), &admission, |_, _| Err("done".into())).unwrap();
        terminal(&mut first);
        first.cancel();
        let mut next = Job::spawn("second".into(), &admission, |cancel, _| {
            cancel.check().map_err(|error| error.to_string())?;
            Err("new token".into())
        })
        .unwrap();
        match terminal(&mut next) {
            Event::Failed(error) => assert_eq!(error, "new token"),
            _ => panic!("unexpected event"),
        }
    }
    #[test]
    fn streaming_events_remain_ordered_before_the_terminal_result() {
        let admission: Admission = Arc::new(Mutex::new(None));
        let mut job = Job::spawn("ordered".into(), &admission, |_, sender| {
            for completed in 0..128 {
                sender
                    .send(Event::Progress(audio_engine::ProgressEvent {
                        stage: "fixture".into(),
                        message: String::new(),
                        completed,
                        total: 128,
                    }))
                    .unwrap();
            }
            Err("terminal".into())
        })
        .unwrap();
        for expected in 0..128 {
            match terminal(&mut job) {
                Event::Progress(progress) => assert_eq!(progress.completed, expected),
                _ => panic!("terminal overtook an ordered event"),
            }
            assert!(!job.is_finished());
        }
        assert!(matches!(terminal(&mut job), Event::Failed(_)));
        assert!(job.is_finished());
    }
}
