use sea_orm_migration::prelude::*;

use super::{m20220803_000001_create_library::Song, m20220803_000001_create_playlists::Playlist};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlaylistEntry::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlaylistEntry::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlaylistEntry::PlaylistId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlaylistEntry::SongHash).integer().not_null())
                    .col(ColumnDef::new(PlaylistEntry::Ordinal).integer())
                    .col(ColumnDef::new(PlaylistEntry::AddedDate).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-playlist-id")
                            .from(PlaylistEntry::Table, PlaylistEntry::PlaylistId)
                            .to(Playlist::Table, Playlist::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-song-hash")
                            .from(PlaylistEntry::Table, PlaylistEntry::SongHash)
                            .to(Song::Table, Song::Hash),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlaylistEntry::Table).to_owned())
            .await
    }
}

/// A Table containing mappings between playlist and song
#[derive(Iden)]
pub enum PlaylistEntry {
    #[iden = "playlist_entries"]
    Table,
    Id,
    /// Id of playlist containing a song
    PlaylistId,
    /// Id of the song in the playlist
    SongHash,
    /// Position of song in playlist
    Ordinal,
    /// Date when the song was added
    AddedDate,
}
