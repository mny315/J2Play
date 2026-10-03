use super::{EmuError, Handle, Machine, Value, reference_argument};

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_allocate_native<T>(
        &mut self,
        roots: &[Value],
        mut allocate: impl FnMut(&mut m3g::Runtime) -> Result<T, EmuError>,
    ) -> Result<T, EmuError> {
        let result = allocate(&mut self.m3g.runtime);
        if !result
            .as_ref()
            .is_err_and(|error| error.code() == "resource-limit")
        {
            return result;
        }

        // Java wrappers can be substantially smaller than the M3G data they
        // retain, so short-lived wrappers may fill the native arena long
        // before the managed heap itself needs collection. Collect with the
        // complete current root set, sweep unreachable native owners, and
        // retry the bounded allocation exactly once.
        let gc_roots = self.roots(&[], roots);
        self.collect_heap(gc_roots);
        allocate(&mut self.m3g.runtime)
    }

    pub(in crate::machine) fn m3g_receiver(&self, args: &[Value]) -> Result<m3g::Handle, EmuError> {
        self.m3g_handle(reference_argument(args, 0)?)
    }

    pub(in crate::machine) fn m3g_handle(&self, guest: Handle) -> Result<m3g::Handle, EmuError> {
        self.m3g.runtime.resolve_guest(guest.to_raw())
    }

    pub(in crate::machine) fn m3g_guest_handle(
        &self,
        native: Option<m3g::Handle>,
    ) -> Option<Handle> {
        native
            .and_then(|handle| self.m3g.runtime.guest_reference(handle))
            .map(Handle::from_raw)
    }
}
