use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "node_client_mappings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub browser_client_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub server_node_id: Uuid,
    pub client_external_id: String,
    pub last_seen_hash: Option<String>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
