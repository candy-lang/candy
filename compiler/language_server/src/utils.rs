use candy_frontend::{
    cst::CstDb,
    error::CompilerError,
    module::{Module, ModuleDb, ModuleKind, Package},
    position::{line_start_offsets_raw, PositionConversionDb},
};
use itertools::Itertools;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Url};
use std::{ops::Range, path::PathBuf};

pub fn error_into_diagnostic<DB>(db: &DB, module: Module, error: CompilerError) -> Diagnostic
where
    DB: CstDb + ModuleDb + PositionConversionDb,
{
    let related_information = error
        .to_related_information()
        .into_iter()
        .filter_map(|(module, cst_id, message)| {
            let uri = module_to_url(module.clone())?;

            let span = db.find_cst(module.clone(), cst_id).display_span();
            let range = db.range_to_lsp_range(module, span);

            Some(lsp_types::DiagnosticRelatedInformation {
                location: lsp_types::Location { uri, range },
                message,
            })
        })
        .collect();
    Diagnostic {
        range: db.range_to_lsp_range(module, error.span),
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

pub fn module_from_package_root_and_url(
    package_root: PathBuf,
    url: Url,
    kind: ModuleKind,
) -> Module {
    match url.scheme() {
        "file" => {
            Module::from_package_root_and_file(package_root, url.to_file_path().unwrap(), kind)
        }
        "untitled" => Module {
            package: Package::Anonymous {
                url: url
                    .to_string()
                    .strip_prefix("untitled:")
                    .unwrap()
                    .to_string(),
            },
            path: vec![],
            kind,
        },
        _ => panic!("Unsupported URI scheme: {}", url.scheme()),
    }
}

pub fn module_to_url(module: Module) -> Option<Url> {
    match module.package {
        Package::User(_) | Package::External(_) => Some(
            Url::from_file_path(
                module
                    .to_possible_paths()
                    .unwrap()
                    .into_iter()
                    .find_or_first(|path| path.exists())
                    .unwrap(),
            )
            .unwrap(),
        ),
        Package::Anonymous { url } => Some(Url::parse(&format!("untitled:{url}",)).unwrap()),
        Package::Tooling(_) => None,
    }
}

// UTF-8 Byte Offset ↔ LSP Position/Range

pub fn lsp_range_to_range_raw(text: &str, range: lsp_types::Range) -> Range<usize> {
    let line_start_offsets = line_start_offsets_raw(text);
    let start = lsp_position_to_offset_raw(text, &line_start_offsets, range.start);
    let end = lsp_position_to_offset_raw(text, &line_start_offsets, range.end);
    start..end
}
fn lsp_position_to_offset_raw(
    text: &str,
    line_start_offsets: &[usize],
    position: Position,
) -> usize {
    let line_offset = line_start_offsets[position.line as usize];
    let line_length = if position.line as usize == line_start_offsets.len() - 1 {
        text.len() - line_offset
    } else {
        line_start_offsets[(position.line + 1) as usize] - line_offset
    };

    let line = &text[line_offset..line_offset + line_length];

    let words = line.encode_utf16().collect::<Vec<_>>();
    let char_offset = if position.character as usize >= words.len() {
        line_length
    } else {
        String::from_utf16(&words[0..position.character as usize])
            .unwrap()
            .len()
    };

    line_offset + char_offset
}
pub trait LspPositionConversion {
    fn lsp_position_to_offset(&self, module: Module, position: Position) -> usize;

    fn range_to_lsp_range(&self, module: Module, range: Range<usize>) -> lsp_types::Range;
    fn offset_to_lsp_position(&self, module: Module, offset: usize) -> Position;
}
impl<DB: ModuleDb + PositionConversionDb + ?Sized> LspPositionConversion for DB {
    fn lsp_position_to_offset(&self, module: Module, position: Position) -> usize {
        let text = self.get_module_content_as_string(module.clone()).unwrap();
        let line_start_offsets = self.line_start_offsets(module);
        lsp_position_to_offset_raw(&text, &line_start_offsets, position)
    }

    fn range_to_lsp_range(&self, module: Module, range: Range<usize>) -> lsp_types::Range {
        lsp_types::Range {
            start: self.offset_to_lsp_position(module.clone(), range.start),
            end: self.offset_to_lsp_position(module, range.end),
        }
    }
    fn offset_to_lsp_position(&self, module: Module, mut offset: usize) -> Position {
        let text = self.get_module_content_as_string(module.clone()).unwrap();
        if offset > text.len() {
            offset = text.len();
        }
        let line_start_offsets = self.line_start_offsets(module);

        let line = line_start_offsets
            .binary_search(&offset)
            .unwrap_or_else(|i| i - 1);

        let line_start = line_start_offsets[line];
        let character_utf16_offset = text[line_start..offset.to_owned()].encode_utf16().count();
        Position {
            line: line as u32,
            character: character_utf16_offset as u32,
        }
    }
}

pub trait JoinWithCommasAndAnd {
    fn join_with_commas_and_and(self) -> String;
}
impl JoinWithCommasAndAnd for Vec<String> {
    fn join_with_commas_and_and(mut self) -> String {
        match &self[..] {
            [] => panic!("Joining no parts."),
            [part] => part.to_string(),
            [first, second] => format!("{first} and {second}"),
            _ => {
                let last = self.pop().unwrap();
                format!("{}, and {last}", self.into_iter().join(", "))
            }
        }
    }
}
