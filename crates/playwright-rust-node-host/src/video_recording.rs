//! Optional Chromium screencast recording for Rust-owned browser contexts.
//!
//! CDP capture, FFmpeg process ownership, frame acknowledgement and finalization
//! stay in Rust. Language facades only pass creation options.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use base64::Engine as _;
use playwright_rust_backend_cdp_topology::{CdpContextOwnership, CdpTopology};
use playwright_rust_cdp::{
    CdpConnection, CdpEvent, CdpSession, connection::CdpError, generated::page,
};
use playwright_rust_core::ObjectId;
use playwright_rust_plugin_chromium_frame_observer::{
    ChromiumFrameEventObserver, FrameObservationResult,
};
use playwright_rust_plugin_chromium_session_initialization::{
    ChromiumSessionInitializationFuture, ChromiumSessionInitializationTarget,
    ChromiumSessionInitializer, ChromiumSessionTargetKind,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::HostError;

const FPS: u64 = 25;
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
static NEXT_VIDEO_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentContextVideoOptions {
    directory: PathBuf,
    ffmpeg: PathBuf,
    width: u32,
    height: u32,
}

impl PersistentContextVideoOptions {
    pub fn new(
        directory: PathBuf,
        ffmpeg: PathBuf,
        width: u32,
        height: u32,
    ) -> Result<Self, HostError> {
        if !directory.is_absolute() {
            return Err(HostError::InvalidArgument(
                "record_video_dir must resolve to an absolute path".into(),
            ));
        }
        if !ffmpeg.is_absolute() {
            return Err(HostError::InvalidArgument(
                "FFmpeg executable must resolve to an absolute path".into(),
            ));
        }
        if width < 2 || height < 2 {
            return Err(HostError::InvalidArgument(
                "record_video_size width and height must be at least 2".into(),
            ));
        }
        Ok(Self {
            directory,
            ffmpeg,
            width: width & !1,
            height: height & !1,
        })
    }
}

struct Frame {
    data: String,
    timestamp: f64,
    session_id: i64,
}

enum RecorderCommand {
    Frame(Frame),
    Stop,
}

struct RecorderEntry {
    context: ObjectId,
    session_id: Box<str>,
    sender: mpsc::UnboundedSender<RecorderCommand>,
    started: watch::Sender<bool>,
    completion: watch::Sender<Option<Result<(), String>>>,
    task: JoinHandle<Result<PathBuf, HostError>>,
}

struct FinishingEntry {
    context: ObjectId,
    task: JoinHandle<Result<PathBuf, HostError>>,
}

#[derive(Clone)]
struct ArtifactEntry {
    path: PathBuf,
    started: watch::Receiver<bool>,
    completion: watch::Receiver<Option<Result<(), String>>>,
    deleted: Arc<AtomicBool>,
    operations: Arc<tokio::sync::Mutex<()>>,
}

impl ArtifactEntry {
    async fn wait_for_started(&self) -> Result<bool, HostError> {
        let mut started = self.started.clone();
        let mut completion = self.completion.clone();
        loop {
            if *started.borrow() {
                return Ok(true);
            }
            if let Some(result) = completion.borrow().clone() {
                result.map_err(HostError::Backend)?;
                return Ok(false);
            }
            tokio::select! {
                changed = started.changed() => {
                    changed.map_err(|_| HostError::TargetClosed)?;
                }
                changed = completion.changed() => {
                    changed.map_err(|_| HostError::TargetClosed)?;
                }
            }
        }
    }

    async fn wait_for_finished(&self) -> Result<(), HostError> {
        let mut completion = self.completion.clone();
        loop {
            if let Some(result) = completion.borrow().clone() {
                return result.map_err(HostError::Backend);
            }
            completion
                .changed()
                .await
                .map_err(|_| HostError::TargetClosed)?;
        }
    }

    async fn save_as(&self, destination: &Path) -> Result<(), HostError> {
        if !self.wait_for_started().await? {
            return Err(no_video_frames());
        }
        self.wait_for_finished().await?;
        let _operation = self.operations.lock().await;
        if self.deleted.load(Ordering::Acquire) {
            return Err(HostError::Backend(
                "File already deleted. Save before deleting.".into(),
            ));
        }
        if destination == self.path {
            return Ok(());
        }
        if let Some(parent) = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(HostError::backend)?;
        }
        tokio::fs::copy(&self.path, destination)
            .await
            .map(|_| ())
            .map_err(HostError::backend)
    }

    async fn delete(&self) -> Result<(), HostError> {
        if !self.wait_for_started().await? {
            return Ok(());
        }
        self.wait_for_finished().await?;
        let _operation = self.operations.lock().await;
        if self.deleted.load(Ordering::Acquire) {
            return Ok(());
        }
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => {
                self.deleted.store(true, Ordering::Release);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.deleted.store(true, Ordering::Release);
                Ok(())
            }
            Err(error) => Err(HostError::backend(error)),
        }
    }
}

#[derive(Default)]
struct State {
    recorders: BTreeMap<ObjectId, RecorderEntry>,
    artifacts: BTreeMap<ObjectId, ArtifactEntry>,
    context_options: BTreeMap<ObjectId, PersistentContextVideoOptions>,
    page_by_session: BTreeMap<Box<str>, ObjectId>,
    finishing: Vec<FinishingEntry>,
    closed: bool,
}

impl State {
    fn options_for_context(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
        default: Option<&PersistentContextVideoOptions>,
    ) -> Option<PersistentContextVideoOptions> {
        self.context_options.get(&context).cloned().or_else(|| {
            if ownership == CdpContextOwnership::Default {
                default.cloned()
            } else {
                None
            }
        })
    }
}

pub(crate) struct Installation {
    connection: Weak<CdpConnection>,
    default_options: Option<PersistentContextVideoOptions>,
    state: Mutex<State>,
}

impl Installation {
    pub(crate) fn new(
        connection: Arc<CdpConnection>,
        options: Option<PersistentContextVideoOptions>,
    ) -> Arc<Self> {
        Arc::new(Self {
            connection: Arc::downgrade(&connection),
            default_options: options,
            state: Mutex::new(State::default()),
        })
    }

    pub(crate) fn install_context(
        &self,
        context: ObjectId,
        options: PersistentContextVideoOptions,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if state.context_options.insert(context, options).is_some() {
            return Err(HostError::InvalidArgument(
                "BrowserContext video recording is already configured".into(),
            ));
        }
        Ok(())
    }

    async fn start_page(
        &self,
        target: ChromiumSessionInitializationTarget<'_>,
    ) -> Result<(), HostError> {
        if target.kind != ChromiumSessionTargetKind::Page {
            return Ok(());
        }
        let connection = self.connection.upgrade().ok_or(HostError::TargetClosed)?;
        {
            let state = self.state.lock().map_err(HostError::backend)?;
            if state.closed {
                return Err(HostError::TargetClosed);
            }
            if state.recorders.contains_key(&target.page) {
                return Ok(());
            }
        }
        let options = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state.options_for_context(
                target.context,
                target.ownership,
                self.default_options.as_ref(),
            )
        };
        let Some(options) = options else {
            return Ok(());
        };
        tokio::fs::create_dir_all(&options.directory)
            .await
            .map_err(HostError::backend)?;
        let id = NEXT_VIDEO_ID
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| HostError::ResourceExhausted("video IDs exhausted".into()))?;
        let path = options
            .directory
            .join(format!("playwright-rust-{}-{id}.webm", std::process::id()));
        let recorder =
            VideoRecorder::start(&options.ffmpeg, &path, options.width, options.height).await?;
        let session = CdpSession::new(
            Arc::clone(&connection),
            target.session_id.to_owned(),
            target.target_id.to_owned(),
        );
        let (sender, receiver) = mpsc::unbounded_channel();
        let (started_sender, started) = watch::channel(false);
        let task_path = path.clone();
        let (completion_sender, completion) = watch::channel(None);
        let task_completion = completion_sender.clone();
        let task = tokio::spawn(async move {
            let result = run_recorder(session, receiver, recorder, task_path).await;
            let completion = result.as_ref().map(|_| ()).map_err(ToString::to_string);
            publish_completion(&task_completion, completion);
            result
        });
        {
            let mut state = self.state.lock().map_err(HostError::backend)?;
            if state.closed {
                let _ = sender.send(RecorderCommand::Stop);
                state.finishing.push(FinishingEntry {
                    context: target.context,
                    task,
                });
                return Err(HostError::TargetClosed);
            }
            state
                .page_by_session
                .insert(target.session_id.to_owned().into_boxed_str(), target.page);
            state.recorders.insert(
                target.page,
                RecorderEntry {
                    context: target.context,
                    session_id: target.session_id.to_owned().into_boxed_str(),
                    sender,
                    started: started_sender,
                    completion: completion_sender,
                    task,
                },
            );
            state.artifacts.insert(
                target.page,
                ArtifactEntry {
                    path,
                    started,
                    completion,
                    deleted: Arc::new(AtomicBool::new(false)),
                    operations: Arc::new(tokio::sync::Mutex::new(())),
                },
            );
        }
        let start = session_send(
            &connection,
            target.session_id,
            target.target_id,
            "Page.startScreencast",
            screencast_params(options.width, options.height),
        )
        .await;
        if let Err(error) = start {
            self.stop_page(target.page)?;
            return Err(error);
        }
        Ok(())
    }

    fn frame(&self, event: &CdpEvent) -> Result<(), HostError> {
        let Some(session_id) = event.session_id.as_deref() else {
            return Ok(());
        };
        let frame: page::ScreencastFrameEvent =
            serde_json::from_value(event.params.clone()).map_err(HostError::backend)?;
        let (sender, started) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let Some(page) = state.page_by_session.get(session_id) else {
                return Ok(());
            };
            let Some(entry) = state.recorders.get(page) else {
                return Ok(());
            };
            (entry.sender.clone(), entry.started.clone())
        };
        // Encoding is an artifact failure, not a topology failure. The
        // recorder task publishes its exact error through the artifact
        // completion channel and Context finalization. A receiver that has
        // already stopped must never tear down the browser's event driver.
        if enqueue_frame(
            &sender,
            Frame {
                data: frame.data,
                timestamp: frame.metadata.timestamp.unwrap_or(0.0),
                session_id: frame.session_id,
            },
        ) {
            let _ = started.send(true);
        }
        Ok(())
    }

    fn stop_page(&self, page: ObjectId) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let Some(entry) = state.recorders.remove(&page) else {
            return Ok(());
        };
        state.page_by_session.remove(entry.session_id.as_ref());
        let _ = entry.sender.send(RecorderCommand::Stop);
        state.finishing.push(FinishingEntry {
            context: entry.context,
            task: entry.task,
        });
        Ok(())
    }

    fn reconcile(&self, topology: &CdpTopology) -> Result<(), HostError> {
        let retired = {
            let state = self.state.lock().map_err(HostError::backend)?;
            recorder_pages_to_retire(
                state.recorders.keys().copied(),
                topology.materialized_page_memberships(),
            )
        };
        for page in retired {
            self.stop_page(page)?;
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        state
            .context_options
            .retain(|context, _| topology.browser_context_route(*context).is_some());
        Ok(())
    }

    pub(crate) async fn finish_context(&self, context: ObjectId) -> Result<(), HostError> {
        let tasks = {
            let mut state = self.state.lock().map_err(HostError::backend)?;
            let pages = state
                .recorders
                .iter()
                .filter_map(|(page, entry)| (entry.context == context).then_some(*page))
                .collect::<Vec<_>>();
            for page in pages {
                let entry = state
                    .recorders
                    .remove(&page)
                    .expect("recording Page was collected from the same map");
                state.page_by_session.remove(entry.session_id.as_ref());
                let _ = entry.sender.send(RecorderCommand::Stop);
                state.finishing.push(FinishingEntry {
                    context,
                    task: entry.task,
                });
            }
            state.context_options.remove(&context);
            let finishing = std::mem::take(&mut state.finishing);
            let (selected, retained): (Vec<_>, Vec<_>) = finishing
                .into_iter()
                .partition(|entry| entry.context == context);
            state.finishing = retained;
            selected
                .into_iter()
                .map(|entry| entry.task)
                .collect::<Vec<_>>()
        };
        await_recorders(tasks).await
    }

    pub(crate) async fn close(&self) -> Result<(), HostError> {
        let tasks = {
            let mut state = self.state.lock().map_err(HostError::backend)?;
            state.closed = true;
            let recorders = std::mem::take(&mut state.recorders);
            state.page_by_session.clear();
            for (_, entry) in recorders {
                publish_completion(&entry.completion, Err("browser has been closed".to_owned()));
                let _ = entry.sender.send(RecorderCommand::Stop);
                state.finishing.push(FinishingEntry {
                    context: entry.context,
                    task: entry.task,
                });
            }
            std::mem::take(&mut state.finishing)
                .into_iter()
                .map(|entry| entry.task)
                .collect::<Vec<_>>()
        };
        await_recorders(tasks).await
    }

    pub(crate) async fn path(&self, page: ObjectId) -> Result<PathBuf, HostError> {
        let artifact = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state.artifacts.get(&page).cloned()
        }
        .ok_or_else(no_video_frames)?;
        if !artifact.wait_for_started().await? {
            return Err(no_video_frames());
        }
        Ok(artifact.path)
    }

    pub(crate) async fn save_as(
        &self,
        page: ObjectId,
        destination: &Path,
    ) -> Result<(), HostError> {
        let artifact = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state.artifacts.get(&page).cloned()
        }
        .ok_or_else(no_video_frames)?;
        artifact.save_as(destination).await
    }

    pub(crate) async fn delete(&self, page: ObjectId) -> Result<(), HostError> {
        let Some(artifact) = ({
            let state = self.state.lock().map_err(HostError::backend)?;
            state.artifacts.get(&page).cloned()
        }) else {
            return Ok(());
        };
        artifact.delete().await
    }

    pub(crate) fn abort(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            for (_, entry) in std::mem::take(&mut state.recorders) {
                publish_completion(&entry.completion, Err("browser has been closed".to_owned()));
                entry.task.abort();
            }
            for entry in state.finishing.drain(..) {
                entry.task.abort();
            }
            state.page_by_session.clear();
        }
    }
}

fn recorder_pages_to_retire(
    recorders: impl Iterator<Item = ObjectId>,
    memberships: impl Iterator<Item = (ObjectId, ObjectId)>,
) -> Vec<ObjectId> {
    let materialized = memberships
        .map(|(page, _context)| page)
        .collect::<BTreeSet<_>>();
    recorders
        .filter(|page| !materialized.contains(page))
        .collect()
}

fn enqueue_frame(sender: &mpsc::UnboundedSender<RecorderCommand>, frame: Frame) -> bool {
    sender.send(RecorderCommand::Frame(frame)).is_ok()
}

fn publish_completion(
    sender: &watch::Sender<Option<Result<(), String>>>,
    result: Result<(), String>,
) {
    sender.send_if_modified(|completion| {
        if completion.is_some() {
            return false;
        }
        *completion = Some(result);
        true
    });
}

fn no_video_frames() -> HostError {
    HostError::Backend("Page did not produce any video frames".into())
}

fn screencast_params(width: u32, height: u32) -> serde_json::Value {
    serde_json::json!({
        "format": "jpeg",
        "quality": 90,
        "maxWidth": width,
        "maxHeight": height,
    })
}

async fn await_recorders(
    tasks: Vec<JoinHandle<Result<PathBuf, HostError>>>,
) -> Result<(), HostError> {
    let mut outcome = Ok(());
    for task in tasks {
        let result = task.await.map_err(HostError::backend)?;
        if outcome.is_ok() {
            outcome = result.map(|_| ());
        }
    }
    outcome
}

async fn session_send(
    connection: &Arc<CdpConnection>,
    session_id: &str,
    target_id: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<(), HostError> {
    CdpSession::new(
        Arc::clone(connection),
        session_id.to_owned(),
        target_id.to_owned(),
    )
    .send(method, params)
    .await
    .map(|_| ())
    .map_err(HostError::backend)
}

async fn run_recorder(
    session: CdpSession,
    mut receiver: mpsc::UnboundedReceiver<RecorderCommand>,
    mut recorder: VideoRecorder,
    path: PathBuf,
) -> Result<PathBuf, HostError> {
    while let Some(command) = receiver.recv().await {
        match command {
            RecorderCommand::Frame(frame) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(frame.data)
                    .map_err(HostError::backend)?;
                let write = recorder.write_frame(&bytes, frame.timestamp).await;
                let ack = match session
                    .send(
                        "Page.screencastFrameAck",
                        serde_json::json!({"sessionId": frame.session_id}),
                    )
                    .await
                {
                    Ok(_) => Ok(()),
                    Err(error) if retired_screencast_session(&error) => Ok(()),
                    Err(error) => Err(HostError::backend(error)),
                };
                write.and(ack)?;
            }
            RecorderCommand::Stop => break,
        }
    }
    let _ = session
        .send("Page.stopScreencast", serde_json::json!({}))
        .await;
    let had_frames = recorder.has_frames();
    recorder.stop().await?;
    if !had_frames {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(HostError::backend(error)),
        }
    }
    Ok(path)
}

fn retired_screencast_session(error: &CdpError) -> bool {
    match error {
        CdpError::SessionDetached | CdpError::PageClosed => true,
        CdpError::Protocol {
            code: -32001,
            message,
        } => [
            "Session with given id not found.",
            "Session with given id not found",
            "No session with given id",
            "No session with given id.",
        ]
        .contains(&message.as_str()),
        _ => false,
    }
}

struct VideoRecorder {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr: Option<JoinHandle<Result<Vec<u8>, std::io::Error>>>,
    first_timestamp: Option<f64>,
    last_frame: Option<(u64, Vec<u8>)>,
}

impl VideoRecorder {
    async fn start(
        ffmpeg: &Path,
        output: &Path,
        width: u32,
        height: u32,
    ) -> Result<Self, HostError> {
        let filter = format!("pad={width}:{height}:0:0:gray,crop={width}:{height}:0:0");
        let mut child = Command::new(ffmpeg)
            .args([
                "-loglevel",
                "error",
                "-f",
                "image2pipe",
                "-avioflags",
                "direct",
                "-fpsprobesize",
                "0",
                "-probesize",
                "32",
                "-analyzeduration",
                "0",
                "-c:v",
                "mjpeg",
                "-i",
                "pipe:0",
                "-y",
                "-an",
                "-r",
                "25",
                "-c:v",
                "vp8",
                "-qmin",
                "0",
                "-qmax",
                "50",
                "-crf",
                "8",
                "-deadline",
                "realtime",
                "-speed",
                "8",
                "-b:v",
                "1M",
                "-threads",
                "1",
                "-vf",
                &filter,
            ])
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| {
                HostError::Backend(format!(
                    "could not launch FFmpeg at {}: {error}",
                    ffmpeg.display()
                ))
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| HostError::Backend("FFmpeg stdin was not piped".into()))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| HostError::Backend("FFmpeg stderr was not piped".into()))?;
        let stderr = tokio::spawn(async move {
            let mut output = Vec::new();
            stderr.read_to_end(&mut output).await?;
            Ok(output)
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            stderr: Some(stderr),
            first_timestamp: None,
            last_frame: None,
        })
    }

    async fn write_frame(&mut self, frame: &[u8], timestamp: f64) -> Result<(), HostError> {
        let first = *self.first_timestamp.get_or_insert(timestamp);
        let number = ((timestamp - first).max(0.0) * FPS as f64).floor() as u64;
        if let Some((previous, bytes)) = self.last_frame.as_ref() {
            let repeat = number.saturating_sub(*previous);
            for _ in 0..repeat {
                self.stdin
                    .as_mut()
                    .ok_or(HostError::TargetClosed)?
                    .write_all(bytes)
                    .await
                    .map_err(HostError::backend)?;
            }
        }
        self.last_frame = Some((number, frame.to_vec()));
        Ok(())
    }

    fn has_frames(&self) -> bool {
        self.last_frame.is_some()
    }

    async fn stop(&mut self) -> Result<(), HostError> {
        if self.last_frame.is_none() {
            drop(self.stdin.take());
            match tokio::time::timeout(STOP_TIMEOUT, self.child.wait()).await {
                Ok(result) => {
                    let _ = result.map_err(HostError::backend)?;
                }
                Err(_) => {
                    self.child.kill().await.map_err(HostError::backend)?;
                }
            }
            if let Some(stderr) = self.stderr.take() {
                let _ = stderr
                    .await
                    .map_err(HostError::backend)?
                    .map_err(HostError::backend)?;
            }
            return Ok(());
        }
        if let Some((_, frame)) = self.last_frame.as_ref() {
            for _ in 0..FPS {
                self.stdin
                    .as_mut()
                    .ok_or(HostError::TargetClosed)?
                    .write_all(frame)
                    .await
                    .map_err(HostError::backend)?;
            }
        }
        if let Some(mut stdin) = self.stdin.take() {
            stdin.shutdown().await.map_err(HostError::backend)?;
        }
        let status = match tokio::time::timeout(STOP_TIMEOUT, self.child.wait()).await {
            Ok(result) => result.map_err(HostError::backend)?,
            Err(_) => {
                let _ = self.child.kill().await;
                return Err(HostError::Backend("FFmpeg finalization timed out".into()));
            }
        };
        let stderr = self
            .stderr
            .take()
            .ok_or_else(|| HostError::Backend("FFmpeg stderr was already consumed".into()))?
            .await
            .map_err(HostError::backend)?
            .map_err(HostError::backend)?;
        if status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&stderr);
        let stderr = stderr.trim();
        Err(HostError::Backend(if stderr.is_empty() {
            format!("FFmpeg exited unsuccessfully: {status}")
        } else {
            format!("FFmpeg exited unsuccessfully: {status}: {stderr}")
        }))
    }
}

pub(crate) struct Initializer<Downstream> {
    downstream: Downstream,
    installation: Arc<Installation>,
}

impl<Downstream> Initializer<Downstream> {
    pub(crate) fn new(downstream: Downstream, installation: Arc<Installation>) -> Self {
        Self {
            downstream,
            installation,
        }
    }
}

impl<Downstream> ChromiumSessionInitializer for Initializer<Downstream>
where
    Downstream: ChromiumSessionInitializer,
{
    fn initialize<'a>(
        &'a self,
        session_id: &'a str,
        target_id: &'a str,
        ownership: CdpContextOwnership,
        is_page: bool,
    ) -> ChromiumSessionInitializationFuture<'a> {
        self.downstream
            .initialize(session_id, target_id, ownership, is_page)
    }

    fn initialize_scoped<'a>(
        &'a self,
        target: ChromiumSessionInitializationTarget<'a>,
    ) -> ChromiumSessionInitializationFuture<'a> {
        Box::pin(async move {
            self.downstream.initialize_scoped(target).await?;
            self.installation
                .start_page(target)
                .await
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
        })
    }
}

pub(crate) struct Observer {
    installation: Arc<Installation>,
    downstream: Arc<dyn ChromiumFrameEventObserver>,
}

impl Observer {
    pub(crate) fn new(
        installation: Arc<Installation>,
        downstream: Arc<dyn ChromiumFrameEventObserver>,
    ) -> Self {
        Self {
            installation,
            downstream,
        }
    }
}

impl ChromiumFrameEventObserver for Observer {
    fn observes_page(&self, page: ObjectId) -> bool {
        self.downstream.observes_page(page)
    }

    fn prepare_page(&self, topology: &CdpTopology, page: ObjectId) -> FrameObservationResult {
        self.downstream.prepare_page(topology, page)
    }

    fn discard_prepared_page(
        &self,
        topology: &CdpTopology,
        page: ObjectId,
    ) -> FrameObservationResult {
        self.downstream.discard_prepared_page(topology, page)
    }

    fn initialize_frame(
        &self,
        topology: &CdpTopology,
        session: &str,
        frame: &page::Frame,
    ) -> FrameObservationResult {
        self.downstream.initialize_frame(topology, session, frame)
    }

    fn reconcile(&self, topology: &CdpTopology) -> FrameObservationResult {
        self.downstream.reconcile(topology)?;
        self.installation
            .reconcile(topology)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
    }

    fn event(&self, topology: &CdpTopology, event: &CdpEvent) -> FrameObservationResult {
        self.downstream.event(topology, event)?;
        if event.method == "Page.screencastFrame" {
            self.installation
                .frame(event)
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?;
        }
        Ok(())
    }

    fn seal(&self) {
        self.installation.abort();
        self.downstream.seal();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactEntry, Frame, PersistentContextVideoOptions, State, enqueue_frame,
        publish_completion, recorder_pages_to_retire, retired_screencast_session,
    };
    use playwright_rust_backend_cdp_topology::CdpContextOwnership;
    use playwright_rust_cdp::connection::CdpError;
    use playwright_rust_core::ObjectId;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::watch;

    #[test]
    fn stopped_encoder_receiver_does_not_become_a_topology_error() {
        // Arrange
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        drop(receiver);
        let frame = Frame {
            data: String::new(),
            timestamp: 0.0,
            session_id: 1,
        };

        // Act
        let accepted = enqueue_frame(&sender, frame);

        // Assert
        assert!(!accepted);
    }

    #[test]
    fn browser_close_error_is_not_overwritten_by_late_encoder_success() {
        // Arrange
        let (sender, receiver) = watch::channel(None);

        // Act
        publish_completion(&sender, Err("browser has been closed".to_owned()));
        publish_completion(&sender, Ok(()));

        // Assert
        assert_eq!(
            receiver.borrow().clone(),
            Some(Err("browser has been closed".to_owned()))
        );
    }

    #[test]
    fn frame_ack_accepts_only_terminal_page_session_errors() {
        // Arrange
        let retired = CdpError::Protocol {
            code: -32001,
            message: "Session with given id not found.".into(),
        };
        let unrelated = CdpError::Protocol {
            code: -32001,
            message: "permission denied".into(),
        };

        // Act
        let retired_is_terminal = retired_screencast_session(&retired);
        let detached_is_terminal = retired_screencast_session(&CdpError::SessionDetached);
        let unrelated_is_terminal = retired_screencast_session(&unrelated);

        // Assert
        assert!(retired_is_terminal);
        assert!(detached_is_terminal);
        assert!(!unrelated_is_terminal);
    }

    #[test]
    fn materialized_page_membership_keeps_recorder_alive_without_a_frame_route() {
        // Arrange
        let live_page = ObjectId::new(1);
        let retired_page = ObjectId::new(2);
        let context = ObjectId::new(3);

        // Act
        let retired = recorder_pages_to_retire(
            [live_page, retired_page].into_iter(),
            [(live_page, context)].into_iter(),
        );

        // Assert
        assert_eq!(retired, vec![retired_page]);
    }

    #[test]
    fn options_normalize_dimensions_to_encoder_safe_even_values() {
        // Arrange
        let directory = std::path::PathBuf::from("/tmp/videos");
        let ffmpeg = std::path::PathBuf::from("/tmp/ffmpeg");

        // Act
        let options = PersistentContextVideoOptions::new(directory, ffmpeg, 801, 601).unwrap();

        // Assert
        assert_eq!(options.width, 800);
        assert_eq!(options.height, 600);
    }

    #[test]
    fn explicit_context_video_options_do_not_leak_from_the_default_context() {
        // Arrange
        let default = PersistentContextVideoOptions::new(
            "/tmp/default-videos".into(),
            "/tmp/ffmpeg".into(),
            800,
            450,
        )
        .unwrap();
        let explicit = PersistentContextVideoOptions::new(
            "/tmp/explicit-videos".into(),
            "/tmp/ffmpeg".into(),
            640,
            360,
        )
        .unwrap();
        let default_context = ObjectId::new(1);
        let plain_owned_context = ObjectId::new(2);
        let recording_owned_context = ObjectId::new(3);
        let mut state = State::default();
        state
            .context_options
            .insert(recording_owned_context, explicit.clone());

        // Act
        let selected_default = state.options_for_context(
            default_context,
            CdpContextOwnership::Default,
            Some(&default),
        );
        let selected_plain = state.options_for_context(
            plain_owned_context,
            CdpContextOwnership::Owned,
            Some(&default),
        );
        let selected_explicit = state.options_for_context(
            recording_owned_context,
            CdpContextOwnership::Owned,
            Some(&default),
        );

        // Assert
        assert_eq!(selected_default, Some(default));
        assert_eq!(selected_plain, None);
        assert_eq!(selected_explicit, Some(explicit));
    }

    #[tokio::test]
    async fn finished_artifact_saves_to_nested_path_and_delete_is_idempotent() {
        // Arrange
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source.webm");
        let destination = temporary.path().join("nested/saved.webm");
        tokio::fs::write(&source, b"video").await.unwrap();
        let (_started_sender, started) = watch::channel(true);
        let (completion_sender, completion) = watch::channel(None);
        let artifact = ArtifactEntry {
            path: source.clone(),
            started,
            completion,
            deleted: Arc::new(AtomicBool::new(false)),
            operations: Arc::new(tokio::sync::Mutex::new(())),
        };
        completion_sender.send(Some(Ok(()))).unwrap();

        // Act
        artifact.save_as(&destination).await.unwrap();
        artifact.delete().await.unwrap();
        artifact.delete().await.unwrap();

        // Assert
        assert_eq!(tokio::fs::read(destination).await.unwrap(), b"video");
        assert!(!source.exists());
        assert_eq!(
            artifact.save_as(&source).await.unwrap_err().to_string(),
            "File already deleted. Save before deleting."
        );
    }

    #[tokio::test]
    async fn failed_artifact_delete_remains_retryable() {
        // Arrange
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source.webm");
        tokio::fs::create_dir(&source).await.unwrap();
        let (_started_sender, started) = watch::channel(true);
        let (completion_sender, completion) = watch::channel(None);
        let deleted = Arc::new(AtomicBool::new(false));
        let artifact = ArtifactEntry {
            path: source,
            started,
            completion,
            deleted: Arc::clone(&deleted),
            operations: Arc::new(tokio::sync::Mutex::new(())),
        };
        completion_sender.send(Some(Ok(()))).unwrap();

        // Act
        let result = artifact.delete().await;

        // Assert
        assert!(result.is_err());
        assert!(!deleted.load(std::sync::atomic::Ordering::Acquire));
    }

    #[tokio::test]
    async fn completed_artifact_without_a_frame_matches_playwright_video_error() {
        // Arrange
        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join("saved.webm");
        let (_started_sender, started) = watch::channel(false);
        let (completion_sender, completion) = watch::channel(None);
        let artifact = ArtifactEntry {
            path: temporary.path().join("source.webm"),
            started,
            completion,
            deleted: Arc::new(AtomicBool::new(false)),
            operations: Arc::new(tokio::sync::Mutex::new(())),
        };
        completion_sender.send(Some(Ok(()))).unwrap();

        // Act
        let error = artifact.save_as(&destination).await.unwrap_err();

        // Assert
        assert_eq!(error.to_string(), "Page did not produce any video frames");
        assert!(!destination.exists());
    }
}
