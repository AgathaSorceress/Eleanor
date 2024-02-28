use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Source::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Source::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Source::LastIndexed).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Source::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Source {
    #[iden = "sources"]
    Table,
    Id,
    LastIndexed,
}
