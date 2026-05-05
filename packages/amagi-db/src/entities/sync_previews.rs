use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sync_previews")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub browser_client_id: Uuid,
    pub library_id: Uuid,
    pub base_clock: i64,
    pub to_clock: i64,
    pub status: String,
    pub request_hash: String,
    pub summary_json: Json,
    pub server_ops_json: Json,
    pub accepted_local_mutations_json: Json,
    pub conflicts_json: Json,
    pub expires_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub applied_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
