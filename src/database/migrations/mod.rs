use crate::database::Database;
use crate::database::migrations::denormalize_ip_identities::DenormalizeIpIdentitiesMigration;

pub mod denormalize_ip_identities;

#[async_trait]
pub trait DatabaseMigration {
    fn get_id(&self) -> String;
    async fn perform(&self, database: &Database);
}

pub struct MigrationExecutor {
    migrations: Vec<Box<dyn DatabaseMigration>>
}

impl MigrationExecutor {
    pub fn new() -> Self {
        let denormalize_ip_identities_migration =
            Box::new(DenormalizeIpIdentitiesMigration {});
        Self {
            migrations: vec![denormalize_ip_identities_migration]
        }
    }

    pub async fn execute_migration_by_name(&self, database: &Database, name: String) {
        match self.migrations.iter().find(|migration| migration.get_id() == name) {
            Some(migration) => {
                migration.perform(database).await
            }
            None => {}
        }
    }
}