//! Hooks for emitting Petri events from the interpreter.

#[cfg(feature = "petri")]
use crate::*;

#[cfg(feature = "petri")]
impl<'tcx> crate::MiriInterpCx<'tcx> {
    /// Emit a Petri event to the monitor. On violation, aborts if fail_fast else logs.
    pub fn emit_petri_event(
        &mut self,
        event: crate::petri::PetriEvent,
        span: Option<crate::petri::SpanLike>,
    ) -> crate::InterpResult<'tcx> {
        let runtime = match self.machine.petri_runtime.as_mut() {
            Some(r) => r,
            None => return crate::interp_ok(()),
        };
        match runtime.on_event(event, span) {
            Ok(()) => crate::interp_ok(()),
            Err(v) => {
                let msg = crate::petri::PetriRuntime::format_violation(&v);
                if runtime.fail_fast() {
                    crate::throw_ub_format!("{}", msg);
                } else {
                    eprintln!("[Petri] {}", msg);
                    crate::interp_ok(())
                }
            }
        }
    }
}
