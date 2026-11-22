use crate::{Plot3DContext, Plot3DUi};

type Pop = unsafe extern "C" fn(i32);

struct StyleScope<'a> {
    context: &'a Plot3DContext,
    count: i32,
    pop: Pop,
}

impl Drop for StyleScope<'_> {
    fn drop(&mut self) {
        self.context
            .with_bound_context(|| unsafe { (self.pop)(self.count) });
    }
}

impl Plot3DUi<'_> {
    pub(super) fn with_style_scope<R>(
        &self,
        count: usize,
        push: impl FnOnce(),
        pop: Pop,
        f: impl FnOnce() -> R,
    ) -> R {
        let count = i32::try_from(count).expect("too many ImPlot3D style overrides");
        self.with_bound_context(push);
        let _restore = StyleScope {
            context: self.context,
            count,
            pop,
        };
        f()
    }
}
