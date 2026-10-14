use lrr_expand_adapter::token_wire::*;
fn group(text: &str, edition: Edition, unit: u8, delimiter: Delimiter) -> TokenForest {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([unit; 32]),
        edition,
    };
    let forest = parse_source(
        text,
        SourceIdentity {
            unit: context.defining_unit,
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: format!("unit-{unit}.rs"),
        },
        context,
        Key([unit + 1; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("missing source root")
    };
    let selected = children
        .iter()
        .find(|t| matches!(t,TokenTree::Group{delimiter:d,..} if *d==delimiter))
        .unwrap()
        .clone();
    TokenForest::new(
        vec![selected],
        forest.contexts().to_vec(),
        forest.spans().to_vec(),
    )
    .unwrap()
}
pub fn definition(text: &str, edition: Edition) -> TokenForest {
    group(
        &format!("macro_rules! fixture {{ {text} }}"),
        edition,
        1,
        Delimiter::Brace,
    )
}
pub fn input(text: &str) -> TokenForest {
    group(
        &format!("fixture!({text});"),
        Edition::E2024,
        2,
        Delimiter::Parenthesis,
    )
}
pub fn call_site(f: &TokenForest) -> Key {
    let [TokenTree::Group { open, .. }] = f.roots() else {
        panic!("missing call root")
    };
    *open
}
pub fn literal_texts(f: &TokenForest) -> Vec<String> {
    let mut result = Vec::new();
    let mut stack: Vec<_> = f.roots().iter().rev().collect();
    while let Some(t) = stack.pop() {
        match t {
            TokenTree::Group { children, .. } => stack.extend(children.iter().rev()),
            TokenTree::Literal { token_text, .. } => result.push(token_text.clone()),
            _ => {}
        }
    }
    result
}
