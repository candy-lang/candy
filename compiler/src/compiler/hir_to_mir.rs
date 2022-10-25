use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    hir,
    mir::{Expression, Id, Mir},
};
use crate::{module::Module, utils::IdGenerator};
use std::{collections::HashMap, sync::Arc};

#[salsa::query_group(HirToMirStorage)]
pub trait HirToMir: CstDb + AstToHir {
    fn mir(&self, module: Module) -> Option<Arc<Mir>>;
}

fn mir(db: &dyn HirToMir, module: Module) -> Option<Arc<Mir>> {
    let (hir, _) = db.hir(module.clone())?;
    let hir = (*hir).clone();
    let mir = compile_module(module, hir);
    Some(Arc::new(mir))
}

fn compile_module(module: Module, hir: hir::Body) -> Mir {
    let mut id_generator = IdGenerator::start_at(0);
    let mut expressions = HashMap::new();
    let mut body = vec![];
    let mut mapping = HashMap::<hir::Id, Id>::new();

    let module_responsibility = {
        let id = id_generator.generate();
        expressions.insert(id, Expression::Responsibility(hir::Id::new(module, vec![])));
        body.push(id);
        id
    };
    for (id, expression) in hir.expressions {
        compile_expression(
            &mut id_generator,
            &mut expressions,
            &mut body,
            &mut mapping,
            module_responsibility,
            &id,
            expression,
        );
    }

    Mir {
        id_generator,
        expressions,
        body,
    }
}

fn compile_expression(
    id_generator: &mut IdGenerator<Id>,
    expressions: &mut HashMap<Id, Expression>,
    body: &mut Vec<Id>,
    mapping: &mut HashMap<hir::Id, Id>,
    responsible_for_needs: Id,
    hir_id: &hir::Id,
    expression: hir::Expression,
) {
    let expression = match expression {
        hir::Expression::Int(int) => Expression::Int(int.into()),
        hir::Expression::Text(text) => Expression::Text(text),
        hir::Expression::Reference(reference) => Expression::Reference(mapping[&reference]),
        hir::Expression::Symbol(symbol) => Expression::Symbol(symbol),
        hir::Expression::Struct(fields) => Expression::Struct(
            fields
                .iter()
                .map(|(key, value)| (mapping[&key], mapping[&value]))
                .collect(),
        ),
        hir::Expression::Lambda(hir::Lambda {
            parameters: original_parameters,
            body: original_body,
            fuzzable,
        }) => {
            let mut parameters = vec![];
            let responsible_parameter: Id = id_generator.generate();
            let mut body = vec![];

            for original_parameter in original_parameters {
                let parameter = id_generator.generate();
                parameters.push(parameter);
                mapping.insert(original_parameter, parameter);
            }

            let responsible = if fuzzable {
                responsible_parameter
            } else {
                // This is a lambda with curly braces, so whoever is responsible
                // for `needs` in the current scope is also responsible for
                // `needs` in the lambda.
                responsible_for_needs
            };

            for (id, expression) in original_body.expressions {
                compile_expression(
                    id_generator,
                    expressions,
                    &mut body,
                    mapping,
                    responsible,
                    &id,
                    expression,
                );
            }

            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                fuzzable,
            }
        }
        hir::Expression::Builtin(builtin) => Expression::Builtin(builtin),
        hir::Expression::Call {
            function,
            arguments,
        } => {
            let responsible = id_generator.generate();
            expressions.insert(responsible, Expression::Responsibility(hir_id.clone()));
            body.push(responsible);

            Expression::Call {
                function: mapping[&function],
                arguments: arguments.iter().map(|arg| mapping[arg]).collect(),
                responsible,
            }
        }
        hir::Expression::UseModule {
            current_module,
            relative_path,
        } => Expression::UseModule {
            current_module,
            relative_path: mapping[&relative_path],
            // The `UseModule` expression only exists in the generated `use`
            // function. If a use fails, that's also the fault of the caller.
            // Essentially, the `UseModule` expression works exactly like a
            // `needs`.
            responsible: responsible_for_needs,
        },
        hir::Expression::Needs { condition, reason } => Expression::Needs {
            responsible: responsible_for_needs,
            condition: mapping[&condition],
            reason: mapping[&reason],
        },
        hir::Expression::Error { child, errors } => Expression::Error {
            child: child.map(|child| mapping[&child]),
            errors,
        },
    };
    let id = id_generator.generate();
    expressions.insert(id, expression);
    body.push(id);
    mapping.insert(hir_id.clone(), id);
}
