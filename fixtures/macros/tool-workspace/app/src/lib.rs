use lrr_tool_probe_macros::echo;

#[echo]
pub fn prepared_value() -> u32 {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"))
}

fn intentionally_invalid_cold_body() {
    lrr_deliberately_missing_function();
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_value() {
        assert_eq!(super::prepared_value(), 42);
    }
}
