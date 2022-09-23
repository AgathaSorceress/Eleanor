//! SeaORM Entity. Generated by sea-orm-codegen 0.9.1

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "playlist_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub playlist_id: i32,
    pub song_hash: i32,
    pub ordinal: Option<i32>,
    pub added_date: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::library::Entity",
        from = "Column::SongHash",
        to = "super::library::Column::Hash",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Library,
    #[sea_orm(
        belongs_to = "super::playlists::Entity",
        from = "Column::PlaylistId",
        to = "super::playlists::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Playlists,
}

impl Related<super::library::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Library.def()
    }
}

impl Related<super::playlists::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Playlists.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
