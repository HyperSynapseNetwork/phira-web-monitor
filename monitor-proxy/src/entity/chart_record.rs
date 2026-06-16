use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "chart_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub chart_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub timestamp: i64,
    pub count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
