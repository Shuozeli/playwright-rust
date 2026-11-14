//! Bounded canonical wire adapters for the independently installable Mouse.
use playwright_rust_cap_mouse::*;
use playwright_rust_codec_binary::{
    BinaryValueDecodeError, BinaryValueEncodeError, BinaryValueLimits, decode_value, encode_value,
};
use playwright_rust_core::{
    CallId, CallMetadata, CancellationId, Command, CommandResult, CommandSpec, FailureKind,
    MethodTag, ObjectId, OperationError,
};
use playwright_rust_transport::{EncodedParts, FrameRoute, WireCodec, WireFrame};
use std::{fmt, time::Duration};

#[doc(hidden)]
#[rustfmt::skip]
pub mod generated_schema;
use generated_schema::*;
pub use generated_schema::{
    CATALOG as MOUSE_SCHEMA_CATALOG, COMMANDS as MOUSE_COMMAND_SCHEMAS, ROOT_SCHEMA as MOUSE_SCHEMA,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MouseCodecLimits {
    pub string_bytes: usize,
}
impl MouseCodecLimits {
    pub const fn new(string_bytes: usize) -> Self {
        Self { string_bytes }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MouseCodec {
    limits: MouseCodecLimits,
}
impl MouseCodec {
    pub const fn new(limits: MouseCodecLimits) -> Self {
        Self { limits }
    }
    pub const fn limits(self) -> MouseCodecLimits {
        self.limits
    }
    fn value_limits(self) -> BinaryValueLimits {
        BinaryValueLimits::new(self.limits.string_bytes, 0, 0, 8, 16)
    }
}

#[derive(Debug)]
pub enum MouseCodecError {
    WrongRoute(FrameRoute),
    WrongMethod {
        expected: MethodTag,
        actual: MethodTag,
    },
    IdentityMismatch {
        field: &'static str,
        outer: u64,
        inner: u64,
    },
    AttachmentsUnsupported(usize),
    WrongPayload {
        expected: &'static str,
        actual: u8,
    },
    Argument(MouseArgumentError),
    Encode(BinaryValueEncodeError),
    Decode(BinaryValueDecodeError),
}
impl fmt::Display for MouseCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRoute(route) => write!(f, "Mouse received wrong route {route:?}"),
            Self::WrongMethod { expected, actual } => {
                write!(f, "Mouse expected method {expected}, received {actual}")
            }
            Self::IdentityMismatch {
                field,
                outer,
                inner,
            } => write!(f, "Mouse {field} mismatch: outer {outer}, inner {inner}"),
            Self::AttachmentsUnsupported(count) => {
                write!(f, "Mouse accepts no attachments, received {count}")
            }
            Self::WrongPayload { expected, actual } => write!(
                f,
                "Mouse expected {expected}, received payload tag {actual}"
            ),
            Self::Argument(error) => error.fmt(f),
            Self::Encode(error) => error.fmt(f),
            Self::Decode(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for MouseCodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Argument(e) => Some(e),
            Self::Encode(e) => Some(e),
            Self::Decode(e) => Some(e),
            _ => None,
        }
    }
}

trait MouseWireSpec: CommandSpec<Result = (), ErrorDetails = MouseErrorDetails> + Sized {
    fn command_ref(command: &Command<Self>) -> PlaywrightRustMouseRef<'_>;
    fn command_from_wire(wire: PlaywrightRustMouse) -> Result<Command<Self>, MouseCodecError>;
    fn result_ref(result: &CommandResult<Self>) -> PlaywrightRustMouseRef<'_>;
    fn result_from_wire(wire: PlaywrightRustMouse) -> Result<CommandResult<Self>, MouseCodecError>;
}

impl<S: MouseWireSpec> WireCodec<Command<S>> for MouseCodec {
    type Error = MouseCodecError;
    fn encode_parts(&self, command: &Command<S>) -> Result<EncodedParts, Self::Error> {
        let payload = encode_value(&S::command_ref(command), self.value_limits())
            .map_err(MouseCodecError::Encode)?;
        Ok(EncodedParts::new(
            FrameRoute::Command {
                call_id: command.call_id(),
                target: command.object_id(),
                method: S::METHOD,
            },
            payload,
            Vec::new(),
        ))
    }
    fn decode_frame(&self, frame: WireFrame) -> Result<Command<S>, Self::Error> {
        let FrameRoute::Command {
            call_id,
            target,
            method,
        } = frame.route()
        else {
            return Err(MouseCodecError::WrongRoute(frame.route()));
        };
        if method != S::METHOD {
            return Err(MouseCodecError::WrongMethod {
                expected: S::METHOD,
                actual: method,
            });
        }
        ensure_no_attachments(&frame)?;
        let wire = decode_value::<PlaywrightRustMouse>(frame.payload(), self.value_limits())
            .map_err(MouseCodecError::Decode)?;
        let command = S::command_from_wire(wire)?;
        identity("call ID", call_id.get(), command.call_id().get())?;
        identity("Page ID", target.get(), command.object_id().get())?;
        Ok(command)
    }
}

impl<S: MouseWireSpec> WireCodec<CommandResult<S>> for MouseCodec {
    type Error = MouseCodecError;
    fn encode_parts(&self, result: &CommandResult<S>) -> Result<EncodedParts, Self::Error> {
        let payload = encode_value(&S::result_ref(result), self.value_limits())
            .map_err(MouseCodecError::Encode)?;
        Ok(EncodedParts::new(
            FrameRoute::Result {
                call_id: result.call_id(),
            },
            payload,
            Vec::new(),
        ))
    }
    fn decode_frame(&self, frame: WireFrame) -> Result<CommandResult<S>, Self::Error> {
        let FrameRoute::Result { call_id } = frame.route() else {
            return Err(MouseCodecError::WrongRoute(frame.route()));
        };
        ensure_no_attachments(&frame)?;
        let wire = decode_value::<PlaywrightRustMouse>(frame.payload(), self.value_limits())
            .map_err(MouseCodecError::Decode)?;
        let result = S::result_from_wire(wire)?;
        identity("call ID", call_id.get(), result.call_id().get())?;
        Ok(result)
    }
}

macro_rules! mouse_spec {
    ($command:ty, $cmd:ident, $result:ident, $encode:ident, $decode:ident) => {
        impl MouseWireSpec for $command {
            fn command_ref(command: &Command<Self>) -> PlaywrightRustMouseRef<'_> {
                PlaywrightRustMouseRef::$cmd {
                    call_id: command.call_id().get(),
                    object_id: command.object_id().get(),
                    metadata: metadata_to_wire(command.metadata()),
                    params: $encode(command.params()),
                }
            }
            fn command_from_wire(
                wire: PlaywrightRustMouse,
            ) -> Result<Command<Self>, MouseCodecError> {
                match wire {
                    PlaywrightRustMouse::$cmd {
                        call_id,
                        object_id,
                        metadata,
                        params,
                    } => Ok(Command::new(
                        CallId::new(call_id),
                        ObjectId::new(object_id),
                        metadata_from_wire(metadata),
                        $decode(params)?,
                    )),
                    other => Err(MouseCodecError::WrongPayload {
                        expected: stringify!($cmd),
                        actual: wire_kind(&other),
                    }),
                }
            }
            fn result_ref(result: &CommandResult<Self>) -> PlaywrightRustMouseRef<'_> {
                PlaywrightRustMouseRef::$result {
                    call_id: result.call_id().get(),
                    outcome: outcome_to_wire(result.outcome()),
                }
            }
            fn result_from_wire(
                wire: PlaywrightRustMouse,
            ) -> Result<CommandResult<Self>, MouseCodecError> {
                match wire {
                    PlaywrightRustMouse::$result { call_id, outcome } => Ok(CommandResult::new(
                        CallId::new(call_id),
                        outcome_from_wire(outcome),
                    )),
                    other => Err(MouseCodecError::WrongPayload {
                        expected: stringify!($result),
                        actual: wire_kind(&other),
                    }),
                }
            }
        }
    };
}
mouse_spec!(
    MouseMove,
    MouseMoveCommand,
    MouseMoveResult,
    move_to_wire,
    move_from_wire
);
mouse_spec!(
    MouseDown,
    MouseDownCommand,
    MouseDownResult,
    down_to_wire,
    down_from_wire
);
mouse_spec!(
    MouseUp,
    MouseUpCommand,
    MouseUpResult,
    up_to_wire,
    up_from_wire
);
mouse_spec!(
    MouseClick,
    MouseClickCommand,
    MouseClickResult,
    click_to_wire,
    click_from_wire
);
mouse_spec!(
    MouseDblclick,
    MouseDblclickCommand,
    MouseDblclickResult,
    dblclick_to_wire,
    dblclick_from_wire
);
mouse_spec!(
    MouseWheel,
    MouseWheelCommand,
    MouseWheelResult,
    wheel_to_wire,
    wheel_from_wire
);
fn move_to_wire(params: &MoveParams) -> PlaywrightRustMouseMoveParams {
    PlaywrightRustMouseMoveParams {
        x: params.x.get(),
        y: params.y.get(),
        steps: params.steps.get(),
    }
}
fn move_from_wire(params: PlaywrightRustMouseMoveParams) -> Result<MoveParams, MouseCodecError> {
    Ok(MoveParams {
        x: MouseNumber::try_from(params.x).map_err(MouseCodecError::Argument)?,
        y: MouseNumber::try_from(params.y).map_err(MouseCodecError::Argument)?,
        steps: MouseMoveSteps::try_from(params.steps)
            .map_err(|error| MouseCodecError::Argument(MouseArgumentError::Steps(error)))?,
    })
}
fn down_to_wire(params: &DownParams) -> PlaywrightRustMouseDownParams {
    PlaywrightRustMouseDownParams {
        button: button_to_wire(params.button),
        click_count: params.click_count,
    }
}
fn down_from_wire(params: PlaywrightRustMouseDownParams) -> Result<DownParams, MouseCodecError> {
    Ok(DownParams {
        button: button_from_wire(params.button),
        click_count: params.click_count,
    })
}
fn up_to_wire(params: &UpParams) -> PlaywrightRustMouseUpParams {
    PlaywrightRustMouseUpParams {
        button: button_to_wire(params.button),
        click_count: params.click_count,
    }
}
fn up_from_wire(params: PlaywrightRustMouseUpParams) -> Result<UpParams, MouseCodecError> {
    Ok(UpParams {
        button: button_from_wire(params.button),
        click_count: params.click_count,
    })
}
fn click_to_wire(params: &ClickParams) -> PlaywrightRustMouseClickParams {
    PlaywrightRustMouseClickParams {
        x: params.x.get(),
        y: params.y.get(),
        button: button_to_wire(params.button),
        click_count: params.click_count.get(),
        delay: params.delay.map(duration_to_wire),
    }
}
fn click_from_wire(params: PlaywrightRustMouseClickParams) -> Result<ClickParams, MouseCodecError> {
    Ok(ClickParams {
        x: MouseNumber::try_from(params.x).map_err(MouseCodecError::Argument)?,
        y: MouseNumber::try_from(params.y).map_err(MouseCodecError::Argument)?,
        button: button_from_wire(params.button),
        click_count: MouseClickCount::try_from(f64::from(params.click_count))
            .map_err(|error| MouseCodecError::Argument(MouseArgumentError::ClickCount(error)))?,
        delay: params.delay.map(duration_from_wire),
    })
}
fn dblclick_to_wire(params: &DblclickParams) -> PlaywrightRustMouseDblclickParams {
    PlaywrightRustMouseDblclickParams {
        x: params.x.get(),
        y: params.y.get(),
        button: button_to_wire(params.button),
        delay: params.delay.map(duration_to_wire),
    }
}
fn dblclick_from_wire(
    params: PlaywrightRustMouseDblclickParams,
) -> Result<DblclickParams, MouseCodecError> {
    Ok(DblclickParams {
        x: MouseNumber::try_from(params.x).map_err(MouseCodecError::Argument)?,
        y: MouseNumber::try_from(params.y).map_err(MouseCodecError::Argument)?,
        button: button_from_wire(params.button),
        delay: params.delay.map(duration_from_wire),
    })
}
fn wheel_to_wire(params: &WheelParams) -> PlaywrightRustMouseWheelParams {
    PlaywrightRustMouseWheelParams {
        delta_x: params.delta_x.get(),
        delta_y: params.delta_y.get(),
    }
}
fn wheel_from_wire(params: PlaywrightRustMouseWheelParams) -> Result<WheelParams, MouseCodecError> {
    Ok(WheelParams {
        delta_x: MouseNumber::try_from(params.delta_x).map_err(MouseCodecError::Argument)?,
        delta_y: MouseNumber::try_from(params.delta_y).map_err(MouseCodecError::Argument)?,
    })
}
fn button_to_wire(button: MouseButton) -> PlaywrightRustMouseButton {
    match button {
        MouseButton::Left => PlaywrightRustMouseButton::Left,
        MouseButton::Right => PlaywrightRustMouseButton::Right,
        MouseButton::Middle => PlaywrightRustMouseButton::Middle,
    }
}
fn button_from_wire(button: PlaywrightRustMouseButton) -> MouseButton {
    match button {
        PlaywrightRustMouseButton::Left => MouseButton::Left,
        PlaywrightRustMouseButton::Right => MouseButton::Right,
        PlaywrightRustMouseButton::Middle => MouseButton::Middle,
    }
}

fn duration_to_wire(value: Duration) -> PlaywrightRustCommonDuration {
    PlaywrightRustCommonDuration {
        seconds: value.as_secs(),
        nanoseconds: value.subsec_nanos(),
    }
}
fn duration_from_wire(value: PlaywrightRustCommonDuration) -> Duration {
    Duration::new(value.seconds, value.nanoseconds)
}
fn metadata_to_wire(value: CallMetadata) -> PlaywrightRustCommonCallMetadata {
    PlaywrightRustCommonCallMetadata {
        timeout: value.timeout.map(duration_to_wire),
        cancellation_id: value.cancellation.map(CancellationId::get),
    }
}
fn metadata_from_wire(value: PlaywrightRustCommonCallMetadata) -> CallMetadata {
    CallMetadata::new(
        value.timeout.map(duration_from_wire),
        value.cancellation_id.map(CancellationId::new),
    )
}
fn identity(field: &'static str, outer: u64, inner: u64) -> Result<(), MouseCodecError> {
    if outer == inner {
        Ok(())
    } else {
        Err(MouseCodecError::IdentityMismatch {
            field,
            outer,
            inner,
        })
    }
}
fn ensure_no_attachments(frame: &WireFrame) -> Result<(), MouseCodecError> {
    if frame.attachments().is_empty() {
        Ok(())
    } else {
        Err(MouseCodecError::AttachmentsUnsupported(
            frame.attachments().len(),
        ))
    }
}
fn outcome_to_wire(
    value: &Result<(), OperationError<MouseErrorDetails>>,
) -> PlaywrightRustMouseOutcomeRef<'_> {
    match value {
        Ok(()) => PlaywrightRustMouseOutcomeRef::Success,
        Err(error) => PlaywrightRustMouseOutcomeRef::Error {
            error: PlaywrightRustMouseErrorRef {
                failure_kind: failure_to_wire(error.kind()),
                message: error.message(),
                details: error.details().map(|details| match details {
                    MouseErrorDetails::InvalidArgument(message) => {
                        PlaywrightRustMouseErrorDetailsRef::InvalidArgument { message }
                    }
                    MouseErrorDetails::Resource(message) => {
                        PlaywrightRustMouseErrorDetailsRef::Resource { message }
                    }
                    MouseErrorDetails::Backend(message) => {
                        PlaywrightRustMouseErrorDetailsRef::Backend { message }
                    }
                }),
            },
        },
    }
}
fn outcome_from_wire(
    value: PlaywrightRustMouseOutcome,
) -> Result<(), OperationError<MouseErrorDetails>> {
    match value {
        PlaywrightRustMouseOutcome::Success => Ok(()),
        PlaywrightRustMouseOutcome::Error { error } => Err(OperationError::new(
            failure_from_wire(error.failure_kind),
            error.message,
            error.details.map(Into::into),
        )),
    }
}
fn wire_kind(wire: &PlaywrightRustMouse) -> u8 {
    match wire {
        PlaywrightRustMouse::MouseMoveCommand { .. } => 1,
        PlaywrightRustMouse::MouseMoveResult { .. } => 2,
        PlaywrightRustMouse::MouseDownCommand { .. } => 3,
        PlaywrightRustMouse::MouseDownResult { .. } => 4,
        PlaywrightRustMouse::MouseUpCommand { .. } => 5,
        PlaywrightRustMouse::MouseUpResult { .. } => 6,
        PlaywrightRustMouse::MouseClickCommand { .. } => 7,
        PlaywrightRustMouse::MouseClickResult { .. } => 8,
        PlaywrightRustMouse::MouseDblclickCommand { .. } => 9,
        PlaywrightRustMouse::MouseDblclickResult { .. } => 10,
        PlaywrightRustMouse::MouseWheelCommand { .. } => 11,
        PlaywrightRustMouse::MouseWheelResult { .. } => 12,
    }
}

macro_rules! failure_mapping {
    ($($variant:ident),+ $(,)?) => {
        fn failure_to_wire(value: FailureKind) -> PlaywrightRustCommonFailureKind {
            match value { $(FailureKind::$variant => PlaywrightRustCommonFailureKind::$variant),+ }
        }
        fn failure_from_wire(value: PlaywrightRustCommonFailureKind) -> FailureKind {
            match value { $(PlaywrightRustCommonFailureKind::$variant => FailureKind::$variant),+ }
        }
    };
}
failure_mapping!(
    Timeout,
    Cancelled,
    TargetClosed,
    BrowserCrashed,
    UnsupportedCapability,
    ResourceExhausted,
    TransportClosed,
    ProtocolViolation,
    Backend,
    Cleanup
);

#[cfg(test)]
mod tests;
