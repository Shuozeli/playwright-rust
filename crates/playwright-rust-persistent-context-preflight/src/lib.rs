//! Pure Chromium decisions for `browserType.launchPersistentContext` preflight.
//!
//! The caller must first finish all client-side certificate material reads,
//! unwrap protocol boxed strings, and establish JavaScript truthiness. This
//! crate does no I/O and never owns a browser, process, transport, or runtime.
//! It preserves the public validation order: certificates, user-data-dir,
//! Chromium's remote-debugging pipe, then page arguments.

/// The only browser type supported by this project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserEngine {
    Chromium,
}

/// Whether each value is JavaScript-truthy after client-side material reads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CertificatePresence {
    pub origin: bool,
    pub cert: bool,
    pub key: bool,
    pub pfx: bool,
    pub passphrase: bool,
}

/// An argument after the caller has unwrapped protocol boxed `String` values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Argument<'a> {
    String(&'a str),
    Other,
}

/// The already-normalized, browser-independent part of launch options.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DefaultArguments<'a> {
    /// `None` means `args` was not an Array; no argument check takes place.
    pub args: Option<&'a [Argument<'a>]>,
    /// Retained for the pinned public transport shape. Chromium ignores it.
    pub channel: Option<&'a str>,
    /// A truthy non-Array `ignoreDefaultArgs` bypasses argument validation.
    pub ignore_default_args_truthy_non_array: bool,
}

/// One complete, already-normalized Chromium preflight request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentContextPreflight<'a> {
    pub engine: BrowserEngine,
    pub certificates: &'a [CertificatePresence],
    pub arguments: DefaultArguments<'a>,
}

/// Exact public preflight reason, without its JavaScript API prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreflightError {
    MissingCertificateOrigin,
    MissingCertificateMaterial,
    CertificateWithoutKey,
    KeyWithoutCertificate,
    PfxWithCertificateOrKey,
    ProfileArgument,
    ChromiumRemoteDebuggingPipe,
    PageArgument,
}

impl PreflightError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::MissingCertificateOrigin => "clientCertificates.origin is required",
            Self::MissingCertificateMaterial => "None of cert, key, passphrase or pfx is specified",
            Self::CertificateWithoutKey => "cert is specified without key",
            Self::KeyWithoutCertificate => "key is specified without cert",
            Self::PfxWithCertificateOrKey => {
                "pfx is specified together with cert, key or passphrase"
            }
            Self::ProfileArgument => {
                "Pass userDataDir parameter to 'browserType.launchPersistentContext(userDataDir, options)' instead of specifying '--user-data-dir' argument"
            }
            Self::ChromiumRemoteDebuggingPipe => {
                "Playwright manages remote debugging connection itself."
            }
            Self::PageArgument => "Arguments can not specify page to be opened",
        }
    }
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for PreflightError {}

/// Verify certificate combinations in source order after all material reads.
pub fn verify_certificates(certificates: &[CertificatePresence]) -> Result<(), PreflightError> {
    for certificate in certificates {
        if !certificate.origin {
            return Err(PreflightError::MissingCertificateOrigin);
        }
        if !certificate.cert && !certificate.key && !certificate.passphrase && !certificate.pfx {
            return Err(PreflightError::MissingCertificateMaterial);
        }
        if certificate.cert && !certificate.key {
            return Err(PreflightError::CertificateWithoutKey);
        }
        if !certificate.cert && certificate.key {
            return Err(PreflightError::KeyWithoutCertificate);
        }
        if certificate.pfx && (certificate.cert || certificate.key) {
            return Err(PreflightError::PfxWithCertificateOrKey);
        }
    }
    Ok(())
}

pub use verify_certificates as verify_client_certificates;

/// Verify the Chromium default-argument preflight.
pub fn verify_default_arguments(
    _engine: BrowserEngine,
    options: DefaultArguments<'_>,
) -> Result<(), PreflightError> {
    let Some(args) = options.args else {
        return Ok(());
    };
    if options.ignore_default_args_truthy_non_array {
        return Ok(());
    }

    if args.iter().any(
        |argument| matches!(argument, Argument::String(value) if value.starts_with("--user-data-dir")),
    ) {
        return Err(PreflightError::ProfileArgument);
    }
    if args.iter().any(
        |argument| matches!(argument, Argument::String(value) if value.starts_with("--remote-debugging-pipe")),
    ) {
        return Err(PreflightError::ChromiumRemoteDebuggingPipe);
    }
    if args
        .iter()
        .any(|argument| matches!(argument, Argument::String(value) if !value.starts_with('-')))
    {
        return Err(PreflightError::PageArgument);
    }
    Ok(())
}

/// Preserve the client certificate versus launch argument error order.
pub fn verify_persistent_context(
    input: PersistentContextPreflight<'_>,
) -> Result<(), PreflightError> {
    verify_certificates(input.certificates)?;
    verify_default_arguments(input.engine, input.arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CERTIFICATE: CertificatePresence = CertificatePresence {
        origin: true,
        cert: true,
        key: true,
        pfx: false,
        passphrase: false,
    };

    #[test]
    fn certificate_combinations_match_all_truthiness_states() {
        for mask in 0u8..32 {
            // Arrange
            let certificate = CertificatePresence {
                origin: mask & 1 != 0,
                cert: mask & 2 != 0,
                key: mask & 4 != 0,
                pfx: mask & 8 != 0,
                passphrase: mask & 16 != 0,
            };
            let expected = if !certificate.origin {
                Err(PreflightError::MissingCertificateOrigin)
            } else if !certificate.cert
                && !certificate.key
                && !certificate.passphrase
                && !certificate.pfx
            {
                Err(PreflightError::MissingCertificateMaterial)
            } else if certificate.cert && !certificate.key {
                Err(PreflightError::CertificateWithoutKey)
            } else if !certificate.cert && certificate.key {
                Err(PreflightError::KeyWithoutCertificate)
            } else if certificate.pfx && (certificate.cert || certificate.key) {
                Err(PreflightError::PfxWithCertificateOrKey)
            } else {
                Ok(())
            };

            // Act
            let result = verify_client_certificates(&[certificate]);

            // Assert
            assert_eq!(result, expected, "combination {mask:05b}");
        }
    }

    #[test]
    fn certificate_errors_follow_input_order() {
        // Arrange
        let certificates = [
            CertificatePresence {
                cert: true,
                key: false,
                ..VALID_CERTIFICATE
            },
            CertificatePresence::default(),
        ];

        // Act
        let result = verify_client_certificates(&certificates);

        // Assert
        assert_eq!(result, Err(PreflightError::CertificateWithoutKey));
    }

    #[test]
    fn absent_argument_array_bypasses_checks() {
        // Arrange
        let options = DefaultArguments::default();

        // Act
        let result = verify_default_arguments(BrowserEngine::Chromium, options);

        // Assert
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn truthy_non_array_ignore_default_args_bypasses_checks() {
        // Arrange
        let args = [
            Argument::String("--user-data-dir=x"),
            Argument::String("page"),
        ];
        let options = DefaultArguments {
            args: Some(&args),
            ignore_default_args_truthy_non_array: true,
            ..DefaultArguments::default()
        };

        // Act
        let result = verify_default_arguments(BrowserEngine::Chromium, options);

        // Assert
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn profile_argument_precedes_remote_pipe_and_page() {
        // Arrange
        let args = [
            Argument::String("page"),
            Argument::String("--remote-debugging-pipe"),
            Argument::String("--user-data-dir=x"),
        ];
        let options = DefaultArguments {
            args: Some(&args),
            ..DefaultArguments::default()
        };

        // Act
        let result = verify_default_arguments(BrowserEngine::Chromium, options);

        // Assert
        assert_eq!(result, Err(PreflightError::ProfileArgument));
    }

    #[test]
    fn remote_pipe_precedes_page() {
        // Arrange
        let args = [
            Argument::String("page"),
            Argument::String("--remote-debugging-pipe=x"),
        ];
        let options = DefaultArguments {
            args: Some(&args),
            ..DefaultArguments::default()
        };

        // Act
        let result = verify_default_arguments(BrowserEngine::Chromium, options);

        // Assert
        assert_eq!(result, Err(PreflightError::ChromiumRemoteDebuggingPipe));
    }

    #[test]
    fn page_argument_is_rejected() {
        // Arrange
        let args = [Argument::String("https://example.test/")];
        let options = DefaultArguments {
            args: Some(&args),
            ..DefaultArguments::default()
        };

        // Act
        let result = verify_default_arguments(BrowserEngine::Chromium, options);

        // Assert
        assert_eq!(result, Err(PreflightError::PageArgument));
    }

    #[test]
    fn non_string_arguments_are_inert() {
        // Arrange
        let args = [Argument::Other];
        let options = DefaultArguments {
            args: Some(&args),
            ..DefaultArguments::default()
        };

        // Act
        let result = verify_default_arguments(BrowserEngine::Chromium, options);

        // Assert
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn certificate_error_precedes_argument_error() {
        // Arrange
        let certificates = [CertificatePresence::default()];
        let args = [Argument::String("--user-data-dir=x")];
        let input = PersistentContextPreflight {
            engine: BrowserEngine::Chromium,
            certificates: &certificates,
            arguments: DefaultArguments {
                args: Some(&args),
                ..DefaultArguments::default()
            },
        };

        // Act
        let result = verify_persistent_context(input);

        // Assert
        assert_eq!(result, Err(PreflightError::MissingCertificateOrigin));
    }

    #[test]
    fn valid_certificate_continues_to_argument_validation() {
        // Arrange
        let args = [Argument::String("page")];
        let input = PersistentContextPreflight {
            engine: BrowserEngine::Chromium,
            certificates: &[VALID_CERTIFICATE],
            arguments: DefaultArguments {
                args: Some(&args),
                ..DefaultArguments::default()
            },
        };

        // Act
        let result = verify_persistent_context(input);

        // Assert
        assert_eq!(result, Err(PreflightError::PageArgument));
    }

    #[test]
    fn every_error_has_a_stable_public_reason() {
        let cases = [
            (
                PreflightError::MissingCertificateOrigin,
                "clientCertificates.origin is required",
            ),
            (
                PreflightError::MissingCertificateMaterial,
                "None of cert, key, passphrase or pfx is specified",
            ),
            (
                PreflightError::CertificateWithoutKey,
                "cert is specified without key",
            ),
            (
                PreflightError::KeyWithoutCertificate,
                "key is specified without cert",
            ),
            (
                PreflightError::PfxWithCertificateOrKey,
                "pfx is specified together with cert, key or passphrase",
            ),
            (
                PreflightError::ProfileArgument,
                "Pass userDataDir parameter to 'browserType.launchPersistentContext(userDataDir, options)' instead of specifying '--user-data-dir' argument",
            ),
            (
                PreflightError::ChromiumRemoteDebuggingPipe,
                "Playwright manages remote debugging connection itself.",
            ),
            (
                PreflightError::PageArgument,
                "Arguments can not specify page to be opened",
            ),
        ];
        for (error, expected) in cases {
            // Act
            let message = error.message();

            // Assert
            assert_eq!(message, expected);
            assert_eq!(error.to_string(), expected);
        }
    }
}
