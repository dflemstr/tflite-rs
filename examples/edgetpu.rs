use std::env::args;
use std::sync;

use tflite::ops::builtin::BuiltinOpResolver;
use tflite::{FlatBufferModel, InterpreterBuilder, Result};

pub fn main() -> Result<()> {
    assert_eq!(args().len(), 2, "edgetpu <tflite model>");
    tflite::edgetpu::Context::enable_debug_printing(10);
    let context = sync::Arc::new(tflite::edgetpu::Context::new()?);

    let filename = args().nth(1).unwrap();

    let model = FlatBufferModel::build_from_file(filename)?;
    let mut resolver = BuiltinOpResolver::default();
    resolver.add_custom(tflite::edgetpu::custom_op_name(), tflite::edgetpu::register_custom_op());

    let builder = InterpreterBuilder::new(&model, &resolver)?;
    let mut interpreter = builder.build()?;
    interpreter.set_external_context(tflite::ExternalContextType::EdgeTpuContext, context);
    interpreter.set_num_threads(1);
    interpreter.allocate_tensors()?;

    println!("=== Pre-invoke Interpreter State ===");
    interpreter.print_state();

    interpreter.invoke()?;

    println!("\n\n=== Post-invoke Interpreter State ===");
    interpreter.print_state();
    Ok(())
}
