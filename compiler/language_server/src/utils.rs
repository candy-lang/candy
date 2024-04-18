use crate::database::Database;
use candy_frontend::{
    cst::CstDb,
    error::CompilerError,
    module::{Module, ModuleDb, ModuleKind, Package, PackagesPath},
    position::{line_start_offsets_raw, Offset, PositionConversionDb},
};
use extension_trait::extension_trait;
use itertools::Itertools;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Url};
use std::ops::Range;

#[must_use]
pub fn error_to_diagnostic(db: &Database, module: Module, error: &CompilerError) -> Diagnostic {
    let related_information = error
        .to_related_information()
        .into_iter()
        .filter_map(|(module, cst_id, message)| {
            let uri = module_to_url(&module, &db.packages_path)?;

            let span = db.find_cst(module.clone(), cst_id).display_span();
            let range = db.range_to_lsp_range(module, span);

            Some(lsp_types::DiagnosticRelatedInformation {
                location: lsp_types::Location { uri, range },
                message,
            })
        })
        .collect();
    Diagnostic {
        range: db.range_to_lsp_range(module, error.span.clone()),
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("🍭 Candy".to_owned()),
        message: error.payload.to_string(),
        related_information: Some(related_information),
        tags: None,
        data: None,
    }
}

pub fn module_from_url(
    url: &Url,
    kind: ModuleKind,
    packages_path: &PackagesPath,
) -> Result<Module, String> {
    match url.scheme() {
        "file" => Module::from_path(packages_path, &url.to_file_path().unwrap(), kind)
            .map_err(|it| it.to_string()),
        "untitled" => Ok(Module::new(
            Package::Anonymous {
                url: url
                    .to_string()
                    .strip_prefix("untitled:")
                    .unwrap()
                    .to_string(),
            },
            vec![],
            kind,
        )),
        _ => Err(format!("Unsupported URI scheme: {}", url.scheme())),
    }
}

#[must_use]
pub fn module_to_url(module: &Module, packages_path: &PackagesPath) -> Option<Url> {
    match &module.package() {
        Package::User(_) | Package::Managed(_) => Some(
            Url::from_file_path(
                module
                    .to_possible_paths(packages_path)
                    .unwrap()
                    .into_iter()
                    .find_or_first(|path| path.exists())
                    .unwrap(),
            )
            .unwrap(),
        ),
        Package::Anonymous { url } => Some(Url::parse(&format!("untitled:{url}")).unwrap()),
        Package::Tooling(_) => None,
    }
}

// UTF-8 Byte Offset ↔ LSP Position/Range

#[extension_trait]
pub impl<DB: ModuleDb + PositionConversionDb + ?Sized> LspPositionConversion for DB {
    fn lsp_position_to_offset(&self, module: Module, position: Position) -> Offset {
        let text = self.get_module_content_as_string(module.clone()).unwrap();
        let line_start_offsets = self.line_start_offsets(module);
        lsp_position_to_offset_raw(&text, &line_start_offsets, position)
    }

    fn range_to_lsp_range(&self, module: Module, range: Range<Offset>) -> lsp_types::Range {
        lsp_types::Range {
            start: self.offset_to_lsp_position(module.clone(), range.start),
            end: self.offset_to_lsp_position(module, range.end),
        }
    }
    fn offset_to_lsp_position(&self, module: Module, offset: Offset) -> Position {
        let text = self.get_module_content_as_string(module.clone()).unwrap();
        let line_start_offsets = self.line_start_offsets(module);
        offset_to_lsp_position_raw(&*text, &*line_start_offsets, offset)
    }
}

#[must_use]
pub fn lsp_range_to_range_raw(text: &str, range: lsp_types::Range) -> Range<Offset> {
    let line_start_offsets = line_start_offsets_raw(text);
    let start = lsp_position_to_offset_raw(text, &line_start_offsets, range.start);
    let end = lsp_position_to_offset_raw(text, &line_start_offsets, range.end);
    start..end
}
#[must_use]
pub fn lsp_position_to_offset_raw(
    text: &str,
    line_start_offsets: &[Offset],
    position: Position,
) -> Offset {
    let line_offset = line_start_offsets[position.line as usize];
    let line_length = if position.line as usize == line_start_offsets.len() - 1 {
        text.len() - *line_offset
    } else {
        *line_start_offsets[(position.line + 1) as usize] - *line_offset
    };

    let line = &text[*line_offset..*line_offset + line_length];

    let words = line.encode_utf16().collect::<Vec<_>>();
    let char_offset = if position.character as usize >= words.len() {
        line_length
    } else {
        String::from_utf16(&words[0..position.character as usize])
            .unwrap()
            .len()
    };

    Offset(*line_offset + char_offset)
}

#[must_use]
pub fn range_to_lsp_range_raw<S, L>(
    text: S,
    line_start_offsets: L,
    range: &Range<Offset>,
) -> lsp_types::Range
where
    S: AsRef<str>,
    L: AsRef<[Offset]>,
{
    let text = text.as_ref();
    let line_start_offsets = line_start_offsets.as_ref();
    lsp_types::Range {
        start: offset_to_lsp_position_raw(text, line_start_offsets, range.start),
        end: offset_to_lsp_position_raw(text, line_start_offsets, range.end),
    }
}
#[must_use]
pub fn offset_to_lsp_position_raw<S, L>(
    text: S,
    line_start_offsets: L,
    mut offset: Offset,
) -> Position
where
    S: AsRef<str>,
    L: AsRef<[Offset]>,
{
    let text = text.as_ref();
    let line_start_offsets = line_start_offsets.as_ref();

    if *offset > text.len() {
        *offset = text.len();
    }

    let line = line_start_offsets
        .binary_search(&offset)
        .unwrap_or_else(|i| i - 1);

    let line_start = line_start_offsets[line];
    let character_utf16_offset = text[*line_start..*offset].encode_utf16().count();
    Position {
        line: line.try_into().unwrap(),
        character: character_utf16_offset.try_into().unwrap(),
    }
}

pub trait JoinWithCommasAndAnd {
    fn join_with_commas_and_and(&self) -> String;
}
impl<S: AsRef<str>> JoinWithCommasAndAnd for [S] {
    #[must_use]
    fn join_with_commas_and_and(&self) -> String {
        match self {
            [] => panic!("Joining no parts."),
            [part] => part.as_ref().to_string(),
            [first, second] => format!("{} and {}", first.as_ref(), second.as_ref()),
            [rest @ .., last] => format!(
                "{}, and {}",
                rest.iter().map(AsRef::as_ref).join(", "),
                last.as_ref(),
            ),
        }
    }
}
