fn main() {
    // https://docs.rs/sqlx/0.8.6/sqlx/macro.migrate.html#triggering-recompilation-on-migration-changes
    println!("cargo:rerun-if-changed=migrations");
}
