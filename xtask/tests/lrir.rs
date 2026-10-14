#[path = "support/cached_integer_fixture.rs"]
mod cached_integer_fixture;
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    ir::{
        op::{DiagnosticTemplate, HintRecipe, LiteralKind, LiteralRecipe, Op},
        pc::{NodeId, Phase, SlotId},
        recipe::{Origin, RecipeDraft, RecipeExit},
        validate::verify,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole, ScopeId},
        parse::ParsedFile,
    },
};
fn body() -> BodyIndex {
    let blob =
        SourceBlob::new(b"fn live(){if true{1}else{unimplemented_call()}}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([41; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap()
}
fn draft(body: &BodyIndex) -> RecipeDraft {
    let key = body.pc(NodeId(0), Phase::Entry).unwrap();
    RecipeDraft {
        origin: Origin {
            span: body.span(NodeId(0)).unwrap(),
            expansion_ancestry: vec![],
        },
        key,
        operations: vec![Op::Unit {
            destination: SlotId(0),
        }],
        exits: vec![RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap())],
        dependencies: vec![],
    }
}
#[test]
fn verifier_rejects_slot_kinds_bounds_and_duplicate_outputs() {
    let body = body();
    for operations in [
        vec![Op::Unit {
            destination: SlotId(1),
        }],
        vec![Op::Unit {
            destination: SlotId(u32::MAX),
        }],
        vec![
            Op::Unit {
                destination: SlotId(0),
            },
            Op::Unit {
                destination: SlotId(0),
            },
        ],
        vec![Op::ResultTransfer {
            source: SlotId(0),
            destination: SlotId(0),
        }],
    ] {
        let mut d = draft(&body);
        d.operations = operations;
        assert_eq!(verify(d, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
    let mut d = draft(&body);
    d.operations = vec![Op::ScopeEnter {
        scope: ScopeId(u32::MAX),
    }];
    assert!(verify(d, &body).is_err());
}
#[test]
fn verifier_rejects_missing_duplicate_exits_and_operation_count_boundaries() {
    let body = body();
    let mut d = draft(&body);
    d.exits.clear();
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.exits.push(d.exits[0].clone());
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.operations.clear();
    let verified = verify(d, &body).unwrap();
    assert!(verified.recipe().operations.is_empty());
    let mut d = draft(&body);
    d.operations = vec![
        Op::SetHint {
            child: SlotId(0),
            hint: HintRecipe::None
        };
        8
    ];
    verify(d.clone(), &body).unwrap();
    d.operations.push(Op::SetHint {
        child: SlotId(0),
        hint: HintRecipe::None,
    });
    assert!(verify(d, &body).is_err());
}
#[test]
fn unsupported_recipe_is_structurally_valid_and_carries_a_deferred_fault() {
    let body = body();
    let mut d = draft(&body);
    d.operations = vec![Op::Unsupported(DiagnosticTemplate {
        message: "reached unsupported operation".into(),
    })];
    let verified = verify(d, &body).unwrap();
    let Op::Unsupported(template) = &verified.recipe().operations[0] else {
        panic!("unsupported opcode lost")
    };
    // This is the diagnostic payload contract; VM execution is accepted in T024.
    assert_eq!(template.reached_unsupported().code, "LRR-E-UNSUPPORTED");
}
#[test]
fn branch_construction_does_not_require_any_successor_recipe() {
    let body = body();
    let node = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::If))
        .unwrap();
    assert_eq!(node.operands.len(), 3);
    let condition = body.nodes()[node.operands[0].0 as usize].value.unwrap();
    let yes = body.pc(node.operands[1], Phase::Entry).unwrap();
    let no = body.pc(node.operands[2], Phase::Entry).unwrap();
    let recipe = lrr::frontend::thunks::branch(
        &body,
        body.pc(node.id, Phase::Commit).unwrap(),
        node.value.unwrap(),
        condition,
        yes.clone(),
        no.clone(),
    )
    .unwrap();
    assert!(matches!(&recipe.recipe().exit,RecipeExit::Branch{yes:y,no:n,..} if *y==yes && *n==no));
    // There is no successor recipe store or provider in this construction path.
    assert_eq!(recipe.recipe().operations.len(), 1);
}
#[test]
fn verifier_validates_pcs_origins_and_dependency_identity_without_typechecking() {
    use lrr::db::dependency::{DependencyStamp, InputKey};
    let body = body();
    let mut d = draft(&body);
    d.key.node = u32::MAX;
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.origin.span.relative_end = u32::MAX;
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.origin.expansion_ancestry.push(Digest([9; 32]));
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.dependencies = vec![
        DependencyStamp::Input {
            key: InputKey::DefinitionBody(Digest([1; 32])),
            fingerprint: Digest([2; 32]),
        },
        DependencyStamp::Input {
            key: InputKey::DefinitionBody(Digest([1; 32])),
            fingerprint: Digest([3; 32]),
        },
    ];
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.operations = vec![Op::Literal {
        literal: LiteralRecipe {
            decoded: Some(cached_integer_fixture::integer("123")),
            kind: LiteralKind::Integer,
            spelling: "123".into(),
            source: body.span(NodeId(0)).unwrap(),
        },
        hint: HintRecipe::None,
        destination: SlotId(0),
    }];
    let verified = verify(d, &body).unwrap();
    assert_ne!(verified.recipe().fingerprint(), Digest([0; 32]));
}

#[test]
fn dependency_encoding_is_canonical_and_keeps_namespace_and_fingerprint_identity() {
    use lrr::{
        db::{
            dependency::{DependencyStamp, InputKey},
            key::BucketKey,
        },
        source::manifest::Namespace,
        syntax::outline::ScopeKey,
    };
    let body = body();
    let mut a = draft(&body);
    let first = DependencyStamp::Input {
        key: InputKey::DefinitionBody(Digest([1; 32])),
        fingerprint: Digest([2; 32]),
    };
    let second = DependencyStamp::Input {
        key: InputKey::DefinitionHeader(Digest([3; 32])),
        fingerprint: Digest([4; 32]),
    };
    a.dependencies = vec![first.clone(), second.clone()];
    let mut b = a.clone();
    b.dependencies.reverse();
    assert_eq!(
        verify(a, &body).unwrap().recipe().fingerprint(),
        verify(b, &body).unwrap().recipe().fingerprint()
    );
    let scope = ScopeKey {
        crate_key: Digest([1; 32]),
        module_path: vec![],
        owner: None,
    };
    let value = InputKey::NameCandidates(BucketKey {
        scope: scope.clone(),
        namespace: Namespace::Value,
        name: "same".into(),
    });
    let ty = InputKey::NameCandidates(BucketKey {
        scope,
        namespace: Namespace::Type,
        name: "same".into(),
    });
    assert_ne!(value.fingerprint(), ty.fingerprint());
    let changed = DependencyStamp::Input {
        key: InputKey::DefinitionBody(Digest([1; 32])),
        fingerprint: Digest([9; 32]),
    };
    assert_ne!(first.fingerprint(), changed.fingerprint());
}

#[test]
fn explicit_call_and_effect_exits_keep_sites_resume_and_result_slots_checked() {
    use lrr::ir::recipe::{CallSiteId, EffectSiteId};
    let body = body();
    let resume = body.pc(NodeId(0), Phase::Exit).unwrap();
    for exit in [
        RecipeExit::Call {
            site: CallSiteId(0),
            resume: resume.clone(),
            result: SlotId(0),
        },
        RecipeExit::Effect {
            site: EffectSiteId(0),
            resume: resume.clone(),
            result: SlotId(0),
        },
    ] {
        let mut d = draft(&body);
        d.exits = vec![exit];
        let verified = verify(d, &body).unwrap();
        assert!(verified.recipe().exit.requires_dispatch_boundary());
    }
    let mut d = draft(&body);
    d.exits = vec![RecipeExit::Call {
        site: CallSiteId(u32::MAX),
        resume: resume.clone(),
        result: SlotId(0),
    }];
    assert!(verify(d, &body).is_err());
    let mut d = draft(&body);
    d.exits = vec![RecipeExit::Effect {
        site: EffectSiteId(0),
        resume,
        result: SlotId(1),
    }];
    assert!(verify(d, &body).is_err());
}

#[test]
fn decoded_literal_payloads_are_admitted_by_kind_and_encoded_in_recipe_identity() {
    use lrr::ir::op::{DecodedLiteral, PrimitiveType};
    let body = body();
    let make = |kind, decoded| {
        let mut d = draft(&body);
        d.operations = vec![Op::Literal {
            literal: LiteralRecipe {
                kind,
                decoded,
                spelling: "source spelling".into(),
                source: body.span(NodeId(0)).unwrap(),
            },
            hint: HintRecipe::None,
            destination: SlotId(0),
        }];
        d
    };
    for (kind, decoded) in [
        (LiteralKind::Integer, None),
        (LiteralKind::NegativeInteger, None),
        (LiteralKind::Char, None),
        (LiteralKind::Byte, Some(DecodedLiteral::Char('a'))),
        (LiteralKind::Integer, Some(DecodedLiteral::Byte(1))),
        (
            LiteralKind::Float,
            Some(DecodedLiteral::Float {
                decimal: String::new(),
                suffix: None,
            }),
        ),
        (
            LiteralKind::Float,
            Some(DecodedLiteral::Float {
                decimal: "1.0".into(),
                suffix: Some(PrimitiveType::I32),
            }),
        ),
    ] {
        assert_eq!(
            verify(make(kind, decoded), &body).unwrap_err().code,
            "LRR-E-INTERNAL-IR"
        );
    }
    let a = verify(
        make(LiteralKind::Char, Some(DecodedLiteral::Char('a'))),
        &body,
    )
    .unwrap();
    let b = verify(
        make(LiteralKind::Char, Some(DecodedLiteral::Char('β'))),
        &body,
    )
    .unwrap();
    assert_ne!(a.recipe().fingerprint(), b.recipe().fingerprint());
    let float = |suffix| {
        Some(DecodedLiteral::Float {
            decimal: "1.0".into(),
            suffix,
        })
    };
    let a = verify(
        make(LiteralKind::Float, float(Some(PrimitiveType::F32))),
        &body,
    )
    .unwrap();
    let b = verify(
        make(LiteralKind::Float, float(Some(PrimitiveType::F64))),
        &body,
    )
    .unwrap();
    assert_ne!(a.recipe().fingerprint(), b.recipe().fingerprint());
}

#[test]
fn local_declaration_hint_and_assignment_admission_reject_invalid_bindings_and_slots() {
    let body = body();
    for op in [
        Op::DeclareLocal {
            binding: NodeId(0),
            hint: HintRecipe::None,
            writable: true,
        },
        Op::LocalHint {
            binding: NodeId(u32::MAX),
            child: SlotId(0),
        },
        Op::AssignLocal {
            binding: NodeId(0),
            source: SlotId(0),
            destination: SlotId(0),
        },
    ] {
        let mut d = draft(&body);
        d.operations = vec![op];
        assert_eq!(verify(d, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
    let blob = SourceBlob::new(b"fn live(){let x=1_i64;}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([41; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let binding = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Pattern)
        .unwrap()
        .id;
    for op in [
        Op::LocalHint {
            binding,
            child: SlotId(u32::MAX),
        },
        Op::AssignLocal {
            binding,
            source: SlotId(0),
            destination: SlotId(0),
        },
        Op::AssignLocal {
            binding,
            source: SlotId(u32::MAX),
            destination: SlotId(0),
        },
    ] {
        let mut d = draft(&body);
        d.operations = vec![op];
        assert_eq!(verify(d, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
}

#[test]
fn selected_branch_hint_admission_rejects_missing_slots_and_aliasing() {
    let body = body();
    let node = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::If))
        .unwrap();
    let parent = node.value.unwrap();
    let condition = body.nodes()[node.operands[0].0 as usize].value.unwrap();
    let yes = body.nodes()[node.operands[1].0 as usize].value.unwrap();
    for child in [condition, parent, SlotId(u32::MAX)] {
        let mut d = draft(&body);
        d.operations = vec![Op::BranchHint {
            parent,
            condition,
            yes: Some(child),
            no: None,
            boolean: false,
        }];
        assert_eq!(verify(d, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
    let make = |boolean| {
        let mut d = draft(&body);
        d.operations = vec![Op::BranchHint {
            parent,
            condition,
            yes: Some(yes),
            no: None,
            boolean,
        }];
        verify(d, &body).unwrap()
    };
    assert_ne!(
        make(false).recipe().fingerprint(),
        make(true).recipe().fingerprint()
    );
}

#[test]
fn backedge_admission_requires_loop_entry_and_preserves_exit_identity() {
    let original = body();
    let mut d = draft(&original);
    d.exits = vec![RecipeExit::Backedge(
        original.pc(NodeId(0), Phase::Entry).unwrap(),
    )];
    assert_eq!(verify(d, &original).unwrap_err().code, "LRR-E-INTERNAL-IR");
    let blob = SourceBlob::new(b"fn live(){while false{}}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([45; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let node = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::While))
        .unwrap();
    let target = body.pc(node.id, Phase::Entry).unwrap();
    let mut d = draft(&body);
    d.exits = vec![RecipeExit::Backedge(target.clone())];
    let backedge = verify(d.clone(), &body).unwrap();
    assert!(backedge.recipe().exit.requires_dispatch_boundary());
    d.exits = vec![RecipeExit::Next(target)];
    assert_ne!(
        backedge.recipe().fingerprint(),
        verify(d.clone(), &body).unwrap().recipe().fingerprint()
    );
    d.exits = vec![RecipeExit::Backedge(
        body.pc(node.id, Phase::Commit).unwrap(),
    )];
    assert_eq!(verify(d, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
}

#[test]
fn control_admission_rejects_non_ancestor_targets_and_invalid_break_resume() {
    use lrr::ir::recipe::ControlRecipe;
    let blob = SourceBlob::new(b"fn live(){loop{break;} loop{break;}}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([49; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let first = body
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Break))
        .unwrap();
    let own = first.enclosing_syntax_target.unwrap();
    let other = body.labels().iter().find(|l| l.id != own).unwrap().id;
    let resume = body.pc(NodeId(0), Phase::Commit).unwrap();
    let make = |exit| {
        let mut d = draft(&body);
        d.key = body.pc(first.id, Phase::Entry).unwrap();
        d.origin.span = body.span(first.id).unwrap();
        d.operations = vec![Op::Unit {
            destination: first.value.unwrap(),
        }];
        d.exits = vec![exit];
        d
    };
    verify(
        make(RecipeExit::Control(ControlRecipe::Break {
            target: own,
            value: Some(first.value.unwrap()),
            resume: resume.clone(),
        })),
        &body,
    )
    .unwrap();
    for exit in [
        RecipeExit::Control(ControlRecipe::Break {
            target: other,
            value: None,
            resume: resume.clone(),
        }),
        RecipeExit::Control(ControlRecipe::Continue { target: other }),
    ] {
        assert_eq!(
            verify(make(exit), &body).unwrap_err().code,
            "LRR-E-INTERNAL-IR"
        );
    }
    let mut bad = resume;
    bad.node = u32::MAX;
    assert_eq!(
        verify(
            make(RecipeExit::Control(ControlRecipe::Break {
                target: own,
                value: None,
                resume: bad
            })),
            &body
        )
        .unwrap_err()
        .code,
        "LRR-E-INTERNAL-IR"
    );
}

#[test]
fn parameter_hint_encoding_retains_definition_identity_and_rejects_foreign_hints() {
    let body = body();
    let mut original = draft(&body);
    original.operations = vec![Op::SetHint {
        child: SlotId(0),
        hint: HintRecipe::Parameter(lrr::traits::term::ParamKey {
            definition: body.body().definition,
            ordinal: 0,
        }),
    }];
    let first = verify(original.clone(), &body).unwrap();
    let mut second = original.clone();
    second.operations = vec![Op::SetHint {
        child: SlotId(0),
        hint: HintRecipe::Parameter(lrr::traits::term::ParamKey {
            definition: body.body().definition,
            ordinal: 1,
        }),
    }];
    let second = verify(second, &body).unwrap();
    assert_ne!(first.recipe().fingerprint(), second.recipe().fingerprint());
    for key in [
        lrr::traits::term::ParamKey {
            definition: Digest([255; 32]),
            ordinal: 0,
        },
        lrr::traits::term::ParamKey {
            definition: body.body().definition,
            ordinal: 100_000,
        },
    ] {
        let mut rejected = original.clone();
        rejected.operations = vec![Op::SetHint {
            child: SlotId(0),
            hint: HintRecipe::Parameter(key),
        }];
        assert_eq!(
            verify(rejected, &body).unwrap_err().code,
            "LRR-E-INTERNAL-IR"
        );
    }
}

#[test]
fn declaration_markers_cannot_replace_expression_operations() {
    let body = body();
    let node = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap();
    let draft = RecipeDraft {
        key: body.pc(node.id, Phase::Entry).unwrap(),
        origin: Origin {
            span: body.span(node.id).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![Op::Declaration],
        exits: vec![RecipeExit::Next(body.pc(node.id, Phase::Exit).unwrap())],
        dependencies: vec![],
    };
    assert_eq!(verify(draft, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
}
