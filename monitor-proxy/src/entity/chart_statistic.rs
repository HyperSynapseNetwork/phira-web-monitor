use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "chart_statistics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub chart_id: i32,
    #[sea_orm(indexed)]
    pub count_hour: i32,
    #[sea_orm(indexed)]
    pub count_day: i32,
    #[sea_orm(indexed)]
    pub count_week: i32,
    #[sea_orm(indexed)]
    pub count_month: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
