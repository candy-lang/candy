use strum_macros::EnumIter;

#[derive(Debug, EnumIter)]
pub enum RuntimeFunction {
    MakeTag,
    MakeInt,
    MakeText,
    MakeList,
    MakeFunction,
    MakeStruct,
    Panic,
    FreeValue,
    DupValue,
    DropValue,
    PrintValue,
    RunMain,
    GetCapture,
    GetFunction,
}

impl RuntimeFunction {
    pub fn name(&self) -> &'static str {
        match self {
            RuntimeFunction::MakeTag => "make_candy_tag",
            RuntimeFunction::MakeInt => "make_candy_int",
            RuntimeFunction::MakeText => "make_candy_text",
            RuntimeFunction::MakeList => "make_candy_list",
            RuntimeFunction::MakeFunction => "make_candy_function",
            RuntimeFunction::MakeStruct => "make_candy_struct",
            RuntimeFunction::Panic => "candy_panic",
            RuntimeFunction::FreeValue => "free_candy_value",
            RuntimeFunction::DupValue => "dup_candy_value",
            RuntimeFunction::DropValue => "drop_candy_value",
            RuntimeFunction::PrintValue => "print_candy_value",
            RuntimeFunction::RunMain => "run_candy_main",
            RuntimeFunction::GetCapture => "get_candy_function_capture",
            RuntimeFunction::GetFunction => "get_candy_function_ptr",
        }
    }

    pub fn num_params(&self) -> usize {
        match self {
            RuntimeFunction::MakeTag => 2,
            RuntimeFunction::MakeInt => 1,
            RuntimeFunction::MakeText => 1,
            RuntimeFunction::MakeList => 1,
            RuntimeFunction::MakeFunction => 3,
            RuntimeFunction::MakeStruct => 3,
            RuntimeFunction::Panic => 1,
            RuntimeFunction::FreeValue => 1,
            RuntimeFunction::DupValue => 2,
            RuntimeFunction::DropValue => 1,
            RuntimeFunction::PrintValue => 1,
            RuntimeFunction::RunMain => 2,
            RuntimeFunction::GetCapture => 1,
            RuntimeFunction::GetFunction => 1,
        }
    }

    pub fn has_return_value(&self) -> bool {
        match self {
            RuntimeFunction::MakeTag => true,
            RuntimeFunction::MakeInt => true,
            RuntimeFunction::MakeText => true,
            RuntimeFunction::MakeList => true,
            RuntimeFunction::MakeFunction => true,
            RuntimeFunction::MakeStruct => true,
            RuntimeFunction::Panic => false,
            RuntimeFunction::FreeValue => false,
            RuntimeFunction::DupValue => false,
            RuntimeFunction::DropValue => false,
            RuntimeFunction::PrintValue => false,
            RuntimeFunction::RunMain => true,
            RuntimeFunction::GetCapture => true,
            RuntimeFunction::GetFunction => true,
        }
    }
}
