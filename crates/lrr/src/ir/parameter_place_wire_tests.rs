//! Original-source parameter locations and explicit recipe/cache wire migration.
//! These are structural controls, not a whole upstream runtime pass.
use crate::{
    core::{
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    ir::{
        op::{LRIR_RECIPE_SCHEMA, OPCODE_COUNT, Op, SOURCE_CONSTANT_OPCODE},
        pc::Phase,
        recipe::{Recipe, RecipeDraft, RecipeExit},
        validate::verify,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParsedFile,
    },
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
);
#[test]
fn unchanged_dereference_parameters_address_original_cells_without_owning_read_all_editions() {
    assert_eq!(LRIR_RECIPE_SCHEMA, 16);
    assert_eq!(Recipe::DOMAIN, "lrr/v16/lrir-recipe");
    assert_eq!(SOURCE_CONSTANT_OPCODE, 43);
    assert_eq!(OPCODE_COUNT, 70);
    for edition in [2015, 2018, 2021, 2024] {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([194; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let handle = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "takes_two")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = parsed.index_body(handle, Default::default()).unwrap();
        let mut count = 0;
        for node in body
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
        {
            let key = body.pc(node.id, Phase::Entry).unwrap();
            let verified = parsed.initial_thunk(handle, &body, &key).unwrap();
            let recipe = verified.recipe();
            let [
                Op::ParameterPlace {
                    ordinal,
                    destination,
                },
            ] = recipe.operations.as_ref()
            else {
                panic!("parameter dereference must address its original cell")
            };
            assert_eq!(*ordinal, count);
            assert_eq!(Some(*destination), node.place);
            assert_eq!(recipe.operations[0].opcode_tag(), 44);
            let original = RecipeDraft {
                key: recipe.key.clone(),
                origin: recipe.origin.clone(),
                operations: recipe.operations.to_vec(),
                exits: vec![recipe.exit.clone()],
                dependencies: recipe.dependencies.to_vec(),
            };
            verify(original.clone(), &body).unwrap();
            let mut wrong = original.clone();
            wrong.operations = vec![Op::ParameterPlace {
                ordinal: *ordinal,
                destination: node.value.unwrap(),
            }];
            assert!(verify(wrong, &body).is_err());
            let mut wrong = original.clone();
            wrong.key.phase = Phase::Exit.code().unwrap();
            assert!(verify(wrong, &body).is_err());
            let mut expected = StableEncoder::new(Recipe::DOMAIN);
            expected.record(&recipe.key);
            expected.record(&recipe.origin.span.body_key);
            expected.u32(recipe.origin.span.relative_start);
            expected.u32(recipe.origin.span.relative_end);
            expected.u64(0);
            expected.u64(1);
            expected.u32(44);
            expected.u32(*ordinal);
            expected.u32(destination.0);
            let RecipeExit::Next(next) = &recipe.exit else {
                panic!("ordinary parameter place continuation")
            };
            expected.u32(0);
            expected.record(next);
            expected.u64(0);
            assert_eq!(recipe.fingerprint(), expected.finish());
            let mut old = StableEncoder::new("lrr/v9/lrir-recipe");
            recipe.encode_fields(&mut old);
            assert_ne!(recipe.fingerprint(), old.finish());
            let mut another = original;
            another.operations = vec![Op::ParameterPlace {
                ordinal: ordinal + 1,
                destination: *destination,
            }];
            assert_ne!(
                verify(another, &body).unwrap().recipe().fingerprint(),
                recipe.fingerprint()
            );
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
