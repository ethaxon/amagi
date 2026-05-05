use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "bookmark_meta")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub node_id: Uuid,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub canonical_url: Option<String>,
    pub page_title: Option<String>,
    pub favicon_asset_id: Option<Uuid>,
    pub reading_state: Option<String>,
    pub starred: bool,
    pub extra_json: Json,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
