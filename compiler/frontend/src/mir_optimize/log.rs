use super::{complexity::Complexity, current_expression::CurrentExpression};
use crate::{
    hir_to_mir::ExecutionTarget,
    mir::{Body, Id},
    rich_ir::RichIrBuilder,
    tracing::TracingConfig,
};
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    sync::{Mutex, OnceLock},
};

static LOGGER: OnceLock<Option<Mutex<OptimizationLogger>>> = OnceLock::new();

#[derive(Debug)]
pub struct OptimizationLogger {
    file: BufWriter<File>,
    indentation: usize,
}
impl OptimizationLogger {
    // We specify Python for code blocks to get some syntax highlighting of
    // comments and expressions.

    pub fn log_optimized_mir_without_tail_calls_start(
        target: &ExecutionTarget,
        tracing: TracingConfig,
    ) {
        Self::run(|logger| {
            logger.write_line("1. `optimized_mir_without_tail_calls(…)`");
            logger.indent();
            logger.write_newline();
            logger.write_line(&format!("Execution target: {target}"));
            logger.write_newline();
            logger.write_line("```python");
            let mut builder = RichIrBuilder::default();
            builder.push_tracing_config(tracing);
            let tracing = builder.finish(false).text;
            logger.write_lines(&tracing);
            logger.write_line("```");
        });
    }
    pub fn log_optimized_mir_without_tail_calls_end(
        complexity_before: Complexity,
        complexity_after: Complexity,
    ) {
        Self::run(|logger| {
            logger.dedent();
            logger.write_line(&format!("Complexity before: {complexity_before}"));
            logger.write_line(&format!("Complexity after: {complexity_after}"));
        });
    }

    pub fn log_optimize_body_start(body: &Body) {
        Self::run(|logger| {
            logger.write_line("1. `optimize_body(…)`");
            logger.indent();
            logger.write_newline();
            logger.write_line("```python");
            logger.write_lines(&body.to_string());
            logger.write_line("```");
        });
    }
    pub fn log_optimize_body_end() {
        Self::run(|logger| {
            logger.dedent();
        });
    }

    pub fn log_optimize_expression_start(expression: &CurrentExpression) {
        Self::run(|logger| {
            logger.write_line(&format!("1. `optimize_expression({})`", expression.id()));
            logger.indent();
            logger.write_newline();
            logger.write_line("```python");
            logger.write_lines(&(*expression).to_string());
            logger.write_line("```");
        });
    }
    pub fn log_optimize_expression_end() {
        Self::run(|logger| {
            logger.dedent();
        });
    }

    pub fn log_replace_id_references(optimization_name: &str, id: Id) {
        Self::run(|logger| {
            logger.write_line(&format!(
                "1. {optimization_name} calls `replace_id_references({id}, …)`",
            ));
        });
    }
    pub fn log_prepend_optimized(optimization_name: &str, following_id: Id, new: &str) {
        Self::run(|logger| {
            logger.write_line(&format!(
                "1. {optimization_name} calls `prepend_optimized(…)`:",
            ));
            logger.indent();
            logger.write_newline();
            logger.write_line("```python");
            logger.write_line(&format!("# Inserts before {following_id}:"));
            logger.write_lines(new);
            logger.write_line("```");
            logger.write_newline();
            logger.dedent();
        });
    }
    pub fn log_replace_with(optimization_name: &str, old: &str, new: &str) {
        Self::run(|logger| {
            logger.write_line(&format!(
                "1. {optimization_name} calls `replace_with(…)`/`replace_with_multiple(…)`:",
            ));
            logger.indent();
            logger.write_newline();
            logger.write_line("```python");
            logger.write_line("# Before:");
            logger.write_lines(old);
            logger.write_newline();
            logger.write_line("# After:");
            logger.write_lines(new);
            logger.write_line("```");
            logger.write_newline();
            logger.dedent();
        });
    }

    pub fn is_enabled() -> bool {
        Self::get().is_some()
    }
    fn get() -> &'static Option<Mutex<Self>> {
        LOGGER.get_or_init(|| {
            env::var("CANDY_MIR_OPTIMIZATION_LOG").ok().map(|path| {
                let mut file = BufWriter::new(File::create(path).unwrap());
                writeln!(file, "# Candy MIR Optimization Log").unwrap();
                writeln!(file).unwrap();

                Mutex::new(Self {
                    file,
                    indentation: 0,
                })
            })
        })
    }
    fn run(run: impl FnOnce(&mut Self)) {
        if let Some(logger) = Self::get() {
            let mut logger = logger.lock().unwrap();
            run(&mut logger);
            logger.file.flush().unwrap();
        }
    }
    fn write_lines(&mut self, text: &str) {
        for line in text.lines() {
            self.write_line(line);
        }
    }
    fn write_line(&mut self, line: &str) {
        self.write_indentation();
        writeln!(self.file, "{line}").unwrap();
    }
    fn write_newline(&mut self) {
        writeln!(self.file).unwrap();
    }
    fn indent(&mut self) {
        self.indentation += 1;
    }
    fn dedent(&mut self) {
        self.indentation -= 1;
    }
    fn write_indentation(&mut self) {
        write!(self.file, "{}", "    ".repeat(self.indentation)).unwrap();
    }
}
