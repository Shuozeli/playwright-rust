//! Bounded canonical wire adapters for the independently installable Keyboard.
use playwright_rust_cap_keyboard::*;
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
    CATALOG as KEYBOARD_SCHEMA_CATALOG, COMMANDS as KEYBOARD_COMMAND_SCHEMAS,
    ROOT_SCHEMA as KEYBOARD_SCHEMA,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardCodecLimits {
    pub string_bytes: usize,
}
impl KeyboardCodecLimits {
    pub const fn new(string_bytes: usize) -> Self {
        Self { string_bytes }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct KeyboardCodec {
    limits: KeyboardCodecLimits,
}
impl KeyboardCodec {
    pub const fn new(limits: KeyboardCodecLimits) -> Self {
        Self { limits }
    }
    pub const fn limits(self) -> KeyboardCodecLimits {
        self.limits
    }
    fn value_limits(self) -> BinaryValueLimits {
        BinaryValueLimits::new(self.limits.string_bytes, 0, 0, 8, 16)
    }
}

#[derive(Debug)]
pub enum KeyboardCodecError {
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
    Argument(KeyboardArgumentTooLarge),
    Encode(BinaryValueEncodeError),
    Decode(BinaryValueDecodeError),
}
impl fmt::Display for KeyboardCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRoute(route) => write!(f, "Keyboard received wrong route {route:?}"),
            Self::WrongMethod { expected, actual } => {
                write!(f, "Keyboard expected method {expected}, received {actual}")
            }
            Self::IdentityMismatch {
                field,
                outer,
                inner,
            } => write!(f, "Keyboard {field} mismatch: outer {outer}, inner {inner}"),
            Self::AttachmentsUnsupported(count) => {
                write!(f, "Keyboard accepts no attachments, received {count}")
            }
            Self::WrongPayload { expected, actual } => write!(
                f,
                "Keyboard expected {expected}, received payload tag {actual}"
            ),
            Self::Argument(error) => error.fmt(f),
            Self::Encode(error) => error.fmt(f),
            Self::Decode(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for KeyboardCodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Argument(e) => Some(e),
            Self::Encode(e) => Some(e),
            Self::Decode(e) => Some(e),
            _ => None,
        }
    }
}

trait KeyboardWireSpec: CommandSpec<Result = (), ErrorDetails = KeyboardErrorDetails> + Sized {
    fn command_ref(command: &Command<Self>) -> PlaywrightRustKeyboardRef<'_>;
    fn command_from_wire(wire: PlaywrightRustKeyboard)
    -> Result<Command<Self>, KeyboardCodecError>;
    fn result_ref(result: &CommandResult<Self>) -> PlaywrightRustKeyboardRef<'_>;
    fn result_from_wire(
        wire: PlaywrightRustKeyboard,
    ) -> Result<CommandResult<Self>, KeyboardCodecError>;
}

impl<S: KeyboardWireSpec> WireCodec<Command<S>> for KeyboardCodec {
    type Error = KeyboardCodecError;
    fn encode_parts(&self, command: &Command<S>) -> Result<EncodedParts, Self::Error> {
        let payload = encode_value(&S::command_ref(command), self.value_limits())
            .map_err(KeyboardCodecError::Encode)?;
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
            return Err(KeyboardCodecError::WrongRoute(frame.route()));
        };
        if method != S::METHOD {
            return Err(KeyboardCodecError::WrongMethod {
                expected: S::METHOD,
                actual: method,
            });
        }
        ensure_no_attachments(&frame)?;
        let wire = decode_value::<PlaywrightRustKeyboard>(frame.payload(), self.value_limits())
            .map_err(KeyboardCodecError::Decode)?;
        let command = S::command_from_wire(wire)?;
        identity("call ID", call_id.get(), command.call_id().get())?;
        identity("Page ID", target.get(), command.object_id().get())?;
        Ok(command)
    }
}

impl<S: KeyboardWireSpec> WireCodec<CommandResult<S>> for KeyboardCodec {
    type Error = KeyboardCodecError;
    fn encode_parts(&self, result: &CommandResult<S>) -> Result<EncodedParts, Self::Error> {
        let payload = encode_value(&S::result_ref(result), self.value_limits())
            .map_err(KeyboardCodecError::Encode)?;
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
            return Err(KeyboardCodecError::WrongRoute(frame.route()));
        };
        ensure_no_attachments(&frame)?;
        let wire = decode_value::<PlaywrightRustKeyboard>(frame.payload(), self.value_limits())
            .map_err(KeyboardCodecError::Decode)?;
        let result = S::result_from_wire(wire)?;
        identity("call ID", call_id.get(), result.call_id().get())?;
        Ok(result)
    }
}

macro_rules! keyboard_spec {
    ($command:ty, $cmd:ident, $result:ident, $encode:ident, $decode:ident) => {
        impl KeyboardWireSpec for $command {
            fn command_ref(command: &Command<Self>) -> PlaywrightRustKeyboardRef<'_> {
                PlaywrightRustKeyboardRef::$cmd {
                    call_id: command.call_id().get(),
                    object_id: command.object_id().get(),
                    metadata: metadata_to_wire(command.metadata()),
                    params: $encode(command.params()),
                }
            }
            fn command_from_wire(
                wire: PlaywrightRustKeyboard,
            ) -> Result<Command<Self>, KeyboardCodecError> {
                match wire {
                    PlaywrightRustKeyboard::$cmd {
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
                    other => Err(KeyboardCodecError::WrongPayload {
                        expected: stringify!($cmd),
                        actual: wire_kind(&other),
                    }),
                }
            }
            fn result_ref(result: &CommandResult<Self>) -> PlaywrightRustKeyboardRef<'_> {
                PlaywrightRustKeyboardRef::$result {
                    call_id: result.call_id().get(),
                    outcome: outcome_to_wire(result.outcome()),
                }
            }
            fn result_from_wire(
                wire: PlaywrightRustKeyboard,
            ) -> Result<CommandResult<Self>, KeyboardCodecError> {
                match wire {
                    PlaywrightRustKeyboard::$result { call_id, outcome } => Ok(CommandResult::new(
                        CallId::new(call_id),
                        outcome_from_wire(outcome),
                    )),
                    other => Err(KeyboardCodecError::WrongPayload {
                        expected: stringify!($result),
                        actual: wire_kind(&other),
                    }),
                }
            }
        }
    };
}
keyboard_spec!(
    KeyboardDown,
    KeyboardDownCommand,
    KeyboardDownResult,
    key_to_wire,
    key_from_wire
);
keyboard_spec!(
    KeyboardUp,
    KeyboardUpCommand,
    KeyboardUpResult,
    up_to_wire,
    up_from_wire
);
keyboard_spec!(
    KeyboardPress,
    KeyboardPressCommand,
    KeyboardPressResult,
    press_to_wire,
    press_from_wire
);
keyboard_spec!(
    KeyboardInsertText,
    KeyboardInsertTextCommand,
    KeyboardInsertTextResult,
    insert_to_wire,
    insert_from_wire
);
keyboard_spec!(
    KeyboardType,
    KeyboardTypeCommand,
    KeyboardTypeResult,
    type_to_wire,
    type_from_wire
);

fn key_to_wire(params: &DownParams) -> PlaywrightRustKeyboardDownParamsRef<'_> {
    PlaywrightRustKeyboardDownParamsRef {
        key: params.key.as_str(),
    }
}
fn key_from_wire(
    params: PlaywrightRustKeyboardDownParams,
) -> Result<DownParams, KeyboardCodecError> {
    Ok(DownParams {
        key: KeyboardKey::try_from(params.key).map_err(KeyboardCodecError::Argument)?,
    })
}
fn up_to_wire(params: &UpParams) -> PlaywrightRustKeyboardUpParamsRef<'_> {
    PlaywrightRustKeyboardUpParamsRef {
        key: params.key.as_str(),
    }
}
fn up_from_wire(params: PlaywrightRustKeyboardUpParams) -> Result<UpParams, KeyboardCodecError> {
    Ok(UpParams {
        key: KeyboardKey::try_from(params.key).map_err(KeyboardCodecError::Argument)?,
    })
}
fn press_to_wire(params: &PressParams) -> PlaywrightRustKeyboardPressParamsRef<'_> {
    PlaywrightRustKeyboardPressParamsRef {
        key: params.key.as_str(),
        delay: params.delay.map(duration_to_wire),
    }
}
fn press_from_wire(
    params: PlaywrightRustKeyboardPressParams,
) -> Result<PressParams, KeyboardCodecError> {
    Ok(PressParams {
        key: KeyboardKey::try_from(params.key).map_err(KeyboardCodecError::Argument)?,
        delay: params.delay.map(duration_from_wire),
    })
}
fn insert_to_wire(params: &InsertTextParams) -> PlaywrightRustKeyboardInsertTextParamsRef<'_> {
    PlaywrightRustKeyboardInsertTextParamsRef {
        text: params.text.as_str(),
    }
}
fn insert_from_wire(
    params: PlaywrightRustKeyboardInsertTextParams,
) -> Result<InsertTextParams, KeyboardCodecError> {
    Ok(InsertTextParams {
        text: KeyboardText::try_from(params.text).map_err(KeyboardCodecError::Argument)?,
    })
}
fn type_to_wire(params: &TypeParams) -> PlaywrightRustKeyboardTypeParamsRef<'_> {
    PlaywrightRustKeyboardTypeParamsRef {
        text: params.text.as_str(),
        delay: params.delay.map(duration_to_wire),
    }
}
fn type_from_wire(
    params: PlaywrightRustKeyboardTypeParams,
) -> Result<TypeParams, KeyboardCodecError> {
    Ok(TypeParams {
        text: KeyboardText::try_from(params.text).map_err(KeyboardCodecError::Argument)?,
        delay: params.delay.map(duration_from_wire),
    })
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
fn identity(field: &'static str, outer: u64, inner: u64) -> Result<(), KeyboardCodecError> {
    if outer == inner {
        Ok(())
    } else {
        Err(KeyboardCodecError::IdentityMismatch {
            field,
            outer,
            inner,
        })
    }
}
fn ensure_no_attachments(frame: &WireFrame) -> Result<(), KeyboardCodecError> {
    if frame.attachments().is_empty() {
        Ok(())
    } else {
        Err(KeyboardCodecError::AttachmentsUnsupported(
            frame.attachments().len(),
        ))
    }
}
fn outcome_to_wire(
    value: &Result<(), OperationError<KeyboardErrorDetails>>,
) -> PlaywrightRustKeyboardOutcomeRef<'_> {
    match value {
        Ok(()) => PlaywrightRustKeyboardOutcomeRef::Success,
        Err(error) => PlaywrightRustKeyboardOutcomeRef::Error {
            error: PlaywrightRustKeyboardErrorRef {
                failure_kind: failure_to_wire(error.kind()),
                message: error.message(),
                details: error.details().map(|details| match details {
                    KeyboardErrorDetails::InvalidKey(message) => {
                        PlaywrightRustKeyboardErrorDetailsRef::InvalidKey { message }
                    }
                    KeyboardErrorDetails::Resource(message) => {
                        PlaywrightRustKeyboardErrorDetailsRef::Resource { message }
                    }
                    KeyboardErrorDetails::Backend(message) => {
                        PlaywrightRustKeyboardErrorDetailsRef::Backend { message }
                    }
                }),
            },
        },
    }
}
fn outcome_from_wire(
    value: PlaywrightRustKeyboardOutcome,
) -> Result<(), OperationError<KeyboardErrorDetails>> {
    match value {
        PlaywrightRustKeyboardOutcome::Success => Ok(()),
        PlaywrightRustKeyboardOutcome::Error { error } => Err(OperationError::new(
            failure_from_wire(error.failure_kind),
            error.message,
            error.details.map(Into::into),
        )),
    }
}
fn wire_kind(wire: &PlaywrightRustKeyboard) -> u8 {
    match wire {
        PlaywrightRustKeyboard::KeyboardDownCommand { .. } => 1,
        PlaywrightRustKeyboard::KeyboardDownResult { .. } => 2,
        PlaywrightRustKeyboard::KeyboardUpCommand { .. } => 3,
        PlaywrightRustKeyboard::KeyboardUpResult { .. } => 4,
        PlaywrightRustKeyboard::KeyboardPressCommand { .. } => 5,
        PlaywrightRustKeyboard::KeyboardPressResult { .. } => 6,
        PlaywrightRustKeyboard::KeyboardInsertTextCommand { .. } => 7,
        PlaywrightRustKeyboard::KeyboardInsertTextResult { .. } => 8,
        PlaywrightRustKeyboard::KeyboardTypeCommand { .. } => 9,
        PlaywrightRustKeyboard::KeyboardTypeResult { .. } => 10,
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
