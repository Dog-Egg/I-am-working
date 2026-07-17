use std::collections::HashMap;

pub(crate) type ActiveGuards = HashMap<String, u32>;

pub(crate) fn adjust_guard_count(active_guards: &mut ActiveGuards, name: &str, active: bool) {
    let name = name.trim();

    if active {
        let count = active_guards.entry(name.to_string()).or_default();
        *count = count.saturating_add(1);
        return;
    }

    let Some(count) = active_guards.get_mut(name) else {
        return;
    };
    if *count > 1 {
        *count -= 1;
    } else {
        active_guards.remove(name);
    }
}

pub(crate) fn sorted_active_guards(active_guards: &ActiveGuards) -> Vec<(String, u32)> {
    let mut active_guards = active_guards
        .iter()
        .map(|(name, count)| (name.clone(), *count))
        .collect::<Vec<_>>();
    active_guards.sort_by(|left, right| left.0.cmp(&right.0));
    active_guards
}

/// Runtime state owned by the no-sleep feature.
#[derive(Default)]
pub(crate) struct NoSleepState {
    pub(crate) active_guards: ActiveGuards,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_count_increments_and_decrements_to_zero() {
        let mut guards = ActiveGuards::new();

        adjust_guard_count(&mut guards, " codex ", true);
        adjust_guard_count(&mut guards, "codex", true);
        assert_eq!(guards.get("codex"), Some(&2));

        adjust_guard_count(&mut guards, "codex", false);
        assert_eq!(guards.get("codex"), Some(&1));

        adjust_guard_count(&mut guards, "codex", false);
        adjust_guard_count(&mut guards, "codex", false);
        assert!(!guards.contains_key("codex"));
    }

    #[test]
    fn active_guards_are_sorted_by_name() {
        let guards = ActiveGuards::from([("zed".to_string(), 1), ("codex".to_string(), 2)]);

        assert_eq!(
            sorted_active_guards(&guards),
            vec![("codex".to_string(), 2), ("zed".to_string(), 1)]
        );
    }
}
