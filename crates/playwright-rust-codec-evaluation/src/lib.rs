//! Canonical bounded binary codec for value-only evaluation.
//!
//! The generated schema owns wire layout and metadata. This adapter performs
//! the semantic validation that a recursive JavaScript value graph requires.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;

use playwright_rust_cap_evaluation::{
    EvaluateParams, EvaluateResult, EvaluateValue, EvaluationErrorDetails, EvaluationMode,
    EvaluationNumber, EvaluationNumberError, EvaluationProperty, EvaluationValue,
    EvaluationValueError, EvaluationValueLimits, TypedArrayKind,
};
use playwright_rust_codec_binary::{
    BinaryValueDecodeError, BinaryValueEncodeError, BinaryValueLimits, decode_value, encode_value,
};
use playwright_rust_core::{
    CallId, CallMetadata, CancellationId, Command, CommandResult, CommandSpec, FailureKind,
    MethodTag, ObjectId, OperationError,
};
use playwright_rust_transport::{EncodedParts, FrameRoute, WireCodec, WireFrame};

const EVALUATE_COMMAND: u8 = 1;
const EVALUATE_RESULT: u8 = 2;

#[doc(hidden)]
#[rustfmt::skip]
pub mod generated_schema;

pub use generated_schema::{
    CATALOG as EVALUATION_SCHEMA_CATALOG, COMMANDS as EVALUATION_COMMAND_SCHEMAS,
    ROOT_SCHEMA as EVALUATION_SCHEMA,
};

use generated_schema::{
    PlaywrightRustCommonCallMetadata, PlaywrightRustCommonFailureKind, PlaywrightRustEvaluation,
    PlaywrightRustEvaluationError, PlaywrightRustEvaluationMode, PlaywrightRustEvaluationOutcome,
    PlaywrightRustEvaluationParams, PlaywrightRustEvaluationProperty,
    PlaywrightRustEvaluationResult, PlaywrightRustEvaluationTypedArrayKind,
    PlaywrightRustEvaluationValue,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluationCodecLimits {
    source_bytes: usize,
    error_string_bytes: usize,
    value: EvaluationValueLimits,
    input_value: EvaluationValueLimits,
    command_binary_value: BinaryValueLimits,
    result_binary_value: BinaryValueLimits,
}

impl EvaluationCodecLimits {
    pub fn new(
        source_bytes: usize,
        error_string_bytes: usize,
        value: EvaluationValueLimits,
    ) -> Result<Self, EvaluationCodecLimitError> {
        let binary_value = binary_limits(source_bytes, error_string_bytes, value)?;
        Ok(Self {
            source_bytes,
            error_string_bytes,
            value,
            input_value: value,
            command_binary_value: binary_value,
            result_binary_value: binary_value,
        })
    }

    /// Raise only command-argument capacity; result decoding keeps the
    /// original value and wire bounds.
    pub fn with_input_value_limits(
        mut self,
        input_value: EvaluationValueLimits,
    ) -> Result<Self, EvaluationCodecLimitError> {
        self.command_binary_value =
            binary_limits(self.source_bytes, self.error_string_bytes, input_value)?;
        self.input_value = input_value;
        Ok(self)
    }

    pub const fn input_value_limits(self) -> EvaluationValueLimits {
        self.input_value
    }

    pub const fn source_bytes(self) -> usize {
        self.source_bytes
    }

    pub const fn error_string_bytes(self) -> usize {
        self.error_string_bytes
    }

    pub const fn value(self) -> EvaluationValueLimits {
        self.value
    }
}

fn binary_limits(
    source_bytes: usize,
    error_string_bytes: usize,
    value: EvaluationValueLimits,
) -> Result<BinaryValueLimits, EvaluationCodecLimitError> {
    let command_strings = source_bytes
        .checked_add(value.max_string_bytes())
        .ok_or(EvaluationCodecLimitError::CapacityOverflow)?;
    let error_strings = error_string_bytes
        .checked_mul(4)
        .ok_or(EvaluationCodecLimitError::CapacityOverflow)?;
    let string_bytes = command_strings.max(error_strings);
    let items = value.max_entries();
    let depth = value
        .max_depth()
        .checked_mul(2)
        .and_then(|count| count.checked_add(16))
        .ok_or(EvaluationCodecLimitError::CapacityOverflow)?;
    let nodes = value
        .max_nodes()
        .checked_add(value.max_entries())
        .and_then(|count| count.checked_add(16))
        .ok_or(EvaluationCodecLimitError::CapacityOverflow)?;
    Ok(BinaryValueLimits::new(
        string_bytes,
        value.max_binary_bytes(),
        items,
        depth,
        nodes,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationCodecLimitError {
    CapacityOverflow,
}

impl Display for EvaluationCodecLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("evaluation codec limit arithmetic overflowed")
    }
}

impl Error for EvaluationCodecLimitError {}

#[derive(Clone, Copy, Debug)]
pub struct EvaluationCodec {
    limits: EvaluationCodecLimits,
}

impl EvaluationCodec {
    pub const fn new(limits: EvaluationCodecLimits) -> Self {
        Self { limits }
    }

    pub const fn limits(self) -> EvaluationCodecLimits {
        self.limits
    }
}

#[derive(Debug)]
pub enum EvaluationCodecError {
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
    AttachmentsUnsupported {
        actual: usize,
    },
    ControlMetadataUnsupported,
    WrongPayloadKind {
        expected: u8,
        actual: u8,
    },
    SourceTooLong {
        actual: usize,
        limit: usize,
    },
    ErrorStringTooLong {
        actual: usize,
        limit: usize,
    },
    ExpressionArgumentUnsupported,
    InvalidReferenceId(u32),
    InvalidNumber(EvaluationNumberError),
    InvalidValue(EvaluationValueError),
    Encode(BinaryValueEncodeError),
    Decode(BinaryValueDecodeError),
}

impl Display for EvaluationCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRoute(route) => write!(formatter, "evaluation payload arrived on {route:?}"),
            Self::WrongMethod { expected, actual } => {
                write!(
                    formatter,
                    "expected evaluation method {expected}, received {actual}"
                )
            }
            Self::IdentityMismatch {
                field,
                outer,
                inner,
            } => write!(
                formatter,
                "evaluation payload {field} {inner} does not match outer route {outer}"
            ),
            Self::AttachmentsUnsupported { actual } => write!(
                formatter,
                "evaluation payload has {actual} attachment(s), but inline value codec accepts none"
            ),
            Self::ControlMetadataUnsupported => formatter
                .write_str("value evaluation does not accept timeout or cancellation metadata"),
            Self::WrongPayloadKind { expected, actual } => write!(
                formatter,
                "expected evaluation payload kind {expected}, received {actual}"
            ),
            Self::SourceTooLong { actual, limit } => {
                write!(
                    formatter,
                    "evaluation source has {actual} bytes, exceeding limit {limit}"
                )
            }
            Self::ErrorStringTooLong { actual, limit } => write!(
                formatter,
                "evaluation error string has {actual} bytes, exceeding limit {limit}"
            ),
            Self::ExpressionArgumentUnsupported => {
                formatter.write_str("expression evaluation cannot carry an argument")
            }
            Self::InvalidReferenceId(id) => {
                write!(formatter, "evaluation reference ID {id} must be nonzero")
            }
            Self::InvalidNumber(error) => Display::fmt(error, formatter),
            Self::InvalidValue(error) => Display::fmt(error, formatter),
            Self::Encode(error) => Display::fmt(error, formatter),
            Self::Decode(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for EvaluationCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidNumber(error) => Some(error),
            Self::InvalidValue(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl WireCodec<Command<EvaluateValue>> for EvaluationCodec {
    type Error = EvaluationCodecError;

    fn encode_parts(&self, command: &Command<EvaluateValue>) -> Result<EncodedParts, Self::Error> {
        ensure_empty_metadata(command.metadata())?;
        validate_params(command.params(), self.limits)?;
        let wire = PlaywrightRustEvaluation::EvaluateValueCommand {
            call_id: command.call_id().get(),
            object_id: command.object_id().get(),
            metadata: empty_metadata(),
            params: params_to_wire(command.params()),
        };
        let payload = encode_value(&wire, self.limits.command_binary_value)
            .map_err(EvaluationCodecError::Encode)?;
        Ok(EncodedParts::new(
            FrameRoute::Command {
                call_id: command.call_id(),
                target: command.object_id(),
                method: EvaluateValue::METHOD,
            },
            payload,
            Vec::new(),
        ))
    }

    fn decode_frame(&self, frame: WireFrame) -> Result<Command<EvaluateValue>, Self::Error> {
        let (outer_call_id, outer_object_id) = command_route::<EvaluateValue>(&frame)?;
        ensure_no_attachments(&frame)?;
        match decode_value::<PlaywrightRustEvaluation>(
            frame.payload(),
            self.limits.command_binary_value,
        )
        .map_err(EvaluationCodecError::Decode)?
        {
            PlaywrightRustEvaluation::EvaluateValueCommand {
                call_id,
                object_id,
                metadata,
                params,
            } => {
                ensure_identity("call ID", outer_call_id.get(), call_id)?;
                ensure_identity("object ID", outer_object_id.get(), object_id)?;
                let metadata = metadata_from_wire(metadata);
                ensure_empty_metadata(metadata)?;
                let params = params_from_wire(params)?;
                validate_params(&params, self.limits)?;
                Ok(Command::new(
                    CallId::new(call_id),
                    ObjectId::new(object_id),
                    metadata,
                    params,
                ))
            }
            other => Err(EvaluationCodecError::WrongPayloadKind {
                expected: EVALUATE_COMMAND,
                actual: wire_kind(&other),
            }),
        }
    }
}

impl WireCodec<CommandResult<EvaluateValue>> for EvaluationCodec {
    type Error = EvaluationCodecError;

    fn encode_parts(
        &self,
        result: &CommandResult<EvaluateValue>,
    ) -> Result<EncodedParts, Self::Error> {
        let outcome = match result.outcome() {
            Ok(result) => {
                result
                    .value
                    .validate(self.limits.value)
                    .map_err(EvaluationCodecError::InvalidValue)?;
                PlaywrightRustEvaluationOutcome::Success {
                    result: PlaywrightRustEvaluationResult {
                        value: value_to_wire(&result.value),
                    },
                }
            }
            Err(error) => {
                validate_operation_error(error, self.limits.error_string_bytes)?;
                PlaywrightRustEvaluationOutcome::Error {
                    error: operation_error_to_wire(error),
                }
            }
        };
        let wire = PlaywrightRustEvaluation::EvaluateValueResult {
            call_id: result.call_id().get(),
            outcome,
        };
        let payload = encode_value(&wire, self.limits.result_binary_value)
            .map_err(EvaluationCodecError::Encode)?;
        Ok(EncodedParts::new(
            FrameRoute::Result {
                call_id: result.call_id(),
            },
            payload,
            Vec::new(),
        ))
    }

    fn decode_frame(&self, frame: WireFrame) -> Result<CommandResult<EvaluateValue>, Self::Error> {
        let outer_call_id = result_route(&frame)?;
        ensure_no_attachments(&frame)?;
        match decode_value::<PlaywrightRustEvaluation>(
            frame.payload(),
            self.limits.result_binary_value,
        )
        .map_err(EvaluationCodecError::Decode)?
        {
            PlaywrightRustEvaluation::EvaluateValueResult { call_id, outcome } => {
                ensure_identity("call ID", outer_call_id.get(), call_id)?;
                let outcome = match outcome {
                    PlaywrightRustEvaluationOutcome::Success { result } => {
                        let value = value_from_wire(result.value)?;
                        value
                            .validate(self.limits.value)
                            .map_err(EvaluationCodecError::InvalidValue)?;
                        Ok(EvaluateResult::new(value))
                    }
                    PlaywrightRustEvaluationOutcome::Error { error } => {
                        let error = operation_error_from_wire(error);
                        validate_operation_error(&error, self.limits.error_string_bytes)?;
                        Err(error)
                    }
                };
                Ok(CommandResult::new(CallId::new(call_id), outcome))
            }
            other => Err(EvaluationCodecError::WrongPayloadKind {
                expected: EVALUATE_RESULT,
                actual: wire_kind(&other),
            }),
        }
    }
}

fn validate_params(
    params: &EvaluateParams,
    limits: EvaluationCodecLimits,
) -> Result<(), EvaluationCodecError> {
    if params.source().len() > limits.source_bytes {
        return Err(EvaluationCodecError::SourceTooLong {
            actual: params.source().len(),
            limit: limits.source_bytes,
        });
    }
    if params.mode() == EvaluationMode::Expression && params.argument().is_some() {
        return Err(EvaluationCodecError::ExpressionArgumentUnsupported);
    }
    if let Some(argument) = params.argument() {
        argument
            .validate(limits.input_value)
            .map_err(EvaluationCodecError::InvalidValue)?;
    }
    Ok(())
}

fn params_to_wire(params: &EvaluateParams) -> PlaywrightRustEvaluationParams {
    PlaywrightRustEvaluationParams {
        source: params.source().to_owned(),
        mode: mode_to_wire(params.mode()),
        argument: params.argument().map(value_to_wire),
    }
}

fn params_from_wire(
    params: PlaywrightRustEvaluationParams,
) -> Result<EvaluateParams, EvaluationCodecError> {
    let argument = params.argument.map(value_from_wire).transpose()?;
    match (mode_from_wire(params.mode), argument) {
        (EvaluationMode::Expression, None) => Ok(EvaluateParams::expression(params.source)),
        (EvaluationMode::Expression, Some(_)) => {
            Err(EvaluationCodecError::ExpressionArgumentUnsupported)
        }
        (EvaluationMode::Function, Some(argument)) => {
            Ok(EvaluateParams::function(params.source, argument))
        }
        (EvaluationMode::Function, None) => {
            Ok(EvaluateParams::function_without_argument(params.source))
        }
    }
}

fn mode_to_wire(mode: EvaluationMode) -> PlaywrightRustEvaluationMode {
    match mode {
        EvaluationMode::Expression => PlaywrightRustEvaluationMode::Expression,
        EvaluationMode::Function => PlaywrightRustEvaluationMode::Function,
    }
}

fn mode_from_wire(mode: PlaywrightRustEvaluationMode) -> EvaluationMode {
    match mode {
        PlaywrightRustEvaluationMode::Expression => EvaluationMode::Expression,
        PlaywrightRustEvaluationMode::Function => EvaluationMode::Function,
    }
}

fn typed_array_to_wire(kind: TypedArrayKind) -> PlaywrightRustEvaluationTypedArrayKind {
    match kind {
        TypedArrayKind::Int8 => PlaywrightRustEvaluationTypedArrayKind::Int8,
        TypedArrayKind::Uint8 => PlaywrightRustEvaluationTypedArrayKind::Uint8,
        TypedArrayKind::Uint8Clamped => PlaywrightRustEvaluationTypedArrayKind::Uint8Clamped,
        TypedArrayKind::Int16 => PlaywrightRustEvaluationTypedArrayKind::Int16,
        TypedArrayKind::Uint16 => PlaywrightRustEvaluationTypedArrayKind::Uint16,
        TypedArrayKind::Int32 => PlaywrightRustEvaluationTypedArrayKind::Int32,
        TypedArrayKind::Uint32 => PlaywrightRustEvaluationTypedArrayKind::Uint32,
        TypedArrayKind::Float32 => PlaywrightRustEvaluationTypedArrayKind::Float32,
        TypedArrayKind::Float64 => PlaywrightRustEvaluationTypedArrayKind::Float64,
        TypedArrayKind::Bigint64 => PlaywrightRustEvaluationTypedArrayKind::Bigint64,
        TypedArrayKind::Biguint64 => PlaywrightRustEvaluationTypedArrayKind::Biguint64,
    }
}

fn typed_array_from_wire(kind: PlaywrightRustEvaluationTypedArrayKind) -> TypedArrayKind {
    match kind {
        PlaywrightRustEvaluationTypedArrayKind::Int8 => TypedArrayKind::Int8,
        PlaywrightRustEvaluationTypedArrayKind::Uint8 => TypedArrayKind::Uint8,
        PlaywrightRustEvaluationTypedArrayKind::Uint8Clamped => TypedArrayKind::Uint8Clamped,
        PlaywrightRustEvaluationTypedArrayKind::Int16 => TypedArrayKind::Int16,
        PlaywrightRustEvaluationTypedArrayKind::Uint16 => TypedArrayKind::Uint16,
        PlaywrightRustEvaluationTypedArrayKind::Int32 => TypedArrayKind::Int32,
        PlaywrightRustEvaluationTypedArrayKind::Uint32 => TypedArrayKind::Uint32,
        PlaywrightRustEvaluationTypedArrayKind::Float32 => TypedArrayKind::Float32,
        PlaywrightRustEvaluationTypedArrayKind::Float64 => TypedArrayKind::Float64,
        PlaywrightRustEvaluationTypedArrayKind::Bigint64 => TypedArrayKind::Bigint64,
        PlaywrightRustEvaluationTypedArrayKind::Biguint64 => TypedArrayKind::Biguint64,
    }
}

fn value_to_wire(value: &EvaluationValue) -> PlaywrightRustEvaluationValue {
    match value {
        EvaluationValue::Undefined => PlaywrightRustEvaluationValue::Undefined,
        EvaluationValue::Null => PlaywrightRustEvaluationValue::Null,
        EvaluationValue::Bool(value) => PlaywrightRustEvaluationValue::Bool { value: *value },
        EvaluationValue::Number(value) => {
            PlaywrightRustEvaluationValue::Number { bits: value.bits() }
        }
        EvaluationValue::Nan => PlaywrightRustEvaluationValue::Nan,
        EvaluationValue::Infinity => PlaywrightRustEvaluationValue::Infinity,
        EvaluationValue::NegativeInfinity => PlaywrightRustEvaluationValue::NegativeInfinity,
        EvaluationValue::NegativeZero => PlaywrightRustEvaluationValue::NegativeZero,
        EvaluationValue::String(value) => PlaywrightRustEvaluationValue::String {
            value: value.clone(),
        },
        EvaluationValue::Bigint(value) => PlaywrightRustEvaluationValue::Bigint {
            value: value.clone(),
        },
        EvaluationValue::Date(value) => PlaywrightRustEvaluationValue::Date {
            value: value.clone(),
        },
        EvaluationValue::Url(value) => PlaywrightRustEvaluationValue::Url {
            value: value.clone(),
        },
        EvaluationValue::Regexp { pattern, flags } => PlaywrightRustEvaluationValue::Regexp {
            pattern: pattern.clone(),
            flags: flags.clone(),
        },
        EvaluationValue::Error {
            name,
            message,
            stack,
        } => PlaywrightRustEvaluationValue::Error {
            name: name.clone(),
            message: message.clone(),
            stack: stack.clone(),
        },
        EvaluationValue::TypedArray { kind, bytes } => PlaywrightRustEvaluationValue::TypedArray {
            kind: typed_array_to_wire(*kind),
            bytes: bytes.clone(),
        },
        EvaluationValue::Array { id, items } => PlaywrightRustEvaluationValue::Array {
            id: id.map(NonZeroU32::get),
            items: items.iter().map(value_to_wire).collect(),
        },
        EvaluationValue::Object { id, properties } => PlaywrightRustEvaluationValue::Object {
            id: id.map(NonZeroU32::get),
            properties: properties
                .iter()
                .map(|property| PlaywrightRustEvaluationProperty {
                    key: property.key.clone(),
                    value: value_to_wire(&property.value),
                })
                .collect(),
        },
        EvaluationValue::Ref(id) => PlaywrightRustEvaluationValue::Ref { id: id.get() },
    }
}

fn value_from_wire(
    value: PlaywrightRustEvaluationValue,
) -> Result<EvaluationValue, EvaluationCodecError> {
    Ok(match value {
        PlaywrightRustEvaluationValue::Undefined => EvaluationValue::Undefined,
        PlaywrightRustEvaluationValue::Null => EvaluationValue::Null,
        PlaywrightRustEvaluationValue::Bool { value } => EvaluationValue::Bool(value),
        PlaywrightRustEvaluationValue::Number { bits } => EvaluationValue::Number(
            EvaluationNumber::from_bits(bits).map_err(EvaluationCodecError::InvalidNumber)?,
        ),
        PlaywrightRustEvaluationValue::Nan => EvaluationValue::Nan,
        PlaywrightRustEvaluationValue::Infinity => EvaluationValue::Infinity,
        PlaywrightRustEvaluationValue::NegativeInfinity => EvaluationValue::NegativeInfinity,
        PlaywrightRustEvaluationValue::NegativeZero => EvaluationValue::NegativeZero,
        PlaywrightRustEvaluationValue::String { value } => EvaluationValue::String(value),
        PlaywrightRustEvaluationValue::Bigint { value } => EvaluationValue::Bigint(value),
        PlaywrightRustEvaluationValue::Date { value } => EvaluationValue::Date(value),
        PlaywrightRustEvaluationValue::Url { value } => EvaluationValue::Url(value),
        PlaywrightRustEvaluationValue::Regexp { pattern, flags } => {
            EvaluationValue::Regexp { pattern, flags }
        }
        PlaywrightRustEvaluationValue::Error {
            name,
            message,
            stack,
        } => EvaluationValue::Error {
            name,
            message,
            stack,
        },
        PlaywrightRustEvaluationValue::TypedArray { kind, bytes } => EvaluationValue::TypedArray {
            kind: typed_array_from_wire(kind),
            bytes,
        },
        PlaywrightRustEvaluationValue::Array { id, items } => EvaluationValue::Array {
            id: nonzero(id)?,
            items: items
                .into_iter()
                .map(value_from_wire)
                .collect::<Result<_, _>>()?,
        },
        PlaywrightRustEvaluationValue::Object { id, properties } => EvaluationValue::Object {
            id: nonzero(id)?,
            properties: properties
                .into_iter()
                .map(|property| {
                    Ok(EvaluationProperty::new(
                        property.key,
                        value_from_wire(property.value)?,
                    ))
                })
                .collect::<Result<_, EvaluationCodecError>>()?,
        },
        PlaywrightRustEvaluationValue::Ref { id } => EvaluationValue::Ref(
            NonZeroU32::new(id).ok_or(EvaluationCodecError::InvalidReferenceId(id))?,
        ),
    })
}

fn nonzero(id: Option<u32>) -> Result<Option<NonZeroU32>, EvaluationCodecError> {
    id.map(|id| NonZeroU32::new(id).ok_or(EvaluationCodecError::InvalidReferenceId(id)))
        .transpose()
}

fn empty_metadata() -> PlaywrightRustCommonCallMetadata {
    PlaywrightRustCommonCallMetadata {
        timeout: None,
        cancellation_id: None,
    }
}

fn metadata_from_wire(metadata: PlaywrightRustCommonCallMetadata) -> CallMetadata {
    let timeout = metadata
        .timeout
        .map(|timeout| std::time::Duration::new(timeout.seconds, timeout.nanoseconds));
    CallMetadata::new(timeout, metadata.cancellation_id.map(CancellationId::new))
}

fn ensure_empty_metadata(metadata: CallMetadata) -> Result<(), EvaluationCodecError> {
    if metadata == CallMetadata::default() {
        Ok(())
    } else {
        Err(EvaluationCodecError::ControlMetadataUnsupported)
    }
}

fn ensure_no_attachments(frame: &WireFrame) -> Result<(), EvaluationCodecError> {
    if frame.attachments().is_empty() {
        Ok(())
    } else {
        Err(EvaluationCodecError::AttachmentsUnsupported {
            actual: frame.attachments().len(),
        })
    }
}

fn ensure_identity(
    field: &'static str,
    outer: u64,
    inner: u64,
) -> Result<(), EvaluationCodecError> {
    if outer == inner {
        Ok(())
    } else {
        Err(EvaluationCodecError::IdentityMismatch {
            field,
            outer,
            inner,
        })
    }
}

fn command_route<S: CommandSpec>(
    frame: &WireFrame,
) -> Result<(CallId, ObjectId), EvaluationCodecError> {
    let FrameRoute::Command {
        call_id,
        target,
        method,
    } = frame.route()
    else {
        return Err(EvaluationCodecError::WrongRoute(frame.route()));
    };
    if method != S::METHOD {
        return Err(EvaluationCodecError::WrongMethod {
            expected: S::METHOD,
            actual: method,
        });
    }
    Ok((call_id, target))
}

fn result_route(frame: &WireFrame) -> Result<CallId, EvaluationCodecError> {
    let FrameRoute::Result { call_id } = frame.route() else {
        return Err(EvaluationCodecError::WrongRoute(frame.route()));
    };
    Ok(call_id)
}

fn wire_kind(value: &PlaywrightRustEvaluation) -> u8 {
    match value {
        PlaywrightRustEvaluation::EvaluateValueCommand { .. } => EVALUATE_COMMAND,
        PlaywrightRustEvaluation::EvaluateValueResult { .. } => EVALUATE_RESULT,
    }
}

fn failure_kind_to_wire(value: FailureKind) -> PlaywrightRustCommonFailureKind {
    match value {
        FailureKind::Timeout => PlaywrightRustCommonFailureKind::Timeout,
        FailureKind::Cancelled => PlaywrightRustCommonFailureKind::Cancelled,
        FailureKind::TargetClosed => PlaywrightRustCommonFailureKind::TargetClosed,
        FailureKind::BrowserCrashed => PlaywrightRustCommonFailureKind::BrowserCrashed,
        FailureKind::UnsupportedCapability => {
            PlaywrightRustCommonFailureKind::UnsupportedCapability
        }
        FailureKind::ResourceExhausted => PlaywrightRustCommonFailureKind::ResourceExhausted,
        FailureKind::TransportClosed => PlaywrightRustCommonFailureKind::TransportClosed,
        FailureKind::ProtocolViolation => PlaywrightRustCommonFailureKind::ProtocolViolation,
        FailureKind::Backend => PlaywrightRustCommonFailureKind::Backend,
        FailureKind::Cleanup => PlaywrightRustCommonFailureKind::Cleanup,
    }
}

fn failure_kind_from_wire(value: PlaywrightRustCommonFailureKind) -> FailureKind {
    match value {
        PlaywrightRustCommonFailureKind::Timeout => FailureKind::Timeout,
        PlaywrightRustCommonFailureKind::Cancelled => FailureKind::Cancelled,
        PlaywrightRustCommonFailureKind::TargetClosed => FailureKind::TargetClosed,
        PlaywrightRustCommonFailureKind::BrowserCrashed => FailureKind::BrowserCrashed,
        PlaywrightRustCommonFailureKind::UnsupportedCapability => {
            FailureKind::UnsupportedCapability
        }
        PlaywrightRustCommonFailureKind::ResourceExhausted => FailureKind::ResourceExhausted,
        PlaywrightRustCommonFailureKind::TransportClosed => FailureKind::TransportClosed,
        PlaywrightRustCommonFailureKind::ProtocolViolation => FailureKind::ProtocolViolation,
        PlaywrightRustCommonFailureKind::Backend => FailureKind::Backend,
        PlaywrightRustCommonFailureKind::Cleanup => FailureKind::Cleanup,
    }
}

fn operation_error_to_wire(
    error: &OperationError<EvaluationErrorDetails>,
) -> PlaywrightRustEvaluationError {
    PlaywrightRustEvaluationError {
        failure_kind: failure_kind_to_wire(error.kind()),
        message: error.message().to_owned(),
        details: error.details().cloned().map(Into::into),
    }
}

fn operation_error_from_wire(
    error: PlaywrightRustEvaluationError,
) -> OperationError<EvaluationErrorDetails> {
    OperationError::new(
        failure_kind_from_wire(error.failure_kind),
        error.message,
        error.details.map(Into::into),
    )
}

fn validate_operation_error(
    error: &OperationError<EvaluationErrorDetails>,
    limit: usize,
) -> Result<(), EvaluationCodecError> {
    validate_error_string(error.message(), limit)?;
    match error.details() {
        None => {}
        Some(EvaluationErrorDetails::Javascript {
            name,
            message,
            stack,
        }) => {
            validate_error_string(name, limit)?;
            validate_error_string(message, limit)?;
            if let Some(stack) = stack {
                validate_error_string(stack, limit)?;
            }
        }
        Some(EvaluationErrorDetails::InvalidValue(message))
        | Some(EvaluationErrorDetails::InvalidBackendResponse(message))
        | Some(EvaluationErrorDetails::Backend(message)) => {
            validate_error_string(message, limit)?;
        }
    }
    Ok(())
}

fn validate_error_string(value: &str, limit: usize) -> Result<(), EvaluationCodecError> {
    if value.len() <= limit {
        Ok(())
    } else {
        Err(EvaluationCodecError::ErrorStringTooLong {
            actual: value.len(),
            limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;
    use std::time::Duration;

    use playwright_rust_cap_evaluation::{
        EVALUATE_VALUE_COMMAND_ID, EVALUATE_VALUE_METHOD, EvaluateParams, EvaluateResult,
        EvaluateValue, EvaluationValue, EvaluationValueLimits, TypedArrayKind,
    };
    use playwright_rust_core::{
        CallId, CallMetadata, Command, CommandResult, ObjectId, ProtocolVersion,
    };
    use playwright_rust_transport::{FrameLimits, VersionPolicy, WireCodec, WireProtocol};

    use super::{
        EVALUATION_COMMAND_SCHEMAS, EVALUATION_SCHEMA, EVALUATION_SCHEMA_CATALOG, EvaluationCodec,
        EvaluationCodecError, EvaluationCodecLimits,
    };

    fn codec() -> EvaluationCodec {
        let values = EvaluationValueLimits::new(8, 64, 64, 1024, 1024).unwrap();
        EvaluationCodec::new(EvaluationCodecLimits::new(1024, 512, values).unwrap())
    }

    fn protocol() -> WireProtocol<EvaluationCodec> {
        WireProtocol::new(
            codec(),
            VersionPolicy::new(ProtocolVersion::new(1, 0), ProtocolVersion::new(1, 0)).unwrap(),
            ProtocolVersion::new(1, 0),
            FrameLimits::new(16 * 1024, 0, 0, 16 * 1024),
        )
        .unwrap()
    }

    #[test]
    fn function_command_round_trips_one_typed_argument() {
        // Arrange
        let command = Command::<EvaluateValue>::new(
            CallId::new(7),
            ObjectId::new(4),
            CallMetadata::default(),
            EvaluateParams::function(
                "value => value",
                EvaluationValue::TypedArray {
                    kind: TypedArrayKind::Uint16,
                    bytes: vec![1, 0, 2, 0],
                },
            ),
        );

        // Act
        let frame = protocol().encode(&command).expect("command should encode");
        let decoded = protocol()
            .decode::<Command<EvaluateValue>>(frame)
            .expect("command should decode");

        // Assert
        assert_eq!(decoded.call_id(), CallId::new(7));
        assert_eq!(decoded.object_id(), ObjectId::new(4));
        assert_eq!(decoded.params(), command.params());
    }

    #[test]
    fn selected_large_input_round_trips_a_two_megabyte_post_body() {
        // Arrange
        let output = EvaluationValueLimits::new(8, 64, 64, 256 * 1024, 1024).unwrap();
        let input = EvaluationValueLimits::new(8, 64, 64, 3 * 1024 * 1024, 1024).unwrap();
        let limits = EvaluationCodecLimits::new(32 * 1024, 4096, output)
            .unwrap()
            .with_input_value_limits(input)
            .unwrap();
        let protocol = WireProtocol::new(
            EvaluationCodec::new(limits),
            VersionPolicy::new(ProtocolVersion::new(1, 0), ProtocolVersion::new(1, 0)).unwrap(),
            ProtocolVersion::new(1, 0),
            FrameLimits::new(4 * 1024 * 1024, 0, 0, 4 * 1024 * 1024),
        )
        .unwrap();
        let command = Command::<EvaluateValue>::new(
            CallId::new(7),
            ObjectId::new(4),
            CallMetadata::default(),
            EvaluateParams::function(
                "body => body.length",
                EvaluationValue::String("0".repeat(2 * 1024 * 1024)),
            ),
        );

        // Act
        let frame = protocol
            .encode(&command)
            .expect("large command should encode");
        let decoded = protocol
            .decode::<Command<EvaluateValue>>(frame)
            .expect("large command should decode");

        // Assert
        assert_eq!(decoded.params(), command.params());
        assert_eq!(
            limits.input_value_limits().max_string_bytes(),
            3 * 1024 * 1024
        );
        assert_eq!(limits.value().max_string_bytes(), 256 * 1024);
    }

    #[test]
    fn selected_large_input_does_not_raise_result_string_limit() {
        // Arrange
        let output = EvaluationValueLimits::new(8, 64, 64, 256 * 1024, 1024).unwrap();
        let input = EvaluationValueLimits::new(8, 64, 64, 3 * 1024 * 1024, 1024).unwrap();
        let codec = EvaluationCodec::new(
            EvaluationCodecLimits::new(32 * 1024, 4096, output)
                .unwrap()
                .with_input_value_limits(input)
                .unwrap(),
        );
        let result = CommandResult::<EvaluateValue>::new(
            CallId::new(8),
            Ok(EvaluateResult::new(EvaluationValue::String(
                "0".repeat(256 * 1024 + 1),
            ))),
        );

        // Act
        let error = codec.encode_parts(&result).unwrap_err();

        // Assert
        assert!(matches!(error, EvaluationCodecError::InvalidValue(_)));
    }

    #[test]
    fn cyclic_value_result_round_trips_without_a_remote_handle() {
        // Arrange
        let id = NonZeroU32::new(1).unwrap();
        let value = EvaluationValue::Array {
            id: Some(id),
            items: vec![
                EvaluationValue::String("ok".to_string()),
                EvaluationValue::Ref(id),
            ],
        };
        let result = CommandResult::<EvaluateValue>::new(
            CallId::new(8),
            Ok(EvaluateResult::new(value.clone())),
        );

        // Act
        let frame = protocol().encode(&result).expect("result should encode");
        let decoded = protocol()
            .decode::<CommandResult<EvaluateValue>>(frame)
            .expect("result should decode");

        // Assert
        assert_eq!(decoded.into_outcome().unwrap().value, value);
    }

    #[test]
    fn source_and_value_limits_reject_before_frame_creation() {
        // Arrange
        let values = EvaluationValueLimits::new(2, 4, 4, 3, 4).unwrap();
        let codec = EvaluationCodec::new(EvaluationCodecLimits::new(3, 8, values).unwrap());
        let source = Command::<EvaluateValue>::new(
            CallId::new(1),
            ObjectId::new(2),
            CallMetadata::default(),
            EvaluateParams::expression("1234"),
        );
        let value = CommandResult::<EvaluateValue>::new(
            CallId::new(1),
            Ok(EvaluateResult::new(EvaluationValue::String(
                "1234".to_string(),
            ))),
        );

        // Act
        let source_error = codec.encode_parts(&source).unwrap_err();
        let value_error = codec.encode_parts(&value).unwrap_err();

        // Assert
        assert!(matches!(
            source_error,
            EvaluationCodecError::SourceTooLong {
                actual: 4,
                limit: 3
            }
        ));
        assert!(matches!(value_error, EvaluationCodecError::InvalidValue(_)));
    }

    #[test]
    fn evaluation_rejects_timeout_metadata() {
        // Arrange
        let command = Command::<EvaluateValue>::new(
            CallId::new(1),
            ObjectId::new(2),
            CallMetadata::new(Some(Duration::from_secs(1)), None),
            EvaluateParams::expression("1"),
        );

        // Act
        let error = codec().encode_parts(&command).unwrap_err();

        // Assert
        assert!(matches!(
            error,
            EvaluationCodecError::ControlMetadataUnsupported
        ));
    }

    #[test]
    fn generated_schema_matches_the_capability_identity() {
        // Arrange
        let command = EVALUATION_COMMAND_SCHEMAS[0];

        // Act
        let catalog_validation = EVALUATION_SCHEMA_CATALOG.validate();

        // Assert
        assert_eq!(EVALUATION_SCHEMA.version(), ProtocolVersion::new(1, 0));
        assert_eq!(catalog_validation, Ok(()));
        assert_eq!(command.id(), EVALUATE_VALUE_COMMAND_ID);
        assert_eq!(command.method(), EVALUATE_VALUE_METHOD);
    }
}
