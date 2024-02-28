use sea_orm_migration::prelude::*;

mod m20220803_000001_create_library;
mod m20220803_000001_create_playlist_entries;
mod m20220803_000001_create_playlists;
mod m20240223_185340_create_sources;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220803_000001_create_library::Migration),
            Box::new(m20220803_000001_create_playlist_entries::Migration),
            Box::new(m20220803_000001_create_playlists::Migration),
            Box::new(m20240223_185340_create_sources::Migration),
        ]
    }
}
