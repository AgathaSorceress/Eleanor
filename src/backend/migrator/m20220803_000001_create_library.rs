use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Song::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Song::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Song::Path).string().not_null())
                    .col(ColumnDef::new(Song::Filename).string().not_null())
                    .col(ColumnDef::new(Song::SourceId).integer().not_null())
                    .col(ColumnDef::new(Song::Hash).integer().not_null().unique_key())
                    .col(ColumnDef::new(Song::Artist).string())
                    .col(ColumnDef::new(Song::AlbumArtist).string())
                    .col(ColumnDef::new(Song::Name).string())
                    .col(ColumnDef::new(Song::Album).string())
                    .col(ColumnDef::new(Song::Duration).integer().not_null())
                    .col(ColumnDef::new(Song::Genres).string())
                    .col(ColumnDef::new(Song::Track).integer())
                    .col(ColumnDef::new(Song::Year).integer())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Song::Table).to_owned())
            .await
    }
}

/// A Table containing every indexed song
#[derive(Iden)]
pub enum Song {
    #[iden = "library"]
    Table,
    /// Id of the song
    Id,
    Path,
    Filename,
    /// Refers to the sources defined in the configuration file and determines if file is remote
    SourceId,
    /// A hash of the song's samples as Vec<f32>
    Hash,
    Artist,
    AlbumArtist,
    Name,
    Album,
    Duration,
    /// Comma separated list of genres
    Genres,
    /// Number of the track in the album
    Track,
    Year,
}
