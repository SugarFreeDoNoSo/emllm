use emllm::native::{NativeErrorKind, NativeLimits, Phase, Resource};

#[test]
fn all_budgets_are_inclusive_and_zero_rejects_the_first_element() {
    let limits = NativeLimits {
        base_nodes: 2,
        template_instructions: 2,
        ports: 2,
        roots: 2,
        closed_nodes: 2,
        ..NativeLimits::default()
    };
    for (phase, resource) in [
        (Phase::Base, Resource::BaseNodes),
        (Phase::Template, Resource::TemplateInstructions),
        (Phase::Template, Resource::Ports),
        (Phase::Compose, Resource::Roots),
        (Phase::Compose, Resource::ClosedNodes),
        (Phase::Load, Resource::PackageBytes),
        (Phase::Load, Resource::Tensors),
        (Phase::Compile, Resource::ReachableNodes),
        (Phase::Plan, Resource::NumericSlots),
    ] {
        let limit = limits.limit(resource);
        assert!(limits.check(phase, resource, 0).is_ok());
        assert!(limits.check(phase, resource, limit).is_ok());
        let error = limits.check(phase, resource, limit + 1).unwrap_err();
        assert_eq!(error.phase, phase);
        assert_eq!(
            error.kind,
            NativeErrorKind::Limit {
                resource,
                requested: limit + 1,
                limit
            }
        );
    }
    let zero = NativeLimits {
        package_bytes: 0,
        tensors: 0,
        base_nodes: 0,
        template_instructions: 0,
        ports: 0,
        roots: 0,
        closed_nodes: 0,
        reachable_nodes: 0,
        numeric_slots: 0,
    };
    for resource in [
        Resource::PackageBytes,
        Resource::Tensors,
        Resource::BaseNodes,
        Resource::TemplateInstructions,
        Resource::Ports,
        Resource::Roots,
        Resource::ClosedNodes,
        Resource::ReachableNodes,
        Resource::NumericSlots,
    ] {
        assert!(zero.check(Phase::Base, resource, 0).is_ok());
        assert!(zero.check(Phase::Base, resource, 1).is_err());
    }
}

#[test]
fn closed_budget_counts_base_plus_delta_and_overflow_is_not_wrapped() {
    let limits = NativeLimits {
        closed_nodes: 5,
        ..NativeLimits::default()
    };
    assert_eq!(
        limits.checked_add(Phase::Compose, Resource::ClosedNodes, 4, 1),
        Ok(5)
    );
    assert!(matches!(
        limits
            .checked_add(Phase::Compose, Resource::ClosedNodes, 4, 2)
            .unwrap_err()
            .kind,
        NativeErrorKind::Limit {
            requested: 6,
            limit: 5,
            ..
        }
    ));
    assert!(matches!(
        limits
            .checked_add(Phase::Compose, Resource::ClosedNodes, usize::MAX, 1)
            .unwrap_err()
            .kind,
        NativeErrorKind::SizeOverflow { .. }
    ));
    assert!(matches!(
        limits
            .checked_elements(Phase::Template, Resource::Ports, &[usize::MAX, 2])
            .unwrap_err()
            .kind,
        NativeErrorKind::SizeOverflow { .. }
    ));
    assert_eq!(
        limits.checked_elements(Phase::Template, Resource::Ports, &[16, 64]),
        Ok(1024)
    );
    assert!(
        limits
            .checked_elements(Phase::Template, Resource::Ports, &[17, 64])
            .is_err()
    );
    assert!(matches!(
        limits
            .checked_bytes::<u64>(Phase::Base, Resource::BaseNodes, usize::MAX / 4)
            .unwrap_err()
            .kind,
        NativeErrorKind::SizeOverflow { .. }
    ));
}

#[test]
fn rejected_reservations_preserve_existing_values_and_capacity() {
    let limits = NativeLimits {
        roots: 2,
        ..NativeLimits::default()
    };
    let mut roots = vec![7usize];
    limits
        .reserve(Phase::Compose, Resource::Roots, &mut roots, 1)
        .unwrap();
    roots.push(9);
    let capacity = roots.capacity();
    assert!(
        limits
            .reserve(Phase::Compose, Resource::Roots, &mut roots, 1)
            .is_err()
    );
    assert_eq!(roots, [7, 9]);
    assert_eq!(roots.capacity(), capacity);
}
