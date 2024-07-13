use crate::database::Database;
use crate::database::migrations::denormalize_ip_identities::DenormalizeIpIdentitiesMigration;
use crate::database::migrations::reset_stats::ResetStatsMigration;

pub mod denormalize_ip_identities;
mod reset_stats;

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
        let reset_stats_migration =
            Box::new(ResetStatsMigration {});
        Self {
            migrations: vec![
                denormalize_ip_identities_migration,
                reset_stats_migration
            ]
        }
    }

    pub async fn execute_migration_by_name(&self, database: &Database, name: String) -> bool {
        match self.migrations.iter().find(|migration| migration.get_id() == name) {
            Some(migration) => {
                migration.perform(database).await;
                true
            }
            None => {
                warn!("Could not find migration '{}'", name);
                false
            }
        }
    }
}