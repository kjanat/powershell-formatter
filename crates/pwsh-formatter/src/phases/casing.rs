//! PSUseCorrectCasing: keyword/operator lowercasing plus catalog-driven
//! command and parameter casing.

use crate::catalog::CommandCatalog;
use crate::engine::Engine;
use pwsh_parser::{OperatorKind, TokenFlags, TokenKind};

pub(crate) fn apply(engine: &mut Engine<'_>, catalog: Option<&dyn CommandCatalog>) {
    let opts = engine.opts.clone();
    let mut current_command: Option<String> = None;

    for pos in 0..engine.len() {
        let kind = engine.kind(pos);
        let text = engine.text(pos);

        match kind {
            TokenKind::Keyword(_) if opts.keyword_casing => {
                // PSSA lowercases the token text as written (it does not
                // re-spell to a canonical form beyond casing).
                let lower = text.to_lowercase();
                if lower != text {
                    engine.respell[pos] = Some(lower);
                }
            }
            TokenKind::Operator(OperatorKind::ComparisonWord | OperatorKind::UnaryWord)
                if opts.operator_casing =>
            {
                let lower = text.to_lowercase();
                if lower != text {
                    engine.respell[pos] = Some(lower);
                }
            }
            TokenKind::Generic | TokenKind::Identifier
                if engine.token(pos).flags.contains(TokenFlags::COMMAND_NAME) =>
            {
                current_command = Some(text.to_owned());
                if opts.command_casing
                    && let Some(catalog) = catalog
                    && let Some(canonical) = catalog.canonical_command(text)
                {
                    current_command = Some(canonical.to_owned());
                    if canonical != text {
                        engine.respell[pos] = Some(canonical.to_owned());
                    }
                }
            }
            TokenKind::Parameter if opts.command_casing => {
                if let (Some(catalog), Some(cmd)) = (catalog, current_command.as_deref()) {
                    // Token text: dash (any Unicode dash) + name + optional
                    // colon.
                    let mut chars = text.chars();
                    let dash = chars.next().unwrap_or('-');
                    let rest = chars.as_str();
                    let (name, colon) = match rest.strip_suffix(':') {
                        Some(n) => (n, ":"),
                        None => (rest, ""),
                    };
                    if let Some(canonical) = catalog.canonical_parameter(cmd, name)
                        && canonical != name
                    {
                        engine.respell[pos] = Some(format!("{dash}{canonical}{colon}"));
                    }
                }
            }
            TokenKind::Pipe | TokenKind::Semicolon => {
                // Next pipeline element / statement gets its own command.
                current_command = None;
            }
            _ => {}
        }
    }
}
