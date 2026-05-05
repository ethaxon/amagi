use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node_revisions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub rev_id: Uuid,
    pub library_id: Uuid,
    pub node_id: Uuid,
    pub actor_type: String,
    pub actor_id: Option<Uuid>,
    pub op_type: String,
    pub payload_json: Json,
    pub logical_clock: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
