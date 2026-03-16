use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(VisitedUsers::Table)
                    .if_not_exists()
                    .col(integer(VisitedUsers::PhiraId).primary_key())
                    .col(string(VisitedUsers::Username))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VisitedUsers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum VisitedUsers {
    Table,
    PhiraId,
    Username,
}
