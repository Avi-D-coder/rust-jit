# Keep the test owner inspector with its private literal records

V115 strict compilation exposed E0616: the test inspector was placed in a sibling module and could not inspect StaticLiteralRecord's private fields. This mechanical follow-on moves the identical read-only helper into `cell_byte_literals.rs`, behind `#[cfg(test)]`, and restores the general owner-inventory test module. Production field visibility and all ownership/lifetime rules remain unchanged; no accessor or runtime authority is introduced.

Apply after the static-owner oracle packet on exact preimages. The engine's exact-root/bytes/transient-owner checks are unchanged. Formatting and exact replay are checked; fresh strict compilation and the all-edition unchanged-main ownership control remain pending. The V114 ownership-oracle failure and V115 compile failure are retained, with no runtime/full-fixture/canonical pass inferred.
