use std::collections::HashSet;

/// Runtime state owned by the no-sleep feature.
#[derive(Default)]
pub(crate) struct NoSleepState {
    pub(crate) active_guards: HashSet<String>,
}
