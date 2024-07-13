use mongodb::bson::doc;
use mongodb::options::UpdateOptions;
use crate::database::Database;
use crate::database::migrations::DatabaseMigration;

pub struct ResetStatsMigration {}

// resets stats for players, in specific, set stats/gamemodeStats objects to empty objects
#[async_trait]
impl DatabaseMigration for ResetStatsMigration {
    fn get_id(&self) -> String {
        String::from("reset_stats")
    }

    async fn perform(&self, database: &Database) {
        info!("Resetting all player stats...");
        let update_result = database.players.update_many(
        doc! {},
        doc! { "$set": {"stats": {}, "gamemodeStats": {}}},
        None
        ).await;
        match update_result {
            Ok(result) => {
                info!(
                    "Successfully reset player statistics, {} documents were modified",
                    result.modified_count
                );
            }
            Err(err) => {
                warn!("Could not reset stats: {}", err);
            }
        };
        info!("Resetting achievement first completions..");
        database.achievements.update_many(
            doc! {},
            doc! {"$set": {"firstCompletion": null}},
            None
        ).await;
        info!("Reset achievement first completions");
    }
}