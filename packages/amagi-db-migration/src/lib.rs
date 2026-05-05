pub use sea_orm_migration::prelude::*;

pub mod defs;
pub mod m20260504_000001_create_core_tables;
pub mod rls;
pub mod schema;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260504_000001_create_core_tables::Migration)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrator_registers_core_tables_migration() {
        let migrations = Migrator::migrations();

        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].name(), "m20260504_000001_create_core_tables");
    }
}
