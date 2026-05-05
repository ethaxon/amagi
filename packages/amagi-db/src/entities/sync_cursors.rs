use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sync_cursors")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub browser_client_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub library_id: Uuid,
    pub last_applied_clock: i64,
    pub last_ack_rev_id: Option<Uuid>,
    pub last_sync_at: Option<DateTimeWithTimeZone>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
