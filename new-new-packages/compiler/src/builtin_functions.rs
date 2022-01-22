use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter)]
pub enum BuiltinFunction {
    Add,
    Call0,
    Call1,
    Call2,
    Call3,
    Call4,
    Call5,
    Equals,
    GetArgumentCount,
    IfElse,
    Panic,
    Print,
    TypeOf,
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}
