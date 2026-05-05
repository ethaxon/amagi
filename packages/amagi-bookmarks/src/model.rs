use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BookmarkError, BookmarkResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    Normal,
    Vault,
}

impl LibraryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Vault => "vault",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Folder,
    Bookmark,
    Separator,
}

impl NodeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Bookmark => "bookmark",
            Self::Separator => "separator",
        }
    }

    pub fn from_db(value: &str) -> BookmarkResult<Self> {
        match value {
            "folder" => Ok(Self::Folder),
            "bookmark" => Ok(Self::Bookmark),
            "separator" => Ok(Self::Separator),
            _ => Err(BookmarkError::InvalidNodeType),
        }
    }

    pub fn validate_url(self, url: Option<&str>) -> BookmarkResult<()> {
        match (self, url.map(str::trim).filter(|value| !value.is_empty())) {
            (Self::Bookmark, Some(_)) => Ok(()),
            (Self::Bookmark, None) => Err(BookmarkError::InvalidUrl),
            (Self::Folder | Self::Separator, None) => Ok(()),
            (Self::Folder | Self::Separator, Some(_)) => Err(BookmarkError::InvalidNodeType),
        }
    }

    pub fn normalized_url(self, url: Option<&str>) -> BookmarkResult<Option<String>> {
        self.validate_url(url)?;
        Ok(match self {
            Self::Bookmark => Some(
                url.expect("bookmark URL exists after validation")
                    .trim()
                    .to_owned(),
            ),
            Self::Folder | Self::Separator => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLibraryRequest {
    pub name: String,
    pub kind: LibraryKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeRequest {
    pub node_type: NodeType,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub url: Option<String>,
    pub sort_key: Option<String>,
}

impl CreateNodeRequest {
    pub fn fallback_sort_key() -> String {
        format!("m{}", Uuid::now_v7().as_simple())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNodeRequest {
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveNodeRequest {
    pub parent_id: Uuid,
    pub sort_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreNodeRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub current_revision_clock: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkNodeView {
    pub id: String,
    pub library_id: String,
    pub parent_id: Option<String>,
    pub node_type: String,
    pub title: String,
    pub sort_key: String,
    pub url: Option<String>,
    pub url_normalized: Option<String>,
    pub is_deleted: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTreeView {
    pub library: LibraryView,
    pub nodes: Vec<BookmarkNodeView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionView {
    pub rev_id: String,
    pub library_id: String,
    pub node_id: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub op_type: String,
    pub payload: serde_json::Value,
    pub logical_clock: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionFeedView {
    pub library_id: String,
    pub from_clock: i64,
    pub to_clock: i64,
    pub revisions: Vec<RevisionView>,
    pub next_cursor: Option<i64>,
}
