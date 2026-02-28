//! Hooks for emitting Petri events from the interpreter.

#[cfg(feature = "petri")]
pub trait PetriEvalContextExt<'tcx>: crate::MiriInterpCxExt<'tcx> {
    /// Emit a Petri event to the monitor. On violation, aborts if fail_fast else logs.
    fn emit_petri_event(
        &mut self,
        event: crate::petri::PetriEvent,
        span: Option<crate::petri::SpanLike>,
    ) -> crate::InterpResult<'tcx>;
}

#[cfg(feature = "petri")]
impl<'tcx, T: crate::MiriInterpCxExt<'tcx>> PetriEvalContextExt<'tcx> for T {
    fn emit_petri_event(
        &mut self,
        event: crate::petri::PetriEvent,
        span: Option<crate::petri::SpanLike>,
    ) -> crate::InterpResult<'tcx> {
        let this = self.eval_context_mut();
        let Some(runtime) = this.machine.petri_runtime.as_mut() else {
            return crate::interp_ok(());
        };
        // let runtime = match this.machine.petri_runtime.as_mut() {
        //     Some(r) => r,
        //     None => return crate::interp_ok(()),
        // };
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
